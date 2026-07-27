use crate::state::AppState;
use deadpool_redis::Config as RedisConfig;
use hive_providers::ProviderManager;
use hive_workflow::WorkflowEngine;
use sqlx::postgres::PgPoolOptions;
use std::{path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod routes;
mod state;
mod ws;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // ── Tracing ───────────────────────────────────────────────────────────────
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "hive_api=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // ── Database ──────────────────────────────────────────────────────────────
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("../../migrations").run(&db).await?;
    tracing::info!("Database migrations complete");

    // Reset any interrupted "in_progress" stories back to "todo" on startup
    let reset_count = sqlx::query("UPDATE stories SET status = 'todo', assigned_member_id = NULL WHERE status = 'in_progress'")
        .execute(&db)
        .await?
        .rows_affected();
    if reset_count > 0 {
        tracing::info!("Reset {} interrupted stories from 'in_progress' back to 'todo'", reset_count);
    }

    // ── Redis ─────────────────────────────────────────────────────────────────
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379".into());
    let redis_cfg = RedisConfig::from_url(&redis_url);
    let redis = redis_cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))?;

    // ── Providers ─────────────────────────────────────────────────────────────
    let providers = Arc::new(ProviderManager::new());

    // ── Workflow engine ───────────────────────────────────────────────────────
    let executor_url  = std::env::var("EXECUTOR_URL")
        .unwrap_or_else(|_| "http://executor:3001".into());
    let projects_dir  = PathBuf::from(
        std::env::var("CONTAINER_PROJECTS_DIR").unwrap_or_else(|_| "/app/projects".into())
    );
    std::fs::create_dir_all(&projects_dir)?;

    let workflow = Arc::new(WorkflowEngine::new(
        db.clone(),
        redis.clone(),
        Arc::clone(&providers),
        executor_url,
        projects_dir,
    ));

    // ── WebSocket hub ─────────────────────────────────────────────────────────
    let ws_hub = Arc::new(ws::hub::WsHub::new(redis_url.clone()));
    ws_hub.start_redis_listener().await?;

    // ── App state ─────────────────────────────────────────────────────────────
    let state = Arc::new(AppState {
        db: db.clone(),
        providers,
        workflow,
        ws_hub,
    });

    // ── Auto-import skills ────────────────────────────────────────────────────
    let mut skill_dirs = Vec::new();
    
    // Collect directories from environment variables
    if let Ok(dir) = std::env::var("OPENCODE_SKILLS_DIR") {
        if !dir.is_empty() {
            skill_dirs.push(dir);
        }
    }
    if let Ok(dir) = std::env::var("MULTICA_SKILLS_DIR") {
        if !dir.is_empty() {
            skill_dirs.push(dir);
        }
    }
    if let Ok(dirs) = std::env::var("CUSTOM_SKILLS_DIRS") {
        for dir in dirs.split(',') {
            let trimmed = dir.trim();
            if !trimmed.is_empty() {
                skill_dirs.push(trimmed.to_string());
            }
        }
    }

    // Import skills from all configured directories
    for dir_str in skill_dirs {
        let dir_path = PathBuf::from(&dir_str);
        if dir_path.exists() && dir_path.is_dir() {
            tracing::info!("Auto-importing skills from: {}", dir_path.display());
            if let Err(e) = import_skills_from_dir(&db, &dir_path).await {
                tracing::warn!("Failed to import skills from {}: {}", dir_path.display(), e);
            }
        } else {
            tracing::warn!("Skill directory does not exist: {}", dir_path.display());
        }
    }

    // ── Router ────────────────────────────────────────────────────────────────
    let app = routes::create_router(state.clone());

    // ── Resume pending workflows ──────────────────────────────────────────────
    let active_projects: Vec<hive_core::Project> = sqlx::query_as(
        "SELECT * FROM projects WHERE status IN ('submitted', 'architect_breakdown', 'in_development', 'in_review')"
    )
    .fetch_all(&db)
    .await?;

    if !active_projects.is_empty() {
        tracing::info!("Resuming {} pending project workflows...", active_projects.len());
        for p in active_projects {
            tracing::info!("Resuming workflow for project: {} ({:?})", p.title, p.status);
            state.workflow.advance(p.id);
        }
    }

    let bind = "0.0.0.0:3000";
    let listener = TcpListener::bind(bind).await?;
    tracing::info!("Hive API listening on {}", bind);

    axum::serve(listener, app).await?;
    Ok(())
}

/// Import all SKILL.md files from a directory tree into the database
async fn import_skills_from_dir(db: &sqlx::PgPool, dir: &PathBuf) -> anyhow::Result<()> {
    use hive_skills::SkillImporter;
    
    let importer = SkillImporter::new();
    let scan_results = importer.scan_directory(dir);
    
    let mut imported_count = 0;
    let mut failed_count = 0;

    for scan_result in scan_results {
        let skill = &scan_result.skill;
        let source_url = scan_result.source_path.display().to_string();

        let result = sqlx::query(
            "INSERT INTO skills (name, description, source_url, source_type, system_prompt, tool_hints)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (name) DO UPDATE SET
               description   = EXCLUDED.description,
               source_url    = EXCLUDED.source_url,
               source_type   = EXCLUDED.source_type,
               system_prompt = EXCLUDED.system_prompt,
               tool_hints    = EXCLUDED.tool_hints"
        )
        .bind(&skill.name)
        .bind(&skill.description)
        .bind(&source_url)
        .bind(&scan_result.source_type)
        .bind(&skill.system_prompt)
        .bind(&skill.tool_hints)
        .execute(db)
        .await;

        match result {
            Ok(_) => {
                tracing::debug!("Imported skill: {}", skill.name);
                imported_count += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to import skill '{}' from {}: {}", 
                    skill.name, source_url, e);
                failed_count += 1;
            }
        }
    }

    tracing::info!(
        "Skill import from {} complete: {} imported, {} failed",
        dir.display(),
        imported_count,
        failed_count
    );

    Ok(())
}
