use axum::{extract::State, Json};

use crate::{
    error::ApiResult,
    services::jobs::{self, DailyAlertsReport},
    state::AppState,
};

/// POST /api/cron/run-daily-alerts
///
/// Trigger the daily alerts job on-demand. Same code path as the cron tick,
/// useful for demos and ops smoke-tests.
pub async fn run_daily_alerts(
    State(state): State<AppState>,
) -> ApiResult<Json<DailyAlertsReport>> {
    let report = jobs::run_daily_alerts(&state.db).await?;
    Ok(Json(report))
}
