//! WhatsApp handling service — [evolution-go](https://github.com/EvolutionAPI/evolution-go) provider.
//!
//! Outbound: `POST {base_url}/send/text` with `{number, text}` + `apikey` header.
//! Inbound: parse evolution-go webhook envelope (text messages only for MVP).
//! Persists to the `messages` table on send.
//! HTTP routes are not wired yet — this module is consumed by future handlers.

use std::env;

use chrono::Utc;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::models::{Channel, Message, MessageStatus, TriggerType};

#[derive(Debug, Clone)]
pub struct EvolutionConfig {
    /// Base URL, e.g. `http://localhost:8080`.
    pub base_url: String,
    /// Instance token, sent as `apikey: <token>`.
    pub api_key: String,
    /// If set, skips real HTTP and just logs. Useful for demo.
    pub simulated: bool,
}

#[derive(Clone)]
pub struct WhatsappService {
    pub db: PgPool,
    pub http: HttpClient,
    pub config: EvolutionConfig,
}

#[derive(Debug, Error)]
pub enum WhatsappError {
    #[error("profile {0} not found")]
    ProfileNotFound(String),
    #[error("profile {0} has no phone")]
    MissingPhone(String),
    #[error("profile {0} not opted in")]
    NotOptedIn(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

impl WhatsappService {
    pub fn from_env(db: PgPool) -> Self {
        let config = EvolutionConfig {
            base_url: env::var("EVOLUTION_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8081".into()),
            api_key: env::var("EVOLUTION_API_KEY").unwrap_or_default(),
            simulated: env::var("WHATSAPP_SIMULATED")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
        };
        Self {
            db,
            http: HttpClient::new(),
            config,
        }
    }

    /// Raw text dispatch. `chat` accepts a bare number or a full
    /// `5511...@s.whatsapp.net` JID — suffix is stripped before send.
    pub async fn send_text(&self, chat: &str, body: &str) -> Result<(), WhatsappError> {
        let number = chat.split('@').next().unwrap_or(chat);
        if number.trim().is_empty() {
            return Err(WhatsappError::Provider("empty number".into()));
        }
        if self.config.simulated {
            tracing::info!(target: "whatsapp", number, body_len = body.len(), "simulated send");
            return Ok(());
        }
        let url = format!("{}/send/text", self.config.base_url.trim_end_matches('/'));
        let res = self
            .http
            .post(&url)
            .header("apikey", &self.config.api_key)
            .json(&serde_json::json!({ "number": number, "text": body }))
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(WhatsappError::Provider(format!(
                "evolution send failed: {status} — {err_body}"
            )));
        }
        Ok(())
    }

    /// Render trigger template, persist `messages` row (status=queued),
    /// dispatch, then update to sent or failed. Returns the final row.
    pub async fn send_trigger(
        &self,
        nis: &str,
        trigger: TriggerType,
    ) -> Result<Message, WhatsappError> {
        let row = sqlx::query_as::<_, ProfileLookup>(
            "SELECT name, phone, opt_in FROM profiles WHERE nis = $1",
        )
        .bind(nis)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| WhatsappError::ProfileNotFound(nis.to_string()))?;

        if !row.opt_in {
            return Err(WhatsappError::NotOptedIn(nis.to_string()));
        }
        let phone = row
            .phone
            .clone()
            .ok_or_else(|| WhatsappError::MissingPhone(nis.to_string()))?;
        let body = render_template(trigger, &row.name);

        let id = Uuid::new_v4();
        let inserted: Message = sqlx::query_as(
            r#"
            INSERT INTO messages (id, nis, trigger, channel, status, body)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, nis, trigger, channel, status, body, sent_at, created_at
            "#,
        )
        .bind(id)
        .bind(nis)
        .bind(trigger)
        .bind(Channel::Whatsapp)
        .bind(MessageStatus::Queued)
        .bind(&body)
        .fetch_one(&self.db)
        .await?;

        match self.send_text(&phone, &inserted.body).await {
            Ok(()) => {
                let updated: Message = sqlx::query_as(
                    r#"
                    UPDATE messages
                       SET status = 'sent'::message_status, sent_at = $2
                     WHERE id = $1
                    RETURNING id, nis, trigger, channel, status, body, sent_at, created_at
                    "#,
                )
                .bind(id)
                .bind(Utc::now())
                .fetch_one(&self.db)
                .await?;
                Ok(updated)
            }
            Err(send_err) => {
                let err_str = send_err.to_string();
                let updated: Message = sqlx::query_as(
                    r#"
                    UPDATE messages
                       SET status = 'failed'::message_status, error = $2
                     WHERE id = $1
                    RETURNING id, nis, trigger, channel, status, body, sent_at, created_at
                    "#,
                )
                .bind(id)
                .bind(&err_str)
                .fetch_one(&self.db)
                .await?;
                tracing::warn!(target: "whatsapp", %nis, error = %err_str, "dispatch failed");
                Ok(updated)
            }
        }
    }

    /// Mark a message delivered. For provider callbacks.
    pub async fn mark_delivered(&self, message_id: Uuid) -> Result<(), WhatsappError> {
        sqlx::query(
            "UPDATE messages
                SET status = 'delivered'::message_status,
                    delivered_at = COALESCE(delivered_at, now())
              WHERE id = $1",
        )
        .bind(message_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ProfileLookup {
    name: String,
    phone: Option<String>,
    opt_in: bool,
}

fn render_template(trigger: TriggerType, name: &str) -> String {
    let first = name.split_whitespace().next().unwrap_or(name);
    match trigger {
        TriggerType::BolsaFamiliaElegivel => format!(
            "Olá, {first}! Você pode ter direito ao Bolsa Família. \
             Procure o CRAS mais próximo com RG, CPF e comprovante de residência para regularizar."
        ),
        TriggerType::RiscoCondicionalidade => format!(
            "Atenção, {first}: o prazo de condicionalidade do Bolsa Família está próximo. \
             Compareça ao CRAS o quanto antes para evitar bloqueio do benefício."
        ),
        TriggerType::RecadastramentoProximo => format!(
            "Olá, {first}! Seu CadÚnico vence em breve. \
             Agende seu recadastramento no CRAS de referência para manter os benefícios ativos."
        ),
        TriggerType::BpcNaoRequerido => format!(
            "Olá, {first}! Identificamos perfil elegível ao BPC. \
             Procure o CRAS para iniciar o requerimento."
        ),
        TriggerType::PerfilScfv => format!(
            "Olá, {first}! Convidamos você para o Serviço de Convivência e \
             Fortalecimento de Vínculos (SCFV). Saiba mais no CRAS."
        ),
    }
}

// ===== Inbound webhook =====

/// Normalized inbound text message from evolution-go.
#[derive(Debug, Clone, Serialize)]
pub struct InboundMessage {
    /// Raw JID, e.g. `5511999999999@s.whatsapp.net`.
    pub chat_id: String,
    /// Bare number, suffix stripped.
    pub from_phone: String,
    pub text: String,
    pub push_name: Option<String>,
}

/// Parse an evolution-go webhook body. Returns `None` for non-text events,
/// self-messages, group chats, or unknown shapes.
pub fn parse_webhook(body: &[u8]) -> Result<Option<InboundMessage>, serde_json::Error> {
    let event: WebhookEvent = serde_json::from_slice(body)?;
    if !event.event.eq_ignore_ascii_case("message") {
        return Ok(None);
    }
    let info = event.data.info;
    if info.is_from_me || info.is_group || !info.r#type.eq_ignore_ascii_case("text") {
        return Ok(None);
    }
    let msg = event.data.message;
    let Some(text) = msg.conversation.or_else(|| {
        msg.extended_text_message
            .and_then(|e| e.text)
    }) else {
        return Ok(None);
    };
    let chat_id = info.chat;
    let from_phone = chat_id.split('@').next().unwrap_or(&chat_id).to_string();
    Ok(Some(InboundMessage {
        chat_id,
        from_phone,
        text,
        push_name: info.push_name,
    }))
}

#[derive(Debug, Deserialize)]
struct WebhookEvent {
    event: String,
    data: WebhookData,
}

#[derive(Debug, Deserialize)]
struct WebhookData {
    #[serde(rename = "Info")]
    info: WebhookInfo,
    #[serde(rename = "Message")]
    message: WebhookMessage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WebhookInfo {
    chat: String,
    #[serde(default)]
    push_name: Option<String>,
    #[serde(default)]
    is_from_me: bool,
    #[serde(default)]
    is_group: bool,
    /// `"text"` or `"media"`.
    r#type: String,
}

#[derive(Debug, Deserialize)]
struct WebhookMessage {
    #[serde(default)]
    conversation: Option<String>,
    #[serde(default, rename = "extendedTextMessage")]
    extended_text_message: Option<ExtendedTextMessage>,
}

#[derive(Debug, Deserialize)]
struct ExtendedTextMessage {
    #[serde(default)]
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body(v: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&v).unwrap()
    }

    #[test]
    fn render_uses_first_name() {
        let b = render_template(TriggerType::BolsaFamiliaElegivel, "Maria da Silva");
        assert!(b.contains("Maria"));
        assert!(!b.contains("da Silva"));
    }

    #[test]
    fn parses_conversation_text() {
        let b = body(json!({
            "event": "Message",
            "data": {
                "Info": {
                    "Chat": "5511999999999@s.whatsapp.net",
                    "PushName": "Alice",
                    "IsFromMe": false,
                    "IsGroup": false,
                    "Type": "text"
                },
                "Message": { "conversation": "oi" }
            }
        }));
        let m = parse_webhook(&b).unwrap().unwrap();
        assert_eq!(m.from_phone, "5511999999999");
        assert_eq!(m.chat_id, "5511999999999@s.whatsapp.net");
        assert_eq!(m.text, "oi");
        assert_eq!(m.push_name.as_deref(), Some("Alice"));
    }

    #[test]
    fn parses_extended_text() {
        let b = body(json!({
            "event": "Message",
            "data": {
                "Info": { "Chat": "x@s.whatsapp.net", "IsFromMe": false, "IsGroup": false, "Type": "text" },
                "Message": { "extendedTextMessage": { "text": "quoted" } }
            }
        }));
        let m = parse_webhook(&b).unwrap().unwrap();
        assert_eq!(m.text, "quoted");
    }

    #[test]
    fn drops_self_messages() {
        let b = body(json!({
            "event": "Message",
            "data": {
                "Info": { "Chat": "x@s.whatsapp.net", "IsFromMe": true, "IsGroup": false, "Type": "text" },
                "Message": { "conversation": "echo" }
            }
        }));
        assert!(parse_webhook(&b).unwrap().is_none());
    }

    #[test]
    fn drops_group_messages() {
        let b = body(json!({
            "event": "Message",
            "data": {
                "Info": { "Chat": "g@g.us", "IsFromMe": false, "IsGroup": true, "Type": "text" },
                "Message": { "conversation": "hi" }
            }
        }));
        assert!(parse_webhook(&b).unwrap().is_none());
    }

    #[test]
    fn drops_media() {
        let b = body(json!({
            "event": "Message",
            "data": {
                "Info": { "Chat": "x@s.whatsapp.net", "IsFromMe": false, "IsGroup": false, "Type": "media" },
                "Message": {}
            }
        }));
        assert!(parse_webhook(&b).unwrap().is_none());
    }

    #[test]
    fn drops_non_message_events() {
        let b = body(json!({
            "event": "Connect",
            "data": {
                "Info": { "Chat": "x", "IsFromMe": false, "IsGroup": false, "Type": "text" },
                "Message": { "conversation": "hi" }
            }
        }));
        assert!(parse_webhook(&b).unwrap().is_none());
    }
}
