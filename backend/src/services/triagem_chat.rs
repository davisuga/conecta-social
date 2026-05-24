//! WhatsApp triagem state machine (Camada 2).
//!
//! Three-question flow keyed by chat phone:
//!   Q1 "precisa_hoje" → service + initial routing
//!   Q2 "tem_cadastro" → recorded only
//!   Q3 "nis_cpf"      → identify, upsert profile, create appointment, confirm
//!
//! Active session = `from_phone` match with `completed_at IS NULL`.
//! `novo` (or `reiniciar`) closes the active session and restarts.

use chrono::{DateTime, Datelike, Duration, NaiveTime, Utc, Weekday};
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{ServiceType, TriagemChannel};
use crate::services::agent::{AgentOutcome, AgentStep, TriagemAgent};
use crate::services::whatsapp::WhatsappService;

const AGENT_ERROR_FALLBACK: &str =
    "❗ Tive um problema agora. Pode tentar de novo em alguns instantes?";

const Q1: &str = "👋 Olá! Vou te ajudar com seu atendimento.\n\n\
                  *Pergunta 1 de 3*\n\
                  O que você precisa hoje?\n\n\
                  1️⃣ Bolsa Família\n\
                  2️⃣ Cadastro Único\n\
                  3️⃣ BPC\n\
                  4️⃣ Outro atendimento\n\
                  5️⃣ Não sei\n\n\
                  _Responda com o número ou o nome._";

const Q2: &str = "*Pergunta 2 de 3*\n\
                  Você já tem cadastro no CadÚnico?\n\n\
                  1️⃣ Sim\n\
                  2️⃣ Não\n\
                  3️⃣ Não sei";

const Q3: &str = "*Pergunta 3 de 3*\n\
                  Qual é o seu NIS ou CPF?\n\
                  _(somente números, 11 dígitos)_";

pub async fn handle_inbound(
    db: &PgPool,
    wa: &WhatsappService,
    agent: &TriagemAgent,
    chat_id: &str,
    from_phone: &str,
    push_name: Option<&str>,
    text: &str,
) -> anyhow::Result<()> {
    let normalized = normalize(text);

    if matches!(normalized.as_str(), "novo" | "reiniciar" | "restart" | "reset") {
        close_active_sessions(db, from_phone).await?;
        let _ = start_session(db, from_phone).await?;
        wa.send_text(chat_id, Q1).await?;
        return Ok(());
    }

    let active = sqlx::query_as::<_, ActiveSession>(
        "SELECT s.id,
                COALESCE(COUNT(a.question_id), 0)::bigint AS n_answers
           FROM triagem_sessions s
      LEFT JOIN triagem_answers a ON a.session_id = s.id
          WHERE s.from_phone = $1 AND s.completed_at IS NULL
          GROUP BY s.id
          ORDER BY s.id DESC
          LIMIT 1",
    )
    .bind(from_phone)
    .fetch_optional(db)
    .await?;

    // For brand-new sessions we still process this very first message: a
    // greeting like "oi" goes through the agent's OnlyReply path (which
    // includes Q1 in its reply), and a substantive first message like
    // "preciso de ajuda" gets classified immediately. Avoids dropping the
    // user's first turn while still hitting the LLM only when there's
    // actually something to say.
    let (sid, step) = match active {
        Some(a) => (a.id, a.n_answers as usize),
        None => {
            let id = start_session(db, from_phone).await?;
            tracing::info!(target: "triagem", %from_phone, sid = %id, "session started");
            (id, 0)
        }
    };

    match step {
        0 => handle_q1(db, wa, agent, sid, chat_id, &normalized, text).await,
        1 => handle_q2(db, wa, agent, sid, chat_id, &normalized, text).await,
        2 => handle_q3(db, wa, agent, sid, chat_id, from_phone, push_name, text).await,
        _ => {
            wa.send_text(
                chat_id,
                "Sua triagem já foi concluída. Para iniciar nova, envie *novo*.",
            )
            .await?;
            Ok(())
        }
    }
}

async fn handle_q1(
    db: &PgPool,
    wa: &WhatsappService,
    agent: &TriagemAgent,
    sid: Uuid,
    chat_id: &str,
    normalized: &str,
    raw_text: &str,
) -> anyhow::Result<()> {
    let label = match parse_q1(normalized) {
        Some((l, _)) => l.to_string(),
        None => match agent.interpret(sid, AgentStep::Q1, raw_text).await {
            AgentOutcome::Recorded { value, reply } => {
                wa.send_text(chat_id, &reply).await?;
                value
            }
            AgentOutcome::OnlyReply(reply) => {
                wa.send_text(chat_id, &reply).await?;
                return Ok(());
            }
            AgentOutcome::Failed => {
                wa.send_text(chat_id, AGENT_ERROR_FALLBACK).await?;
                return Ok(());
            }
        },
    };
    let parser_hit = parse_q1(normalized).is_some();
    let service = match label.as_str() {
        "bolsa_familia" => ServiceType::BolsaFamilia,
        "cadastro_unico" => ServiceType::CadastroUnico,
        "bpc" => ServiceType::Bpc,
        _ => ServiceType::OutroAtendimento,
    };
    sqlx::query(
        "INSERT INTO triagem_answers (session_id, question_id, value) VALUES ($1, 'precisa_hoje', $2)",
    )
    .bind(sid)
    .bind(&label)
    .execute(db)
    .await?;
    sqlx::query("UPDATE triagem_sessions SET result_service = $1 WHERE id = $2")
        .bind(service)
        .bind(sid)
        .execute(db)
        .await?;
    if parser_hit {
        wa.send_text(chat_id, Q2).await?;
    }
    Ok(())
}

async fn handle_q2(
    db: &PgPool,
    wa: &WhatsappService,
    agent: &TriagemAgent,
    sid: Uuid,
    chat_id: &str,
    normalized: &str,
    raw_text: &str,
) -> anyhow::Result<()> {
    let val = match parse_q2(normalized) {
        Some(v) => v.to_string(),
        None => match agent.interpret(sid, AgentStep::Q2, raw_text).await {
            AgentOutcome::Recorded { value, reply } => {
                wa.send_text(chat_id, &reply).await?;
                value
            }
            AgentOutcome::OnlyReply(reply) => {
                wa.send_text(chat_id, &reply).await?;
                return Ok(());
            }
            AgentOutcome::Failed => {
                wa.send_text(chat_id, AGENT_ERROR_FALLBACK).await?;
                return Ok(());
            }
        },
    };
    let parser_hit = parse_q2(normalized).is_some();
    sqlx::query(
        "INSERT INTO triagem_answers (session_id, question_id, value) VALUES ($1, 'tem_cadastro', $2)",
    )
    .bind(sid)
    .bind(&val)
    .execute(db)
    .await?;
    if parser_hit {
        wa.send_text(chat_id, Q3).await?;
    }
    Ok(())
}

async fn handle_q3(
    db: &PgPool,
    wa: &WhatsappService,
    agent: &TriagemAgent,
    sid: Uuid,
    chat_id: &str,
    from_phone: &str,
    push_name: Option<&str>,
    text: &str,
) -> anyhow::Result<()> {
    let nis = match parse_q3(text) {
        Some(n) => n,
        None => match agent.interpret(sid, AgentStep::Q3, text).await {
            AgentOutcome::Recorded { value, reply } => {
                wa.send_text(chat_id, &reply).await?;
                value
            }
            AgentOutcome::OnlyReply(reply) => {
                wa.send_text(chat_id, &reply).await?;
                return Ok(());
            }
            AgentOutcome::Failed => {
                wa.send_text(chat_id, AGENT_ERROR_FALLBACK).await?;
                return Ok(());
            }
        },
    };
    sqlx::query(
        "INSERT INTO triagem_answers (session_id, question_id, value) VALUES ($1, 'nis_cpf', $2)",
    )
    .bind(sid)
    .bind(&nis)
    .execute(db)
    .await?;
    finalize(db, wa, sid, &nis, chat_id, from_phone, push_name).await
}

async fn finalize(
    db: &PgPool,
    wa: &WhatsappService,
    sid: Uuid,
    nis: &str,
    chat_id: &str,
    from_phone: &str,
    push_name: Option<&str>,
) -> anyhow::Result<()> {
    let row: (Option<ServiceType>,) =
        sqlx::query_as("SELECT result_service FROM triagem_sessions WHERE id = $1")
            .bind(sid)
            .fetch_one(db)
            .await?;
    let service = row.0.unwrap_or(ServiceType::OutroAtendimento);

    let (unit_id, docs) = route_service(service);

    // Upsert a minimal profile so the FK on appointments.nis is satisfied.
    // opt_in flips to true here — the user reached us via the official channel
    // and is providing their NIS willingly. LGPD record source: 'whatsapp'.
    sqlx::query(
        "INSERT INTO profiles (nis, name, phone, opt_in, opt_in_at)
         VALUES ($1, $2, $3, true, now())
         ON CONFLICT (nis) DO UPDATE
            SET phone     = COALESCE(profiles.phone, EXCLUDED.phone),
                opt_in    = true,
                opt_in_at = COALESCE(profiles.opt_in_at, now())",
    )
    .bind(nis)
    .bind(push_name.unwrap_or("Triagem WhatsApp"))
    .bind(from_phone)
    .execute(db)
    .await?;
    sqlx::query(
        "INSERT INTO opt_in_log (nis, opt_in, source) VALUES ($1, true, 'whatsapp')",
    )
    .bind(nis)
    .execute(db)
    .await
    .ok();

    let code = format!("AG-{:05}", rand::thread_rng().gen_range(10_000..100_000));
    let scheduled = next_business_day_9am_utc();
    let appt_id: Uuid = sqlx::query_scalar(
        "INSERT INTO appointments (code, nis, service, unit_id, scheduled_at, required_documents, status)
         VALUES ($1, $2, $3, $4, $5, $6, 'confirmado')
         RETURNING id",
    )
    .bind(&code)
    .bind(nis)
    .bind(service)
    .bind(unit_id)
    .bind(scheduled)
    .bind(&docs)
    .fetch_one(db)
    .await?;

    sqlx::query(
        "UPDATE triagem_sessions
            SET completed_at         = now(),
                nis                  = $2,
                result_unit_id       = $3,
                result_documents     = $4,
                result_appointment_id = $5
          WHERE id = $1",
    )
    .bind(sid)
    .bind(nis)
    .bind(unit_id)
    .bind(&docs)
    .bind(appt_id)
    .execute(db)
    .await?;

    let unit: (String, String) =
        sqlx::query_as("SELECT name, address FROM units WHERE id = $1")
            .bind(unit_id)
            .fetch_one(db)
            .await?;

    let docs_list = docs
        .iter()
        .map(|d| format!("• {d}"))
        .collect::<Vec<_>>()
        .join("\n");
    let confirmation = format!(
        "✅ *Agendamento confirmado!*\n\n\
         📌 *Serviço:* {svc}\n\
         🏢 *Unidade:* {uname}\n\
         📍 *Endereço:* {addr}\n\
         📅 *Data:* {date}\n\n\
         📄 *Documentos para levar:*\n{docs}\n\n\
         🎫 *Código:* {code}\n\n\
         _Para iniciar nova triagem, envie *novo*._",
        svc = service_label_pt(service),
        uname = unit.0,
        addr = unit.1,
        date = scheduled.format("%d/%m/%Y às %H:%M"),
        docs = docs_list,
        code = code,
    );
    wa.send_text(chat_id, &confirmation).await?;
    tracing::info!(target: "triagem", sid = %sid, %code, ?service, "session completed");
    Ok(())
}

async fn start_session(db: &PgPool, from_phone: &str) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO triagem_sessions (channel, from_phone) VALUES ($1, $2) RETURNING id",
    )
    .bind(TriagemChannel::Whatsapp)
    .bind(from_phone)
    .fetch_one(db)
    .await?;
    Ok(id)
}

async fn close_active_sessions(db: &PgPool, from_phone: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE triagem_sessions SET completed_at = now()
          WHERE from_phone = $1 AND completed_at IS NULL",
    )
    .bind(from_phone)
    .execute(db)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ActiveSession {
    id: Uuid,
    n_answers: i64,
}

// ===== parsers =====

fn parse_q1(normalized: &str) -> Option<(&'static str, ServiceType)> {
    if normalized == "1" || normalized.contains("bolsa") {
        Some(("bolsa_familia", ServiceType::BolsaFamilia))
    } else if normalized == "2" || normalized.contains("cadastro") || normalized.contains("cadunico") {
        Some(("cadastro_unico", ServiceType::CadastroUnico))
    } else if normalized == "3" || normalized.contains("bpc") {
        Some(("bpc", ServiceType::Bpc))
    } else if normalized == "4" || normalized.contains("outro") {
        Some(("outro_atendimento", ServiceType::OutroAtendimento))
    } else if normalized == "5" || normalized.contains("nao sei") || normalized == "nsei" {
        Some(("nao_sei", ServiceType::OutroAtendimento))
    } else {
        None
    }
}

fn parse_q2(normalized: &str) -> Option<&'static str> {
    if matches!(normalized, "1" | "s" | "sim") {
        Some("sim")
    } else if matches!(normalized, "2" | "n" | "nao") {
        Some("nao")
    } else if normalized == "3" || normalized.contains("nao sei") {
        Some("nao_sei")
    } else {
        None
    }
}

fn parse_q3(text: &str) -> Option<String> {
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    (digits.len() == 11).then_some(digits)
}

fn normalize(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}

// ===== routing =====

fn route_service(s: ServiceType) -> (&'static str, Vec<String>) {
    let docs = |xs: &[&str]| xs.iter().map(|x| (*x).to_string()).collect();
    match s {
        ServiceType::BolsaFamilia => (
            "cras-centro",
            docs(&["RG", "CPF", "Comprovante de residência"]),
        ),
        ServiceType::CadastroUnico => (
            "cras-centro",
            docs(&[
                "RG",
                "CPF",
                "Comprovante de residência",
                "Certidão de nascimento dos filhos",
            ]),
        ),
        ServiceType::Bpc => (
            "creas",
            docs(&[
                "RG",
                "CPF",
                "Comprovante de residência",
                "Laudo médico",
                "Comprovante de renda",
            ]),
        ),
        ServiceType::OutroAtendimento => ("cras-centro", docs(&["RG", "CPF"])),
    }
}

fn service_label_pt(s: ServiceType) -> &'static str {
    match s {
        ServiceType::BolsaFamilia => "Bolsa Família",
        ServiceType::CadastroUnico => "Cadastro Único",
        ServiceType::Bpc => "BPC",
        ServiceType::OutroAtendimento => "Outro atendimento",
    }
}

fn next_business_day_9am_utc() -> DateTime<Utc> {
    // 09:00 BRT == 12:00 UTC (BRT = UTC-3, no DST since 2019).
    let mut d = Utc::now().date_naive() + Duration::days(1);
    while matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
        d += Duration::days(1);
    }
    let t = NaiveTime::from_hms_opt(12, 0, 0).expect("valid time");
    DateTime::from_naive_utc_and_offset(d.and_time(t), Utc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q1_number_keyword_and_diacritics() {
        assert_eq!(parse_q1("1"), Some(("bolsa_familia", ServiceType::BolsaFamilia)));
        assert_eq!(
            parse_q1(&normalize("Bolsa Família")),
            Some(("bolsa_familia", ServiceType::BolsaFamilia))
        );
        assert_eq!(
            parse_q1(&normalize("Cadastro Único")),
            Some(("cadastro_unico", ServiceType::CadastroUnico))
        );
        assert_eq!(parse_q1("bpc"), Some(("bpc", ServiceType::Bpc)));
        assert_eq!(
            parse_q1(&normalize("não sei")),
            Some(("nao_sei", ServiceType::OutroAtendimento))
        );
        assert_eq!(parse_q1("xyz"), None);
    }

    #[test]
    fn q2_parses() {
        assert_eq!(parse_q2("sim"), Some("sim"));
        assert_eq!(parse_q2("s"), Some("sim"));
        assert_eq!(parse_q2("1"), Some("sim"));
        assert_eq!(parse_q2(&normalize("Não")), Some("nao"));
        assert_eq!(parse_q2("2"), Some("nao"));
        assert_eq!(parse_q2(&normalize("não sei")), Some("nao_sei"));
        assert_eq!(parse_q2("maybe"), None);
    }

    #[test]
    fn q3_extracts_11_digits() {
        assert_eq!(parse_q3("123.456.789-01"), Some("12345678901".into()));
        assert_eq!(parse_q3("  12345678901 "), Some("12345678901".into()));
        assert_eq!(parse_q3("123"), None);
        assert_eq!(parse_q3("123456789012"), None);
    }

    #[test]
    fn next_business_day_skips_weekend() {
        let dt = next_business_day_9am_utc();
        assert!(!matches!(dt.weekday(), Weekday::Sat | Weekday::Sun));
        assert_eq!(dt.time(), NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    }
}
