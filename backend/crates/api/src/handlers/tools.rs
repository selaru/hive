use crate::{handlers::ApiResult, state::AppState};
use axum::{extract::State, Json};
use hive_core::Tool;
use std::sync::Arc;

pub async fn list(State(s): State<Arc<AppState>>) -> ApiResult<Json<Vec<Tool>>> {
    let rows = sqlx::query_as::<_, Tool>("SELECT * FROM tools ORDER BY name")
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows))
}
