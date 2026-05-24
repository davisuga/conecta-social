//! LLM-assisted triagem interpreter (rig.rs + Anthropic Claude).
//!
//! Hook point: invoked by `triagem_chat` only when the deterministic parser
//! (`parse_q1`/`parse_q2`/`parse_q3`) returns `None`. The agent inspects the
//! user's free-form message, the running chat history for the session, and a
//! step-specific preamble — then calls the `record_answer` tool with one
//! canonical value from a whitelist. The whitelist is enforced inside the
//! tool's `call()`, so hallucinated values become `Outcome::Unparseable`.
//!
//! Chat history persists in `triagem_chat_turns` (last 20 turns replayed).

use std::env;
use std::sync::{Arc, Mutex};

use rig_core::client::{CompletionClient, ProviderClient};
use rig_core::completion::{Chat, Message, ToolDefinition};
use rig_core::providers::anthropic;
use rig_core::tool::Tool;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";
const HISTORY_LIMIT: i64 = 20;
const MAX_TURNS: usize = 3;

#[derive(Debug, Clone, Copy)]
pub enum AgentStep {
    Q1,
    Q2,
    Q3,
}

#[derive(Debug)]
pub enum AgentOutcome {
    /// Tool was called with a whitelisted canonical value. Feed to handle_qN.
    Recorded(String),
    /// LLM responded but no valid tool call captured. Caller should reprompt.
    Unparseable,
    /// Agent unavailable (no API key, network error, etc.). Caller should
    /// emit the generic apology configured in the design.
    Failed,
}

#[derive(Clone)]
pub struct TriagemAgent {
    db: PgPool,
    client: Option<anthropic::Client>,
    model: String,
}

impl TriagemAgent {
    /// Build from env. Returns an agent with `client = None` when
    /// `ANTHROPIC_API_KEY` is unset — every `interpret()` call short-circuits
    /// to `AgentOutcome::Failed`, keeping the bot operable in dev without keys.
    pub fn from_env(db: PgPool) -> Self {
        let client = match env::var("ANTHROPIC_API_KEY") {
            Ok(_) => match anthropic::Client::from_env() {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!(target: "agent", error = %e, "anthropic client init failed");
                    None
                }
            },
            Err(_) => {
                tracing::info!(target: "agent", "ANTHROPIC_API_KEY unset; LLM fallback disabled");
                None
            }
        };
        let model = env::var("RIG_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        Self { db, client, model }
    }

    pub async fn interpret(
        &self,
        session_id: Uuid,
        step: AgentStep,
        user_text: &str,
    ) -> AgentOutcome {
        let Some(client) = self.client.as_ref() else {
            return AgentOutcome::Failed;
        };

        let history = match load_history(&self.db, session_id).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(target: "agent", %session_id, error = %e, "history load failed");
                Vec::new()
            }
        };

        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let tool = RecordAnswer {
            step,
            captured: captured.clone(),
        };

        let agent = client
            .agent(&self.model)
            .preamble(&preamble_for(step))
            .tool(tool)
            .max_tokens(512)
            .temperature(0.2)
            .build();

        let mut chat_history = history;
        let chat_result = agent
            .chat(user_text, &mut chat_history)
            .await;

        let assistant_text = match chat_result {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!(target: "agent", %session_id, error = %e, "agent chat failed");
                return AgentOutcome::Failed;
            }
        };

        if let Err(e) = persist_turn(&self.db, session_id, "user", user_text).await {
            tracing::warn!(target: "agent", %session_id, error = %e, "persist user turn failed");
        }
        if !assistant_text.trim().is_empty() {
            if let Err(e) = persist_turn(&self.db, session_id, "assistant", &assistant_text).await {
                tracing::warn!(target: "agent", %session_id, error = %e, "persist assistant turn failed");
            }
        }

        let value = captured.lock().ok().and_then(|g| g.clone());
        match value {
            Some(v) => {
                tracing::info!(target: "agent", %session_id, value = %v, "interpreted");
                AgentOutcome::Recorded(v)
            }
            None => AgentOutcome::Unparseable,
        }
    }
}

// ===== Tool =====

#[derive(Clone)]
struct RecordAnswer {
    step: AgentStep,
    captured: Arc<Mutex<Option<String>>>,
}

#[derive(Deserialize)]
struct RecordArgs {
    value: String,
}

#[derive(Debug, Error)]
enum RecordError {
    #[error("value '{0}' not in allowed set for this step")]
    NotAllowed(String),
}

impl Tool for RecordAnswer {
    const NAME: &'static str = "record_answer";
    type Args = RecordArgs;
    type Output = String;
    type Error = RecordError;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let (description, value_schema) = match self.step {
            AgentStep::Q1 => (
                "Record the social-assistance service the user needs. \
                 The `value` parameter MUST be exactly one of: \
                 bolsa_familia, cadastro_unico, bpc, outro_atendimento, nao_sei. \
                 Call this tool only when you can confidently classify the user's intent.",
                json!({
                    "type": "string",
                    "enum": ["bolsa_familia", "cadastro_unico", "bpc", "outro_atendimento", "nao_sei"]
                }),
            ),
            AgentStep::Q2 => (
                "Record whether the user has a CadÚnico registration. \
                 The `value` parameter MUST be exactly one of: sim, nao, nao_sei.",
                json!({ "type": "string", "enum": ["sim", "nao", "nao_sei"] }),
            ),
            AgentStep::Q3 => (
                "Record the user's NIS or CPF as 11 ASCII digits with no separators. \
                 Only call this tool when the user has clearly provided a valid 11-digit identifier.",
                json!({ "type": "string", "pattern": "^[0-9]{11}$" }),
            ),
        };
        ToolDefinition {
            name: "record_answer".into(),
            description: description.into(),
            parameters: json!({
                "type": "object",
                "properties": { "value": value_schema },
                "required": ["value"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let v = args.value.trim().to_string();
        let ok = match self.step {
            AgentStep::Q1 => matches!(
                v.as_str(),
                "bolsa_familia" | "cadastro_unico" | "bpc" | "outro_atendimento" | "nao_sei"
            ),
            AgentStep::Q2 => matches!(v.as_str(), "sim" | "nao" | "nao_sei"),
            AgentStep::Q3 => v.len() == 11 && v.chars().all(|c| c.is_ascii_digit()),
        };
        if !ok {
            return Err(RecordError::NotAllowed(v));
        }
        if let Ok(mut g) = self.captured.lock() {
            *g = Some(v.clone());
        }
        Ok(v)
    }
}

// ===== preambles =====

fn preamble_for(step: AgentStep) -> String {
    let common = "Você é o assistente do CRAS no WhatsApp ajudando a triar atendimentos. \
                  Escreva sempre em português brasileiro, com frases curtas e linguagem simples. \
                  Quando o usuário expressar uma intenção compatível com uma das opções abaixo, \
                  chame a ferramenta `record_answer` com o valor canônico correspondente. \
                  Se o usuário fizer uma pergunta ou estiver claramente perdido, explique brevemente \
                  e termine a resposta reapresentando a pergunta atual.";
    match step {
        AgentStep::Q1 => format!(
            "{common}\n\nPergunta atual: \"O que você precisa hoje?\"\n\n\
             Mapeamento livre → canônico:\n\
             - Bolsa Família, auxílio, ajuda pros filhos, cesta básica → bolsa_familia\n\
             - CadÚnico, cadastro único, recadastrar, atualizar cadastro → cadastro_unico\n\
             - BPC, LOAS, benefício para idoso ou pessoa com deficiência → bpc\n\
             - Qualquer outro serviço social → outro_atendimento\n\
             - Não sabe, não tem certeza → nao_sei"
        ),
        AgentStep::Q2 => format!(
            "{common}\n\nPergunta atual: \"Você já tem cadastro no CadÚnico?\"\n\n\
             Mapeamento livre → canônico:\n\
             - Afirmativo (sim, tenho, já fiz) → sim\n\
             - Negativo (não, nunca fiz) → nao\n\
             - Incerteza (não lembro, talvez) → nao_sei"
        ),
        AgentStep::Q3 => format!(
            "{common}\n\nPergunta atual: \"Qual é o seu NIS ou CPF?\"\n\n\
             Procure por um identificador de 11 dígitos no texto, ignorando pontos, traços e espaços. \
             Se houver exatamente 11 dígitos, chame `record_answer` com a string só de dígitos. \
             Se houver número com tamanho diferente, peça que o usuário envie novamente, somente números."
        ),
    }
}

// ===== history persistence =====

async fn load_history(db: &PgPool, session_id: Uuid) -> sqlx::Result<Vec<Message>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT role, content FROM (
             SELECT role, content, created_at
               FROM triagem_chat_turns
              WHERE session_id = $1
              ORDER BY created_at DESC
              LIMIT $2
         ) AS t
         ORDER BY created_at ASC",
    )
    .bind(session_id)
    .bind(HISTORY_LIMIT)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(role, content)| match role.as_str() {
            "assistant" => Message::assistant(content),
            _ => Message::user(content),
        })
        .collect())
}

async fn persist_turn(
    db: &PgPool,
    session_id: Uuid,
    role: &str,
    content: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO triagem_chat_turns (session_id, role, content) VALUES ($1, $2, $3)",
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .execute(db)
    .await?;
    Ok(())
}

// Quiet `MAX_TURNS` for now — rig's `.chat()` does a single completion turn,
// but we keep the constant in scope so a future switch to `.prompt().max_turns(_)`
// can read from one place.
#[allow(dead_code)]
const _MAX_TURNS_HINT: usize = MAX_TURNS;
