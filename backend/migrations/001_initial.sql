-- Hive database schema
-- PostgreSQL 16

-- ── Extensions ──────────────────────────────────────────────────────────────
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ── project_status enum ─────────────────────────────────────────────────────
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

-- ── Helper: auto-update updated_at ──────────────────────────────────────────
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN NEW.updated_at = now(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

-- ── providers ────────────────────────────────────────────────────────────────
CREATE TABLE providers (
    id          UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    name        TEXT        NOT NULL,
    type        TEXT        NOT NULL CHECK (type IN ('openai_compat', 'aws_bedrock')),
    base_url    TEXT,
    api_key     TEXT,                           -- AES-256 encrypted at rest
    aws_profile TEXT,
    aws_region  TEXT,
    model_name  TEXT,
    config      JSONB       NOT NULL DEFAULT '{}',
    is_active   BOOLEAN     NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ── skills ───────────────────────────────────────────────────────────────────
CREATE TABLE skills (
    id            UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    name          TEXT        NOT NULL UNIQUE,
    description   TEXT        NOT NULL,
    source_url    TEXT,
    source_type   TEXT        CHECK (source_type IN ('github', 'skills_sh', 'local')),
    system_prompt TEXT        NOT NULL,
    tool_hints    TEXT[]      NOT NULL DEFAULT '{}',
    version       TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ── tools ────────────────────────────────────────────────────────────────────
CREATE TABLE tools (
    id          UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    name        TEXT        NOT NULL UNIQUE,
    description TEXT,
    type        TEXT        NOT NULL CHECK (type IN ('file', 'code_exec', 'web', 'shell', 'mcp')),
    config      JSONB       NOT NULL DEFAULT '{}'
);

-- Seed built-in tools
INSERT INTO tools (name, description, type) VALUES
    ('file_read',     'Read a file within the project folder',               'file'),
    ('file_write',    'Write a file within the project folder',              'file'),
    ('file_list',     'List files and directories in the project folder',    'file'),
    ('code_execute',  'Run Python, JavaScript, or Bash in a sandbox',        'code_exec'),
    ('web_search',    'Search the web and return results',                   'web'),
    ('web_fetch',     'Fetch a URL and return its content as text',          'web'),
    ('shell_command', 'Run a whitelisted shell command',                     'shell');

-- ── agents ───────────────────────────────────────────────────────────────────
CREATE TABLE agents (
    id                     UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    name                   TEXT        NOT NULL,
    description            TEXT,
    provider_id            UUID        NOT NULL REFERENCES providers(id),
    skill_id               UUID        NOT NULL REFERENCES skills(id),
    system_prompt_override TEXT,
    temperature            FLOAT       NOT NULL DEFAULT 0.7,
    max_tokens             INTEGER,
    tool_ids               UUID[]      NOT NULL DEFAULT '{}',
    memory_type            TEXT        NOT NULL DEFAULT 'none'
                               CHECK (memory_type IN ('none', 'window', 'summary')),
    memory_window          INTEGER     NOT NULL DEFAULT 20,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ── teams ────────────────────────────────────────────────────────────────────
CREATE TABLE teams (
    id          UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    name        TEXT        NOT NULL,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ── team_members ─────────────────────────────────────────────────────────────
CREATE TABLE team_members (
    id         UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    team_id    UUID        NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    agent_id   UUID        NOT NULL REFERENCES agents(id),
    role       TEXT        NOT NULL,
    position_x FLOAT       NOT NULL DEFAULT 0,
    position_y FLOAT       NOT NULL DEFAULT 0,
    UNIQUE(team_id, agent_id)
);

-- ── team_hierarchy ───────────────────────────────────────────────────────────
CREATE TABLE team_hierarchy (
    id                 UUID NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    team_id            UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    parent_member_id   UUID NOT NULL REFERENCES team_members(id) ON DELETE CASCADE,
    child_member_id    UUID NOT NULL REFERENCES team_members(id) ON DELETE CASCADE,
    communication_mode TEXT NOT NULL DEFAULT 'request_response'
                           CHECK (communication_mode IN
                               ('request_response', 'broadcast', 'parallel')),
    UNIQUE(parent_member_id, child_member_id)
);

-- ── projects ─────────────────────────────────────────────────────────────────
CREATE TABLE projects (
    id                   UUID           NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    title                TEXT           NOT NULL,
    description          TEXT,
    acceptance_criteria  TEXT,
    team_id              UUID           NOT NULL REFERENCES teams(id),
    status               project_status NOT NULL DEFAULT 'submitted',
    folder_path          TEXT,
    created_at           TIMESTAMPTZ    NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ    NOT NULL DEFAULT now()
);

CREATE TRIGGER projects_updated_at
    BEFORE UPDATE ON projects
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- ── stories ──────────────────────────────────────────────────────────────────
CREATE TABLE stories (
    id                   UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    project_id           UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title                TEXT        NOT NULL,
    description          TEXT,
    acceptance_criteria  TEXT,
    status               TEXT        NOT NULL DEFAULT 'todo'
                             CHECK (status IN
                                 ('todo', 'in_progress', 'in_review', 'done', 'blocked')),
    assigned_member_id   UUID        REFERENCES team_members(id),
    parent_story_id      UUID        REFERENCES stories(id),
    priority             INTEGER     NOT NULL DEFAULT 0,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX stories_project_id ON stories(project_id);

-- ── approvals ────────────────────────────────────────────────────────────────
CREATE TABLE approvals (
    id             UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    project_id     UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    story_id       UUID        REFERENCES stories(id),
    type           TEXT        NOT NULL
                       CHECK (type IN ('lead_rewrite', 'story_breakdown', 'final_review')),
    status         TEXT        NOT NULL DEFAULT 'pending'
                       CHECK (status IN ('pending', 'approved', 'rejected')),
    payload        JSONB,
    human_comment  TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at    TIMESTAMPTZ
);

CREATE INDEX approvals_project_id ON approvals(project_id);

-- ── agent_messages ───────────────────────────────────────────────────────────
CREATE TABLE agent_messages (
    id           UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    from_role    TEXT,
    to_role      TEXT,
    content      TEXT        NOT NULL,
    tokens_used  INTEGER,
    latency_ms   INTEGER,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX agent_messages_project_id ON agent_messages(project_id);
CREATE INDEX agent_messages_project_role ON agent_messages(project_id, from_role);

-- ── mcp_servers ──────────────────────────────────────────────────────────────
CREATE TABLE mcp_servers (
    id         UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    name       TEXT        NOT NULL,
    url        TEXT        NOT NULL,
    is_active  BOOLEAN     NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
