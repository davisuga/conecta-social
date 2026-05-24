use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::{
    error::{ApiError, ApiResult},
    models::{ListResponse, Profile, ProfileRow, PROFILE_SELECT},
    state::AppState,
};

fn default_limit() -> i64 {
    50
}

fn default_offset() -> i64 {
    0
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default = "default_offset")]
    offset: i64,
}

fn is_valid_nis(s: &str) -> bool {
    s.len() == 11 && s.bytes().all(|b| b.is_ascii_digit())
}

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<ListResponse<Profile>>> {
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);

    let sql = format!(
        "SELECT {PROFILE_SELECT} FROM profiles ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    );

    let rows = sqlx::query_as::<_, ProfileRow>(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;

    let items: Vec<Profile> = rows.into_iter().map(Profile::from).collect();

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM profiles")
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ListResponse { items, total }))
}

pub async fn get(
    State(state): State<AppState>,
    Path(nis): Path<String>,
) -> ApiResult<Json<Profile>> {
    if !is_valid_nis(&nis) {
        return Err(ApiError::BadRequest("nis must be 11 digits".into()));
    }

    let sql = format!("SELECT {PROFILE_SELECT} FROM profiles WHERE nis = $1");

    let row = sqlx::query_as::<_, ProfileRow>(&sql)
        .bind(&nis)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(Profile::from(row)))
}

#[derive(Deserialize)]
pub struct OptInReq {
    opt_in: bool,
}

pub async fn opt_in(
    State(state): State<AppState>,
    Path(nis): Path<String>,
    Json(body): Json<OptInReq>,
) -> ApiResult<Json<Profile>> {
    if !is_valid_nis(&nis) {
        return Err(ApiError::BadRequest("nis must be 11 digits".into()));
    }

    let mut tx = state.db.begin().await?;

    let update_sql = format!(
        "UPDATE profiles SET opt_in = $1, opt_in_at = CASE WHEN $1 THEN now() ELSE NULL END \
         WHERE nis = $2 RETURNING {PROFILE_SELECT}"
    );

    let row_opt: Option<ProfileRow> = sqlx::query_as::<_, ProfileRow>(&update_sql)
        .bind(body.opt_in)
        .bind(&nis)
        .fetch_optional(&mut *tx)
        .await?;

    let row = match row_opt {
        Some(r) => r,
        None => return Err(ApiError::NotFound),
    };

    sqlx::query("INSERT INTO opt_in_log (nis, opt_in, source) VALUES ($1, $2, 'admin')")
        .bind(&nis)
        .bind(body.opt_in)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(Profile::from(row)))
}
