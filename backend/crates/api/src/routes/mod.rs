use crate::handlers;
use crate::state::AppState;
use crate::ws;
use axum::{routing::{delete, get, post}, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // ── Health ──────────────────────────────────────────────────────────
        .route("/health", get(|| async { "ok" }))

        // ── Providers ───────────────────────────────────────────────────────
        .route("/api/v1/providers",              get(handlers::providers::list).post(handlers::providers::create))
        .route("/api/v1/providers/{id}",         get(handlers::providers::get).put(handlers::providers::update).delete(handlers::providers::delete))
        .route("/api/v1/providers/{id}/test",    post(handlers::providers::test))
        .route("/api/v1/providers/{id}/models",  get(handlers::providers::list_models))

        // ── Skills ──────────────────────────────────────────────────────────
        .route("/api/v1/skills",                    get(handlers::skills::list))
        .route("/api/v1/skills/import",             post(handlers::skills::import))
        .route("/api/v1/skills/import-directories", post(handlers::skills::import_from_directories))
        .route("/api/v1/skills/{id}",               get(handlers::skills::get).delete(handlers::skills::delete))

        // ── Tools ────────────────────────────────────────────────────────────
        .route("/api/v1/tools",           get(handlers::tools::list))

        // ── Agents ──────────────────────────────────────────────────────────
        .route("/api/v1/agents",           get(handlers::agents::list).post(handlers::agents::create))
        .route("/api/v1/agents/{id}",      get(handlers::agents::get).put(handlers::agents::update).delete(handlers::agents::delete))
        .route("/api/v1/agents/{id}/test", post(handlers::agents::test))

        // ── Teams ────────────────────────────────────────────────────────────
        .route("/api/v1/teams",                    get(handlers::teams::list).post(handlers::teams::create))
        .route("/api/v1/teams/{id}",               get(handlers::teams::get).put(handlers::teams::update).delete(handlers::teams::delete))
        .route("/api/v1/teams/{id}/hierarchy",     get(handlers::teams::get_hierarchy).put(handlers::teams::save_hierarchy))

        // ── Projects ─────────────────────────────────────────────────────────
        .route("/api/v1/projects",                get(handlers::projects::list).post(handlers::projects::create))
        .route("/api/v1/projects/{id}",           get(handlers::projects::get).delete(handlers::projects::delete))
        .route("/api/v1/projects/{id}/stories",   get(handlers::projects::stories))
        .route("/api/v1/projects/{id}/stories/{story_id}", delete(handlers::projects::delete_story))
        .route("/api/v1/projects/{id}/messages",  get(handlers::projects::messages))
        .route("/api/v1/projects/{id}/approve",   post(handlers::projects::approve))
        .route("/api/v1/projects/{id}/reject",    post(handlers::projects::reject))

        // ── MCP Servers ──────────────────────────────────────────────────────
        .route("/api/v1/mcp-servers",           get(handlers::mcp::list).post(handlers::mcp::create))
        .route("/api/v1/mcp-servers/{id}",      delete(handlers::mcp::delete))
        .route("/api/v1/mcp-servers/{id}/sync", post(handlers::mcp::sync))

        // ── WebSocket ────────────────────────────────────────────────────────
        .route("/ws/projects/{id}", get(ws::project_ws_handler))
        .route("/ws/board",         get(ws::board_ws_handler))

        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
