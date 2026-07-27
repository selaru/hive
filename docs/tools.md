# Tool Registry

## Overview

Tools are capabilities that agents can invoke during their work.
The workflow engine intercepts `tool_calls` in LLM responses and executes
the appropriate handler, returning the result as a `tool` role message.

Agents are granted access to specific tools in their configuration.
The LLM only sees tools the agent is allowed to use.

---

## Built-in Tools

### file_read

Read the contents of a file within the project folder.

```json
{
  "name": "file_read",
  "description": "Read the contents of a file in the project working directory.",
  "parameters": {
    "path": { "type": "string", "description": "Relative path from the project root" }
  }
}
```

**Implementation**: Direct filesystem read. Path is resolved relative to
`projects/{id}/` and validated with `canonicalize()` to prevent traversal.

---

### file_write

Write or overwrite a file within the project folder.

```json
{
  "name": "file_write",
  "description": "Write content to a file in the project working directory. Creates parent directories as needed.",
  "parameters": {
    "path":    { "type": "string", "description": "Relative path from the project root" },
    "content": { "type": "string", "description": "File content to write" }
  }
}
```

---

### file_list

List files and directories.

```json
{
  "name": "file_list",
  "description": "List files and directories at a path in the project working directory.",
  "parameters": {
    "path": { "type": "string", "description": "Relative path (use '.' for root)" }
  }
}
```

---

### code_execute

Run code in a sandboxed Docker container.

```json
{
  "name": "code_execute",
  "description": "Execute code in a sandboxed container. The working directory is the project src/ folder.",
  "parameters": {
    "language": { "type": "string", "enum": ["python", "javascript", "bash"] },
    "code":     { "type": "string", "description": "Code to execute" },
    "timeout":  { "type": "integer", "description": "Timeout in seconds (max 60)", "default": 30 }
  }
}
```

**Implementation**: HTTP POST to Executor service (`http://executor:3001/execute`).
Executor runs `docker run --rm --network none -v {project_src}:/workspace -w /workspace {image}`.

Docker images used:
- Python: `python:3.12-slim`
- JavaScript: `node:22-alpine`
- Bash: `debian:bookworm-slim`

Output: `{ stdout, stderr, exit_code, duration_ms }`

---

### web_search

Search the web.

```json
{
  "name": "web_search",
  "description": "Search the web and return a list of relevant results.",
  "parameters": {
    "query": { "type": "string" },
    "count": { "type": "integer", "default": 5 }
  }
}
```

**Implementation**: Brave Search API (configure `BRAVE_API_KEY` in `.env`).
Falls back to SearXNG if configured (`SEARXNG_URL`).

---

### web_fetch

Fetch a URL and return its content as text.

```json
{
  "name": "web_fetch",
  "description": "Fetch the content of a URL and return it as plain text.",
  "parameters": {
    "url": { "type": "string" }
  }
}
```

**Implementation**: `reqwest` HTTP GET. HTML is converted to markdown via `htmd`.

---

### shell_command

Run a whitelisted shell command.

```json
{
  "name": "shell_command",
  "description": "Run a shell command in the project directory.",
  "parameters": {
    "command": { "type": "string", "description": "Command to run" }
  }
}
```

**Implementation**: Executor service. Command is validated against an allowlist
(`cargo`, `npm`, `python`, `pip`, `node`, `ls`, `cat`, `grep`, `find`, `mkdir`, `cp`, `mv`).
Blocked: `rm -rf`, `curl`, `wget`, `chmod +x`, `sudo`.

---

## MCP Tools

Tools discovered from registered MCP servers are inserted into the `tools` table
with `type = 'mcp'` and `config.mcp_server_url` pointing to the server's SSE endpoint.

MCP tools appear alongside built-in tools in the agent tool permissions list.

See [mcp.md](mcp.md) for details on MCP server registration and tool discovery.

---

## Tool Execution Flow (Rust)

```rust
// crates/workflow/src/steps/tool_runner.rs

pub async fn run_tool_call(
    tool_name:  &str,
    arguments:  &serde_json::Value,
    project:    &Project,
    tools:      &ToolRegistry,
) -> Result<String> {
    match tool_name {
        "file_read"     => tools.file_read(arguments, &project.folder_path).await,
        "file_write"    => tools.file_write(arguments, &project.folder_path).await,
        "file_list"     => tools.file_list(arguments, &project.folder_path).await,
        "code_execute"  => tools.code_execute(arguments, &project.folder_path).await,
        "web_search"    => tools.web_search(arguments).await,
        "web_fetch"     => tools.web_fetch(arguments).await,
        "shell_command" => tools.shell_command(arguments, &project.folder_path).await,
        name if tools.is_mcp_tool(name) => tools.mcp_call(name, arguments).await,
        _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name))
    }
}
```

---

## Adding a New Built-in Tool

1. Add a row to the `tools` table in `backend/migrations/001_initial.sql` (seed data)
2. Implement the handler in `crates/workflow/src/steps/tool_runner.rs`
3. Add the match arm in `run_tool_call()`
4. Add the tool to the agent builder tool permissions list in the frontend
