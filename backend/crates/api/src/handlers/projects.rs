use crate::{handlers::ApiResult, state::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use hive_core::{AgentMessage, Approval, CreateProject, Project, ProjectStatus, Story, WsEvent};
use serde::Deserialize;
use slug::slugify;
use std::sync::Arc;
use uuid::Uuid;

pub async fn list(State(s): State<Arc<AppState>>) -> ApiResult<Json<Vec<Project>>> {
    let rows = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects ORDER BY created_at DESC"
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Project {} not found", id))?;

    let pending_approval: Option<Approval> = sqlx::query_as(
        "SELECT * FROM approvals WHERE project_id = $1 AND status = 'pending' \
         ORDER BY created_at DESC LIMIT 1"
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?;

    Ok(Json(serde_json::json!({
        "project": project,
        "pending_approval": pending_approval,
    })))
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateProject>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    let id = Uuid::new_v4();
    let slug = slugify(&body.title);
    let folder_path = format!("{}/{}-{}", s.workflow.projects_dir.display(), id, slug);

    // Create project folder
    std::fs::create_dir_all(format!("{}/.hive/stories", folder_path))
        .map_err(|e| anyhow::anyhow!("Failed to create project folder: {}", e))?;
    std::fs::create_dir_all(format!("{}/src", folder_path))
        .map_err(|e| anyhow::anyhow!("Failed to create src folder: {}", e))?;

    // Write project metadata
    let meta = serde_json::json!({
        "id":    id,
        "title": body.title,
        "status": "submitted",
    });
    std::fs::write(
        format!("{}/.hive/project.json", folder_path),
        serde_json::to_string_pretty(&meta)?,
    )?;

    let project: Project = sqlx::query_as(
        "INSERT INTO projects (id, title, description, acceptance_criteria, team_id, folder_path)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"
    )
    .bind(id)
    .bind(&body.title)
    .bind(&body.description)
    .bind(&body.acceptance_criteria)
    .bind(body.team_id)
    .bind(&folder_path)
    .fetch_one(&s.db)
    .await?;

    // Kick off the workflow
    s.workflow.advance(id);

    Ok((StatusCode::CREATED, Json(project)))
}

pub async fn stories(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<Story>>> {
    let rows = sqlx::query_as::<_, Story>(
        "SELECT * FROM stories WHERE project_id = $1 ORDER BY priority, created_at"
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

pub async fn delete_story(
    State(s): State<Arc<AppState>>,
    Path((project_id, story_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    // 1. Interruption: Cancel any active execution of the story
    s.workflow.cancel_story(story_id).await;

    // 2. Database Cleanup: Delete associated approvals and then the story itself
    let mut tx = s.db.begin().await?;

    sqlx::query("DELETE FROM approvals WHERE story_id = $1")
        .bind(story_id)
        .execute(&mut *tx)
        .await?;

    let res = sqlx::query("DELETE FROM stories WHERE id = $1 AND project_id = $2")
        .bind(story_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

    if res.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Story not found").into());
    }

    tx.commit().await?;

    // 3. Post-deletion transition check: if this was the last non-done story, transition to InReview
    s.workflow.handle_story_deleted(project_id).await?;

    // 4. WebSocket Notification
    let _ = s.workflow.publish_event(
        project_id,
        WsEvent::StoryDeleted { story_id, project_id },
    ).await;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct MessagesQuery {
    limit:  Option<i64>,
    offset: Option<i64>,
}

pub async fn messages(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(q): Query<MessagesQuery>,
) -> ApiResult<Json<Vec<AgentMessage>>> {
    let limit  = q.limit.unwrap_or(100);
    let offset = q.offset.unwrap_or(0);
    let rows = sqlx::query_as::<_, AgentMessage>(
        "SELECT * FROM agent_messages WHERE project_id = $1 \
         ORDER BY created_at LIMIT $2 OFFSET $3"
    )
    .bind(id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

pub async fn approve(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<Project>> {
    let comment = body["comment"].as_str();
    resolve_approval(&s, id, "approved", comment).await
}

pub async fn reject(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<Project>> {
    let comment = body["comment"].as_str();
    resolve_approval(&s, id, "rejected", comment).await
}

async fn resolve_approval(
    s:         &AppState,
    project_id: Uuid,
    status:    &str,
    comment:   Option<&str>,
) -> ApiResult<Json<Project>> {
    let approval: Approval = sqlx::query_as(
        "SELECT * FROM approvals WHERE project_id = $1 AND status = 'pending' \
         ORDER BY created_at DESC LIMIT 1"
    )
    .bind(project_id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| anyhow::anyhow!("No pending approval for project {}", project_id))?;

    // Update approval
    sqlx::query(
        "UPDATE approvals SET status = $1, human_comment = $2, resolved_at = now() WHERE id = $3"
    )
    .bind(status)
    .bind(comment)
    .bind(approval.id)
    .execute(&s.db)
    .await?;

    if status == "approved" {
        // Advance the project
        let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&s.db)
            .await?;

        let next_status = match project.status {
            ProjectStatus::AwaitingApprovalLead    => {
                // Create stories from the approved story breakdown (if applicable)
                ProjectStatus::ArchitectBreakdown
            }
            ProjectStatus::AwaitingApprovalStories => {
                // Insert stories from the approved payload
                let payload = approval.payload.unwrap_or(serde_json::Value::Null);
                if let Some(stories) = payload.get("stories").and_then(|v| v.as_array()) {
                    for (i, story_val) in stories.iter().enumerate() {
                        sqlx::query(
                            "INSERT INTO stories (project_id, title, description, acceptance_criteria, priority)
                             VALUES ($1, $2, $3, $4, $5)"
                        )
                        .bind(project_id)
                        .bind(story_val["title"].as_str().unwrap_or("Story"))
                        .bind(story_val["description"].as_str())
                        .bind(story_val["acceptance_criteria"].as_str())
                        .bind(i as i32)
                        .execute(&s.db)
                        .await?;
                    }
                }
                ProjectStatus::InDevelopment
            }
            ProjectStatus::AwaitingFinalApproval   => ProjectStatus::Done,
            other => other,
        };

        sqlx::query("UPDATE projects SET status = $1 WHERE id = $2")
            .bind(&next_status)
            .bind(project_id)
            .execute(&s.db)
            .await?;

        // Advance the workflow
        s.workflow.advance(project_id);
    } else {
        // Rejected — roll back project status to allow revision based on feedback
        let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&s.db)
            .await?;

        let next_status = match &project.status {
            ProjectStatus::AwaitingApprovalLead    => ProjectStatus::Submitted,
            ProjectStatus::AwaitingApprovalStories => ProjectStatus::ArchitectBreakdown,
            ProjectStatus::AwaitingFinalApproval   => ProjectStatus::InDevelopment,
            other => other.clone(),
        };

        // Save the rejection feedback comment as a system message to the agent history!
        if let Some(ref feedback) = comment {
            if !feedback.trim().is_empty() {
                let role_name = match project.status {
                    ProjectStatus::AwaitingApprovalLead    => "lead",
                    ProjectStatus::AwaitingApprovalStories => "architect",
                    ProjectStatus::AwaitingFinalApproval   => "developer",
                    _ => "system",
                };
                let feedback_msg = format!("USER REJECTION FEEDBACK / REQUIREMENT REVISION:\n{}", feedback);
                let _ = s.workflow.save_message(project_id, "system", role_name, &feedback_msg, None).await;
            }
        }

        sqlx::query("UPDATE projects SET status = $1 WHERE id = $2")
            .bind(&next_status)
            .bind(project_id)
            .execute(&s.db)
            .await?;

        // Re-advance the workflow so the respective agent gets triggered and runs again with the new feedback!
        s.workflow.advance(project_id);
    }

    let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(&s.db)
        .await?;

    Ok(Json(project))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // 1. Get the project to find its folder_path (and verify it exists)
    let project: Option<Project> = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?;

    let project = match project {
        Some(p) => p,
        None => return Err(anyhow::anyhow!("Project not found").into()),
    };

    // 2. Stop all agent work for active stories associated with this project
    let story_ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM stories WHERE project_id = $1")
        .bind(id)
        .fetch_all(&s.db)
        .await?;

    for story_id in story_ids {
        s.workflow.cancel_story(story_id).await;
    }

    // 3. Delete the project directory from disk
    if let Some(ref path) = project.folder_path {
        let path_buf = std::path::PathBuf::from(path);
        if path_buf.exists() && path_buf.is_dir() {
            let _ = std::fs::remove_dir_all(&path_buf);
        }
    }

    // 4. Delete the project from the database (cascade deletes stories, approvals, agent_messages)
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;

    // 5. Broadcast a WebSocket event to notify the frontend to invalidate the board
    let _ = s.workflow.publish_event(
        id,
        WsEvent::ProjectStatusChanged { project_id: id, status: ProjectStatus::Rejected }
    ).await;

    Ok(StatusCode::NO_CONTENT)
}
