use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiError, ApiResult},
    models::{Channel, Message, MessageStatus, ProfileRow, TriggerType, PROFILE_SELECT},
    services::triggers as trigger_service,
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct TriggerCatalogEntry {
    #[serde(rename = "type")]
    r#type: TriggerType,
    label: String,
    description: String,
}

pub async fn list_triggers() -> ApiResult<Json<Vec<TriggerCatalogEntry>>> {
    let items = trigger_service::catalog()
        .into_iter()
        .map(|m| TriggerCatalogEntry {
            r#type: m.r#type,
            label: m.label.to_string(),
            description: m.description.to_string(),
        })
        .collect();
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
pub struct EvaluateBody {
    pub nis: Option<String>,
}

fn is_valid_nis(nis: &str) -> bool {
    nis.len() == 11 && nis.chars().all(|c| c.is_ascii_digit())
}

async fn insert_messages_for_profile(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    profile: &ProfileRow,
) -> ApiResult<Vec<Message>> {
    let triggers = trigger_service::applicable_triggers(profile);
    let mut out = Vec::with_capacity(triggers.len());
    for t in triggers {
        let body = trigger_service::message_body(t, &profile.name);
        let msg: Message = sqlx::query_as::<_, Message>(
            "INSERT INTO messages (nis, trigger, channel, body, status, sent_at) \
             VALUES ($1, $2::trigger_type, $3::channel, $4, $5::message_status, now()) \
             RETURNING id, nis, trigger, channel, status, body, sent_at, created_at",
        )
        .bind(&profile.nis)
        .bind(t)
        .bind(Channel::Whatsapp)
        .bind(&body)
        .bind(MessageStatus::Sent)
        .fetch_one(&mut **tx)
        .await?;
        out.push(msg);
    }
    Ok(out)
}

pub async fn evaluate_triggers(
    State(state): State<AppState>,
    Json(body): Json<EvaluateBody>,
) -> ApiResult<Json<Vec<Message>>> {
    let mut created: Vec<Message> = Vec::new();
    let mut tx = state.db.begin().await?;

    match body.nis {
        Some(nis) => {
            if !is_valid_nis(&nis) {
                return Err(ApiError::BadRequest(
                    "nis must contain exactly 11 digits".into(),
                ));
            }
            let select_sql = format!("SELECT {PROFILE_SELECT} FROM profiles WHERE nis = $1");
            let profile: ProfileRow = sqlx::query_as::<_, ProfileRow>(&select_sql)
                .bind(&nis)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(ApiError::NotFound)?;
            let mut msgs = insert_messages_for_profile(&mut tx, &profile).await?;
            created.append(&mut msgs);
        }
        None => {
            let select_sql = format!(
                "SELECT {PROFILE_SELECT} FROM profiles WHERE opt_in = true ORDER BY updated_at DESC LIMIT 100"
            );
            let profiles: Vec<ProfileRow> = sqlx::query_as::<_, ProfileRow>(&select_sql)
                .fetch_all(&mut *tx)
                .await?;
            for p in &profiles {
                let mut msgs = insert_messages_for_profile(&mut tx, p).await?;
                created.append(&mut msgs);
            }
        }
    }

    tx.commit().await?;
    Ok(Json(created))
}
