use crate::{
    aws_bedrock::AwsBedrockProvider,
    openai_compat::OpenAICompatProvider,
    LlmProvider,
};
use anyhow::Result;
use hive_core::Provider;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Holds instantiated provider adapters keyed by DB UUID.
/// Lazily constructed on first use; invalidated on update.
pub struct ProviderManager {
    cache: RwLock<HashMap<Uuid, Arc<dyn LlmProvider>>>,
}

impl ProviderManager {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get a provider, building it from DB record if not cached.
    pub async fn get(&self, provider: &Provider) -> Result<Arc<dyn LlmProvider>> {
        {
            let cache = self.cache.read().await;
            if let Some(p) = cache.get(&provider.id) {
                return Ok(Arc::clone(p));
            }
        }

        let built = self.build(provider).await?;
        let mut cache = self.cache.write().await;
        cache.insert(provider.id, Arc::clone(&built));
        Ok(built)
    }

    /// Drop a cached provider (call after update/delete).
    pub async fn invalidate(&self, provider_id: Uuid) {
        self.cache.write().await.remove(&provider_id);
    }

    async fn build(&self, p: &Provider) -> Result<Arc<dyn LlmProvider>> {
        match p.kind.as_str() {
            "openai_compat" => {
                let base_url  = p.base_url.as_deref().unwrap_or("http://localhost:8080");
                let api_key   = p.api_key.as_deref().unwrap_or("");
                let model     = p.model_name.as_deref().unwrap_or("default");
                Ok(Arc::new(OpenAICompatProvider::new(base_url, api_key, model)))
            }
            "aws_bedrock" => {
                let profile = p.aws_profile.as_deref().unwrap_or("default");
                let region  = p.aws_region.as_deref().unwrap_or("us-east-1");
                let model   = p.model_name.as_deref()
                    .unwrap_or("anthropic.claude-3-5-sonnet-20241022-v2:0");
                Ok(Arc::new(AwsBedrockProvider::new(profile, region, model).await?))
            }
            other => anyhow::bail!("Unknown provider type: {}", other),
        }
    }
}

impl Default for ProviderManager {
    fn default() -> Self { Self::new() }
}
