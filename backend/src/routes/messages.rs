use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use sqlx::{Postgres, QueryBuilder};

use crate::{
    error::{ApiError, ApiResult},
    models::{Channel, ListResponse, Message, MessageStatus, ProfileRow, TriggerType, PROFILE_SELECT},
    services::triggers as trigger_service,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub trigger: Option<TriggerType>,
    pub channel: Option<Channel>,
    pub status: Option<MessageStatus>,
}

fn push_filters<'a>(
    qb: &mut QueryBuilder<'a, Postgres>,
    trigger: Option<&'a TriggerType>,
    channel: Option<&'a Channel>,
    status: Option<&'a MessageStatus>,
) {
    let mut first = true;
    let mut start = |qb: &mut QueryBuilder<'a, Postgres>| {
        if first {
            qb.push(" WHERE ");
            first = false;
        } else {
            qb.push(" AND ");
        }
    };

    if let Some(t) = trigger {
        start(qb);
        qb.push("trigger = ").push_bind(t);
    }
    if let Some(c) = channel {
        start(qb);
        qb.push("channel = ").push_bind(c);
    }
    if let Some(s) = status {
        start(qb);
        qb.push("status = ").push_bind(s);
    }
}

pub async fn list_messages(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<ListResponse<Message>>> {
    let limit = q.limit.unwrap_or(50).min(200) as i64;
    let offset = q.offset.unwrap_or(0) as i64;

    let mut data_qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, nis, trigger, channel, status, body, sent_at, created_at FROM messages",
    );
    push_filters(
        &mut data_qb,
        q.trigger.as_ref(),
        q.channel.as_ref(),
        q.status.as_ref(),
    );
    data_qb.push(" ORDER BY created_at DESC LIMIT ");
    data_qb.push_bind(limit);
    data_qb.push(" OFFSET ");
    data_qb.push_bind(offset);

    let items: Vec<Message> = data_qb
        .build_query_as::<Message>()
        .fetch_all(&state.db)
        .await?;

    let mut count_qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT count(*) FROM messages");
    push_filters(
        &mut count_qb,
        q.trigger.as_ref(),
        q.channel.as_ref(),
        q.status.as_ref(),
    );
    let total: i64 = count_qb
        .build_query_scalar::<i64>()
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ListResponse { items, total }))
}

#[derive(Debug, Deserialize)]
pub struct RecentQuery {
    pub limit: Option<u32>,
}

pub async fn recent_messages(
    State(state): State<AppState>,
    Query(q): Query<RecentQuery>,
) -> ApiResult<Json<Vec<Message>>> {
    let limit = q.limit.unwrap_or(5).min(50) as i64;
    let items: Vec<Message> = sqlx::query_as::<_, Message>(
        "SELECT id, nis, trigger, channel, status, body, sent_at, created_at \
         FROM messages ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
pub struct DispatchBody {
    pub nis: String,
    pub trigger: TriggerType,
    pub channel: Option<Channel>,
}

fn is_valid_nis(nis: &str) -> bool {
    nis.len() == 11 && nis.chars().all(|c| c.is_ascii_digit())
}

pub async fn dispatch_message(
    State(state): State<AppState>,
    Json(body): Json<DispatchBody>,
) -> ApiResult<Json<Message>> {
    if !is_valid_nis(&body.nis) {
        return Err(ApiError::BadRequest(
            "nis must contain exactly 11 digits".into(),
        ));
    }

    let select_sql = format!("SELECT {PROFILE_SELECT} FROM profiles WHERE nis = $1");
    let profile: ProfileRow = sqlx::query_as::<_, ProfileRow>(&select_sql)
        .bind(&body.nis)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    let channel = body.channel.unwrap_or(Channel::Whatsapp);
    let text = trigger_service::message_body(body.trigger, &profile.name);

    let inserted: Message = sqlx::query_as::<_, Message>(
        "INSERT INTO messages (nis, trigger, channel, body, status, sent_at) \
         VALUES ($1, $2::trigger_type, $3::channel, $4, $5::message_status, now()) \
         RETURNING id, nis, trigger, channel, status, body, sent_at, created_at",
    )
    .bind(&body.nis)
    .bind(body.trigger)
    .bind(channel)
    .bind(&text)
    .bind(MessageStatus::Sent)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(inserted))
}
