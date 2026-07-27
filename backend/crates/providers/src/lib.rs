pub mod aws_bedrock;
pub mod manager;
pub mod openai_compat;

use anyhow::Result;
use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;

pub use manager::ProviderManager;

/// A single chat message exchanged with a model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role:         String,            // "system" | "user" | "assistant" | "tool"
    pub content:      String,
    pub tool_call_id: Option<String>,    // present when role == "tool"
}

/// Tool definition passed to the model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name:        String,
    pub description: String,
    pub parameters:  serde_json::Value,  // JSON Schema object
}

/// A tool call the model wants to execute.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id:        String,
    pub name:      String,
    pub arguments: serde_json::Value,
}

/// Final response from a model.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content:     Option<String>,
    pub tool_calls:  Vec<ToolCall>,
    pub tokens_used: Option<u32>,
}

/// Token chunk from a streaming response.
#[derive(Debug, Clone)]
pub struct TokenChunk {
    pub token:      String,
    pub is_final:   bool,
}

pub type TokenStream = Pin<Box<dyn Stream<Item = Result<TokenChunk>> + Send>>;

/// Unified trait all LLM providers must implement.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Non-streaming chat completion.
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        tools:    Vec<ToolDefinition>,
        max_tokens: Option<i32>,
    ) -> Result<ChatResponse>;

    /// Streaming chat completion — yields token chunks.
    async fn stream(
        &self,
        messages: Vec<ChatMessage>,
        tools:    Vec<ToolDefinition>,
        max_tokens: Option<i32>,
    ) -> Result<TokenStream>;

    /// List models available through this provider.
    async fn list_models(&self) -> Result<Vec<String>>;
}
