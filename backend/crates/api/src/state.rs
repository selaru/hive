use crate::ws::hub::WsHub;
use hive_providers::ProviderManager;
use hive_workflow::WorkflowEngine;
use sqlx::PgPool;
use std::sync::Arc;

/// Shared application state injected into all Axum handlers.
pub struct AppState {
    pub db:        PgPool,
    pub providers: Arc<ProviderManager>,
    pub workflow:  Arc<WorkflowEngine>,
    pub ws_hub:    Arc<WsHub>,
}
