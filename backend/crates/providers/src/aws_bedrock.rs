use crate::{ChatMessage, ChatResponse, LlmProvider, ToolCall, ToolDefinition, TokenChunk, TokenStream};
use anyhow::Result;
use async_trait::async_trait;
use aws_sdk_bedrockruntime::{
    Client,
    types::{
        Message, ContentBlock, SystemContentBlock, ConversationRole, 
        ToolConfiguration, Tool, ToolInputSchema,
        ToolResultBlock, ToolResultContentBlock, ToolResultStatus,
    },
};
use aws_config::BehaviorVersion;
use tokio_stream::wrappers::ReceiverStream;

fn document_to_json(doc: &aws_smithy_types::Document) -> serde_json::Value {
    match doc {
        aws_smithy_types::Document::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), document_to_json(v));
            }
            serde_json::Value::Object(out)
        }
        aws_smithy_types::Document::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(document_to_json).collect())
        }
        aws_smithy_types::Document::Number(n) => {
            if let Some(f) = serde_json::Number::from_f64(n.to_f64_lossy()) {
                serde_json::Value::Number(f)
            } else {
                serde_json::Value::Null
            }
        }
        aws_smithy_types::Document::String(s) => serde_json::Value::String(s.clone()),
        aws_smithy_types::Document::Bool(b) => serde_json::Value::Bool(*b),
        aws_smithy_types::Document::Null => serde_json::Value::Null,
    }
}

pub struct AwsBedrockProvider {
    client: Client,
    model_id: String,
}

impl AwsBedrockProvider {
    pub async fn new(profile: &str, region: &str, model_id: &str) -> Result<Self> {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .profile_name(profile)
            .region(aws_config::Region::new(region.to_string()))
            .load()
            .await;
        
        Ok(Self {
            client: Client::new(&config),
            model_id: model_id.to_string(),
        })
    }

    fn convert_messages(messages: Vec<ChatMessage>) -> (Vec<SystemContentBlock>, Vec<Message>) {
        let mut sys = Vec::new();
        let mut out = Vec::new();

        for m in messages {
            match m.role.as_str() {
                "system" => {
                    sys.push(SystemContentBlock::Text(m.content));
                }
                "assistant" => {
                    out.push(
                        Message::builder()
                            .role(ConversationRole::Assistant)
                            .content(ContentBlock::Text(m.content))
                            .build()
                            .unwrap()
                    );
                }
                "tool" => {
                    out.push(
                        Message::builder()
                            .role(ConversationRole::User)
                            .content(ContentBlock::ToolResult(
                                ToolResultBlock::builder()
                                    .tool_use_id(m.tool_call_id.unwrap_or_default())
                                    .content(ToolResultContentBlock::Text(m.content))
                                    .status(ToolResultStatus::Success)
                                    .build()
                                    .unwrap()
                            ))
                            .build()
                            .unwrap()
                    );
                }
                _ => { // user
                    out.push(
                        Message::builder()
                            .role(ConversationRole::User)
                            .content(ContentBlock::Text(m.content))
                            .build()
                            .unwrap()
                    );
                }
            }
        }
        (sys, out)
    }

    fn convert_tools(tools: Vec<ToolDefinition>) -> Option<ToolConfiguration> {
        if tools.is_empty() {
            return None;
        }

        let mut out = Vec::new();
        for t in tools {
            out.push(
                Tool::ToolSpec(
                    aws_sdk_bedrockruntime::types::ToolSpecification::builder()
                        .name(t.name)
                        .description(t.description)
                        .input_schema(ToolInputSchema::Json(
                            aws_smithy_types::Document::String(serde_json::to_string(&t.parameters).unwrap_or_default())
                        ))
                        .build()
                        .unwrap()
                )
            );
        }

        Some(
            ToolConfiguration::builder()
                .set_tools(Some(out))
                .build()
                .unwrap()
        )
    }
}

#[async_trait]
impl LlmProvider for AwsBedrockProvider {
    async fn chat(&self, messages: Vec<ChatMessage>, tools: Vec<ToolDefinition>, max_tokens: Option<i32>) -> Result<ChatResponse> {
        let (sys, msgs) = Self::convert_messages(messages);
        
        let mut req = self.client.converse()
            .model_id(&self.model_id)
            .set_system(if sys.is_empty() { None } else { Some(sys) })
            .set_messages(Some(msgs));

        let max_tokens_val = match max_tokens {
            Some(t) if t > 0 => t,
            _ => 8192,
        };
        let inf_config = aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
            .max_tokens(max_tokens_val)
            .build();
        req = req.inference_config(inf_config);

        if let Some(tc) = Self::convert_tools(tools) {
            req = req.tool_config(tc);
        }

        let resp = req.send().await?;

        let mut content = None;
        let mut tool_calls = Vec::new();

        if let Some(aws_sdk_bedrockruntime::types::ConverseOutput::Message(m)) = resp.output {
            for block in m.content {
                match block {
                    ContentBlock::Text(t) => {
                        content = Some(t);
                    }
                    ContentBlock::ToolUse(tu) => {
                        let args = if let aws_smithy_types::Document::Object(_) = &tu.input {
                            document_to_json(&tu.input)
                        } else {
                            serde_json::Value::Null
                        };
                        tool_calls.push(ToolCall {
                            id: tu.tool_use_id,
                            name: tu.name,
                            arguments: args,
                        });
                    }
                    _ => {}
                }
            }
        }

        let tokens_used = resp.usage.map(|u| u.total_tokens as u32);

        Ok(ChatResponse {
            content,
            tool_calls,
            tokens_used,
        })
    }

    async fn stream(&self, messages: Vec<ChatMessage>, tools: Vec<ToolDefinition>, max_tokens: Option<i32>) -> Result<TokenStream> {
        let (sys, msgs) = Self::convert_messages(messages);
        
        let mut req = self.client.converse_stream()
            .model_id(&self.model_id)
            .set_system(if sys.is_empty() { None } else { Some(sys) })
            .set_messages(Some(msgs));

        let max_tokens_val = match max_tokens {
            Some(t) if t > 0 => t,
            _ => 8192,
        };
        let inf_config = aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
            .max_tokens(max_tokens_val)
            .build();
        req = req.inference_config(inf_config);

        if let Some(tc) = Self::convert_tools(tools) {
            req = req.tool_config(tc);
        }

        let resp = req.send().await?;
        let mut stream = resp.stream;

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            loop {
                match stream.recv().await {
                    Ok(Some(aws_sdk_bedrockruntime::types::ConverseStreamOutput::ContentBlockDelta(delta))) => {
                        if let Some(aws_sdk_bedrockruntime::types::ContentBlockDelta::Text(text)) = delta.delta {
                            let _ = tx.send(Ok(TokenChunk {
                                token: text,
                                is_final: false,
                            })).await;
                        }
                    }
                    Ok(Some(_)) => {} // other block types
                    Ok(None) => break, // EOF
                    Err(e) => {
                        let _ = tx.send(Err(anyhow::anyhow!("Stream error: {:?}", e))).await;
                        break;
                    }
                }
            }
            let _ = tx.send(Ok(TokenChunk { token: String::new(), is_final: true })).await;
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        Ok(vec![self.model_id.clone()])
    }
}
