//! Background jobs (cron).
//!
//! Daily alerts: once a day, scan opt-in profiles and dispatch at most one
//! WhatsApp message per family with the highest-priority applicable trigger.
//! Skips profiles that already received any message in the current day to
//! enforce the "uma mensagem por família por dia" rule.

use std::env;

use sqlx::PgPool;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};

use crate::models::{ProfileRow, PROFILE_SELECT};
use crate::services::triggers as trigger_service;
use crate::services::whatsapp::WhatsappService;

/// Default cron: 08:00 BRT (= 11:00 UTC) every day. 6-field tokio-cron-scheduler
/// format: `sec min hour dom mon dow`.
const DEFAULT_CRON: &str = "0 0 11 * * *";

#[derive(Debug, Default, serde::Serialize)]
pub struct DailyAlertsReport {
    pub profiles_scanned: usize,
    pub messages_sent: usize,
    pub skipped_no_trigger: usize,
    pub skipped_already_sent: usize,
    pub skipped_no_phone: usize,
    pub failures: usize,
}

/// Start the scheduler. Reads `ALERTS_CRON` from env (default `0 0 11 * * *`)
/// and `ALERTS_ENABLED` (default true). Returns the scheduler so callers can
/// keep it alive (drop = shutdown).
pub async fn start_scheduler(db: PgPool) -> Result<Option<JobScheduler>, JobSchedulerError> {
    let enabled = env::var("ALERTS_ENABLED")
        .map(|v| !matches!(v.as_str(), "0" | "false" | "no"))
        .unwrap_or(true);
    if !enabled {
        tracing::info!("alerts cron disabled via ALERTS_ENABLED");
        return Ok(None);
    }

    let cron_expr = env::var("ALERTS_CRON").unwrap_or_else(|_| DEFAULT_CRON.to_string());

    let sched = JobScheduler::new().await?;

    let db_for_job = db.clone();
    let job = Job::new_async(cron_expr.as_str(), move |_uuid, _l| {
        let db = db_for_job.clone();
        Box::pin(async move {
            match run_daily_alerts(&db).await {
                Ok(report) => tracing::info!(
                    profiles = report.profiles_scanned,
                    sent = report.messages_sent,
                    skipped_no_trigger = report.skipped_no_trigger,
                    skipped_already_sent = report.skipped_already_sent,
                    skipped_no_phone = report.skipped_no_phone,
                    failures = report.failures,
                    "daily alerts run complete"
                ),
                Err(err) => tracing::error!(error = %err, "daily alerts run failed"),
            }
        })
    })?;
    sched.add(job).await?;
    sched.start().await?;
    tracing::info!(cron = %cron_expr, "alerts cron scheduled");
    Ok(Some(sched))
}

/// Evaluate all opt-in profiles, pick the highest-priority trigger per
/// profile, and dispatch via WhatsApp. Skips families that already received a
/// message today.
pub async fn run_daily_alerts(db: &PgPool) -> Result<DailyAlertsReport, sqlx::Error> {
    let whatsapp = WhatsappService::from_env(db.clone());
    let mut report = DailyAlertsReport::default();

    let select_sql = format!(
        "SELECT {PROFILE_SELECT} FROM profiles WHERE opt_in = true ORDER BY updated_at ASC"
    );
    let profiles: Vec<ProfileRow> = sqlx::query_as::<_, ProfileRow>(&select_sql)
        .fetch_all(db)
        .await?;

    for p in profiles {
        report.profiles_scanned += 1;

        if p.phone.as_deref().map(str::trim).unwrap_or("").is_empty() {
            report.skipped_no_phone += 1;
            continue;
        }

        let Some(trigger) = trigger_service::pick_one(&p) else {
            report.skipped_no_trigger += 1;
            continue;
        };

        let already: Option<(i64,)> = sqlx::query_as(
            "SELECT 1::bigint FROM messages \
             WHERE nis = $1 AND created_at >= date_trunc('day', now()) \
             LIMIT 1",
        )
        .bind(&p.nis)
        .fetch_optional(db)
        .await?;
        if already.is_some() {
            report.skipped_already_sent += 1;
            continue;
        }

        match whatsapp.send_trigger(&p.nis, trigger).await {
            Ok(_) => report.messages_sent += 1,
            Err(err) => {
                report.failures += 1;
                tracing::warn!(nis = %p.nis, ?trigger, error = %err, "send_trigger failed");
            }
        }
    }

    Ok(report)
}
