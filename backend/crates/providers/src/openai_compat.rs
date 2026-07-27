use crate::{ChatMessage, ChatResponse, LlmProvider, ToolCall, ToolDefinition, TokenChunk, TokenStream};
use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
        ChatCompletionTool, ChatCompletionToolType, CreateChatCompletionRequest,
        FunctionObject,
    },
    Client,
};
use async_trait::async_trait;
use futures::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use serde_json::Value;

/// Adapter for any OpenAI-compatible endpoint.
///
/// Covers:
///   - MLX LM  (base_url = http://host.docker.internal:8080)
///   - Gemini  (base_url = https://generativelanguage.googleapis.com/v1beta/openai/)
pub struct OpenAICompatProvider {
    client: Client<OpenAIConfig>,
    model:  String,
    base_url: String,
}

impl OpenAICompatProvider {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        let key = if api_key.is_empty() { "none" } else { api_key };
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key(key);
        Self {
            client: Client::with_config(config),
            model:  model.to_string(),
            base_url: base_url.to_string(),
        }
    }

    fn convert_messages(msgs: Vec<ChatMessage>) -> Vec<ChatCompletionRequestMessage> {
        msgs.into_iter()
            .map(|m| match m.role.as_str() {
                "system" => ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage {
                        content: m.content.into(),
                        ..Default::default()
                    },
                ),
                "assistant" => ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessage {
                        content: Some(m.content.into()),
                        ..Default::default()
                    },
                ),
                "tool" => ChatCompletionRequestMessage::Tool(
                    ChatCompletionRequestToolMessage {
                        content: m.content.into(),
                        tool_call_id: m.tool_call_id.unwrap_or_default(),
                        ..Default::default()
                    },
                ),
                _ => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage {
                        content: ChatCompletionRequestUserMessageContent::Text(m.content),
                        ..Default::default()
                    },
                ),
            })
            .collect()
    }

    fn convert_tools(tools: Vec<ToolDefinition>) -> Vec<ChatCompletionTool> {
        tools
            .into_iter()
            .map(|t| ChatCompletionTool {
                r#type: ChatCompletionToolType::Function,
                function: FunctionObject {
                    name:        t.name,
                    description: Some(t.description),
                    parameters:  Some(t.parameters),
                    strict:      None,
                },
            })
            .collect()
    }
}

#[async_trait]
impl LlmProvider for OpenAICompatProvider {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        tools:    Vec<ToolDefinition>,
        max_tokens: Option<i32>,
    ) -> Result<ChatResponse> {
        let max_tokens_val = match max_tokens {
            Some(t) if t > 0 => t as u32,
            _ => 8192,
        };

        let req = CreateChatCompletionRequest {
            model:    self.model.clone(),
            messages: Self::convert_messages(messages),
            tools:    if tools.is_empty() { None } else { Some(Self::convert_tools(tools)) },
            max_completion_tokens: Some(max_tokens_val),
            ..Default::default()
        };

        let resp = self.client.chat().create(req).await
            .context("OpenAI-compat chat call failed")?;

        let choice = resp.choices.into_iter().next();
        let content = choice
            .as_ref()
            .and_then(|c| c.message.content.as_ref())
            .map(|c| c.to_string());
        let tool_calls = choice
            .and_then(|c| c.message.tool_calls)
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id:        tc.id,
                name:      tc.function.name,
                arguments: serde_json::from_str(&tc.function.arguments)
                               .unwrap_or(serde_json::Value::Null),
            })
            .collect();

        Ok(ChatResponse {
            content,
            tool_calls,
            tokens_used: resp.usage.map(|u| u.total_tokens),
        })
    }

    async fn stream(
        &self,
        messages: Vec<ChatMessage>,
        tools:    Vec<ToolDefinition>,
        max_tokens: Option<i32>,
    ) -> Result<TokenStream> {
        let max_tokens_val = match max_tokens {
            Some(t) if t > 0 => t as u32,
            _ => 8192,
        };

        let req = CreateChatCompletionRequest {
            model:    self.model.clone(),
            messages: Self::convert_messages(messages),
            tools:    if tools.is_empty() { None } else { Some(Self::convert_tools(tools)) },
            stream:   Some(true),
            max_completion_tokens: Some(max_tokens_val),
            ..Default::default()
        };

        let (tx, rx) = tokio::sync::mpsc::channel(128);
        let client = self.client.clone();

        tokio::spawn(async move {
            match client.chat().create_stream(req).await {
                Err(e) => { let _ = tx.send(Err(anyhow::anyhow!("{}", e))).await; }
                Ok(mut stream) => {
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Err(e) => { let _ = tx.send(Err(anyhow::anyhow!("{}", e))).await; break; }
                            Ok(c) => {
                                if let Some(delta) = c.choices.first()
                                    .and_then(|ch| ch.delta.content.as_ref())
                                {
                                    let _ = tx.send(Ok(TokenChunk {
                                        token:    delta.clone(),
                                        is_final: false,
                                    })).await;
                                }
                            }
                        }
                    }
                    let _ = tx.send(Ok(TokenChunk { token: String::new(), is_final: true })).await;
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let req_client = reqwest::Client::new();
        let url = format!("{}/models", self.base_url);
        
        let resp = req_client.get(&url).send().await
            .context("Failed to fetch models endpoint")?;
            
        let json: Value = resp.json().await
            .context("Failed to parse models response as JSON")?;
            
        let mut models = Vec::new();
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
                    // Filter out gemma-4 models - MLX server reports them but doesn't support them
                    // Error: "Model type gemma4_unified not supported"
                    if !id.contains("gemma-4") {
                        models.push(id.to_string());
                    }
                }
            }
        }
        
        Ok(models)
    }
}
