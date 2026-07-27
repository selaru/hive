# Hive Platform Improvement & Architecture Roadmap

This file tracks recommended architectural, security, and quality-of-life enhancements to transition Hive from a local development prototype to a secure, enterprise-grade self-hosted multi-agent platform.

---

## 🚀 1. Core Workflow & Reliability Enhancements

- [ ] **Resilient State Reconstruction (Startup Task Resumption)**
  - *Problem:* Currently, if the backend server restarts, active background loops for projects in progress (`LeadReview`, `ArchitectBreakdown`, `InDevelopment`) are orphaned, leaving projects permanently stuck in their state.
  - *Fix:* Create a startup hook in `main.rs` to query non-finalized projects and automatically queue them back into the `WorkflowEngine` using `s.workflow.advance()`.

- [ ] **LLM Call Timeout & Hanging Guardrails**
  - *Problem:* LLM providers experiencing severe latency spikes or hanging connections can block Tokio tasks indefinitely, leading to memory leaks and resource exhaustion.
  - *Fix:* Wrap LLM provider requests inside the story/agentic loops with `tokio::time::timeout(...)` with a graceful maximum duration (e.g., 120 seconds).

- [ ] **Smart Dependency Adjustments on Story Deletion**
  - *Problem:* Deleting a story currently deletes it and transitions status. However, if the deleted story was a dependency for other stories, the resulting codebase might compile but remain functionally broken.
  - *Fix:* Upon story deletion, trigger a quick re-evaluation pass with the Lead/Architect agents to verify and adjust acceptance criteria of remaining stories to accommodate the deletion.

---

## 🔒 2. Security & Sandboxing

- [ ] **Secure MicroVM Isolation for Executor**
  - *Problem:* The `executor` container currently mounts the host `/projects` volume directly and exposes the host Docker socket, opening up host breakout vulnerabilities if an agent generates malicious code.
  - *Fix:* Transition the code runner sandbox to run within secure microVMs (e.g., **Firecracker**) or sandboxed runtimes (e.g., **gVisor**) rather than native host-sharing containers.

- [ ] **WebSocket Handshake Authentication & Authorization**
  - *Problem:* WebSocket endpoints (`/ws/board` and `/ws/projects/{id}`) are publicly accessible, exposing streaming agent tokens and payloads.
  - *Fix:* Restrict connection upgrades in Axum by validating JWT/session tokens and authorize subscription topics dynamically on a per-project basis.

---

## 🧹 3. Codebase Cleanup & Warning Resolution

Based on backend compilations, the following dead-code, unused-variables, and compiler warnings should be cleaned up:

- [x] **Skills Parser Warnings**
  - [x] Remove unused `version` field in `crates/skills/src/parser.rs` (under `Frontmatter` struct).
- [x] **Provider Adapter Warnings**
  - [x] Remove unused `bail` import in `crates/providers/src/aws_bedrock.rs`.
- [x] **Workflow Engine Warnings**
  - [x] Clean up unused imports `Approval` and `Team` at the top of `crates/workflow/src/engine.rs`.
  - [x] Clean up unused import `std::path::Path` in `crates/workflow/src/steps/tool_runner.rs`.
  - [x] Clean up variable mutability warning on `messages` in `crates/workflow/src/engine.rs`.
  - [x] Resolve unused `count` argument warning in `crates/workflow/src/steps/tool_runner.rs`.
- [x] **API Server Warnings**
  - [x] Remove unused import `serde_json::Value` in `crates/api/src/bin_test_req.rs`.
  - [x] Remove unused import `axum::Router` in `crates/api/src/main.rs`.
  - [x] Remove unused import `Tool` in `crates/api/src/handlers/mcp.rs`.
  - [x] Remove unused import `path::PathBuf` in `crates/api/src/handlers/projects.rs`.
  - [x] Remove unused import `put` routing in `crates/api/src/routes/mod.rs`.
  - [x] Remove or utilize dead field `redis` in `crates/api/src/state.rs`.
  - [x] Address unused method `publish` warning in `crates/api/src/ws/hub.rs`.
