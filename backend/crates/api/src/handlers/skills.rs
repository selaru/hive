use crate::{handlers::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use hive_core::{ImportSkillRequest, Skill};
use hive_skills::{SkillImporter, SkillScanResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub imported: Vec<Skill>,
    pub failed: Vec<ImportError>,
}

#[derive(Debug, Serialize)]
pub struct ImportError {
    pub path: String,
    pub error: String,
}

pub async fn list(State(s): State<Arc<AppState>>) -> ApiResult<Json<Vec<Skill>>> {
    let rows = sqlx::query_as::<_, Skill>("SELECT * FROM skills ORDER BY name")
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows))
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Skill>> {
    let row = sqlx::query_as::<_, Skill>("SELECT * FROM skills WHERE id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Skill {} not found", id))?;
    Ok(Json(row))
}

pub async fn import(
    State(s): State<Arc<AppState>>,
    Json(body): Json<ImportSkillRequest>,
) -> ApiResult<(StatusCode, Json<Skill>)> {
    let importer = SkillImporter::new();

    let parsed = match body.source_type.as_str() {
        "github" => {
            let url = body.url.as_deref().ok_or_else(|| anyhow::anyhow!("url required for github source"))?;
            importer.from_github(url).await?
        }
        "skills_sh" => {
            let slug = body.slug.as_deref()
                .or(body.url.as_deref())
                .ok_or_else(|| anyhow::anyhow!("slug or url required for skills_sh source"))?;
            importer.from_skills_sh(slug).await?
        }
        "local" => {
            let content = body.content.as_deref().ok_or_else(|| anyhow::anyhow!("content required for local source"))?;
            importer.from_content(content)?
        }
        other => return Err(anyhow::anyhow!("Unknown source_type: {}", other).into()),
    };

    // Upsert by name
    let row: Skill = sqlx::query_as(
        "INSERT INTO skills (name, description, source_url, source_type, system_prompt, tool_hints)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (name) DO UPDATE SET
           description   = EXCLUDED.description,
           source_url    = EXCLUDED.source_url,
           source_type   = EXCLUDED.source_type,
           system_prompt = EXCLUDED.system_prompt,
           tool_hints    = EXCLUDED.tool_hints
         RETURNING *"
    )
    .bind(&parsed.name)
    .bind(&parsed.description)
    .bind(body.url.as_deref().or(body.slug.as_deref()))
    .bind(&body.source_type)
    .bind(&parsed.system_prompt)
    .bind(&parsed.tool_hints)
    .fetch_one(&s.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM skills WHERE id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Scan filesystem directories for SKILL.md files and import them.
/// 
/// Request body:
/// ```json
/// {
///   "directories": ["/path/to/skills", "/another/path"]
/// }
/// ```
pub async fn import_from_directories(
    State(s): State<Arc<AppState>>,
    Json(body): Json<ImportDirectoriesRequest>,
) -> ApiResult<Json<ImportResult>> {
    let importer = SkillImporter::new();
    let mut imported = Vec::new();
    let mut failed = Vec::new();

    for dir_str in &body.directories {
        let dir_path = std::path::Path::new(dir_str);
        let scan_results = importer.scan_directory(dir_path);

        for scan_result in scan_results {
            match insert_skill(&s, &scan_result).await {
                Ok(skill) => {
                    imported.push(skill);
                }
                Err(e) => {
                    failed.push(ImportError {
                        path: scan_result.source_path.display().to_string(),
                        error: e.to_string(),
                    });
                }
            }
        }
    }

    Ok(Json(ImportResult { imported, failed }))
}

#[derive(Debug, Deserialize)]
pub struct ImportDirectoriesRequest {
    pub directories: Vec<String>,
}

async fn insert_skill(state: &Arc<AppState>, scan_result: &SkillScanResult) -> anyhow::Result<Skill> {
    let skill = &scan_result.skill;
    let source_url = scan_result.source_path.display().to_string();

    let row: Skill = sqlx::query_as(
        "INSERT INTO skills (name, description, source_url, source_type, system_prompt, tool_hints)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (name) DO UPDATE SET
           description   = EXCLUDED.description,
           source_url    = EXCLUDED.source_url,
           source_type   = EXCLUDED.source_type,
           system_prompt = EXCLUDED.system_prompt,
           tool_hints    = EXCLUDED.tool_hints
         RETURNING *"
    )
    .bind(&skill.name)
    .bind(&skill.description)
    .bind(&source_url)
    .bind(&scan_result.source_type)
    .bind(&skill.system_prompt)
    .bind(&skill.tool_hints)
    .fetch_one(&state.db)
    .await?;

    Ok(row)
}
