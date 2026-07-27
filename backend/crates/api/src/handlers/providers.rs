use crate::{handlers::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use hive_core::{CreateProvider, Provider};
use std::sync::Arc;
use uuid::Uuid;

pub async fn list(State(s): State<Arc<AppState>>) -> ApiResult<Json<Vec<Provider>>> {
    let rows = sqlx::query_as::<_, Provider>("SELECT * FROM providers ORDER BY created_at")
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows))
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Provider>> {
    let row = sqlx::query_as::<_, Provider>("SELECT * FROM providers WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Provider {} not found", id))?;
    Ok(Json(row))
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateProvider>,
) -> ApiResult<(StatusCode, Json<Provider>)> {
    let row: Provider = sqlx::query_as(
        "INSERT INTO providers (name, type, base_url, api_key, aws_profile, aws_region, model_name, config)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING *"
    )
    .bind(&body.name)
    .bind(&body.kind)
    .bind(&body.base_url)
    .bind(&body.api_key)
    .bind(&body.aws_profile)
    .bind(&body.aws_region)
    .bind(&body.model_name)
    .bind(body.config.unwrap_or(serde_json::json!({})))
    .fetch_one(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateProvider>,
) -> ApiResult<Json<Provider>> {
    let row: Provider = sqlx::query_as(
        "UPDATE providers SET name=$1, type=$2, base_url=$3, api_key=$4,
         aws_profile=$5, aws_region=$6, model_name=$7, config=$8
         WHERE id=$9 RETURNING *"
    )
    .bind(&body.name)
    .bind(&body.kind)
    .bind(&body.base_url)
    .bind(&body.api_key)
    .bind(&body.aws_profile)
    .bind(&body.aws_region)
    .bind(&body.model_name)
    .bind(body.config.unwrap_or(serde_json::json!({})))
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| anyhow::anyhow!("Provider {} not found", id))?;
    s.providers.invalidate(id).await;
    Ok(Json(row))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM providers WHERE id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    s.providers.invalidate(id).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let provider_row: Provider = sqlx::query_as("SELECT * FROM providers WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Provider {} not found", id))?;

    let provider = s.providers.get(&provider_row).await?;
    let models = provider.list_models().await?;
    Ok(Json(serde_json::json!({ "ok": true, "models": models })))
}

/// List available models from a provider
pub async fn list_models(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<String>>> {
    let provider_row: Provider = sqlx::query_as("SELECT * FROM providers WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Provider {} not found", id))?;

    let provider = s.providers.get(&provider_row).await?;
    let models = provider.list_models().await?;
    Ok(Json(models))
}
