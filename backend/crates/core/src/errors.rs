use thiserror::Error;

#[derive(Debug, Error)]
pub enum HiveError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Skill parse error: {0}")]
    SkillParse(String),

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Tool execution error: {0}")]
    Tool(String),

    #[error("Path traversal attempt: {0}")]
    PathTraversal(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl HiveError {
    pub fn not_found(resource: &str, id: impl std::fmt::Display) -> Self {
        Self::NotFound(format!("{} '{}' not found", resource, id))
    }
}
