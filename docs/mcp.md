# MCP Integration

## Overview

Hive supports the **Model Context Protocol (MCP)** — an open standard for connecting
AI agents to external tools and data sources.

Any MCP server running in your local Docker environment can be registered in Hive.
Its tools are automatically discovered and made available to agents.

---

## Architecture

```
Agent (Hive workflow) ──── mcp_call tool ────► MCP Client (Rust)
                                                     │ SSE
                                                     ▼
                                            MCP Server (Docker)
                                         e.g. mcp-filesystem
                                              mcp-github
                                              mcp-postgres
                                              mcp-puppeteer
```

The MCP client in Hive connects over SSE (Server-Sent Events) transport,
which is the standard for remotely-hosted MCP servers.

---

## Registering an MCP Server

### Via UI

Settings → MCP Servers → Add Server

Fields:
- **Name**: display name (e.g. "Filesystem MCP")
- **URL**: SSE endpoint (e.g. `http://host.docker.internal:3100/sse`)

On save, Hive immediately calls `tools/list` on the server and imports all
discovered tools into the tool registry.

### Via API

```http
POST /api/v1/mcp-servers
{
  "name": "Filesystem MCP",
  "url": "http://host.docker.internal:3100/sse"
}
```

---

## Tool Discovery

When an MCP server is registered, Hive:

1. Opens an SSE connection to the server URL
2. Sends `{ "method": "tools/list" }` (MCP protocol)
3. Receives the tool list with names, descriptions, and JSON schemas
4. Upserts each tool into the `tools` table:
   ```sql
   INSERT INTO tools (name, description, type, config)
   VALUES ($1, $2, 'mcp', '{"mcp_server_url": "..."}')
   ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description
   ```

Discovered tools then appear in the agent builder tool permissions list.

Re-sync tools: `POST /api/v1/mcp-servers/:id/sync`

---

## MCP Tool Execution

When the workflow engine encounters a `mcp_call` tool invocation:

```rust
// crates/workflow/src/steps/tool_runner.rs
async fn mcp_call(tool: &Tool, arguments: &serde_json::Value) -> Result<String> {
    let server_url = tool.config["mcp_server_url"].as_str().unwrap();
    let client = McpClient::connect(server_url).await?;
    let result = client.call_tool(&tool.name, arguments).await?;
    client.close().await?;
    Ok(result.to_string())
}
```

A new SSE connection is opened per tool call (stateless).

---

## Running MCP Servers in Docker

### Example: MCP Filesystem Server

Add to your `docker-compose.yml`:

```yaml
  mcp-filesystem:
    image: mcp/filesystem:latest   # or build from source
    command: ["--allowed-paths", "/data"]
    volumes:
      - ./data:/data
    ports:
      - "3100:3100"
```

Then register in Hive:
```
Name: Filesystem MCP
URL:  http://host.docker.internal:3100/sse
```

### Example: MCP GitHub Server

```yaml
  mcp-github:
    image: ghcr.io/github/github-mcp-server:latest
    environment:
      GITHUB_PERSONAL_ACCESS_TOKEN: ${GITHUB_TOKEN}
    ports:
      - "3101:3101"
```

Register:
```
Name: GitHub MCP
URL:  http://host.docker.internal:3101/sse
```

### Example: MCP Postgres Server

```yaml
  mcp-postgres:
    image: mcp/postgres:latest
    environment:
      DATABASE_URL: postgres://user:pass@postgres:5432/mydb
    ports:
      - "3102:3102"
```

---

## MCP Client (Rust)

`crates/workflow/src/mcp/client.rs`

```rust
pub struct McpClient {
    url:    String,
    client: reqwest::Client,
}

impl McpClient {
    pub async fn connect(url: &str) -> Result<Self>;

    /// Discover all tools this server provides.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>>;

    /// Call a tool by name with arguments.
    pub async fn call_tool(
        &self,
        name:      &str,
        arguments: &serde_json::Value,
    ) -> Result<serde_json::Value>;
}

pub struct McpTool {
    pub name:        String,
    pub description: String,
    pub schema:      serde_json::Value,   // JSON Schema for parameters
}
```

The MCP protocol over SSE:
- Client sends JSON-RPC requests as POST to the SSE endpoint
- Server sends responses as SSE events
- Messages follow MCP spec: `{ "jsonrpc": "2.0", "method": "...", "params": {...} }`

---

## Notes

- MCP servers must be reachable from inside the `backend` Docker container
- Use `host.docker.internal` to reach servers on the host machine
- Use the Docker Compose service name to reach servers in the same Compose stack
- SSE connections are short-lived (one per tool call); no persistent connection needed
- Tool schemas from MCP are passed directly to the LLM as OpenAI function definitions
