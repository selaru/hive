# Hive — Technical Plan

## Overview

Hive is a self-hosted, single-user, Docker-composed AI multi-agent orchestration platform.
It allows you to:

- Configure multiple LLM providers (local MLX LM, Google Gemini, AWS Bedrock via SSO)
- Import agent skills from GitHub or skills.sh (SKILL.md format)
- Build agents by combining provider + skill + tool permissions
- Build teams of agents with a visual hierarchy editor
- Submit projects via a board/form and step through an approval-gated pipeline
- Execute sandboxed code via ephemeral Docker containers
- Connect agents to MCP servers running in local Docker

---

## Design Constraints

| Constraint | Decision |
|---|---|
| Single user | No authentication, no login — API and UI are open on localhost |
| Notifications | UI state only — approval checkpoints surface as board highlights |
| Git integration | None — agent file output goes directly to project folder |
| MCP | Connect to Docker-hosted MCP servers via SSE |
| Deployment | Docker Compose, everything bundled |
| Performance | Rust backend for low-latency orchestration |

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Browser (React SPA)                 │
│  Board | Agent Builder | Team Builder | Settings     │
└──────────────────────┬──────────────────────────────┘
                       │ HTTP REST + WebSocket
┌──────────────────────▼──────────────────────────────┐
│              API Server  (Rust / Axum)              │
│         REST API  |  WebSocket Hub  |  No auth      │
└──────┬───────────┬────────────┬────────┬────────────┘
       │           │            │        │
  Provider     Skill         Agent    Workflow
  Manager      Registry      Factory  Engine
       └───────────┴────────────┴────────┘
                            │
                     PostgreSQL 16
                            │
              ┌─────────────┴──────────────┐
           Redis 7                  Executor Service
        (pub-sub, queue)        (sandboxed Docker exec)
```

### Component Responsibilities

**API Server (crate: `api`)**
- Axum HTTP server on `:3000`
- REST handlers for all CRUD operations
- WebSocket hub — fans out project events to connected browsers
- Spawns workflow Tokio tasks
- No auth middleware (single-user local)

**Workflow Engine (crate: `workflow`)**
- State machine per project (see workflow.md)
- Runs agent steps as async Tokio tasks
- Pauses at human approval checkpoints
- Persists all state to PostgreSQL
- Publishes events to Redis → WebSocket

**Provider Manager (crate: `providers`)**
- Unified `LlmProvider` trait
- `OpenAICompatProvider` — handles MLX LM and Gemini (both OpenAI-compat)
- `AwsBedrockProvider` — uses official AWS SDK with SSO credential chain
- Health check endpoint

**Skill Registry (crate: `skills`)**
- Parses SKILL.md (YAML frontmatter + markdown body)
- GitHub importer: fetches raw SKILL.md from any repo
- skills.sh importer: wraps GitHub fetch (same format)

**Executor Service (crate: `executor`)**
- Separate Axum binary on `:3001` (internal only)
- Receives `POST /execute { language, code, working_dir, timeout_ms }`
- Runs `docker run --rm` with scoped volume mount
- Returns `{ stdout, stderr, exit_code, duration_ms }`

---

## Tech Stack

### Backend

| Crate / Lib | Version | Role |
|---|---|---|
| axum | 0.8 | Web framework |
| tokio | 1 | Async runtime |
| sqlx | 0.8 | PostgreSQL driver (compile-time checked) |
| async-openai | 0.27 | OpenAI-compat LLM client (MLX LM + Gemini) |
| aws-config + aws-sdk-bedrockruntime | 1.x | AWS Bedrock + SSO credentials |
| deadpool-redis | 0.18 | Redis connection pool + pub/sub |
| serde + serde_json | 1 | Serialisation |
| tower-http | 0.6 | CORS, tracing middleware |
| anyhow + thiserror | 1/2 | Error handling |
| dotenvy | 0.15 | .env loading |
| tracing + tracing-subscriber | 0.1/0.3 | Structured logging |

### Frontend

| Lib | Version | Role |
|---|---|---|
| react + react-dom | 18 | UI framework |
| typescript | 5 | Type safety |
| vite | 5 | Build + dev server |
| @xyflow/react | 12 | Visual team hierarchy graph editor |
| @tanstack/react-query | 5 | Server state, caching, mutations |
| @tanstack/react-router | 1 | Type-safe file-based routing |
| zustand | 4 | Client state |
| react-hook-form + zod | 7/3 | Forms + validation |
| @dnd-kit/core + sortable | 6/7 | Drag-and-drop Kanban board |
| tailwindcss | 3 | Styling |
| shadcn/ui | latest | Component library (run `npx shadcn@latest init`) |
| lucide-react | latest | Icons |

### Infrastructure

| Service | Image | Role |
|---|---|---|
| nginx | nginx:alpine | Reverse proxy, serves frontend |
| backend | custom Rust build | API + orchestration |
| executor | custom Rust build | Sandboxed code execution |
| postgres | postgres:16-alpine | Primary database |
| redis | redis:7-alpine | Pub-sub + job queue |

---

## Data Models

See [docs/data-models.md](docs/data-models.md) for full SQL schemas.

Summary:

| Table | Description |
|---|---|
| `providers` | LLM provider configs (URL, key, AWS profile) |
| `skills` | Imported SKILL.md content |
| `tools` | Available tools agents can use |
| `agents` | provider + skill + tool permissions |
| `teams` | Named agent teams |
| `team_members` | Agent ↔ team with role and canvas position |
| `team_hierarchy` | Parent/child edges between team members |
| `projects` | Work units with status machine |
| `stories` | User stories within a project |
| `approvals` | Human checkpoint records |
| `agent_messages` | Full conversation log per project |

---

## Workflow

See [docs/workflow.md](docs/workflow.md) for full state machine.

States: `submitted → lead_review → awaiting_approval_lead → architect_breakdown →
awaiting_approval_stories → in_development → in_review → awaiting_final_approval → done`

Human checkpoints (🧑):
1. After lead rewrites the spec
2. After architect breaks down into stories
3. After code review passes

---

## API

See [docs/api-design.md](docs/api-design.md) for full REST + WebSocket reference.

Base URL: `http://localhost/api/v1`
WebSocket: `ws://localhost/ws/projects/:id`

---

## Providers

See [docs/providers.md](docs/providers.md).

All three providers use the same `LlmProvider` trait.
MLX LM and Gemini both go through `OpenAICompatProvider` (different `base_url`).
AWS mounts `~/.aws` read-only; the SDK handles SSO token refresh automatically.

---

## Skills

See [docs/skills.md](docs/skills.md).

Skills are SKILL.md files (Anthropic/Vercel Labs Agent Skills open standard).
Frontmatter: `name`, `description`. Body = system prompt.
Import from any GitHub repo or skills.sh slug.

---

## Tools

See [docs/tools.md](docs/tools.md).

| Tool | Type |
|---|---|
| file_read, file_write, file_list | `file` |
| code_execute | `code_exec` (via Executor service) |
| web_search, web_fetch | `web` |
| shell_command | `shell` (restricted) |
| mcp_call | `mcp` (connects to Docker MCP servers) |

---

## MCP Integration

See [docs/mcp.md](docs/mcp.md).

Agents can call tools exposed by any SSE-based MCP server running in Docker.
Configure MCP server URLs in Settings → MCP Servers.
The backend calls `tools/list` on startup to discover available tools, which then
appear in the tool registry and can be granted to agents.

---

## Project Folder Structure (Runtime)

Each project gets its own folder:

```
projects/
└── {uuid}-{slug}/
    ├── .hive/
    │   ├── project.json        metadata + current status
    │   ├── stories/
    │   │   └── {id}.json
    │   └── messages.jsonl      agent conversation log
    └── src/                    agent-generated code
```

All agent file operations are path-validated against the project root.
Directory traversal (`../`) is rejected at the handler level.

---

## Docker Compose

```yaml
services:
  nginx      — reverse proxy + frontend static files
  backend    — Rust API + orchestration engine
  executor   — sandboxed code execution (Docker-in-Docker)
  postgres   — primary database
  redis      — pub-sub + queue
```

`~/.aws` is mounted read-only into `backend` for SSO credentials.
`projects/` is mounted into both `backend` and `executor`.
`/var/run/docker.sock` is mounted into `executor` for Docker-in-Docker execution.

---

## Development Phases

See [docs/development-phases.md](docs/development-phases.md) for the detailed phased roadmap.

| Phase | Scope | Effort |
|---|---|---|
| 1 | Foundation: backend skeleton, DB, provider CRUD, MLX LM, basic UI | 2–3 weeks |
| 2 | Skill system: parser, GitHub importer, skill UI | 1–2 weeks |
| 3 | Team builder: CRUD, React Flow canvas, hierarchy save | 1–2 weeks |
| 4 | Workflow engine: state machine, agent steps, WebSocket, board UI | 2–3 weeks |
| 5 | Tools: registry, executor service, file/web/code tools | 1–2 weeks |
| 6 | Cloud providers: Gemini + AWS Bedrock adapters | 1 week |
| 7 | MCP integration: client, Docker server discovery | 1 week |
| 8 | Polish: error handling, retries, production Docker Compose | 1–2 weeks |

**Total: ~10–16 weeks** for one developer.

---

## Key Implementation Notes

1. **Single provider client for MLX LM + Gemini**: `async-openai` with `OpenAIConfig::with_api_base()`.
   MLX LM = `http://mlx-lm:8080`, Gemini = `https://generativelanguage.googleapis.com/v1beta/openai/`.

2. **AWS SSO — zero extra code**: Mount `~/.aws` read-only. Call
   `aws_config::from_env().profile_name(profile).load().await`. SDK handles token refresh.

3. **Approval blocking**: On `awaiting_*` status, emit WebSocket event and end the Tokio task.
   Resume with a fresh task triggered by `POST /api/v1/projects/:id/approve`.

4. **Streaming tokens to UI**: Use `async-openai`'s `create_stream()`. Each chunk is published to
   Redis `project:{id}:tokens`. WebSocket hub subscribes and fans out to connected clients.

5. **Parallel dev agents**: On story approval, spawn one Tokio task per story. Redis counter tracks
   completion. When all done, transition project to `in_review`.

6. **React Flow save/restore**: Store hierarchy as `{ nodes, edges }` in PostgreSQL JSONB on the
   `teams` table. Restore on team page load.

7. **SKILL.md parsing**: Split on first `---` delimiters to extract YAML frontmatter.
   Everything after closing `---` is the system prompt.

8. **Path safety**: All file tool calls call `canonicalize()` then assert the result starts with
   `project.folder_path`. Reject anything that doesn't match.

9. **MCP tool discovery**: On `POST /api/v1/mcp-servers` (register server), immediately call
   `tools/list` on the MCP server and upsert discovered tools into the `tools` table.

10. **Memory management**: Sliding window — load last `memory_window` messages per
    `(project_id, agent_role)` from `agent_messages` table before each LLM call.

---

## Build Status

### Phase 1 — Complete (2026-07-20)

`docker compose up -d` builds and runs all services end-to-end:

| Service | Status | Notes |
|---|---|---|
| postgres | ✅ Healthy | Migrations applied on startup |
| redis | ✅ Running | Pub-sub listener active in `WsHub` |
| backend | ✅ Running | `hive-api` on `:3000` |
| executor | ✅ Running | `hive-executor` on `:3001` |
| frontend | ✅ Built | React SPA served as static files via volume |
| nginx | ✅ Running | Proxies `/api/` and `/ws/` to backend; serves SPA on `/` |

Verified:
- `GET /api/v1/providers` → `[]` (empty list, DB connected)
- `GET /api/v1/tools` → 7 seeded built-in tools
- `GET /` → React SPA index.html

### Implementation Decisions Made During Phase 1

| Decision | Rationale |
|---|---|
| **Axum 0.8 path syntax** — `{id}` not `:id` | Axum 0.8 changed capture group syntax. Updated all routes. |
| **Redis pub-sub via dedicated `Client`** | `deadpool_redis::Pool` connections are multiplexed and cannot be converted to pub-sub connections. `WsHub` now holds the Redis URL and opens a dedicated `redis::Client` pubsub connection per reconnect cycle. |
| **`Arc<RwLock<...>>` for `WsHub` channels** | `tokio::sync::RwLock` is not `Clone`. Wrapping in `Arc` allows the map to be shared with the background reconnect task. |
| **AWS Bedrock stubbed for Phase 1** | `aws-config` / `aws-sdk-bedrockruntime` 1.x now require Rust ≥ 1.94.1. The workspace pins `rust:1.88-slim` (minimum to satisfy all other locked dep MSRVs). AWS deps are commented out and `AwsBedrockProvider` is a stub. Restore for Phase 7. |
| **Dockerfile `rust:1.88-slim`** | `home 0.5.12` (transitive via sqlx) uses edition 2024 → needs Rust ≥ 1.85. `hdrhistogram` and ICU crates need ≥ 1.88. Updated from original `rust:1.80`. |
| **Nginx `resolver 127.0.0.11`** | Using `upstream {}` blocks caused nginx to fail at startup if the backend hostname was not yet resolvable. Switching to `set $var` + Docker's internal DNS resolver defers resolution to request time. |
| **`broadcast::Receiver::recv()` returns `Result`** | The original scaffolding used `Some(event) = rx.recv()` which doesn't type-check (it returns `Result<T, RecvError>`). Fixed to `Ok(event) = rx.recv()`. |
| **`query_async::<()>` not `<_, ()>`** | redis 0.27 changed `Cmd::query_async` to a single type parameter. Fixed publish call in `WorkflowEngine::publish_event`. |
| **Frontend volume at `/dist`** | The alpine second-stage CMD does `cp -r . /dist` (copies from image's `/app/dist` to the volume). The compose file was mounting the volume at `/app/dist` (conflicting with the image's files). Changed to `/dist` so the CMD writes into the volume correctly. |
| **`vite/client` types** | `import.meta.env` was unknown to TypeScript because `tsconfig.json` didn't include `"types": ["vite/client"]`. Added. |
| **Vite `resolve.alias`** | Without this, `@/lib/api` etc. are valid TypeScript paths (via `tsconfig.paths`) but Vite's bundler can't resolve them. Added `resolve.alias: { '@': './src' }` to `vite.config.ts`. |
