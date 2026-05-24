use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::Deserialize;
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    models::{
        Appointment, AppointmentRow, AppointmentStatus, ListResponse, ServiceType,
        APPOINTMENT_SELECT,
    },
    state::AppState,
};

fn default_limit() -> i64 {
    50
}

fn default_offset() -> i64 {
    0
}

fn is_valid_nis(s: &str) -> bool {
    s.len() == 11 && s.bytes().all(|b| b.is_ascii_digit())
}

fn generate_code() -> String {
    let n: u32 = rand::thread_rng().gen_range(0..100_000);
    format!("AG-{:05}", n)
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
    pub service: Option<ServiceType>,
    pub status: Option<AppointmentStatus>,
    pub unit_id: Option<String>,
}

fn push_filters<'a>(
    qb: &mut QueryBuilder<'a, Postgres>,
    service: Option<&'a ServiceType>,
    status: Option<&'a AppointmentStatus>,
    unit_id: Option<&'a String>,
) {
    if let Some(s) = service {
        qb.push(" AND a.service = ");
        qb.push_bind(s);
        qb.push("::service_type");
    }
    if let Some(s) = status {
        qb.push(" AND a.status = ");
        qb.push_bind(s);
        qb.push("::appointment_status");
    }
    if let Some(u) = unit_id {
        qb.push(" AND a.unit_id = ");
        qb.push_bind(u);
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<ListResponse<Appointment>>> {
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);

    let mut data_qb: QueryBuilder<Postgres> = QueryBuilder::new(format!(
        "SELECT {APPOINTMENT_SELECT} FROM appointments a \
         JOIN units u ON u.id = a.unit_id WHERE 1=1"
    ));
    push_filters(
        &mut data_qb,
        q.service.as_ref(),
        q.status.as_ref(),
        q.unit_id.as_ref(),
    );
    data_qb.push(" ORDER BY a.created_at DESC LIMIT ");
    data_qb.push_bind(limit);
    data_qb.push(" OFFSET ");
    data_qb.push_bind(offset);

    let rows: Vec<AppointmentRow> = data_qb
        .build_query_as::<AppointmentRow>()
        .fetch_all(&state.db)
        .await?;
    let items: Vec<Appointment> = rows.into_iter().map(Appointment::from).collect();

    let mut count_qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT count(*) FROM appointments a WHERE 1=1");
    push_filters(
        &mut count_qb,
        q.service.as_ref(),
        q.status.as_ref(),
        q.unit_id.as_ref(),
    );
    let total: i64 = count_qb
        .build_query_scalar::<i64>()
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ListResponse { items, total }))
}

#[derive(Debug, Deserialize)]
pub struct RecentQuery {
    pub limit: Option<i64>,
}

pub async fn recent(
    State(state): State<AppState>,
    Query(q): Query<RecentQuery>,
) -> ApiResult<Json<Vec<Appointment>>> {
    let limit = q.limit.unwrap_or(5).clamp(1, 50);

    let sql = format!(
        "SELECT {APPOINTMENT_SELECT} FROM appointments a \
         JOIN units u ON u.id = a.unit_id \
         ORDER BY a.created_at DESC LIMIT $1"
    );

    let rows: Vec<AppointmentRow> = sqlx::query_as::<_, AppointmentRow>(&sql)
        .bind(limit)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(rows.into_iter().map(Appointment::from).collect()))
}

#[derive(Debug, Deserialize)]
pub struct CreateReq {
    pub nis: String,
    pub service: ServiceType,
    pub unit_id: String,
    pub scheduled_at: DateTime<Utc>,
    #[serde(default)]
    pub required_documents: Vec<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateReq>,
) -> ApiResult<Json<Appointment>> {
    if !is_valid_nis(&body.nis) {
        return Err(ApiError::BadRequest(
            "nis must contain exactly 11 digits".into(),
        ));
    }

    let mut tx = state.db.begin().await?;

    let unit_exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM units WHERE id = $1")
        .bind(&body.unit_id)
        .fetch_optional(&mut *tx)
        .await?;
    if unit_exists.is_none() {
        return Err(ApiError::Conflict("unknown unit".into()));
    }

    let profile_exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM profiles WHERE nis = $1")
        .bind(&body.nis)
        .fetch_optional(&mut *tx)
        .await?;
    if profile_exists.is_none() {
        return Err(ApiError::NotFound);
    }

    let code = generate_code();

    let new_id: Uuid = sqlx::query_scalar(
        "INSERT INTO appointments (code, nis, service, unit_id, scheduled_at, required_documents) \
         VALUES ($1, $2, $3::service_type, $4, $5, $6) RETURNING id",
    )
    .bind(&code)
    .bind(&body.nis)
    .bind(body.service)
    .bind(&body.unit_id)
    .bind(body.scheduled_at)
    .bind(&body.required_documents)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let sql = format!(
        "SELECT {APPOINTMENT_SELECT} FROM appointments a \
         JOIN units u ON u.id = a.unit_id WHERE a.id = $1"
    );
    let row: AppointmentRow = sqlx::query_as::<_, AppointmentRow>(&sql)
        .bind(new_id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(Appointment::from(row)))
}

#[derive(Debug, Deserialize)]
pub struct PatchStatusReq {
    pub status: AppointmentStatus,
}

pub async fn patch_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchStatusReq>,
) -> ApiResult<Json<Appointment>> {
    let updated: Option<Uuid> = sqlx::query_scalar(
        "UPDATE appointments SET status = $1::appointment_status WHERE id = $2 RETURNING id",
    )
    .bind(body.status)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    if updated.is_none() {
        return Err(ApiError::NotFound);
    }

    let sql = format!(
        "SELECT {APPOINTMENT_SELECT} FROM appointments a \
         JOIN units u ON u.id = a.unit_id WHERE a.id = $1"
    );
    let row: AppointmentRow = sqlx::query_as::<_, AppointmentRow>(&sql)
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(Appointment::from(row)))
}
