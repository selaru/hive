import type {
  Agent, AgentMessage, Approval, McpServer, Project,
  Provider, Skill, Story, Team
} from './types'

const BASE = import.meta.env.VITE_API_URL ?? '/api/v1'

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json', ...init?.headers },
    ...init,
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: { message: res.statusText } }))
    throw new Error(err?.error?.message ?? res.statusText)
  }
  if (res.status === 204) return undefined as T
  return res.json()
}

// ── Providers ─────────────────────────────────────────────────────────────────
export const providers = {
  list:   ()                   => request<Provider[]>('/providers'),
  get:    (id: string)         => request<Provider>(`/providers/${id}`),
  create: (body: unknown)      => request<Provider>('/providers', { method: 'POST', body: JSON.stringify(body) }),
  update: (id: string, b: unknown) => request<Provider>(`/providers/${id}`, { method: 'PUT', body: JSON.stringify(b) }),
  delete: (id: string)         => request<void>(`/providers/${id}`, { method: 'DELETE' }),
  test:   (id: string)         => request<{ ok: boolean; models: string[] }>(`/providers/${id}/test`, { method: 'POST', body: '{}' }),
  models: (id: string)         => request<string[]>(`/providers/${id}/models`),
}

// ── Skills ────────────────────────────────────────────────────────────────────
export const skills = {
  list:   ()              => request<Skill[]>('/skills'),
  get:    (id: string)    => request<Skill>(`/skills/${id}`),
  import: (body: unknown) => request<Skill>('/skills/import', { method: 'POST', body: JSON.stringify(body) }),
  delete: (id: string)    => request<void>(`/skills/${id}`, { method: 'DELETE' }),
}

// ── Tools ─────────────────────────────────────────────────────────────────────
export const tools = {
  list: () => request<import('./types').Tool[]>('/tools'),
}

// ── Agents ────────────────────────────────────────────────────────────────────
export const agents = {
  list:   ()                      => request<Agent[]>('/agents'),
  get:    (id: string)            => request<Agent>(`/agents/${id}`),
  create: (body: unknown)         => request<Agent>('/agents', { method: 'POST', body: JSON.stringify(body) }),
  update: (id: string, b: unknown)=> request<Agent>(`/agents/${id}`, { method: 'PUT', body: JSON.stringify(b) }),
  delete: (id: string)            => request<void>(`/agents/${id}`, { method: 'DELETE' }),
  test:   (id: string, msg: string) =>
    request<{ response: string }>(`/agents/${id}/test`, {
      method: 'POST', body: JSON.stringify({ message: msg }),
    }),
}

// ── Teams ─────────────────────────────────────────────────────────────────────
export const teams = {
  list:          ()                       => request<Team[]>('/teams'),
  get:           (id: string)             => request<Team>(`/teams/${id}`),
  create:        (body: unknown)          => request<Team>('/teams', { method: 'POST', body: JSON.stringify(body) }),
  update:        (id: string, b: unknown) => request<Team>(`/teams/${id}`, { method: 'PUT', body: JSON.stringify(b) }),
  delete:        (id: string)             => request<void>(`/teams/${id}`, { method: 'DELETE' }),
  getHierarchy:  (id: string)             => request<import('./types').TeamGraph>(`/teams/${id}/hierarchy`),
  saveHierarchy: (id: string, g: unknown) => request<void>(`/teams/${id}/hierarchy`, { method: 'PUT', body: JSON.stringify(g) }),
}

// ── Projects ──────────────────────────────────────────────────────────────────
export const projects = {
  list:     ()              => request<Project[]>('/projects'),
  get:      (id: string)    => request<{ project: Project; pending_approval: Approval | null }>(`/projects/${id}`),
  create:   (body: unknown) => request<Project>('/projects', { method: 'POST', body: JSON.stringify(body) }),
  delete:   (id: string)    => request<void>(`/projects/${id}`, { method: 'DELETE' }),
  stories:     (id: string)    => request<Story[]>(`/projects/${id}/stories`),
  deleteStory: (projectId: string, storyId: string) =>
    request<void>(`/projects/${projectId}/stories/${storyId}`, { method: 'DELETE' }),
  messages:    (id: string)    => request<AgentMessage[]>(`/projects/${id}/messages`),
  approve:  (id: string, comment?: string) =>
    request<Project>(`/projects/${id}/approve`, { method: 'POST', body: JSON.stringify({ comment }) }),
  reject:   (id: string, comment?: string) =>
    request<Project>(`/projects/${id}/reject`, { method: 'POST', body: JSON.stringify({ comment }) }),
}

// ── MCP Servers ───────────────────────────────────────────────────────────────
export const mcpServers = {
  list:   ()              => request<McpServer[]>('/mcp-servers'),
  create: (body: unknown) => request<McpServer>('/mcp-servers', { method: 'POST', body: JSON.stringify(body) }),
  delete: (id: string)    => request<void>(`/mcp-servers/${id}`, { method: 'DELETE' }),
  sync:   (id: string)    => request<{ imported: number }>(`/mcp-servers/${id}/sync`, { method: 'POST', body: '{}' }),
}
