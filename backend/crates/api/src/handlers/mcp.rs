use crate::{handlers::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use hive_core::McpServer;
use hive_workflow::mcp;
use std::sync::Arc;
use uuid::Uuid;

pub async fn list(State(s): State<Arc<AppState>>) -> ApiResult<Json<Vec<McpServer>>> {
    let rows = sqlx::query_as::<_, McpServer>("SELECT * FROM mcp_servers ORDER BY name")
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows))
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<(StatusCode, Json<McpServer>)> {
    let name = body["name"].as_str().unwrap_or("MCP Server");
    let url  = body["url"].as_str()
        .ok_or_else(|| anyhow::anyhow!("url is required"))?;

    let server: McpServer = sqlx::query_as(
        "INSERT INTO mcp_servers (name, url) VALUES ($1, $2) RETURNING *"
    )
    .bind(name)
    .bind(url)
    .fetch_one(&s.db)
    .await?;

    // Discover + import tools
    import_mcp_tools(&s, url).await?;

    Ok((StatusCode::CREATED, Json(server)))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM mcp_servers WHERE id = $1").bind(id).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn sync(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let server: McpServer = sqlx::query_as("SELECT * FROM mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("MCP server {} not found", id))?;

    let count = import_mcp_tools(&s, &server.url).await?;
    Ok(Json(serde_json::json!({ "imported": count })))
}

async fn import_mcp_tools(s: &AppState, url: &str) -> ApiResult<usize> {
    let tools = mcp::list_tools(url).await
        .map_err(|e| anyhow::anyhow!("MCP tool discovery failed: {}", e))?;

    let count = tools.len();
    for tool in &tools {
        sqlx::query(
            "INSERT INTO tools (name, description, type, config)
             VALUES ($1, $2, 'mcp', $3)
             ON CONFLICT (name) DO UPDATE SET
               description = EXCLUDED.description,
               config      = EXCLUDED.config"
        )
        .bind(&tool.name)
        .bind(&tool.description)
        .bind(serde_json::json!({
            "mcp_server_url": url,
            "schema": tool.input_schema,
        }))
        .execute(&s.db)
        .await?;
    }
    Ok(count)
}
