pub mod appointments;
pub mod cron;
pub mod health;
pub mod messages;
pub mod profiles;
pub mod stats;
pub mod triagem;
pub mod triggers;
pub mod units;

use axum::{
    routing::{get, patch, post},
    Router,
};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/api/stats/summary", get(stats::summary))
        .route("/api/messages", get(messages::list_messages))
        .route("/api/messages/recent", get(messages::recent_messages))
        .route("/api/messages/dispatch", post(messages::dispatch_message))
        .route(
            "/api/appointments",
            get(appointments::list).post(appointments::create),
        )
        .route("/api/appointments/recent", get(appointments::recent))
        .route("/api/appointments/:id", patch(appointments::patch_status))
        .route("/api/profiles", get(profiles::list))
        .route("/api/profiles/:nis", get(profiles::get))
        .route("/api/profiles/:nis/opt-in", post(profiles::opt_in))
        .route("/api/units", get(units::list))
        .route("/api/triagem/sessions", get(triagem::sessions))
        .route("/api/triagem/start", post(triagem::start))
        .route("/api/triagem/:id/answer", post(triagem::answer))
        .route("/api/triagem/:id/finalize", post(triagem::finalize))
        .route("/api/triggers", get(triggers::list_triggers))
        .route("/api/triggers/evaluate", post(triggers::evaluate_triggers))
        .route("/api/cron/run-daily-alerts", post(cron::run_daily_alerts))
        .with_state(state)
}
