use crate::{handlers::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use hive_core::{FlowEdge, FlowEdgeData, FlowNode, FlowNodeData, FlowPosition, Team, TeamGraph, TeamHierarchy, TeamMember};
use std::sync::Arc;
use uuid::Uuid;

pub async fn list(State(s): State<Arc<AppState>>) -> ApiResult<Json<Vec<Team>>> {
    let rows = sqlx::query_as::<_, Team>("SELECT * FROM teams ORDER BY name")
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows))
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Team>> {
    let row = sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Team {} not found", id))?;
    Ok(Json(row))
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<(StatusCode, Json<Team>)> {
    let row: Team = sqlx::query_as(
        "INSERT INTO teams (name, description) VALUES ($1, $2) RETURNING *"
    )
    .bind(body["name"].as_str().unwrap_or("New Team"))
    .bind(body["description"].as_str())
    .fetch_one(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<Team>> {
    let row: Team = sqlx::query_as(
        "UPDATE teams SET name=$1, description=$2 WHERE id=$3 RETURNING *"
    )
    .bind(body["name"].as_str().unwrap_or("Team"))
    .bind(body["description"].as_str())
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| anyhow::anyhow!("Team {} not found", id))?;
    Ok(Json(row))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM teams WHERE id = $1").bind(id).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /teams/:id/hierarchy — return React Flow { nodes, edges }
pub async fn get_hierarchy(
    State(s): State<Arc<AppState>>,
    Path(team_id): Path<Uuid>,
) -> ApiResult<Json<TeamGraph>> {
    let members: Vec<TeamMember> = sqlx::query_as(
        "SELECT tm.*, a.name as agent_name FROM team_members tm \
         JOIN agents a ON a.id = tm.agent_id \
         WHERE tm.team_id = $1"
    )
    .bind(team_id)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();

    let edges_db: Vec<TeamHierarchy> = sqlx::query_as(
        "SELECT * FROM team_hierarchy WHERE team_id = $1"
    )
    .bind(team_id)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();

    // Build agent name map
    let agent_names: std::collections::HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT a.id, a.name FROM agents a \
         JOIN team_members tm ON tm.agent_id = a.id WHERE tm.team_id = $1"
    )
    .bind(team_id)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let nodes = members.iter().map(|m| FlowNode {
        id:   m.id.to_string(),
        kind: "agentNode".into(),
        position: FlowPosition { x: m.position_x, y: m.position_y },
        data: FlowNodeData {
            member_id:  Some(m.id),
            agent_id:   m.agent_id,
            agent_name: agent_names.get(&m.agent_id).cloned().unwrap_or_default(),
            role:       m.role.clone(),
        },
    }).collect();

    let edges = edges_db.iter().map(|e| FlowEdge {
        id:     e.id.to_string(),
        source: e.parent_member_id.to_string(),
        target: e.child_member_id.to_string(),
        data:   FlowEdgeData {
            hierarchy_id:       Some(e.id),
            communication_mode: e.communication_mode.clone(),
        },
    }).collect();

    Ok(Json(TeamGraph { nodes, edges }))
}

/// PUT /teams/:id/hierarchy — save React Flow { nodes, edges }
pub async fn save_hierarchy(
    State(s): State<Arc<AppState>>,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TeamGraph>,
) -> ApiResult<StatusCode> {
    let mut tx = s.db.begin().await?;

    // Delete existing members + hierarchy (cascade)
    sqlx::query("DELETE FROM team_members WHERE team_id = $1")
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

    // Build mapping from React Flow node IDs to actual member UUIDs
    let mut node_id_map = std::collections::HashMap::new();

    // Re-insert nodes as team_members
    for node in &body.nodes {
        let member_id = Uuid::parse_str(&node.id).unwrap_or_else(|_| Uuid::new_v4());
        node_id_map.insert(node.id.clone(), member_id);
        
        sqlx::query(
            "INSERT INTO team_members (id, team_id, agent_id, role, position_x, position_y) \
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(member_id)
        .bind(team_id)
        .bind(node.data.agent_id)
        .bind(&node.data.role)
        .bind(node.position.x)
        .bind(node.position.y)
        .execute(&mut *tx)
        .await?;
    }

    // Re-insert edges as hierarchy
    for edge in &body.edges {
        // Map React Flow node IDs to actual member UUIDs
        let parent = node_id_map.get(&edge.source)
            .ok_or_else(|| anyhow::anyhow!("Parent node {} not found", edge.source))?;
        let child = node_id_map.get(&edge.target)
            .ok_or_else(|| anyhow::anyhow!("Child node {} not found", edge.target))?;
        
        sqlx::query(
            "INSERT INTO team_hierarchy (team_id, parent_member_id, child_member_id, communication_mode)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (parent_member_id, child_member_id) DO UPDATE SET
               communication_mode = EXCLUDED.communication_mode"
        )
        .bind(team_id)
        .bind(parent)
        .bind(child)
        .bind(&edge.data.communication_mode)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
