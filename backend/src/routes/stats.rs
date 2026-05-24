use axum::{extract::State, Json};
use serde_json::json;

use crate::{error::ApiResult, state::AppState};

#[derive(sqlx::FromRow)]
struct StatsSummaryRow {
    messages_total: i64,
    messages_today: i64,
    appointments_total: i64,
    appointments_today: i64,
    profiles_active: i64,
    opt_in_granted: i64,
    opt_in_total: i64,
}

pub async fn summary(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let row = sqlx::query_as::<_, StatsSummaryRow>(
        "SELECT messages_total, messages_today, appointments_total, appointments_today, \
         profiles_active, opt_in_granted, opt_in_total FROM v_stats_summary",
    )
    .fetch_one(&state.db)
    .await?;

    let rate = if row.opt_in_total == 0 {
        0.0
    } else {
        row.opt_in_granted as f64 / row.opt_in_total as f64
    };

    Ok(Json(json!({
        "messages":     { "total": row.messages_total,     "today": row.messages_today },
        "appointments": { "total": row.appointments_total, "today": row.appointments_today },
        "profiles":     { "active": row.profiles_active },
        "opt_in":       { "rate": rate, "granted": row.opt_in_granted, "total": row.opt_in_total }
    })))
}
