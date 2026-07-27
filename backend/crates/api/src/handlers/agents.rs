use crate::{handlers::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use hive_core::{Agent, CreateAgent};
use std::sync::Arc;
use uuid::Uuid;

pub async fn list(State(s): State<Arc<AppState>>) -> ApiResult<Json<Vec<Agent>>> {
    let rows = sqlx::query_as::<_, Agent>("SELECT * FROM agents ORDER BY name")
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows))
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Agent>> {
    let row = sqlx::query_as::<_, Agent>("SELECT * FROM agents WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent {} not found", id))?;
    Ok(Json(row))
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateAgent>,
) -> ApiResult<(StatusCode, Json<Agent>)> {
    // Validate at least one skill is provided
    if body.skill_ids.is_empty() {
        return Err(anyhow::anyhow!("At least one skill must be selected").into());
    }

    let row: Agent = sqlx::query_as(
        "INSERT INTO agents (name, description, provider_id, skill_ids, model_name,
         system_prompt_override, temperature, max_tokens, tool_ids, memory_type, memory_window)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING *"
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.provider_id)
    .bind(&body.skill_ids)
    .bind(&body.model_name)
    .bind(&body.system_prompt_override)
    .bind(body.temperature.unwrap_or(0.7))
    .bind(body.max_tokens)
    .bind(body.tool_ids.unwrap_or_default())
    .bind(body.memory_type.as_deref().unwrap_or("none"))
    .bind(body.memory_window.unwrap_or(20))
    .fetch_one(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateAgent>,
) -> ApiResult<Json<Agent>> {
    // Validate at least one skill is provided
    if body.skill_ids.is_empty() {
        return Err(anyhow::anyhow!("At least one skill must be selected").into());
    }

    let row: Agent = sqlx::query_as(
        "UPDATE agents SET name=$1, description=$2, provider_id=$3, skill_ids=$4, model_name=$5,
         system_prompt_override=$6, temperature=$7, max_tokens=$8, tool_ids=$9,
         memory_type=$10, memory_window=$11
         WHERE id=$12 RETURNING *"
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.provider_id)
    .bind(&body.skill_ids)
    .bind(&body.model_name)
    .bind(&body.system_prompt_override)
    .bind(body.temperature.unwrap_or(0.7))
    .bind(body.max_tokens)
    .bind(body.tool_ids.unwrap_or_default())
    .bind(body.memory_type.as_deref().unwrap_or("none"))
    .bind(body.memory_window.unwrap_or(20))
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| anyhow::anyhow!("Agent {} not found", id))?;
    Ok(Json(row))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM agents WHERE id = $1").bind(id).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let agent: Agent = sqlx::query_as("SELECT * FROM agents WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent {} not found", id))?;

    let mut provider_row: hive_core::Provider = sqlx::query_as("SELECT * FROM providers WHERE id = $1")
        .bind(agent.provider_id)
        .fetch_one(&s.db)
        .await?;

    // If agent has a specific model, use it instead of provider's default
    if let Some(ref model) = agent.model_name {
        provider_row.model_name = Some(model.clone());
    }

    // Fetch all skills and combine their system prompts
    let mut combined_prompt = String::new();
    for skill_id in &agent.skill_ids {
        let skill: hive_core::Skill = sqlx::query_as("SELECT * FROM skills WHERE id = $1")
            .bind(skill_id)
            .fetch_one(&s.db)
            .await?;
        
        if !combined_prompt.is_empty() {
            combined_prompt.push_str("\n\n---\n\n");
        }
        combined_prompt.push_str(&format!("# Skill: {}\n\n{}", skill.name, skill.system_prompt));
    }

    let provider = s.providers.get(&provider_row).await?;

    let msg = body["message"].as_str().unwrap_or("Hello! Who are you?");
    let system_prompt = agent.system_prompt_override.as_deref().unwrap_or(&combined_prompt);

    let messages = vec![
        hive_providers::ChatMessage { role: "system".into(), content: system_prompt.to_string(), tool_call_id: None },
        hive_providers::ChatMessage { role: "user".into(), content: msg.to_string(), tool_call_id: None },
    ];

    let resp = provider.chat(messages, vec![], agent.max_tokens).await?;
    Ok(Json(serde_json::json!({ "response": resp.content.unwrap_or_default() })))
}
