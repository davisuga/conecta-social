//! Standalone smoke test for the rig + Anthropic triagem interpreter.
//!
//! Run:
//!   cargo run --example agent_smoke -- q1 "preciso de dinheiro pros meus filhos"
//!   cargo run --example agent_smoke -- q2 "ainda nao fiz"
//!   cargo run --example agent_smoke -- q3 "meu cpf é 123.456.789-09"
//!
//! Requires `ANTHROPIC_API_KEY` in env (the example loads `.env` automatically).
//! Mirrors `services::agent` logic but skips the DB so you don't need postgres
//! running to verify that the LLM + `record_answer` tool + canonical whitelist
//! plumbing works end-to-end.

use std::sync::{Arc, Mutex};

use rig_core::client::CompletionClient;
use rig_core::completion::{Chat, Message, ToolDefinition};
use rig_core::providers::anthropic;
use rig_core::tool::Tool;
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
enum Step {
    Q1,
    Q2,
    Q3,
}

#[derive(Clone)]
struct RecordAnswer {
    step: Step,
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
            Step::Q1 => (
                "Record the social-assistance service the user needs. \
                 The `value` parameter MUST be exactly one of: \
                 bolsa_familia, cadastro_unico, bpc, outro_atendimento, nao_sei.",
                json!({
                    "type": "string",
                    "enum": ["bolsa_familia", "cadastro_unico", "bpc", "outro_atendimento", "nao_sei"]
                }),
            ),
            Step::Q2 => (
                "Record whether the user has a CadÚnico registration. \
                 The `value` parameter MUST be exactly one of: sim, nao, nao_sei.",
                json!({ "type": "string", "enum": ["sim", "nao", "nao_sei"] }),
            ),
            Step::Q3 => (
                "Record the user's NIS or CPF as 11 ASCII digits with no separators.",
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
            Step::Q1 => matches!(
                v.as_str(),
                "bolsa_familia" | "cadastro_unico" | "bpc" | "outro_atendimento" | "nao_sei"
            ),
            Step::Q2 => matches!(v.as_str(), "sim" | "nao" | "nao_sei"),
            Step::Q3 => v.len() == 11 && v.chars().all(|c| c.is_ascii_digit()),
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

const Q1_TEXT: &str =
    "O que você precisa hoje? Opções: Bolsa Família, Cadastro Único, BPC, outro atendimento, ou não sei.";
const Q2_TEXT: &str = "Você já tem cadastro no CadÚnico? (sim / não / não sei)";
const Q3_TEXT: &str = "Qual é o seu NIS ou CPF? (somente números, 11 dígitos)";

fn preamble_for(step: Step) -> String {
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
                  1. Se o usuário respondeu a pergunta atual, chame a ferramenta record_answer \
                     com o valor canônico E responda em 1–2 frases confirmando o que você \
                     entendeu e o porquê. Termine com a PRÓXIMA pergunta.\n\
                  2. Se o usuário pediu ajuda, fez uma pergunta ou está confuso, NÃO chame a \
                     ferramenta. Responda a dúvida em 2–4 frases e termine reapresentando a \
                     PERGUNTA ATUAL.\n\
                  3. Nunca invente valores, datas, telefones, CPFs ou endereços.\n\
                  4. Não cumprimente nem se apresente de novo.";

    match step {
        Step::Q1 => format!(
            "{common}\n\n\
             === PERGUNTA ATUAL (Q1 de 3) ===\n{q1}\n\n\
             === PRÓXIMA PERGUNTA (Q2) ===\n{q2}\n\n\
             Mapeamento livre → canônico:\n\
             - Bolsa Família, auxílio, ajuda pros filhos, cesta básica → bolsa_familia\n\
             - CadÚnico, cadastro único, recadastrar → cadastro_unico\n\
             - BPC, LOAS, benefício para idoso ou pessoa com deficiência → bpc\n\
             - Qualquer outro serviço social → outro_atendimento\n\
             - Não sabe → nao_sei",
            q1 = Q1_TEXT,
            q2 = Q2_TEXT
        ),
        Step::Q2 => format!(
            "{common}\n\n\
             === PERGUNTA ATUAL (Q2 de 3) ===\n{q2}\n\n\
             === PRÓXIMA PERGUNTA (Q3) ===\n{q3}\n\n\
             Mapeamento livre → canônico:\n\
             - Afirmativo → sim\n- Negativo → nao\n- Incerteza → nao_sei",
            q2 = Q2_TEXT,
            q3 = Q3_TEXT
        ),
        Step::Q3 => format!(
            "{common}\n\n\
             === PERGUNTA ATUAL (Q3 de 3) ===\n{q3}\n\n\
             === DEPOIS DESTA ===\nO sistema gera o agendamento e envia a confirmação. NÃO peça \
             mais dados e NÃO mostre próxima pergunta — apenas confirme que recebeu o número.",
            q3 = Q3_TEXT
        ),
    }
}

fn parse_step(s: &str) -> Option<Step> {
    match s.to_lowercase().as_str() {
        "q1" | "1" => Some(Step::Q1),
        "q2" | "2" => Some(Step::Q2),
        "q3" | "3" => Some(Step::Q3),
        _ => None,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // dotenv() doesn't override pre-existing env vars; if the parent shell
    // exports an empty ANTHROPIC_API_KEY (it does), the .env value is ignored.
    match dotenvy::from_filename_override(".env") {
        Ok(p) => println!("(loaded .env from {p:?} with override)"),
        Err(e) => println!("(dotenv: {e})"),
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let step_arg = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: agent_smoke <q1|q2|q3> <text...>"))?;
    let text = args.collect::<Vec<_>>().join(" ");
    if text.trim().is_empty() {
        anyhow::bail!("usage: agent_smoke <q1|q2|q3> <text...>");
    }
    let step = parse_step(&step_arg)
        .ok_or_else(|| anyhow::anyhow!("step must be q1, q2 or q3 (got '{step_arg}')"))?;

    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        anyhow::bail!("ANTHROPIC_API_KEY not set");
    }
    let model =
        std::env::var("RIG_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string());

    println!("step  = {step:?}");
    println!("model = {model}");
    println!("input = {text:?}");

    let api_key = std::env::var("ANTHROPIC_API_KEY")?;
    let client = anthropic::Client::new(&api_key)?;
    println!("(client built from explicit key, len={} chars)", api_key.len());
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let tool = RecordAnswer {
        step,
        captured: captured.clone(),
    };

    let agent = client
        .agent(&model)
        .preamble(&preamble_for(step))
        .tool(tool)
        .max_tokens(512)
        .temperature(0.2)
        .build();

    let mut history: Vec<Message> = Vec::new();
    let started = std::time::Instant::now();
    let response = agent.chat(text.as_str(), &mut history).await;
    let elapsed = started.elapsed();

    match response {
        Ok(text) => {
            println!("--- assistant reply ({elapsed:?}) ---");
            println!("{text}");
        }
        Err(e) => {
            println!("--- agent error ({elapsed:?}) ---");
            println!("{e}");
        }
    }
    let value = captured.lock().ok().and_then(|g| g.clone());
    println!("--- record_answer captured ---");
    match value {
        Some(v) => println!("recorded = {v}"),
        None => println!("no tool call captured (Unparseable)"),
    }
    Ok(())
}
