//! LLM-assisted triagem interpreter (rig.rs + Anthropic Claude).
//!
//! Hook point: invoked by `triagem_chat` whenever the deterministic parser
//! (`parse_q1`/`parse_q2`/`parse_q3`) returns `None`. The agent inspects the
//! user's free-form message together with the running chat history and a
//! step-specific preamble. Two paths:
//!
//! - The user clearly answered the current question → the model calls
//!   `record_answer` with the canonical value AND replies with a short
//!   acknowledgment that includes the next question. The caller persists the
//!   answer and forwards the reply as-is — no canned next-question text.
//! - The user asked a question, requested help, or is confused → the model
//!   replies in plain text (no tool call) explaining briefly and re-asking the
//!   current question. The caller sends the reply; session state stays put.
//!
//! Whitelists are enforced inside the tool's `call()`, so hallucinated
//! canonical values become `AgentOutcome::OnlyReply`. If anything in the rig
//! pipeline errors out the caller gets `AgentOutcome::Failed` and falls back
//! to a generic apology. Chat turns persist in `triagem_chat_turns` (last 20
//! replayed) so multi-turn context survives across webhook calls.

use std::env;
use std::sync::{Arc, Mutex};

use rig_core::client::CompletionClient;
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
    /// Model tool-called with a whitelisted canonical value. `reply` is the
    /// natural-language acknowledgment to forward to the user — it should
    /// already include or lead into the next question, so callers should NOT
    /// also send the canned next-question text.
    Recorded { value: String, reply: String },
    /// Model didn't record an answer but produced a useful reply (general
    /// help, clarifying explanation, re-asking the current question). State
    /// stays put; just forward `reply`.
    OnlyReply(String),
    /// Agent unavailable (no API key, network error, etc.). Caller falls back
    /// to the generic apology message.
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
    /// `ANTHROPIC_API_KEY` is unset/empty — every `interpret()` call
    /// short-circuits to `AgentOutcome::Failed`, keeping the bot operable in
    /// dev without keys.
    pub fn from_env(db: PgPool) -> Self {
        let raw_key = env::var("ANTHROPIC_API_KEY").ok();
        let client = match raw_key.as_deref() {
            Some(k) if !k.trim().is_empty() => match anthropic::Client::new(k.trim()) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!(target: "agent", error = %e, "anthropic client init failed");
                    None
                }
            },
            _ => {
                tracing::info!(target: "agent", "ANTHROPIC_API_KEY missing/empty; LLM fallback disabled");
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
        let chat_result = agent.chat(user_text, &mut chat_history).await;

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
        let reply = if assistant_text.trim().is_empty() {
            default_reply(step, value.as_deref())
        } else {
            assistant_text
        };
        match value {
            Some(v) => {
                tracing::info!(target: "agent", %session_id, value = %v, "interpreted");
                AgentOutcome::Recorded { value: v, reply }
            }
            None => AgentOutcome::OnlyReply(reply),
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
                 Call this tool only when the user's intent is clear.",
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

const Q1_TEXT: &str =
    "O que você precisa hoje? Opções: Bolsa Família, Cadastro Único, BPC, outro atendimento, ou não sei.";
const Q2_TEXT: &str = "Você já tem cadastro no CadÚnico? (sim / não / não sei)";
const Q3_TEXT: &str = "Qual é o seu NIS ou CPF? (somente números, 11 dígitos)";

fn preamble_for(step: AgentStep) -> String {
    let common = "Você é o assistente do CRAS no WhatsApp. Sua função é ajudar a triar \
                  atendimentos socioassistenciais e tirar dúvidas sobre Bolsa Família, BPC, \
                  Cadastro Único, CRAS, CREAS e serviços de assistência social.\n\
                  \n\
                  Regras de estilo:\n\
                  - Sempre em português brasileiro, frases curtas, linguagem simples.\n\
                  - Use emojis com moderação, no estilo WhatsApp.\n\
                  - Nunca peça mais dados além do necessário para a pergunta atual.\n\
                  \n\
                  Regras de comportamento:\n\
                  1. Se o usuário respondeu a pergunta atual de forma compreensível, chame a \
                     ferramenta `record_answer` com o valor canônico E responda em 1–2 frases \
                     curtas confirmando o que você entendeu e justificando rapidamente o porquê. \
                     Termine a mensagem com a PRÓXIMA pergunta do roteiro.\n\
                  2. Se o usuário fez uma pergunta, pediu ajuda, demonstrou dúvida sobre algum \
                     benefício, ou se está confuso, NÃO chame a ferramenta. Responda a dúvida \
                     com 2–4 frases informativas e termine a mensagem reapresentando a PERGUNTA \
                     ATUAL — assim ele sabe como prosseguir.\n\
                  3. Nunca invente valores, datas, telefones, CPFs ou endereços. Se não souber, \
                     diga para o usuário procurar o CRAS de referência.\n\
                  4. Não cumprimente nem se apresente de novo — o cidadão já está em atendimento.";

    match step {
        AgentStep::Q1 => format!(
            "{common}\n\n\
             === PERGUNTA ATUAL (Q1 de 3) ===\n{q1}\n\n\
             === PRÓXIMA PERGUNTA (Q2) ===\n{q2}\n\n\
             Mapeamento livre → canônico:\n\
             - Bolsa Família, auxílio, ajuda pros filhos, cesta básica → bolsa_familia\n\
             - CadÚnico, cadastro único, recadastrar, atualizar cadastro → cadastro_unico\n\
             - BPC, LOAS, benefício para idoso ou pessoa com deficiência → bpc\n\
             - Qualquer outro serviço social → outro_atendimento\n\
             - Não sabe, não tem certeza → nao_sei\n\n\
             Exemplos de boa resposta com tool-call:\n\
             User: \"preciso de uma ajuda pra alimentar meus filhos\"\n\
             → record_answer(value=\"bolsa_familia\"); reply: \"Entendi! Pelo que você descreveu, \
             o caminho é o Bolsa Família, que é o benefício de transferência de renda para \
             famílias com filhos. {q2}\"\n\n\
             User: \"o que é BPC mesmo?\"\n\
             → NÃO chame a ferramenta; reply: \"O BPC é um benefício mensal de 1 salário mínimo \
             para idosos com 65+ anos ou pessoas com deficiência, sem precisar ter contribuído \
             com o INSS. {q1}\"",
            q1 = Q1_TEXT,
            q2 = Q2_TEXT
        ),
        AgentStep::Q2 => format!(
            "{common}\n\n\
             === PERGUNTA ATUAL (Q2 de 3) ===\n{q2}\n\n\
             === PRÓXIMA PERGUNTA (Q3) ===\n{q3}\n\n\
             Mapeamento livre → canônico:\n\
             - Afirmativo (sim, tenho, já fiz, faz tempo) → sim\n\
             - Negativo (não, nunca fiz, sem cadastro) → nao\n\
             - Incerteza (não lembro, talvez, acho que sim mas faz tempo) → nao_sei",
            q2 = Q2_TEXT,
            q3 = Q3_TEXT
        ),
        AgentStep::Q3 => format!(
            "{common}\n\n\
             === PERGUNTA ATUAL (Q3 de 3) ===\n{q3}\n\n\
             === DEPOIS DESTA ===\nO sistema gera o agendamento automaticamente e envia uma \
             confirmação com data, unidade e documentos. NÃO peça mais dados ao usuário e NÃO \
             diga próxima pergunta — apenas confirme que recebeu o número.\n\n\
             Regras de extração:\n\
             - Procure por exatamente 11 dígitos no texto, ignorando pontos, traços e espaços.\n\
             - Encontrou 11 dígitos? Chame record_answer com a string só de dígitos e responda: \
             \"Recebi seu número, {{primeiros4}}...{{ultimos2}}. Vou gerar seu agendamento agora.\"\n\
             - Mais ou menos que 11 dígitos? Não chame a ferramenta; peça que o usuário envie de \
             novo somente os 11 números.",
            q3 = Q3_TEXT
        ),
    }
}

fn default_reply(step: AgentStep, value: Option<&str>) -> String {
    match (step, value) {
        (AgentStep::Q1, Some(_)) => format!("Entendi! Vamos para a próxima.\n\n{Q2_TEXT}"),
        (AgentStep::Q2, Some(_)) => format!("Anotado. Última pergunta:\n\n{Q3_TEXT}"),
        (AgentStep::Q3, Some(_)) => "Recebi seu número. Vou gerar seu agendamento agora.".into(),
        (AgentStep::Q1, None) => format!("Não consegui entender. {Q1_TEXT}"),
        (AgentStep::Q2, None) => format!("Não consegui entender. {Q2_TEXT}"),
        (AgentStep::Q3, None) => format!("Não consegui ler o número. {Q3_TEXT}"),
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

#[allow(dead_code)]
const _MAX_TURNS_HINT: usize = MAX_TURNS;
