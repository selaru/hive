# Hive

A self-hosted, Docker-composed AI multi-agent orchestration platform. Build teams of AI agents
with defined roles and communication hierarchies, then submit projects through an approval-gated
workflow from idea to working code — all running locally.

## Status

| Phase | Status |
|---|---|
| **1 — Foundation** | **✅ Complete** — `docker compose up -d` works end-to-end |
| 2 — Skill System | 🔧 Scaffolded (code exists, not tested) |
| 3 — Agent Builder | 🔧 Scaffolded |
| 4 — Team Builder | 🔧 Scaffolded |
| 5 — Workflow Engine | 🔧 Scaffolded |
| 6 — Tool Access | 🔧 Scaffolded |
| 7 — Cloud Providers | ⏸ AWS Bedrock stub only (see below) |
| 8 — MCP Integration | 🔧 Scaffolded |
| 9 — Polish | ⏳ Not started |

## What It Does

1. **Configure providers** — MLX LM (local Apple Silicon), Google Gemini, or AWS Bedrock via SSO
2. **Import skills** — pull `SKILL.md` files from GitHub or skills.sh to define agent behaviour
3. **Build agents** — combine a provider + skill + tool permissions into a named agent
4. **Build teams** — drag agents onto a canvas, draw edges to define who reports to whom
5. **Submit work** — fill out a project form on the board; the team picks it up automatically
6. **Approve checkpoints** — review and approve (or reject) the lead's rewrite, the architect's
   stories, and the final delivered code before the project closes

## Workflow

```
User submits → Lead rewrites → [You approve] → Architect breaks down →
[You approve stories] → Dev agents build → Review agent checks →
[You approve] → Done
```

## Tech Stack

| Layer | Technology |
|---|---|
| Backend | Rust + Axum 0.8 + Tokio |
| Database | PostgreSQL 16 |
| Pub-Sub / Queue | Redis 7 |
| Frontend | React 18 + TypeScript + Vite 5 |
| Graph editor | React Flow (@xyflow/react 12) |
| UI | Tailwind CSS |
| Proxy | Nginx |
| Code execution | Sandboxed Docker containers (via Executor service) |
| MCP | SSE-based MCP client (connects to Docker-hosted MCP servers) |

## Prerequisites

- Docker + Docker Compose v2
- macOS with Apple Silicon (for MLX LM local inference) — or just use Gemini
- AWS SSO configured in `~/.aws/config` only if using Bedrock (Phase 7, currently stubbed)

## Quick Start

```bash
# 1. Clone and configure
cp .env.example .env
# Edit .env if needed — defaults work out of the box for local dev

# 2. Start everything
docker compose up -d

# 3. Open the UI
open http://localhost
```

On first run Docker builds both Rust binaries (~2–3 min). Subsequent starts use cached images.

## Known Limitations (Phase 1)

- **AWS Bedrock** — `AwsBedrockProvider` is a compile-time stub that returns an error at call
  time. The AWS SDK (`aws-config`, `aws-sdk-bedrockruntime`) requires Rust ≥ 1.94.1 and is
  excluded from Phase 1. Uncomment the workspace deps in `backend/Cargo.toml` and replace
  `crates/providers/src/aws_bedrock.rs` when working on Phase 7.

- **shadcn/ui** — Not yet initialised. The UI uses raw Tailwind CSS classes. Functional for
  Phase 1; add `npx shadcn@latest init` when polishing the UI.

- **TanStack Router** — Installed but routing is implemented with `useState` in `App.tsx`.
  Replace with `RouterProvider` for full type-safe file-based routing in a later pass.

- **Docker build — Rust version** — The `Dockerfile` pins `rust:1.88-slim`. This satisfies
  the MSRV of all locked dependencies. Do not downgrade below 1.85 (edition 2024 crates).

## Development

```bash
# Hot-reload backend (requires cargo-watch in image)
docker compose -f docker-compose.dev.yml up
```

Backend runs on `:3000`, frontend dev server on `:5173`, both proxied by nginx on `:80`.

## Project Structure

```
hive/
├── backend/          Rust workspace (api, core, workflow, providers, skills, executor)
├── frontend/         React + TypeScript SPA
├── nginx/            Reverse proxy config
├── projects/         Runtime: generated project folders (gitignored)
├── docs/             Architecture and design documentation
├── docker-compose.yml
└── .env.example
```

## Documentation

| Document | Description |
|---|---|
| [docs/architecture.md](docs/architecture.md) | System architecture and component overview |
| [docs/data-models.md](docs/data-models.md) | Database schema |
| [docs/api-design.md](docs/api-design.md) | REST API + WebSocket event reference |
| [docs/workflow.md](docs/workflow.md) | Project state machine and agent steps |
| [docs/providers.md](docs/providers.md) | LLM provider adapters |
| [docs/skills.md](docs/skills.md) | Skill system and SKILL.md format |
| [docs/tools.md](docs/tools.md) | Agent tool registry |
| [docs/mcp.md](docs/mcp.md) | MCP integration |
| [docs/development-phases.md](docs/development-phases.md) | Build roadmap and phases |
