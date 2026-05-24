use axum::{extract::State, Json};

use crate::{error::ApiResult, models::Unit, state::AppState};

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<Unit>>> {
    let units = sqlx::query_as::<_, Unit>(
        "SELECT id, name, address, type FROM units ORDER BY name",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(units))
}
