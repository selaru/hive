# Data Models

Full PostgreSQL schema for Hive.

Migration file: `backend/migrations/001_initial.sql`

---

## providers

Stores LLM provider configurations.

```sql
CREATE TABLE providers (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    -- 'openai_compat' covers MLX LM and Gemini
    type        TEXT NOT NULL CHECK (type IN ('openai_compat', 'aws_bedrock')),
    base_url    TEXT,                  -- http://host.docker.internal:8080 for MLX LM
                                       -- https://generativelanguage.googleapis.com/v1beta/openai/ for Gemini
    api_key     TEXT,                  -- AES-256 encrypted; also holds Gemini key
    aws_profile TEXT,                  -- SSO profile name from ~/.aws/config
    aws_region  TEXT,
    model_name  TEXT,                  -- default model for this provider
    config      JSONB    DEFAULT '{}', -- provider-specific extras
    is_active   BOOLEAN  DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Notes**:
- `api_key` is encrypted at rest using `SECRET_KEY` from environment
- For AWS Bedrock, `api_key` is unused; auth comes from the mounted `~/.aws`
- `config` can hold extras like `max_retries`, `timeout_secs`

---

## skills

Imported SKILL.md content (Agent Skills open standard).

```sql
CREATE TABLE skills (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name          TEXT NOT NULL UNIQUE,   -- from SKILL.md frontmatter
    description   TEXT NOT NULL,          -- from SKILL.md frontmatter (activation hint)
    source_url    TEXT,                   -- original GitHub URL or skills.sh slug
    source_type   TEXT CHECK (source_type IN ('github', 'skills_sh', 'local')),
    system_prompt TEXT NOT NULL,          -- full SKILL.md body (markdown)
    tool_hints    TEXT[] DEFAULT '{}',    -- tools mentioned in the skill body
    version       TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## tools

Registry of tools available to agents.

```sql
CREATE TABLE tools (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    type        TEXT NOT NULL CHECK (
                    type IN ('file', 'code_exec', 'web', 'shell', 'mcp')
                ),
    config      JSONB DEFAULT '{}'   -- e.g. { "mcp_server_url": "..." }
);
```

Built-in tools (seeded on startup):

| name | type | description |
|---|---|---|
| `file_read` | file | Read a file within the project folder |
| `file_write` | file | Write a file within the project folder |
| `file_list` | file | List files/directories in the project folder |
| `code_execute` | code_exec | Run code in a sandboxed Docker container |
| `web_search` | web | Search the web (Brave API or SearXNG) |
| `web_fetch` | web | Fetch and return URL content as text |
| `shell_command` | shell | Run a whitelisted shell command |

MCP tools are inserted dynamically when an MCP server is registered.

---

## agents

An agent = provider + skill + tool permissions + memory settings.

```sql
CREATE TABLE agents (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                   TEXT NOT NULL,
    description            TEXT,
    provider_id            UUID NOT NULL REFERENCES providers(id),
    skill_id               UUID NOT NULL REFERENCES skills(id),
    system_prompt_override TEXT,        -- overrides skill body if set
    temperature            FLOAT   NOT NULL DEFAULT 0.7,
    max_tokens             INTEGER,
    tool_ids               UUID[]  NOT NULL DEFAULT '{}',
    memory_type            TEXT    NOT NULL DEFAULT 'none'
                               CHECK (memory_type IN ('none', 'window', 'summary')),
    memory_window          INTEGER NOT NULL DEFAULT 20,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## teams

Named groups of agents.

```sql
CREATE TABLE teams (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## team_members

Agents assigned to a team with a role and a canvas position (for React Flow).

```sql
CREATE TABLE team_members (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id    UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    agent_id   UUID NOT NULL REFERENCES agents(id),
    role       TEXT NOT NULL,      -- 'lead', 'architect', 'developer', 'reviewer', custom
    position_x FLOAT NOT NULL DEFAULT 0,  -- React Flow canvas x
    position_y FLOAT NOT NULL DEFAULT 0,  -- React Flow canvas y
    UNIQUE(team_id, agent_id)
);
```

Roles used by the default workflow:

| role | step |
|---|---|
| `lead` | Rewrites the incoming spec |
| `architect` | Breaks the spec into user stories |
| `developer` | Implements a story (one per story; multiple ok) |
| `reviewer` | Reviews completed code across all stories |

---

## team_hierarchy

Communication edges between team members (React Flow edges).

```sql
CREATE TABLE team_hierarchy (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id            UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    parent_member_id   UUID NOT NULL REFERENCES team_members(id) ON DELETE CASCADE,
    child_member_id    UUID NOT NULL REFERENCES team_members(id) ON DELETE CASCADE,
    communication_mode TEXT NOT NULL DEFAULT 'request_response'
                           CHECK (communication_mode IN
                               ('request_response', 'broadcast', 'parallel')),
    UNIQUE(parent_member_id, child_member_id)
);
```

---

## projects

Top-level work units submitted by the user.

```sql
CREATE TYPE project_status AS ENUM (
    'submitted',
    'lead_review',
    'awaiting_approval_lead',
    'architect_breakdown',
    'awaiting_approval_stories',
    'in_development',
    'in_review',
    'awaiting_final_approval',
    'done',
    'rejected'
);

CREATE TABLE projects (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title                TEXT NOT NULL,
    description          TEXT,
    acceptance_criteria  TEXT,
    team_id              UUID NOT NULL REFERENCES teams(id),
    status               project_status NOT NULL DEFAULT 'submitted',
    folder_path          TEXT,   -- absolute path inside container: /app/projects/{id}-{slug}
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Auto-update updated_at
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN NEW.updated_at = now(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER projects_updated_at
    BEFORE UPDATE ON projects
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
```

---

## stories

User stories within a project, created by the architect agent.

```sql
CREATE TABLE stories (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id           UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title                TEXT NOT NULL,
    description          TEXT,
    acceptance_criteria  TEXT,
    status               TEXT NOT NULL DEFAULT 'todo'
                             CHECK (status IN
                                 ('todo', 'in_progress', 'in_review', 'done', 'blocked')),
    assigned_member_id   UUID REFERENCES team_members(id),
    parent_story_id      UUID REFERENCES stories(id),
    priority             INTEGER NOT NULL DEFAULT 0,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## approvals

Human checkpoint records. The workflow pauses until one is resolved.

```sql
CREATE TABLE approvals (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id     UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    story_id       UUID REFERENCES stories(id),
    type           TEXT NOT NULL CHECK (
                       type IN ('lead_rewrite', 'story_breakdown', 'final_review')
                   ),
    status         TEXT NOT NULL DEFAULT 'pending'
                       CHECK (status IN ('pending', 'approved', 'rejected')),
    payload        JSONB,        -- content to review (rewritten spec, story list, review report)
    human_comment  TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at    TIMESTAMPTZ
);
```

---

## agent_messages

Full conversation log per project, per agent role.

```sql
CREATE TABLE agent_messages (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id   UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    from_role    TEXT,    -- 'user', 'lead', 'architect', 'developer', 'reviewer', 'system'
    to_role      TEXT,
    content      TEXT NOT NULL,
    tokens_used  INTEGER,
    latency_ms   INTEGER,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX agent_messages_project_id ON agent_messages(project_id);
CREATE INDEX agent_messages_project_role ON agent_messages(project_id, from_role);
```

---

## mcp_servers

Registered MCP servers (Docker-hosted).

```sql
CREATE TABLE mcp_servers (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    url        TEXT NOT NULL,   -- SSE endpoint, e.g. http://host.docker.internal:3000/sse
    is_active  BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```
