# Skills

## Overview

Skills define how an agent behaves. A skill is a `SKILL.md` file following the
**Agent Skills open standard** (originally developed by Anthropic, maintained at
`agentskills.io` / `vercel-labs/agent-skills`).

This is the same format used by Claude Code, Cursor, OpenCode, Codex, and 70+ other AI agents.
Skills are therefore portable — a skill used in OpenCode will work in Hive.

---

## SKILL.md Format

A SKILL.md file has two parts:

### 1. YAML Frontmatter

```yaml
---
name: senior-developer
description: Use when implementing features, writing code, or fixing bugs in any language
---
```

| Field | Required | Description |
|---|---|---|
| `name` | Yes | Unique slug for the skill |
| `description` | Yes | One-sentence summary — used by agents to decide when to activate this skill |

### 2. Markdown Body (System Prompt)

Everything after the closing `---` becomes the agent's system prompt.

```markdown
# Senior Developer

You are an expert software engineer with deep experience in system design,
clean code practices, and test-driven development.

## Core Behaviours
- Write clean, idiomatic code with meaningful names
- Add inline comments only where the why is non-obvious
- Prefer simple solutions over clever ones
- Always validate inputs and handle errors explicitly

## When Using File Tools
- Read existing files before modifying them
- Keep changes minimal and focused on the story
- Write files to the working directory you are given

## Output Format
When you complete a task, summarise:
- Files created/modified
- Key decisions made
- Anything the reviewer should pay attention to
```

---

## Importing Skills

### From GitHub

Any public (or accessible private) GitHub repository that contains a `SKILL.md` file.

**Via UI**: Settings → Skills → Import → GitHub tab → enter URL or `owner/repo`

**Via API**:
```http
POST /api/v1/skills/import
{
  "source_type": "github",
  "url": "owner/repo"
}
```

The backend fetches the raw file from these locations in order:
1. `https://raw.githubusercontent.com/{owner}/{repo}/main/SKILL.md`
2. `https://raw.githubusercontent.com/{owner}/{repo}/main/.agents/skills/{name}/SKILL.md`
3. `https://raw.githubusercontent.com/{owner}/{repo}/master/SKILL.md`

### From skills.sh / vercel-labs/agent-skills

Skills on skills.sh are backed by GitHub repos. Use the same GitHub import.

```http
POST /api/v1/skills/import
{
  "source_type": "skills_sh",
  "slug": "vercel-labs/agent-skills"
}
```

This resolves to `github.com/vercel-labs/agent-skills` and fetches `SKILL.md`.

### Local

Paste SKILL.md content directly in the UI (Settings → Skills → Import → Paste tab).

---

## Skill Parser (Rust)

`crates/skills/src/parser.rs`

```rust
pub struct ParsedSkill {
    pub name:          String,
    pub description:   String,
    pub system_prompt: String,
    pub tool_hints:    Vec<String>,  // extracted by scanning body for tool names
}

pub fn parse_skill_md(content: &str) -> Result<ParsedSkill> {
    // Split on --- delimiters
    // Parse YAML frontmatter with serde_yaml
    // Body = everything after second ---
    // Extract tool hints by scanning for known tool names (file_read, etc.)
}
```

---

## Recommended Skills to Import

| Skill | GitHub | Role |
|---|---|---|
| Project Manager | Create your own | Lead agent |
| Software Architect | Create your own | Architect agent |
| Senior Developer | Create your own or `vercel-labs/agent-skills` | Developer agents |
| Code Reviewer | Create your own | Reviewer agent |

### Minimal Lead Skill Example

Create `SKILL.md`:
```markdown
---
name: project-lead
description: Use to rewrite, clarify, and structure incoming project requirements
---

# Project Lead

You are an experienced technical project lead. Your job is to take raw project
requests and transform them into clear, complete, actionable specifications.

## When You Receive a Project Request

Rewrite it as a structured spec with these sections:
1. **Overview** — what this project does and why
2. **Goals** — specific, measurable outcomes
3. **Out of Scope** — what this project does NOT include
4. **Technical Constraints** — language, framework, or environment requirements
5. **Acceptance Criteria** — testable conditions for completion

Be specific. If anything is ambiguous, make a reasonable assumption and state it.
Do not ask clarifying questions — produce the best spec you can from the input.
```

---

## Skill Versioning

When re-importing a skill with the same name, the existing record is updated.
The `source_url` and `version` fields are updated to reflect the new import.

Future: track version history in a `skill_versions` table.
