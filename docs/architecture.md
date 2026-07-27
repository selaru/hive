# Architecture

## System Overview

```
┌─────────────────────────────────────────────────────┐
│                  Browser (React SPA)                 │
│  Board | Agent Builder | Team Builder | Settings     │
└──────────────────────┬──────────────────────────────┘
                       │ HTTP REST + WebSocket
┌──────────────────────▼──────────────────────────────┐
│              API Server  (Rust / Axum :3000)        │
│         REST handlers  |  WebSocket hub  |  CORS    │
└──────┬───────────┬────────────┬────────┬────────────┘
       │           │            │        │
  Provider     Skill         Agent    Workflow
  Manager      Registry      Factory  Engine (Tokio tasks)
  (crate:      (crate:       (inline) (crate: workflow)
  providers)   skills)
       └───────────┴────────────┴────────┘
                            │ sqlx async
                    ┌───────▼──────────┐
                    │  PostgreSQL 16   │
                    └───────┬──────────┘
                            │
              ┌─────────────┴──────────────┐
           Redis 7                  Executor Service
        (pub-sub/queue)         (Rust Axum :3001, internal)
        workflow events          Docker-in-Docker sandboxed
        token streaming          code execution
```

## Crate Structure

```
backend/
└── crates/
    ├── core/       Domain models, shared traits, error types
    ├── providers/  LLM provider adapters (OpenAI-compat, AWS Bedrock)
    ├── skills/     SKILL.md parser and GitHub/skills.sh importer
    ├── workflow/   Project state machine and agent step runners
    ├── api/        Axum HTTP server, routes, handlers, WebSocket hub
    └── executor/   Separate binary: sandboxed code execution service
```

Dependency graph (→ = depends on):

```
api → workflow → providers → core
api → skills    → core
api → core
executor → core
```

## Request Lifecycle

### REST request
1. Nginx receives request → proxies to `backend:3000`
2. Axum router dispatches to handler
3. Handler reads/writes PostgreSQL via SQLx
4. Handler may spawn a Tokio task (workflow step)
5. Handler returns JSON response

### Workflow step
1. Workflow engine task wakes up
2. Loads project + team + agent from DB
3. Loads message history for agent role (last N messages)
4. Calls `ProviderManager::chat()` (streaming)
5. Each token chunk → Redis publish → WebSocket fan-out → browser
6. Final response saved to `agent_messages`
7. Project status updated
8. If approval checkpoint → emit `approval.required` event, task ends
9. If next step → repeat

### Tool execution within a step
1. LLM response contains a `tool_calls` array
2. Workflow engine intercepts tool calls
3. Dispatches to appropriate handler:
   - `file_*` → direct filesystem (scoped to project dir)
   - `code_execute` → HTTP POST to Executor service
   - `web_*` → reqwest HTTP client
   - `mcp_call` → MCP SSE client
4. Tool result returned as `tool` role message
5. LLM continues until final (non-tool) response

## WebSocket Architecture

```
Browser ─── WS /ws/projects/:id ───► WS Hub (Axum)
                                          │
                                    tokio::sync::broadcast
                                          │
                           Redis SUBSCRIBE project:{id}:events
```

The WebSocket hub subscribes to a Redis channel per project.
Any component (workflow engine, handlers) publishes to Redis.
The hub fans out to all connected browsers for that project.

## Approval Flow

```
Workflow engine reaches checkpoint
         │
         ▼
Set project.status = awaiting_*
Persist Approval record (status=pending, payload=content_to_review)
Publish { type: "approval.required" } to Redis
Tokio task ends (does NOT block a thread)
         │
         ▼ (User clicks Approve in UI)
POST /api/v1/projects/:id/approve
Handler updates Approval record (status=approved)
Handler advances project.status to next state
Handler spawns new Tokio workflow task
```

## Security Notes

- No authentication (single-user local tool)
- CORS allows `localhost` origins only (configured in Nginx and Axum)
- File tool operations validate paths with `canonicalize()` + prefix check
- Code execution is in ephemeral Docker containers with no network by default
- AWS credentials mounted read-only; never stored in DB
- API keys in DB are AES-256 encrypted at rest (key from env `SECRET_KEY`)
