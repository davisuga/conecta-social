use axum::{body::Bytes, extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::services::{triagem_chat, whatsapp::parse_webhook};
use crate::state::AppState;

/// POST /api/whatsapp/webhook
///
/// Evolution-go inbound. Always returns 200 so the provider doesn't retry.
/// Parses envelope → dispatches to triagem state machine.
pub async fn webhook(State(state): State<AppState>, body: Bytes) -> impl IntoResponse {
    let parsed = match parse_webhook(&body) {
        Ok(Some(m)) => m,
        Ok(None) => {
            return (
                StatusCode::OK,
                Json(json!({ "ok": true, "handled": false, "reason": "ignored" })),
            );
        }
        Err(e) => {
            tracing::warn!(target: "whatsapp", error = %e, "webhook parse failed");
            return (
                StatusCode::OK,
                Json(json!({ "ok": true, "handled": false, "parse_error": e.to_string() })),
            );
        }
    };

    tracing::info!(
        target: "whatsapp",
        from = %parsed.from_phone,
        push_name = parsed.push_name.as_deref().unwrap_or(""),
        text = %parsed.text,
        "inbound"
    );

    if let Err(err) = triagem_chat::handle_inbound(
        &state.db,
        &state.whatsapp,
        &state.agent,
        &parsed.chat_id,
        &parsed.from_phone,
        parsed.push_name.as_deref(),
        &parsed.text,
    )
    .await
    {
        tracing::error!(target: "triagem", error = ?err, "handler failed");
        return (
            StatusCode::OK,
            Json(json!({ "ok": true, "handled": false, "error": err.to_string() })),
        );
    }

    (StatusCode::OK, Json(json!({ "ok": true, "handled": true })))
}
