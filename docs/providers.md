# LLM Providers

## Overview

All providers implement the `LlmProvider` trait from `crates/providers`.
The workflow engine calls `ProviderManager::get(provider_id)` to resolve the correct adapter.

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse>;
    async fn stream(&self, req: ChatRequest) -> Result<Pin<Box<dyn Stream<Item=Result<String>> + Send>>>;
    async fn list_models(&self) -> Result<Vec<String>>;
}
```

---

## OpenAI-Compatible Provider

**Covers**: MLX LM (local), Google Gemini

Both providers expose an OpenAI-compatible REST API. The `async-openai` crate handles both
by setting a different `base_url`.

### MLX LM

Start the server locally:
```bash
mlx_lm.server --model mlx-community/Mistral-7B-Instruct-v0.3-4bit
# Listens on http://localhost:8080 by default
```

Configuration in Hive:
```
type:     openai_compat
base_url: http://host.docker.internal:8080   (reaches host from Docker)
api_key:  (leave empty — MLX LM has no auth)
model:    mlx-community/Mistral-7B-Instruct-v0.3-4bit
```

API is OpenAI-compatible: `/v1/chat/completions`, `/v1/models`.
Supports streaming (SSE), tool calls, system messages.

### Google Gemini

Google provides an OpenAI-compatible endpoint:
`https://generativelanguage.googleapis.com/v1beta/openai/`

Configuration in Hive:
```
type:     openai_compat
base_url: https://generativelanguage.googleapis.com/v1beta/openai/
api_key:  AIza...   (your Gemini API key)
model:    gemini-2.0-flash
```

Recommended models: `gemini-2.0-flash`, `gemini-2.5-pro`, `gemini-2.5-flash`

### Rust Implementation

```rust
// crates/providers/src/openai_compat.rs
use async_openai::{Client, config::OpenAIConfig};

pub struct OpenAICompatProvider {
    client: Client<OpenAIConfig>,
    model:  String,
}

impl OpenAICompatProvider {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key(if api_key.is_empty() { "none" } else { api_key });
        Self {
            client: Client::with_config(config),
            model:  model.to_string(),
        }
    }
}
```

---

## AWS Bedrock Provider

**Auth**: AWS SSO via the standard credential chain.

The backend container mounts `~/.aws` read-only:
```yaml
volumes:
  - ${HOME}/.aws:/root/.aws:ro
```

The SDK reads the profile from `AWS_PROFILE` env var and handles SSO token refresh
automatically — no manual credential management needed.

### Configuration in Hive

```
type:        aws_bedrock
aws_profile: my-sso-profile   (profile name in ~/.aws/config)
aws_region:  us-east-1
model:       anthropic.claude-3-5-sonnet-20241022-v2:0
```

### AWS SSO Config Example (`~/.aws/config`)

```ini
[profile my-sso-profile]
sso_start_url = https://my-org.awsapps.com/start
sso_account_id = 123456789012
sso_role_name = DeveloperAccess
sso_region = eu-west-1
region = us-east-1
```

Before using, run `aws sso login --profile my-sso-profile` on the host.

### Rust Implementation

```rust
// crates/providers/src/aws_bedrock.rs
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;

pub struct AwsBedrockProvider {
    client:   Client,
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
            client:   Client::new(&config),
            model_id: model_id.to_string(),
        })
    }
}
```

Bedrock uses the Converse API (`converse` / `converse_stream`) which supports
the same message format as OpenAI (system, user, assistant, tool).

---

## Provider Manager

`crates/providers/src/manager.rs`

Holds a registry of instantiated providers, keyed by DB UUID.

```rust
pub struct ProviderManager {
    providers: RwLock<HashMap<Uuid, Arc<dyn LlmProvider>>>,
    db:        PgPool,
}

impl ProviderManager {
    pub async fn get_or_load(&self, provider_id: Uuid) -> Result<Arc<dyn LlmProvider>>;
    pub async fn invalidate(&self, provider_id: Uuid);
    pub async fn test(&self, provider_id: Uuid) -> Result<Vec<String>>;
}
```

When a provider is created or updated via the API, `invalidate()` is called so
the next request rebuilds the adapter with fresh credentials.

---

## Adding a New Provider Type

1. Add variant to `ProviderType` enum in `crates/core/src/models.rs`
2. Add migration to update the DB `CHECK` constraint
3. Implement `LlmProvider` trait in a new file under `crates/providers/src/`
4. Add match arm in `ProviderManager::get_or_load()`
5. Add provider type to the UI provider form (`frontend/src/routes/settings/providers.tsx`)
