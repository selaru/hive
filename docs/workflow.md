# Workflow Engine

## State Machine

Each project moves through the following states:

```
submitted
    │
    │  WorkflowEngine::run_lead_review()
    ▼
lead_review
    │
    │  Lead agent produces rewritten spec
    ▼
awaiting_approval_lead          ← 🧑 Human checkpoint #1
    │
    │  POST /projects/:id/approve
    ▼
architect_breakdown
    │
    │  WorkflowEngine::run_architect()
    ▼  Architect produces story list
awaiting_approval_stories        ← 🧑 Human checkpoint #2
    │
    │  POST /projects/:id/approve
    ▼
in_development
    │
    │  WorkflowEngine::run_development()
    │  One Tokio task per story, developer agents run in parallel
    ▼
in_review
    │
    │  WorkflowEngine::run_review()
    │  Review agent reads all stories + code
    ▼
awaiting_final_approval          ← 🧑 Human checkpoint #3
    │
    │  POST /projects/:id/approve
    ▼
done
```

At any state, a rejection sends the project to `rejected`
(future: re-queue to previous state with feedback).

---

## Implementation (Rust)

The engine lives in `crates/workflow/src/engine.rs`.

```rust
pub struct WorkflowEngine {
    db:       PgPool,
    redis:    deadpool_redis::Pool,
    providers: Arc<ProviderManager>,
    projects_dir: PathBuf,
}

impl WorkflowEngine {
    /// Called when a project is created or after an approval.
    /// Determines the next step and spawns a Tokio task for it.
    pub async fn advance(&self, project_id: Uuid) -> Result<()> {
        let project = self.get_project(project_id).await?;
        match project.status {
            Submitted             => self.run_lead_review(project).await,
            ArchitectBreakdown    => self.run_architect(project).await,
            InDevelopment         => self.run_development(project).await,
            InReview              => self.run_review(project).await,
            // Awaiting* states: do nothing, wait for human
            _ => Ok(())
        }
    }
}
```

---

## Agent Steps

### Step 1: Lead Review (`lead_review`)

**Trigger**: project submitted (status = `submitted`)

**Agent role**: `lead`

**Input** (system prompt from skill + user message):
```
Project title: {title}
Description: {description}
Acceptance criteria: {acceptance_criteria}
```

**Task**:
- Rewrite the requirement with clarity and completeness
- Add missing context, resolve ambiguities
- Output structured markdown with: Overview, Goals, Out of Scope, Acceptance Criteria

**Output**: rewritten spec saved to `approvals.payload.rewritten_spec`

**Transition**: `lead_review` → `awaiting_approval_lead`

---

### Step 2: Architect Breakdown (`architect_breakdown`)

**Trigger**: user approves lead rewrite

**Agent role**: `architect`

**Input**: approved rewritten spec

**Task**:
- Break the spec into user stories
- Each story must have: title, description, acceptance criteria
- Stories should be independently deliverable
- Output as JSON array:
  ```json
  [{ "title": "...", "description": "...", "acceptance_criteria": "..." }]
  ```

**Output**: story list saved to `approvals.payload.stories`; stories created in DB when approved

**Transition**: `architect_breakdown` → `awaiting_approval_stories`

---

### Step 3: Development (`in_development`)

**Trigger**: user approves story breakdown

**Behaviour**: one Tokio task is spawned **per story**

**Agent role**: `developer` (all members with role `developer`)

Stories are round-robin assigned to available developer agents.
If only one developer, all stories go to that agent sequentially.

**Input** (per story):
```
You are working on project: {project_title}
Your story: {story_title}
Description: {story_description}
Acceptance criteria: {acceptance_criteria}
Working directory: {project_folder}/src/
```

**Tools available**: as configured on the agent (file_read, file_write, code_execute, etc.)

**Completion**: story status → `in_review` when agent signals done (no more tool calls)

**Project transition**: when ALL stories reach `in_review` → project transitions to `in_review`

---

### Step 4: Code Review (`in_review`)

**Trigger**: all stories completed

**Agent role**: `reviewer`

**Input**:
- List of all stories with descriptions + acceptance criteria
- Contents of `projects/{id}/src/` (via file_list + file_read tools)

**Task**:
- Review the code for each story against its acceptance criteria
- Note any gaps, bugs, or missing implementations
- Output a structured review report

**Output**: review report saved to `approvals.payload.review_report`

**Transition**: `in_review` → `awaiting_final_approval`

---

## Approval Records

Each checkpoint creates an `approvals` row:

```
awaiting_approval_lead       → type = 'lead_rewrite',    payload = { rewritten_spec }
awaiting_approval_stories    → type = 'story_breakdown', payload = { stories: [...] }
awaiting_final_approval      → type = 'final_review',    payload = { review_report }
```

The frontend shows the payload in the ApprovalPanel component.

---

## Message History

Before each LLM call, the engine loads the conversation history:

```rust
let messages = db.get_messages(project_id, role, memory_window).await?;
// Prepend system prompt (from agent skill)
// Append the new user message
// Call provider
// Save assistant response
```

This gives agents context about previous turns within their role.

---

## Error Handling

- LLM call timeout: retry up to 3 times with exponential backoff
- LLM call failure: set project to `error` state, publish error event to WebSocket
- Tool execution failure: return error as tool result to LLM (let LLM decide how to handle)
- JSON parse failure (architect stories): retry with explicit JSON format reminder

---

## Extending the Workflow

To add a new step or role:
1. Add the new status to the `project_status` enum in DB and Rust models
2. Add a branch in `WorkflowEngine::advance()`
3. Implement the step function
4. Add the new role to the team builder UI role list

The team builder enforces that required roles (lead, architect, at least one developer,
reviewer) are present before a team can be used in a project.
