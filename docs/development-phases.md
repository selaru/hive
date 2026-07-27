# Development Phases

Phased roadmap for building Hive. Each phase is independently testable.

---

## Phase 1 — Foundation ✅

**Goal**: Backend skeleton, database, provider management, basic UI shell.

**Status**: Complete — `docker compose up -d` runs end-to-end. All services healthy.

### Backend
- [x] Rust workspace with 6 crates (`api`, `core`, `workflow`, `providers`, `skills`, `executor`)
- [x] PostgreSQL schema + SQLx migrations (run automatically on startup via `sqlx::migrate!`)
- [x] `AppState` with DB pool + Redis pool
- [x] Axum server with CORS + tracing
- [x] Provider CRUD handlers (`GET`, `POST`, `PUT`, `DELETE`)
- [x] `OpenAICompatProvider` adapter (`async-openai`, pointing at MLX LM)
- [x] `POST /providers/{id}/test` — calls `/v1/models` and returns model list
- [x] Docker Compose working locally

> **Note**: `AwsBedrockProvider` is a Phase-7 stub (returns an error at call time).
> The AWS SDK crates (`aws-config`, `aws-sdk-bedrockruntime`) require Rust ≥ 1.94.1
> and are commented out in `Cargo.toml` until Phase 7.

### Frontend
- [x] Vite + React + TypeScript scaffold
- [x] Page routing with routes: `/board`, `/agents`, `/teams`, `/skills`, `/settings/providers`
  (implemented as `useState`-based navigation in `App.tsx`; `@tanstack/react-router` installed
  but full file-based routing can replace this in a later pass)
- [x] TanStack Query setup
- [x] API client (`src/lib/api.ts`)
- [x] Settings → Providers page: list, add, edit, delete, test providers
- [x] Tailwind CSS configured and working
- [ ] shadcn/ui components not yet initialised — UI uses raw Tailwind classes (functional but
  unstyled relative to shadcn design system; replace in a later pass)

### Milestone: Add an MLX LM provider, call `/test`, see model list in UI.

---

## Phase 2 — Skill System

**Goal**: Import and manage SKILL.md skills.

### Backend
- [ ] `crates/skills`: SKILL.md parser (YAML frontmatter + markdown body)
- [ ] GitHub importer: `GET raw.githubusercontent.com/...`
- [ ] `skills.sh` importer (same as GitHub)
- [ ] Skill CRUD handlers
- [ ] `tool_hints` extraction from skill body

### Frontend
- [ ] Settings → Skills page: list skills, import from GitHub / paste
- [ ] Skill preview panel (shows description + system prompt)

### Milestone: Import `vercel-labs/agent-skills`, see skill in list, preview prompt.

---

## Phase 3 — Agent Builder

**Goal**: Create agents from provider + skill + tools.

### Backend
- [ ] Tool seeding (built-in tools inserted on startup)
- [ ] Agent CRUD handlers
- [ ] `POST /agents/:id/test` — sends a simple test message

### Frontend
- [ ] Agents page: list agents
- [ ] Agent builder form: name, provider, skill, temperature, max_tokens, tool permissions (checkboxes), memory settings
- [ ] Agent test panel: send message, stream response

### Milestone: Create a "Lead" agent using MLX LM + a lead skill, send test message, see response.

---

## Phase 4 — Team Builder

**Goal**: Visual team hierarchy editor.

### Backend
- [ ] Team CRUD handlers
- [ ] `GET /teams/:id/hierarchy` — returns `{ nodes, edges }` for React Flow
- [ ] `PUT /teams/:id/hierarchy` — saves React Flow graph (updates `team_members` + `team_hierarchy`)

### Frontend
- [ ] Teams page: list teams
- [ ] Team builder canvas (React Flow):
  - [ ] `AgentNode` custom node (shows agent name, role, skill)
  - [ ] `HierarchyEdge` custom edge (shows communication mode)
  - [ ] Agent sidebar: drag agents onto canvas
  - [ ] Connect nodes to create hierarchy edges
  - [ ] Role selector per node
  - [ ] Communication mode selector per edge
  - [ ] Save / auto-layout buttons

### Milestone: Build a team with lead → architect → 2 developers → reviewer.

---

## Phase 5 — Workflow Engine

**Goal**: Full project lifecycle from submission to done.

### Backend
- [ ] `WorkflowEngine` state machine
- [ ] Lead review step (LLM call, save rewrite to approval)
- [ ] Architect breakdown step (LLM call, parse story JSON, save stories)
- [ ] Development step (parallel Tokio tasks, one per story)
- [ ] Review step (LLM reads project files, produces review)
- [ ] Approval CRUD handlers (`POST /projects/:id/approve`, `/reject`)
- [ ] WebSocket hub (Redis pub-sub → broadcast to browsers)
- [ ] Token streaming (LLM chunks → Redis → WebSocket)

### Frontend
- [ ] Board page (Kanban):
  - [ ] Columns matching workflow states
  - [ ] Project cards with status badges
  - [ ] Awaiting-approval columns highlighted
  - [ ] "Submit Project" button → form modal
- [ ] Project detail page:
  - [ ] Message stream (real-time agent tokens)
  - [ ] Approval panel (shows payload + Approve/Reject with comment)
  - [ ] Stories panel (list with status)
- [ ] WebSocket hook (`src/lib/ws.ts`)
- [ ] Real-time board updates

### Milestone: Submit a project, watch lead rewrite it, approve, watch architect create stories, approve, watch developers run, approve final.

---

## Phase 6 — Tool Access

**Goal**: Agents can read/write files and execute code.

### Backend
- [ ] `ToolRegistry` struct with tool dispatch
- [ ] File tools (`file_read`, `file_write`, `file_list`) with path validation
- [ ] `Executor` service binary:
  - [ ] `POST /execute` handler
  - [ ] Docker-in-Docker execution
  - [ ] Language image selection
  - [ ] Stdout/stderr capture + timeout
- [ ] `code_execute` tool (calls Executor)
- [ ] `web_search` tool (Brave API)
- [ ] `web_fetch` tool (reqwest + HTML→markdown)
- [ ] `shell_command` tool (Executor, allowlisted commands)
- [ ] Tool call interception in workflow steps

### Frontend
- [ ] Project detail: show tool calls inline in message stream (expandable)

### Milestone: Developer agent creates a Python file, executes it, and the test passes.

---

## Phase 7 — Cloud Providers

**Goal**: Gemini and AWS Bedrock work.

### Backend
- [ ] Gemini via `OpenAICompatProvider` (same adapter, different URL + key) — auto-works
- [ ] `AwsBedrockProvider` using Bedrock Converse API
- [ ] AWS SSO credential loading via `aws-config`
- [ ] Test endpoint works for Bedrock

### Frontend
- [ ] Provider form: show AWS-specific fields when type = `aws_bedrock`
- [ ] AWS profile selector (reads available profiles from backend)

### Milestone: Create a Bedrock provider, test it, use it in an agent.

---

## Phase 8 — MCP Integration

**Goal**: Agents can use tools from Docker-hosted MCP servers.

### Backend
- [ ] `McpClient` (SSE-based)
- [ ] `POST /mcp-servers` — register + discover tools
- [ ] `POST /mcp-servers/:id/sync` — re-sync tools
- [ ] `mcp_call` tool dispatch in workflow

### Frontend
- [ ] Settings → MCP Servers page: list, add, remove, sync
- [ ] MCP tools appear in agent tool permissions list

### Milestone: Register `mcp-filesystem` Docker container, agent reads a file via MCP.

---

## Phase 9 — Polish

**Goal**: Production-ready Docker Compose, solid error handling, good UX.

- [ ] Error boundaries in React
- [ ] Backend: LLM call retries (3x, exponential backoff)
- [ ] Backend: project `error` state + recovery
- [ ] Backend: graceful shutdown
- [ ] Frontend: loading states everywhere
- [ ] Frontend: toast notifications for errors
- [ ] Frontend: project folder path visible in UI
- [ ] README: complete setup guide
- [ ] Docker Compose: production build (minified frontend, optimised Rust binary)
- [ ] Health check endpoint `GET /health`

### Milestone: `docker compose up`, everything works first time.

---

## Total Estimate

| Phase | Effort |
|---|---|
| 1 Foundation | 2–3 weeks |
| 2 Skills | 1 week |
| 3 Agents | 1 week |
| 4 Teams | 1–2 weeks |
| 5 Workflow | 2–3 weeks |
| 6 Tools | 1–2 weeks |
| 7 Cloud providers | 1 week |
| 8 MCP | 1 week |
| 9 Polish | 1–2 weeks |
| **Total** | **~11–17 weeks** |

Phases 1–5 deliver the core value (end-to-end workflow).
Phases 6–8 extend capability. Phase 9 makes it production-ready.
