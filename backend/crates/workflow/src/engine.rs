use crate::steps::tool_runner::run_tool;
use anyhow::{Context, Result};
use deadpool_redis::Pool as RedisPool;
use futures::StreamExt;
use hive_core::{
    Agent, AgentMessage, Project, ProjectStatus, Skill, Story,
    TeamMember, Tool, WsEvent,
};
use hive_providers::{ChatMessage, LlmProvider, ProviderManager, ToolDefinition, ToolCall};
use sqlx::PgPool;
use std::{collections::HashMap, path::PathBuf, sync::Arc, sync::Mutex as StdMutex};
use tokio::sync::{Mutex as TokioMutex, Notify};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct MemoryGuard {
    engine:    Arc<WorkflowEngine>,
    story_id:  Uuid,
    memory_gb: f64,
}

impl Drop for MemoryGuard {
    fn drop(&mut self) {
        self.engine.release_memory(self.memory_gb);
        let active = Arc::clone(&self.engine.active_stories);
        let story_id = self.story_id;
        tokio::spawn(async move {
            let mut active = active.lock().await;
            active.remove(&story_id);
        });
    }
}

pub struct WorkflowEngine {
    pub db:             PgPool,
    pub redis:          RedisPool,
    pub providers:      Arc<ProviderManager>,
    pub executor_url:   String,
    pub projects_dir:   PathBuf,
    pub active_stories: Arc<TokioMutex<HashMap<Uuid, CancellationToken>>>,
    pub max_memory_gb:  f64,
    pub used_memory_gb: Arc<StdMutex<f64>>,
    pub memory_changed: Arc<Notify>,
    pub active_semaphore: Arc<tokio::sync::Semaphore>,
}

impl WorkflowEngine {
    pub fn new(
        db:           PgPool,
        redis:        RedisPool,
        providers:    Arc<ProviderManager>,
        executor_url: String,
        projects_dir: PathBuf,
    ) -> Self {
        let max_memory_gb = std::env::var("MAX_SYSTEM_MEMORY_GB")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(16.0); // Default to 16 GB

        let max_concurrent = std::env::var("MAX_CONCURRENT_AGENTS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4); // Default to 4 parallel agents

        Self {
            db,
            redis,
            providers,
            executor_url,
            projects_dir,
            active_stories: Arc::new(TokioMutex::new(HashMap::new())),
            max_memory_gb,
            used_memory_gb: Arc::new(StdMutex::new(0.0)),
            memory_changed: Arc::new(Notify::new()),
            active_semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrent)),
        }
    }

    /// Called when a project is created or after an approval is resolved.
    /// Spawns the appropriate Tokio task for the current status.
    pub fn advance(&self, project_id: Uuid) {
        let engine = self.clone_arc();
        tokio::spawn(async move {
            if let Err(e) = engine.do_advance(project_id).await {
                error!("Workflow error for project {}: {}", project_id, e);
                let _ = engine.publish_event(
                    project_id,
                    WsEvent::ProjectError {
                        project_id,
                        message: e.to_string(),
                    },
                ).await;
            }
        });
    }

    async fn do_advance(&self, project_id: Uuid) -> Result<()> {
        let project = self.get_project(project_id).await?;
        info!("Advancing project {} (status: {:?})", project_id, project.status);

        match project.status {
            ProjectStatus::Submitted          => self.run_lead_review(project).await,
            ProjectStatus::ArchitectBreakdown => self.run_architect(project).await,
            ProjectStatus::InDevelopment      => self.run_development(project).await,
            ProjectStatus::InReview           => self.run_review(project).await,
            // Awaiting* states: human must act — do nothing
            _ => Ok(())
        }
    }

    // ── Step 1: Lead Review ───────────────────────────────────────────────────

    async fn run_lead_review(&self, project: Project) -> Result<()> {
        self.set_status(&project.id, ProjectStatus::LeadReview).await?;
        let (agent, provider, skill) = self.get_agent_for_role(&project.team_id, "lead").await?;

        let user_msg = format!(
            "Project title: {}\n\nDescription:\n{}\n\nAcceptance Criteria:\n{}",
            project.title,
            project.description.as_deref().unwrap_or("(none)"),
            project.acceptance_criteria.as_deref().unwrap_or("(none)"),
        );

        let system_prompt = agent.system_prompt_override.as_deref()
            .unwrap_or(&skill.system_prompt);

        let messages = vec![
            ChatMessage { role: "system".into(), content: system_prompt.to_string(), tool_call_id: None },
            ChatMessage { role: "user".into(),   content: user_msg, tool_call_id: None },
        ];

        let rewritten = self.stream_and_collect(
            &project.id, "lead", &provider, messages, vec![],
            agent.max_tokens,
        ).await?;

        // Create approval record
        let payload = serde_json::json!({ "rewritten_spec": rewritten });
        self.create_approval(&project.id, "lead_rewrite", payload).await?;
        self.set_status(&project.id, ProjectStatus::AwaitingApprovalLead).await?;

        Ok(())
    }

// Helper to robustly parse stories JSON, handling both full arrays and lists of independent objects
fn parse_stories_json(raw: &str) -> Result<Vec<serde_json::Value>> {
    // 1. Try parsing direct array extracted via brackets
    let stories_json = extract_json_array(raw);
    if let Ok(stories) = serde_json::from_str::<Vec<serde_json::Value>>(&stories_json) {
        return Ok(stories);
    }

    // 2. Fallback: Parse individual JSON objects sequentially
    let mut stories = Vec::new();
    let mut depth = 0;
    let mut start_idx = None;

    let chars: Vec<char> = raw.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c == '{' {
            if depth == 0 {
                start_idx = Some(i);
            }
            depth += 1;
        } else if *c == '}' {
            depth -= 1;
            if depth == 0 {
                if let Some(start) = start_idx {
                    let obj_str: String = chars[start..=i].iter().collect();
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&obj_str) {
                        stories.push(obj);
                    }
                }
                start_idx = None;
            }
        }
    }

    if !stories.is_empty() {
        Ok(stories)
    } else {
        anyhow::bail!("No valid JSON user stories found in the architect response. Raw response: {}", raw)
    }
}

    // ── Step 2: Architect Breakdown ──────────────────────────────────────────

    async fn run_architect(&self, project: Project) -> Result<()> {
        let (agent, provider, skill) = self.get_agent_for_role(&project.team_id, "architect").await?;

        // Get the approved rewritten spec
        let rewritten_spec = self.get_latest_approval_payload(&project.id, "lead_rewrite")
            .await?
            .get("rewritten_spec")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let user_msg = format!(
            "Break this project specification into user stories.\n\n\
            Output a valid JSON array of stories, like:\n\
            [\n\
              {{ \"title\": \"Story 1\", \"description\": \"...\", \"acceptance_criteria\": \"...\" }},\n\
              {{ \"title\": \"Story 2\", \"description\": \"...\", \"acceptance_criteria\": \"...\" }}\n\
            ]\n\n\
            Do not output any other text or explanation. Only output the JSON array.\n\n\
            Spec:\n{}",
            rewritten_spec
        );

        let system_prompt = agent.system_prompt_override.as_deref()
            .unwrap_or(&skill.system_prompt);

        let messages = vec![
            ChatMessage { role: "system".into(), content: system_prompt.to_string(), tool_call_id: None },
            ChatMessage { role: "user".into(),   content: user_msg, tool_call_id: None },
        ];

        let raw = self.stream_and_collect(
            &project.id, "architect", &provider, messages, vec![],
            agent.max_tokens,
        ).await?;

        // Parse the story JSON robustly
        let stories = Self::parse_stories_json(&raw)?;

        let payload = serde_json::json!({ "stories": stories });
        self.create_approval(&project.id, "story_breakdown", payload).await?;
        self.set_status(&project.id, ProjectStatus::AwaitingApprovalStories).await?;

        Ok(())
    }

    // ── Step 3: Development ──────────────────────────────────────────────────

    async fn run_development(&self, project: Project) -> Result<()> {
        let stories: Vec<Story> = sqlx::query_as(
            "SELECT * FROM stories WHERE project_id = $1 AND status = 'todo' ORDER BY priority"
        )
        .bind(project.id)
        .fetch_all(&self.db)
        .await?;

        if stories.is_empty() {
            // All done — move to review
            self.set_status(&project.id, ProjectStatus::InReview).await?;
            return self.run_review(project).await;
        }

        let developer_members = self.get_members_by_role(&project.team_id, "developer").await?;
        if developer_members.is_empty() {
            anyhow::bail!("Team has no developer agents");
        }

        // Spawn one task per story (round-robin assign developers)
        let mut handles = Vec::new();

        for (i, story) in stories.into_iter().enumerate() {
            let member = developer_members[i % developer_members.len()].clone();
            let engine = self.clone_arc();
            let project = project.clone();
            let story_id = story.id;

            let token = CancellationToken::new();
            {
                let mut active = self.active_stories.lock().await;
                active.insert(story_id, token.clone());
            }

            let handle = tokio::spawn(async move {
                if let Err(e) = engine.run_story_with_cancel(&project, &story, &member, token).await {
                    error!("Error running story {}: {}", story_id, e);
                }
            });

            handles.push(handle);
        }

        // Wait for all stories, then transition
        let project_id = project.id;
        let engine = self.clone_arc();
        tokio::spawn(async move {
            for h in handles {
                if let Err(e) = h.await {
                    error!("Story task panicked: {}", e);
                }
            }
            // Check if all stories are done
            let pending: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM stories WHERE project_id = $1 AND status != 'done'"
            )
            .bind(project_id)
            .fetch_one(&engine.db)
            .await
            .unwrap_or(1);

            if pending == 0 {
                if let Err(e) = engine.set_status(&project_id, ProjectStatus::InReview).await {
                    error!("Failed to set InReview: {}", e);
                } else {
                    // Trigger review step
                    let project = engine.get_project(project_id).await.unwrap();
                    let _ = engine.run_review(project).await;
                }
            }
        });

        Ok(())
    }

    async fn run_story_with_cancel(
        &self,
        project: &Project,
        story:   &Story,
        member:  &TeamMember,
        token:   CancellationToken,
    ) -> Result<()> {
        tokio::select! {
            _ = token.cancelled() => {
                info!("Story {} was cancelled before execution started.", story.id);
                Ok(())
            }
            res = self.do_run_story(project, story, member, token.clone()) => {
                res
            }
        }
    }

    async fn do_run_story(
        &self,
        project: &Project,
        story:   &Story,
        member:  &TeamMember,
        token:   CancellationToken,
    ) -> Result<()> {
        // 1. Resolve agent and provider row to check memory requirements
        let agent_db: Agent = sqlx::query_as("SELECT * FROM agents WHERE id = $1")
            .bind(member.agent_id)
            .fetch_one(&self.db)
            .await?;

        let mut provider_row: hive_core::Provider = sqlx::query_as(
            "SELECT * FROM providers WHERE id = $1"
        )
        .bind(agent_db.provider_id)
        .fetch_one(&self.db)
        .await?;

        if let Some(ref model) = agent_db.model_name {
            provider_row.model_name = Some(model.clone());
        }

        let needed_memory = self.get_model_memory_requirement(&provider_row);

        // Concurrency Control: Acquire active agent concurrency permit before starting, selecting on cancel token
        let _concurrency_permit = tokio::select! {
            _ = token.cancelled() => {
                info!("Story {} cancelled while waiting for concurrency slot.", story.id);
                return Ok(());
            }
            permit = self.active_semaphore.acquire() => {
                permit?
            }
        };

        // 2. Concurrency Control: Acquire model memory before starting, selecting on cancel token
        let _guard = tokio::select! {
            _ = token.cancelled() => {
                info!("Story {} cancelled while waiting for memory.", story.id);
                return Ok(());
            }
            guard = self.acquire_memory(story.id, needed_memory) => {
                guard
            }
        };

        // 3. Mark story in progress
        sqlx::query("UPDATE stories SET status = 'in_progress', assigned_member_id = $1 WHERE id = $2")
            .bind(member.id)
            .bind(story.id)
            .execute(&self.db)
            .await?;

        self.publish_event(
            project.id,
            WsEvent::StoryUpdated { story_id: story.id, status: "in_progress".into() },
        ).await?;

        let (agent, provider, skill) = self.get_agent_by_member(member).await?;
        let tools = self.get_tools_for_agent(&agent).await?;
        let _tool_defs = tools_to_definitions(&tools);

        let system_prompt = agent.system_prompt_override.as_deref()
            .unwrap_or(&skill.system_prompt);

        let folder = project.folder_path.as_deref().unwrap_or("/tmp");

        let user_msg = format!(
            "Project: {}\n\nYour story:\nTitle: {}\nDescription: {}\nAcceptance Criteria: {}\n\n\
            Working directory: {}/src/\n\n\
            CRITICAL INSTRUCTIONS:\n\
            - You MUST physically create and save all code files to disk inside your working directory ({}/src/) using the `file_write` tool. Do NOT just execute code inside python/bash using `code_execute` without saving it.\n\
            - Every source file, script, or application component must be written and saved using `file_write` so that they are stored in the workspace permanently.\n\
            - Once all files are successfully written, saved, and verified, output a brief summary of what you built.",
            project.title,
            story.title,
            story.description.as_deref().unwrap_or(""),
            story.acceptance_criteria.as_deref().unwrap_or(""),
            folder,
            folder,
        );

        // Load message history for this developer
        let history = self.load_history(&project.id, &member.role, agent.memory_window).await?;
        let mut messages = vec![
            ChatMessage { role: "system".into(), content: system_prompt.to_string(), tool_call_id: None },
        ];
        messages.extend(history);
        messages.push(ChatMessage { role: "user".into(), content: user_msg, tool_call_id: None });

        // Agentic loop: keep calling LLM until no tool calls remain, selecting on cancel token
        tokio::select! {
            _ = token.cancelled() => {
                info!("Story {} cancelled during execution.", story.id);
                return Ok(());
            }
            res = self.run_agent_loop(project, &member.role, &provider, messages, &tools, agent.max_tokens) => {
                res?;
            }
        };

        // Mark story done
        sqlx::query("UPDATE stories SET status = 'done' WHERE id = $1")
            .bind(story.id)
            .execute(&self.db)
            .await?;

        self.publish_event(
            project.id,
            WsEvent::StoryUpdated { story_id: story.id, status: "done".into() },
        ).await?;

        Ok(())
    }

    // ── Step 4: Review ───────────────────────────────────────────────────────

    async fn run_review(&self, project: Project) -> Result<()> {
        let (agent, provider, skill) = self.get_agent_for_role(&project.team_id, "reviewer").await?;
        let stories: Vec<Story> = sqlx::query_as(
            "SELECT * FROM stories WHERE project_id = $1 ORDER BY priority"
        )
        .bind(project.id)
        .fetch_all(&self.db)
        .await?;

        let story_list = stories
            .iter()
            .map(|s| format!(
                "- **{}**: {}\n  Criteria: {}",
                s.title,
                s.description.as_deref().unwrap_or(""),
                s.acceptance_criteria.as_deref().unwrap_or(""),
            ))
            .collect::<Vec<_>>()
            .join("\n");

        let folder = project.folder_path.as_deref().unwrap_or("/tmp");

        let user_msg = format!(
            "Review the completed project '{}'\n\nStories implemented:\n{}\n\n\
            Project folder: {}/src/\n\
            Use file_list and file_read to inspect the code.\n\
            Produce a review report covering:\n\
            1. What was implemented\n2. Code quality observations\n\
            3. Any gaps vs acceptance criteria\n4. Overall assessment",
            project.title, story_list, folder,
        );

        let system_prompt = agent.system_prompt_override.as_deref()
            .unwrap_or(&skill.system_prompt);

        let tools = self.get_tools_for_agent(&agent).await?;
        let messages = vec![
            ChatMessage { role: "system".into(), content: system_prompt.to_string(), tool_call_id: None },
            ChatMessage { role: "user".into(),   content: user_msg, tool_call_id: None },
        ];

        let report = self.run_agent_loop(&project, "reviewer", &provider, messages, &tools, agent.max_tokens).await?;

        let payload = serde_json::json!({ "review_report": report });
        self.create_approval(&project.id, "final_review", payload).await?;
        self.set_status(&project.id, ProjectStatus::AwaitingFinalApproval).await?;

        Ok(())
    }

    // ── Agentic loop ─────────────────────────────────────────────────────────

    /// Run an agent loop: LLM → tool calls → tool results → LLM, until no more tool calls.
    /// Returns the final text response.
    async fn run_agent_loop(
        &self,
        project:    &Project,
        role:       &str,
        provider:   &Arc<dyn LlmProvider>,
        mut messages: Vec<ChatMessage>,
        tools:      &[Tool],
        max_tokens: Option<i32>,
    ) -> Result<String> {
        // Centralized fallback tool-injection for local and compatible LLM models that ignore native API tool-calling
        if !tools.is_empty() {
            if let Some(sys_msg) = messages.get_mut(0) {
                if sys_msg.role == "system" {
                    sys_msg.content.push_str("\n\n## AVAILABLE TOOLS FOR THIS TASK\n\
                        You are equipped with specialized tools. Since your model runner may not support native API tool-calling schemas, you MUST invoke tools by outputting a standard Markdown code block containing a single JSON object in the exact format shown below.\n\n\
                        ### HOW TO CALL A TOOL\n\
                        Your response must contain a tool invocation block in this format:\n\
                        ```json\n\
                        {\n\
                          \"name\": \"tool_name\",\n\
                          \"parameters\": {\n\
                            \"param1\": \"value1\"\n\
                          }\n\
                        }\n\
                        ```\n\n\
                        ### DETAILED TOOL SCHEMAS\n");

                    for t in tools {
                        let schema = t.config.get("schema")
                            .and_then(|v| serde_json::to_string_pretty(v).ok())
                            .unwrap_or_else(|| "{}".into());
                        sys_msg.content.push_str(&format!(
                            "- **{}**: {}\n  Schema:\n  ```json\n  {}\n  ```\n\n",
                            t.name,
                            t.description.as_deref().unwrap_or("(no description)"),
                            schema
                        ));
                    }

                    sys_msg.content.push_str("\nCRITICAL: When you need to read, write, list files or run code, you MUST output a tool call. Specifically, to save files, call **file_write** with 'path' and 'content' parameters. Do not just output raw text or explanations when action is required!");
                }
            }
        }

        let tool_defs = tools_to_definitions(tools);
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > 20 {
                warn!("Agent loop exceeded 20 iterations for project {}", project.id);
                break;
            }

            let mut resp = provider.chat(messages.clone(), tool_defs.clone(), max_tokens).await?;

            // If no native tool calls were found, but the model generated them in the text content, extract them!
            if resp.tool_calls.is_empty() {
                if let Some(ref text) = resp.content {
                    let extracted = Self::extract_tool_calls_from_text(text);
                    if !extracted.is_empty() {
                        info!("Extracted {} tool calls from response text", extracted.len());
                        resp.tool_calls = extracted;
                    }
                }
            }

            // Stream final content via WebSocket (simplified: just emit full content)
            if let Some(ref content) = resp.content {
                self.save_message(project.id, role, "system", content, resp.tokens_used).await?;
            }

            if resp.tool_calls.is_empty() {
                let final_content = resp.content.unwrap_or_default();
                return Ok(final_content);
            }

            // Add assistant message with tool calls
            let assistant_content = resp.content.unwrap_or_default();
            messages.push(ChatMessage {
                role:         "assistant".into(),
                content:      assistant_content,
                tool_call_id: None,
            });

            // Execute each tool call and add results
            for tc in &resp.tool_calls {
                let tool = tools.iter().find(|t| t.name == tc.name);
                let result = if let Some(t) = tool {
                    run_tool(t, &tc.arguments, project, &self.executor_url).await
                        .unwrap_or_else(|e| format!("Tool error: {}", e))
                } else {
                    format!("Unknown tool: {}", tc.name)
                };

                messages.push(ChatMessage {
                    role:         "tool".into(),
                    content:      result,
                    tool_call_id: Some(tc.id.clone()),
                });
            }
        }

        Ok(String::new())
    }

    fn extract_tool_calls_from_text(text: &str) -> Vec<ToolCall> {
        let mut tool_calls = Vec::new();
        let mut start = 0;
        
        while let Some(json_start) = text[start..].find("```json") {
            let block_start = start + json_start + 7;
            if let Some(json_end) = text[block_start..].find("```") {
                let block_end = block_start + json_end;
                let json_str = text[block_start..block_end].trim();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(name) = val["name"].as_str() {
                        let args = val["parameters"].clone();
                        tool_calls.push(ToolCall {
                            id: format!("call_{}", Uuid::new_v4()),
                            name: name.to_string(),
                            arguments: args,
                        });
                    }
                }
                start = block_end + 3;
            } else {
                break;
            }
        }

        if tool_calls.is_empty() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                if let Some(name) = val["name"].as_str() {
                    let args = val["parameters"].clone();
                    tool_calls.push(ToolCall {
                        id: format!("call_{}", Uuid::new_v4()),
                        name: name.to_string(),
                        arguments: args,
                    });
                }
            }
        }

        tool_calls
    }

    // ── Streaming helper ─────────────────────────────────────────────────────

    async fn stream_and_collect(
        &self,
        project_id: &Uuid,
        role:       &str,
        provider:   &Arc<dyn LlmProvider>,
        messages:   Vec<ChatMessage>,
        tools:      Vec<ToolDefinition>,
        max_tokens: Option<i32>,
    ) -> Result<String> {
        let mut stream = provider.stream(messages, tools, max_tokens).await?;
        let mut full   = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if !chunk.is_final {
                let _ = self.publish_event(
                    *project_id,
                    WsEvent::AgentToken {
                        project_id: *project_id,
                        role:       role.to_string(),
                        token:      chunk.token.clone(),
                    },
                ).await;
                full.push_str(&chunk.token);
            }
        }

        self.save_message(*project_id, role, "system", &full, None).await?;
        Ok(full)
    }

    // ── DB helpers ────────────────────────────────────────────────────────────

    async fn get_project(&self, id: Uuid) -> Result<Project> {
        sqlx::query_as("SELECT * FROM projects WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Project {} not found", id))
    }

    async fn set_status(&self, project_id: &Uuid, status: ProjectStatus) -> Result<()> {
        sqlx::query("UPDATE projects SET status = $1 WHERE id = $2")
            .bind(&status)
            .bind(project_id)
            .execute(&self.db)
            .await?;

        self.publish_event(
            *project_id,
            WsEvent::ProjectStatusChanged { project_id: *project_id, status },
        ).await?;

        Ok(())
    }

    async fn get_agent_for_role(&self, team_id: &Uuid, role: &str)
        -> Result<(Agent, Arc<dyn LlmProvider>, Skill)>
    {
        let member: TeamMember = sqlx::query_as(
            "SELECT * FROM team_members WHERE team_id = $1 AND role = $2 LIMIT 1"
        )
        .bind(team_id)
        .bind(role)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Team has no '{}' agent", role))?;

        self.get_agent_by_member(&member).await
    }

    async fn get_agent_by_member(&self, member: &TeamMember)
        -> Result<(Agent, Arc<dyn LlmProvider>, Skill)>
    {
        let agent: Agent = sqlx::query_as("SELECT * FROM agents WHERE id = $1")
            .bind(member.agent_id)
            .fetch_one(&self.db)
            .await?;

        let mut provider_row: hive_core::Provider = sqlx::query_as(
            "SELECT * FROM providers WHERE id = $1"
        )
        .bind(agent.provider_id)
        .fetch_one(&self.db)
        .await?;

        // If agent has a specific model, use it instead of provider's default
        if let Some(ref model) = agent.model_name {
            provider_row.model_name = Some(model.clone());
        }

        // Fetch all skills and combine their system prompts
        let mut combined_prompt = String::new();
        let mut first_skill_name = String::new();
        
        for skill_id in &agent.skill_ids {
            let skill: Skill = sqlx::query_as("SELECT * FROM skills WHERE id = $1")
                .bind(skill_id)
                .fetch_one(&self.db)
                .await?;
            
            if first_skill_name.is_empty() {
                first_skill_name = skill.name.clone();
            }
            
            if !combined_prompt.is_empty() {
                combined_prompt.push_str("\n\n---\n\n");
            }
            combined_prompt.push_str(&format!("# Skill: {}\n\n{}", skill.name, skill.system_prompt));
        }

        // Create a combined skill for backwards compatibility
        let combined_skill = Skill {
            id: agent.skill_ids.first().copied().unwrap_or_default(),
            name: if agent.skill_ids.len() > 1 {
                format!("{} (+{} more)", first_skill_name, agent.skill_ids.len() - 1)
            } else {
                first_skill_name
            },
            description: "Combined skills".to_string(),
            source_url: None,
            source_type: None,
            system_prompt: combined_prompt,
            tool_hints: vec![],
            version: None,
            created_at: chrono::Utc::now(),
        };

        let provider = self.providers.get(&provider_row).await?;
        Ok((agent, provider, combined_skill))
    }

    async fn get_members_by_role(&self, team_id: &Uuid, role: &str) -> Result<Vec<TeamMember>> {
        Ok(sqlx::query_as(
            "SELECT * FROM team_members WHERE team_id = $1 AND role = $2"
        )
        .bind(team_id)
        .bind(role)
        .fetch_all(&self.db)
        .await?)
    }

    async fn get_tools_for_agent(&self, agent: &Agent) -> Result<Vec<Tool>> {
        if agent.tool_ids.is_empty() { return Ok(vec![]); }
        Ok(sqlx::query_as(
            "SELECT * FROM tools WHERE id = ANY($1)"
        )
        .bind(&agent.tool_ids)
        .fetch_all(&self.db)
        .await?)
    }

    async fn load_history(&self, project_id: &Uuid, role: &str, limit: i32) -> Result<Vec<ChatMessage>> {
        let msgs: Vec<AgentMessage> = sqlx::query_as(
            "SELECT * FROM agent_messages WHERE project_id = $1 AND from_role = $2 \
             ORDER BY created_at DESC LIMIT $3"
        )
        .bind(project_id)
        .bind(role)
        .bind(limit)
        .fetch_all(&self.db)
        .await?;

        Ok(msgs.into_iter().rev().map(|m| ChatMessage {
            role:         m.from_role.unwrap_or_else(|| "assistant".into()),
            content:      m.content,
            tool_call_id: None,
        }).collect())
    }

    pub async fn save_message(
        &self,
        project_id: Uuid,
        from_role:  &str,
        to_role:    &str,
        content:    &str,
        tokens:     Option<u32>,
    ) -> Result<()> {
        let msg: AgentMessage = sqlx::query_as(
            "INSERT INTO agent_messages (project_id, from_role, to_role, content, tokens_used) \
             VALUES ($1, $2, $3, $4, $5) RETURNING *"
        )
        .bind(project_id)
        .bind(from_role)
        .bind(to_role)
        .bind(content)
        .bind(tokens.map(|t| t as i32))
        .fetch_one(&self.db)
        .await?;

        self.publish_event(
            project_id,
            WsEvent::MessageCreated { message: msg },
        ).await?;

        Ok(())
    }

    async fn create_approval(
        &self,
        project_id: &Uuid,
        kind:       &str,
        payload:    serde_json::Value,
    ) -> Result<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO approvals (project_id, type, payload) VALUES ($1, $2, $3) RETURNING id"
        )
        .bind(project_id)
        .bind(kind)
        .bind(payload.clone())
        .fetch_one(&self.db)
        .await?;

        self.publish_event(
            *project_id,
            WsEvent::ApprovalRequired {
                approval_id: id,
                kind:        kind.to_string(),
                payload,
            },
        ).await?;

        Ok(id)
    }

    async fn get_latest_approval_payload(
        &self,
        project_id: &Uuid,
        kind:       &str,
    ) -> Result<serde_json::Value> {
        let payload: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT payload FROM approvals \
             WHERE project_id = $1 AND type = $2 AND status = 'approved' \
             ORDER BY created_at DESC LIMIT 1"
        )
        .bind(project_id)
        .bind(kind)
        .fetch_optional(&self.db)
        .await?
        .flatten();

        Ok(payload.unwrap_or(serde_json::Value::Null))
    }

    pub async fn acquire_memory(&self, story_id: Uuid, needed_memory: f64) -> MemoryGuard {
        info!("Story {} requesting {} GB of memory", story_id, needed_memory);
        loop {
            {
                let mut used = self.used_memory_gb.lock().unwrap();
                if *used + needed_memory <= self.max_memory_gb || *used == 0.0 {
                    *used += needed_memory;
                    info!(
                        "Story {} acquired {} GB of memory. Total used: {}/{} GB",
                        story_id, needed_memory, *used, self.max_memory_gb
                    );
                    return MemoryGuard {
                        engine: self.clone_arc(),
                        story_id,
                        memory_gb: needed_memory,
                    };
                }
            }
            self.memory_changed.notified().await;
        }
    }

    pub fn release_memory(&self, memory_gb: f64) {
        let mut used = self.used_memory_gb.lock().unwrap();
        *used = (*used - memory_gb).max(0.0);
        info!(
            "Released {} GB of memory. Total used: {}/{} GB",
            memory_gb, *used, self.max_memory_gb
        );
        self.memory_changed.notify_waiters();
    }

    pub fn get_model_memory_requirement(&self, provider: &hive_core::Provider) -> f64 {
        if let Some(mem) = provider.config.get("model_memory_gb").and_then(|v| v.as_f64()) {
            return mem;
        }

        if let Some(ref model_name) = provider.model_name {
            let name_lower = model_name.to_lowercase();
            if name_lower.contains("gpt-")
                || name_lower.contains("claude-")
                || name_lower.contains("gemini")
                || name_lower.contains("bedrock")
                || name_lower.contains("nova-")
                || provider.kind == "aws_bedrock"
            {
                return 0.0;
            }

            if name_lower.contains("70b") {
                48.0
            } else if name_lower.contains("32b") {
                24.0
            } else if name_lower.contains("14b") || name_lower.contains("13b") {
                10.0
            } else if name_lower.contains("8b") || name_lower.contains("7b") || name_lower.contains("9b") {
                6.0
            } else if name_lower.contains("3b") || name_lower.contains("2b") || name_lower.contains("1b") {
                2.0
            } else {
                4.0
            }
        } else {
            if provider.kind == "aws_bedrock" {
                0.0
            } else {
                4.0
            }
        }
    }

    pub async fn cancel_story(&self, story_id: Uuid) -> bool {
        let mut active = self.active_stories.lock().await;
        if let Some(token) = active.remove(&story_id) {
            info!("Cancelling token for story {}", story_id);
            token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn handle_story_deleted(&self, project_id: Uuid) -> Result<()> {
        let project = self.get_project(project_id).await?;
        if project.status == ProjectStatus::InDevelopment {
            let pending: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM stories WHERE project_id = $1 AND status != 'done'"
            )
            .bind(project_id)
            .fetch_one(&self.db)
            .await
            .unwrap_or(0);

            if pending == 0 {
                if let Err(e) = self.set_status(&project_id, ProjectStatus::InReview).await {
                    error!("Failed to set InReview after story deletion: {}", e);
                } else {
                    let _ = self.run_review(project).await;
                }
            }
        }
        Ok(())
    }

    // ── Redis pub/sub ─────────────────────────────────────────────────────────

    pub async fn publish_event(&self, project_id: Uuid, event: WsEvent) -> Result<()> {
        let json = serde_json::to_string(&event)?;
        let channel = format!("project:{}:events", project_id);
        let mut conn = self.redis.get().await?;
        
        // Publish to project-specific channel
        redis::cmd("PUBLISH")
            .arg(&channel)
            .arg(&json)
            .query_async::<()>(&mut *conn)
            .await?;

        // Publish to global board channel
        redis::cmd("PUBLISH")
            .arg("board:events")
            .arg(&json)
            .query_async::<()>(&mut *conn)
            .await?;

        Ok(())
    }

    fn clone_arc(&self) -> Arc<Self> {
        Arc::new(Self {
            db:             self.db.clone(),
            redis:          self.redis.clone(),
            providers:      Arc::clone(&self.providers),
            executor_url:   self.executor_url.clone(),
            projects_dir:   self.projects_dir.clone(),
            active_stories: Arc::clone(&self.active_stories),
            max_memory_gb:  self.max_memory_gb,
            used_memory_gb: Arc::clone(&self.used_memory_gb),
            memory_changed: Arc::clone(&self.memory_changed),
            active_semaphore: Arc::clone(&self.active_semaphore),
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn tools_to_definitions(tools: &[Tool]) -> Vec<ToolDefinition> {
    tools.iter().map(|t| ToolDefinition {
        name:        t.name.clone(),
        description: t.description.clone().unwrap_or_default(),
        parameters:  t.config.get("schema")
                         .cloned()
                         .unwrap_or_else(|| serde_json::json!({ "type": "object", "properties": {} })),
    }).collect()
}

/// Extract the first JSON array from an LLM response (which may have surrounding text).
fn extract_json_array(s: &str) -> String {
    if let (Some(start), Some(end)) = (s.find('['), s.rfind(']')) {
        s[start..=end].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_get_model_memory_requirement() {
        let db = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let redis = deadpool_redis::Config::from_url("redis://localhost").create_pool(Some(deadpool_redis::Runtime::Tokio1)).unwrap();
        let providers = Arc::new(ProviderManager::new());
        let engine = WorkflowEngine::new(
            db,
            redis,
            providers,
            "http://localhost:3001".into(),
            PathBuf::from("/tmp"),
        );

        // 1. Cloud models should return 0.0 GB
        let gpt4 = hive_core::Provider {
            id: Uuid::new_v4(),
            name: "GPT-4".into(),
            kind: "openai_compat".into(),
            base_url: Some("https://api.openai.com/v1".into()),
            api_key: Some("key".into()),
            aws_profile: None,
            aws_region: None,
            model_name: Some("gpt-4-turbo".into()),
            config: json!({}),
            is_active: true,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(engine.get_model_memory_requirement(&gpt4), 0.0);

        let claude = hive_core::Provider {
            id: Uuid::new_v4(),
            name: "Claude".into(),
            kind: "aws_bedrock".into(),
            base_url: None,
            api_key: None,
            aws_profile: Some("default".into()),
            aws_region: Some("us-east-1".into()),
            model_name: Some("anthropic.claude-3-5-sonnet-20241022-v2:0".into()),
            config: json!({}),
            is_active: true,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(engine.get_model_memory_requirement(&claude), 0.0);

        // 2. Local models should parse size suffixes
        let llama70b = hive_core::Provider {
            id: Uuid::new_v4(),
            name: "Llama 70B".into(),
            kind: "openai_compat".into(),
            base_url: Some("http://localhost:8080".into()),
            api_key: None,
            aws_profile: None,
            aws_region: None,
            model_name: Some("llama3-70b-instruct".into()),
            config: json!({}),
            is_active: true,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(engine.get_model_memory_requirement(&llama70b), 48.0);

        let llama8b = hive_core::Provider {
            id: Uuid::new_v4(),
            name: "Llama 8B".into(),
            kind: "openai_compat".into(),
            base_url: Some("http://localhost:8080".into()),
            api_key: None,
            aws_profile: None,
            aws_region: None,
            model_name: Some("llama3-8b-instruct".into()),
            config: json!({}),
            is_active: true,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(engine.get_model_memory_requirement(&llama8b), 6.0);

        // 3. Custom config override should take precedence
        let custom_prov = hive_core::Provider {
            id: Uuid::new_v4(),
            name: "Custom Llama 8B Override".into(),
            kind: "openai_compat".into(),
            base_url: Some("http://localhost:8080".into()),
            api_key: None,
            aws_profile: None,
            aws_region: None,
            model_name: Some("llama3-8b-instruct".into()),
            config: json!({ "model_memory_gb": 12.5 }),
            is_active: true,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(engine.get_model_memory_requirement(&custom_prov), 12.5);
    }

    #[tokio::test]
    async fn test_acquire_and_release_memory() {
        let db = sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let redis = deadpool_redis::Config::from_url("redis://localhost").create_pool(Some(deadpool_redis::Runtime::Tokio1)).unwrap();
        let providers = Arc::new(ProviderManager::new());
        let mut engine = WorkflowEngine::new(
            db,
            redis,
            providers,
            "http://localhost:3001".into(),
            PathBuf::from("/tmp"),
        );
        engine.max_memory_gb = 16.0;

        // Acquire first memory permit (6.0 GB)
        let story1 = Uuid::new_v4();
        let guard1 = engine.acquire_memory(story1, 6.0).await;
        assert_eq!(*engine.used_memory_gb.lock().unwrap(), 6.0);

        // Acquire second memory permit (8.0 GB, total 14.0 GB)
        let story2 = Uuid::new_v4();
        let guard2 = engine.acquire_memory(story2, 8.0).await;
        assert_eq!(*engine.used_memory_gb.lock().unwrap(), 14.0);

        // Try to acquire a third permit (4.0 GB) - this would exceed the 16.0 GB limit, so it should wait.
        let engine_clone = Arc::new(engine);
        let story3 = Uuid::new_v4();
        
        let waiter_task = tokio::spawn(async move {
            let _guard3 = engine_clone.acquire_memory(story3, 4.0).await;
            assert_eq!(*engine_clone.used_memory_gb.lock().unwrap(), 4.0); // guard1 and guard2 are dropped below, leaving 4.0
        });

        // Sleep briefly to ensure task is waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        // Drop the first two guards
        drop(guard1);
        drop(guard2);

        // Now the waiter task should proceed and finish successfully
        waiter_task.await.unwrap();
    }

    #[test]
    fn test_extract_json_array() {
        let input_with_text = "Here is some JSON:\n```json\n[\n  {\"title\": \"Task 1\"},\n  {\"title\": \"Task 2\"}\n]\n```\nHope you like it!";
        let expected = "[\n  {\"title\": \"Task 1\"},\n  {\"title\": \"Task 2\"}\n]";
        assert_eq!(extract_json_array(input_with_text), expected);

        let input_raw = "[{\"title\": \"Only array\"}]";
        assert_eq!(extract_json_array(input_raw), input_raw);

        let input_no_brackets = "plain text without brackets";
        assert_eq!(extract_json_array(input_no_brackets), input_no_brackets);
    }

    #[test]
    fn test_parse_stories_json() {
        // Test standard array format
        let array_input = "[\n  {\"title\": \"Story 1\"},\n  {\"title\": \"Story 2\"}\n]";
        let res = WorkflowEngine::parse_stories_json(array_input).unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0]["title"], "Story 1");
        assert_eq!(res[1]["title"], "Story 2");

        // Test non-array, multiple objects list format
        let raw_objects_input = "Here are stories:\n```\n{\n  \"title\": \"Story A\"\n}\n\n{\n  \"title\": \"Story B\"\n}\n```";
        let res = WorkflowEngine::parse_stories_json(raw_objects_input).unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0]["title"], "Story A");
        assert_eq!(res[1]["title"], "Story B");
    }
}
