// Shared TypeScript types mirroring backend Rust models

export type ProviderType = 'openai_compat' | 'aws_bedrock'

export interface Provider {
  id:          string
  name:        string
  type:        ProviderType
  base_url?:   string
  api_key?:    string
  aws_profile?: string
  aws_region?: string
  model_name?: string
  config:      Record<string, unknown>
  is_active:   boolean
  created_at:  string
}

export interface Skill {
  id:            string
  name:          string
  description:   string
  source_url?:   string
  source_type?:  'github' | 'skills_sh' | 'local'
  system_prompt: string
  tool_hints:    string[]
  version?:      string
  created_at:    string
}

export interface Tool {
  id:          string
  name:        string
  description?: string
  type:        'file' | 'code_exec' | 'web' | 'shell' | 'mcp'
  config:      Record<string, unknown>
}

export interface Agent {
  id:                      string
  name:                    string
  description?:            string
  provider_id:             string
  skill_ids:               string[]
  model_name?:             string
  system_prompt_override?: string
  temperature:             number
  max_tokens?:             number
  tool_ids:                string[]
  memory_type:             'none' | 'window' | 'summary'
  memory_window:           number
  created_at:              string
}

export interface Team {
  id:          string
  name:        string
  description?: string
  created_at:  string
}

export interface TeamMember {
  id:         string
  team_id:    string
  agent_id:   string
  role:       string
  position_x: number
  position_y: number
}

export type ProjectStatus =
  | 'submitted'
  | 'lead_review'
  | 'awaiting_approval_lead'
  | 'architect_breakdown'
  | 'awaiting_approval_stories'
  | 'in_development'
  | 'in_review'
  | 'awaiting_final_approval'
  | 'done'
  | 'rejected'

export interface Project {
  id:                   string
  title:                string
  description?:         string
  acceptance_criteria?: string
  team_id:              string
  status:               ProjectStatus
  folder_path?:         string
  created_at:           string
  updated_at:           string
}

export interface Story {
  id:                   string
  project_id:           string
  title:                string
  description?:         string
  acceptance_criteria?: string
  status:               'todo' | 'in_progress' | 'in_review' | 'done' | 'blocked'
  assigned_member_id?:  string
  parent_story_id?:     string
  priority:             number
  created_at:           string
}

export interface Approval {
  id:             string
  project_id:     string
  story_id?:      string
  type:           'lead_rewrite' | 'story_breakdown' | 'final_review'
  status:         'pending' | 'approved' | 'rejected'
  payload?:       Record<string, unknown>
  human_comment?: string
  created_at:     string
  resolved_at?:   string
}

export interface AgentMessage {
  id:          string
  project_id:  string
  from_role?:  string
  to_role?:    string
  content:     string
  tokens_used?: number
  latency_ms?:  number
  created_at:  string
}

export interface McpServer {
  id:         string
  name:       string
  url:        string
  is_active:  boolean
  created_at: string
}

// React Flow team graph shapes (mirrors Rust FlowNode/FlowEdge)
export interface FlowNodeData {
  member_id:  string | null
  agent_id:   string
  agent_name: string
  role:       string
  [key: string]: unknown   // React Flow requires index signature
}

export interface FlowNode {
  id:       string
  type:     string
  position: { x: number; y: number }
  data:     FlowNodeData
}

export interface FlowEdgeData {
  hierarchy_id?:      string
  communication_mode: string
  [key: string]: unknown
}

export interface FlowEdge {
  id:     string
  source: string
  target: string
  data:   FlowEdgeData
}

export interface TeamGraph {
  nodes: FlowNode[]
  edges: FlowEdge[]
}

// WebSocket events
export type WsEvent =
  | { type: 'project_status_changed'; data: { project_id: string; status: ProjectStatus } }
  | { type: 'agent_token';            data: { project_id: string; role: string; token: string } }
  | { type: 'message_created';        data: { message: AgentMessage } }
  | { type: 'approval_required';      data: { approval_id: string; kind: string; payload: Record<string, unknown> } }
  | { type: 'story_updated';          data: { story_id: string; status: string } }
  | { type: 'story_deleted';          data: { story_id: string; project_id: string } }
  | { type: 'project_error';          data: { project_id: string; message: string } }
