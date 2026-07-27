# API Design

Base URL: `http://localhost/api/v1`
WebSocket: `ws://localhost/ws`

All responses are JSON. No authentication required (single-user local tool).

---

## Providers

```
GET    /api/v1/providers              List all providers
POST   /api/v1/providers              Create a provider
GET    /api/v1/providers/:id          Get a provider
PUT    /api/v1/providers/:id          Update a provider
DELETE /api/v1/providers/:id          Delete a provider
POST   /api/v1/providers/:id/test     Test connection; returns { ok, models: string[] }
```

**POST /providers body**:
```json
{
  "name": "Local MLX",
  "type": "openai_compat",
  "base_url": "http://host.docker.internal:8080",
  "api_key": "",
  "model_name": "mlx-community/Mistral-7B-Instruct-v0.3-4bit"
}
```

```json
{
  "name": "Gemini Flash",
  "type": "openai_compat",
  "base_url": "https://generativelanguage.googleapis.com/v1beta/openai/",
  "api_key": "AIza...",
  "model_name": "gemini-2.0-flash"
}
```

```json
{
  "name": "AWS Bedrock",
  "type": "aws_bedrock",
  "aws_profile": "my-sso-profile",
  "aws_region": "us-east-1",
  "model_name": "anthropic.claude-3-5-sonnet-20241022-v2:0"
}
```

---

## Skills

```
GET    /api/v1/skills                List all skills
POST   /api/v1/skills/import         Import from GitHub or skills.sh
GET    /api/v1/skills/:id            Get a skill (includes full system_prompt)
DELETE /api/v1/skills/:id            Delete a skill
```

**POST /skills/import body**:
```json
{
  "source_type": "github",
  "url": "https://github.com/owner/repo"
}
```
```json
{
  "source_type": "github",
  "url": "owner/repo"
}
```
```json
{
  "source_type": "skills_sh",
  "slug": "vercel-labs/agent-skills"
}
```

The backend fetches `SKILL.md` from the repo root (or `.agents/skills/{name}/SKILL.md`),
parses frontmatter, and stores the result.

---

## Tools

```
GET    /api/v1/tools                 List all tools (built-in + MCP-discovered)
```

---

## Agents

```
GET    /api/v1/agents                List all agents
POST   /api/v1/agents                Create an agent
GET    /api/v1/agents/:id            Get an agent
PUT    /api/v1/agents/:id            Update an agent
DELETE /api/v1/agents/:id            Delete an agent
POST   /api/v1/agents/:id/test       Send a test message; returns { response: string }
```

**POST /agents body**:
```json
{
  "name": "Project Lead",
  "description": "Rewrites and clarifies incoming project specs",
  "provider_id": "uuid",
  "skill_id": "uuid",
  "temperature": 0.7,
  "max_tokens": 4096,
  "tool_ids": [],
  "memory_type": "window",
  "memory_window": 20
}
```

---

## Teams

```
GET    /api/v1/teams                 List all teams
POST   /api/v1/teams                 Create a team
GET    /api/v1/teams/:id             Get a team
PUT    /api/v1/teams/:id             Update team metadata
DELETE /api/v1/teams/:id             Delete a team
GET    /api/v1/teams/:id/hierarchy   Get React Flow graph { nodes, edges }
PUT    /api/v1/teams/:id/hierarchy   Save React Flow graph
```

**GET /teams/:id/hierarchy response**:
```json
{
  "nodes": [
    { "id": "member-uuid", "type": "agentNode", "position": { "x": 100, "y": 50 },
      "data": { "member_id": "uuid", "agent_name": "Lead", "role": "lead" } }
  ],
  "edges": [
    { "id": "edge-uuid", "source": "member-uuid-lead", "target": "member-uuid-architect",
      "data": { "communication_mode": "request_response" } }
  ]
}
```

---

## Projects

```
GET    /api/v1/projects              List all projects (with status)
POST   /api/v1/projects              Submit a new project
GET    /api/v1/projects/:id          Get project + current approval
GET    /api/v1/projects/:id/stories  Get all stories for a project
GET    /api/v1/projects/:id/messages Get agent message log (paginated)
POST   /api/v1/projects/:id/approve  Approve current checkpoint
POST   /api/v1/projects/:id/reject   Reject current checkpoint
```

**POST /projects body**:
```json
{
  "title": "Build a user authentication system",
  "description": "We need login, registration, and password reset...",
  "acceptance_criteria": "- JWT tokens\n- Email verification\n- Password strength rules",
  "team_id": "uuid"
}
```

**POST /projects/:id/approve body**:
```json
{ "comment": "Looks good, proceed." }
```

**POST /projects/:id/reject body**:
```json
{ "comment": "Please add more detail on the OAuth flow." }
```

---

## MCP Servers

```
GET    /api/v1/mcp-servers           List registered MCP servers
POST   /api/v1/mcp-servers           Register a new MCP server
DELETE /api/v1/mcp-servers/:id       Remove an MCP server
POST   /api/v1/mcp-servers/:id/sync  Re-discover tools from this server
```

**POST /mcp-servers body**:
```json
{
  "name": "Filesystem MCP",
  "url": "http://host.docker.internal:3100/sse"
}
```

On registration, the backend immediately calls `tools/list` on the MCP server
and upserts discovered tools into the `tools` table with `type = 'mcp'` and
`config.mcp_server_url` set.

---

## WebSocket

Connect: `ws://localhost/ws/projects/:id`

The server sends newline-delimited JSON event objects.

### Event Types

```json
{ "type": "project.status_changed",
  "data": { "project_id": "uuid", "status": "lead_review" } }

{ "type": "agent.token",
  "data": { "project_id": "uuid", "role": "lead", "token": "..." } }

{ "type": "message.created",
  "data": { "id": "uuid", "from_role": "lead", "to_role": "architect",
            "content": "...", "created_at": "..." } }

{ "type": "approval.required",
  "data": { "approval_id": "uuid", "type": "lead_rewrite",
            "payload": { "rewritten_spec": "..." } } }

{ "type": "story.updated",
  "data": { "story_id": "uuid", "status": "in_progress" } }

{ "type": "project.error",
  "data": { "project_id": "uuid", "message": "LLM call failed: timeout" } }
```

### Board WebSocket

Connect: `ws://localhost/ws/board`

Receives project-level events (status changes, new projects) for the Kanban board view.

---

## Error Responses

All errors use a consistent shape:

```json
{ "error": { "code": "NOT_FOUND", "message": "Project not found" } }
```

HTTP status codes:
- `400` — bad request / validation error
- `404` — resource not found
- `409` — conflict (e.g. skill name already exists)
- `422` — unprocessable (e.g. provider test failed)
- `500` — internal server error
