use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ── Provider ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Provider {
    pub id:          Uuid,
    pub name:        String,
    #[sqlx(rename = "type")]
    pub kind:        String,           // "openai_compat" | "aws_bedrock"
    pub base_url:    Option<String>,
    pub api_key:     Option<String>,   // encrypted at rest
    pub aws_profile: Option<String>,
    pub aws_region:  Option<String>,
    pub model_name:  Option<String>,
    pub config:      serde_json::Value,
    pub is_active:   bool,
    pub created_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProvider {
    pub name:        String,
    #[serde(rename = "type")]
    pub kind:        String,
    pub base_url:    Option<String>,
    pub api_key:     Option<String>,
    pub aws_profile: Option<String>,
    pub aws_region:  Option<String>,
    pub model_name:  Option<String>,
    pub config:      Option<serde_json::Value>,
}

// ── Skill ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Skill {
    pub id:            Uuid,
    pub name:          String,
    pub description:   String,
    pub source_url:    Option<String>,
    pub source_type:   Option<String>,
    pub system_prompt: String,
    pub tool_hints:    Vec<String>,
    pub version:       Option<String>,
    pub created_at:    DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSkillRequest {
    pub source_type: String,  // "github" | "skills_sh" | "local"
    pub url:         Option<String>,
    pub slug:        Option<String>,
    pub content:     Option<String>,  // for "local" paste
}

// ── Tool ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tool {
    pub id:          Uuid,
    pub name:        String,
    pub description: Option<String>,
    #[sqlx(rename = "type")]
    pub kind:        String,
    pub config:      serde_json::Value,
}

// ── Agent ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Agent {
    pub id:                     Uuid,
    pub name:                   String,
    pub description:            Option<String>,
    pub provider_id:            Uuid,
    pub skill_ids:              Vec<Uuid>,
    pub model_name:             Option<String>,
    pub system_prompt_override: Option<String>,
    pub temperature:            f64,
    pub max_tokens:             Option<i32>,
    pub tool_ids:               Vec<Uuid>,
    pub memory_type:            String,
    pub memory_window:          i32,
    pub created_at:             DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgent {
    pub name:                   String,
    pub description:            Option<String>,
    pub provider_id:            Uuid,
    pub skill_ids:              Vec<Uuid>,
    pub model_name:             Option<String>,
    pub system_prompt_override: Option<String>,
    pub temperature:            Option<f64>,
    pub max_tokens:             Option<i32>,
    pub tool_ids:               Option<Vec<Uuid>>,
    pub memory_type:            Option<String>,
    pub memory_window:          Option<i32>,
}

// ── Team ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Team {
    pub id:          Uuid,
    pub name:        String,
    pub description: Option<String>,
    pub created_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TeamMember {
    pub id:         Uuid,
    pub team_id:    Uuid,
    pub agent_id:   Uuid,
    pub role:       String,
    pub position_x: f64,
    pub position_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TeamHierarchy {
    pub id:                 Uuid,
    pub team_id:            Uuid,
    pub parent_member_id:   Uuid,
    pub child_member_id:    Uuid,
    pub communication_mode: String,
}

/// React Flow serialisation format for team hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamGraph {
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNode {
    pub id:       String,
    #[serde(rename = "type")]
    pub kind:     String,
    pub position: FlowPosition,
    pub data:     FlowNodeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNodeData {
    pub member_id:  Option<Uuid>,
    pub agent_id:   Uuid,
    pub agent_name: String,
    pub role:       String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEdge {
    pub id:     String,
    pub source: String,
    pub target: String,
    pub data:   FlowEdgeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEdgeData {
    pub hierarchy_id:       Option<Uuid>,
    pub communication_mode: String,
}

// ── Project ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "project_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Submitted,
    LeadReview,
    AwaitingApprovalLead,
    ArchitectBreakdown,
    AwaitingApprovalStories,
    InDevelopment,
    InReview,
    AwaitingFinalApproval,
    Done,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id:                  Uuid,
    pub title:               String,
    pub description:         Option<String>,
    pub acceptance_criteria: Option<String>,
    pub team_id:             Uuid,
    pub status:              ProjectStatus,
    pub folder_path:         Option<String>,
    pub created_at:          DateTime<Utc>,
    pub updated_at:          DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub title:               String,
    pub description:         Option<String>,
    pub acceptance_criteria: Option<String>,
    pub team_id:             Uuid,
}

// ── Story ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Story {
    pub id:                  Uuid,
    pub project_id:          Uuid,
    pub title:               String,
    pub description:         Option<String>,
    pub acceptance_criteria: Option<String>,
    pub status:              String,
    pub assigned_member_id:  Option<Uuid>,
    pub parent_story_id:     Option<Uuid>,
    pub priority:            i32,
    pub created_at:          DateTime<Utc>,
}

// ── Approval ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Approval {
    pub id:            Uuid,
    pub project_id:    Uuid,
    pub story_id:      Option<Uuid>,
    #[sqlx(rename = "type")]
    pub kind:          String,
    pub status:        String,
    pub payload:       Option<serde_json::Value>,
    pub human_comment: Option<String>,
    pub created_at:    DateTime<Utc>,
    pub resolved_at:   Option<DateTime<Utc>>,
}

// ── AgentMessage ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentMessage {
    pub id:          Uuid,
    pub project_id:  Uuid,
    pub from_role:   Option<String>,
    pub to_role:     Option<String>,
    pub content:     String,
    pub tokens_used: Option<i32>,
    pub latency_ms:  Option<i32>,
    pub created_at:  DateTime<Utc>,
}

// ── MCP Server ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct McpServer {
    pub id:         Uuid,
    pub name:       String,
    pub url:        String,
    pub is_active:  bool,
    pub created_at: DateTime<Utc>,
}

// ── WebSocket Events ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum WsEvent {
    ProjectStatusChanged { project_id: Uuid, status: ProjectStatus },
    AgentToken           { project_id: Uuid, role: String, token: String },
    MessageCreated       { message: AgentMessage },
    ApprovalRequired     { approval_id: Uuid, kind: String, payload: serde_json::Value },
    StoryUpdated         { story_id: Uuid, status: String },
    StoryDeleted         { story_id: Uuid, project_id: Uuid },
    ProjectError         { project_id: Uuid, message: String },
}
