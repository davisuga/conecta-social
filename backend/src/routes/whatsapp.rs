use axum::{body::Bytes, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::services::whatsapp::parse_webhook;

/// POST /api/whatsapp/webhook
///
/// Evolution-go inbound. Always responds 200 so the provider doesn't retry.
/// Parses the envelope and logs the message; triagem state machine is
/// dispatched downstream (TODO).
pub async fn webhook(body: Bytes) -> impl IntoResponse {
    match parse_webhook(&body) {
        Ok(Some(msg)) => {
            tracing::info!(
                target: "whatsapp",
                from = %msg.from_phone,
                push_name = msg.push_name.as_deref().unwrap_or(""),
                text = %msg.text,
                "inbound"
            );
            (StatusCode::OK, Json(json!({ "ok": true, "handled": true })))
        }
        Ok(None) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "handled": false, "reason": "ignored" })),
        ),
        Err(e) => {
            tracing::warn!(target: "whatsapp", error = %e, "webhook parse failed");
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "handled": false, "parse_error": e.to_string() })),
            )
        }
    }
}
