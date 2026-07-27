import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useState, useEffect } from 'react'
import { Plus, Bot, Trash2, Play, Edit3 } from 'lucide-react'
import { agents as api, providers, skills, tools } from '@/lib/api'
import type { Agent } from '@/lib/types'

export default function AgentsPage() {
  const qc = useQueryClient()
  const [editing, setEditing] = useState<Agent | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [testResult, setTestResult] = useState<Record<string, string>>({})

  const { data: agentList = [] } = useQuery({ queryKey: ['agents'], queryFn: api.list })
  const { data: providerList = [] } = useQuery({ queryKey: ['providers'], queryFn: providers.list })
  const { data: skillList = [] } = useQuery({ queryKey: ['skills'], queryFn: skills.list })
  const { data: toolList = [] } = useQuery({ queryKey: ['tools'], queryFn: tools.list })

  const deleteMut = useMutation({
    mutationFn: (id: string) => api.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['agents'] }),
  })

  const testMut = useMutation({
    mutationFn: ({ id, msg }: { id: string; msg: string }) => api.test(id, msg),
    onSuccess: (data, vars) => setTestResult(prev => ({ ...prev, [vars.id]: data.response })),
  })

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">Agents</h1>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm transition-colors"
        >
          <Plus className="w-4 h-4" /> New Agent
        </button>
      </div>

      {/* Agent cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {agentList.map(agent => {
          const provider = providerList.find(p => p.id === agent.provider_id)
          const agentSkills = skillList.filter(s => agent.skill_ids.includes(s.id))
          return (
            <div key={agent.id} className="bg-gray-900 rounded-xl border border-gray-800 p-4 flex flex-col justify-between group">
              <div>
                <div className="flex items-start justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <Bot className="w-4 h-4 text-amber-400" />
                    <span className="font-medium">{agent.name}</span>
                  </div>
                  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button
                      onClick={() => setEditing(agent)}
                      className="p-1 text-gray-600 hover:text-amber-400 transition-colors"
                      title="Edit Agent"
                    >
                      <Edit3 className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => {
                        if (confirm(`Are you sure you want to delete "${agent.name}"?`)) {
                          deleteMut.mutate(agent.id)
                        }
                      }}
                      className="p-1 text-gray-600 hover:text-red-400 transition-colors"
                      title="Delete Agent"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                {agent.description && (
                  <p className="text-sm text-gray-400 mb-3">{agent.description}</p>
                )}

                <div className="space-y-1 text-xs text-gray-500 mb-3">
                  <div>Provider: <span className="text-gray-300">{provider?.name ?? agent.provider_id}</span></div>
                  {agent.model_name && <div>Model: <span className="text-gray-300">{agent.model_name}</span></div>}
                  <div>Skills: <span className="text-gray-300">{agentSkills.length > 0 ? agentSkills.map(s => s.name).join(', ') : agent.skill_ids.length}</span></div>
                  <div>Temp: <span className="text-gray-300">{agent.temperature}</span></div>
                  {agent.max_tokens && <div>Max Tokens: <span className="text-gray-300">{agent.max_tokens}</span></div>}
                  <div>Tools: <span className="text-gray-300">{agent.tool_ids.length}</span></div>
                </div>
              </div>

              <div>
                <button
                  onClick={() => testMut.mutate({ id: agent.id, msg: 'Hello! What can you do?' })}
                  disabled={testMut.isPending}
                  className="flex items-center gap-1 text-xs px-3 py-1.5 bg-gray-800 hover:bg-gray-700 rounded-lg transition-colors"
                >
                  <Play className="w-3 h-3" /> Test
                </button>

                {testResult[agent.id] && (
                  <div className="mt-3 p-3 bg-gray-950 rounded-lg text-xs text-gray-300 max-h-32 overflow-y-auto">
                    {testResult[agent.id]}
                  </div>
                )}
              </div>
            </div>
          )
        })}
      </div>

      {/* Create/edit form */}
      {(showCreate || editing) && (
        <AgentForm
          providers={providerList}
          skills={skillList}
          tools={toolList}
          agent={editing}
          onClose={() => {
            setShowCreate(false)
            setEditing(null)
            qc.invalidateQueries({ queryKey: ['agents'] })
          }}
        />
      )}
    </div>
  )
}

function AgentForm({ providers: providerList, skills: skillList, tools: toolList, agent, onClose }: {
  providers: import('@/lib/types').Provider[]
  skills: import('@/lib/types').Skill[]
  tools: import('@/lib/types').Tool[]
  agent?: Agent | null
  onClose: () => void
}) {
  const [name, setName] = useState(agent?.name ?? '')
  const [providerId, setProviderId] = useState(agent?.provider_id ?? providerList[0]?.id ?? '')
  const [skillIds, setSkillIds] = useState<string[]>(agent?.skill_ids ?? [])
  const [modelName, setModelName] = useState<string>(agent?.model_name ?? '')
  const [availableModels, setAvailableModels] = useState<string[]>([])
  const [loadingModels, setLoadingModels] = useState(false)
  const [toolIds, setToolIds] = useState<string[]>(agent?.tool_ids ?? [])
  const [temperature, setTemperature] = useState(agent?.temperature ?? 0.7)
  const [maxTokens, setMaxTokens] = useState<number | undefined>(agent?.max_tokens ?? undefined)
  const [memoryType, setMemoryType] = useState<'none' | 'window'>(agent?.memory_type as any ?? 'window')

  const saveMut = useMutation({
    mutationFn: (body: unknown) => {
      return import('@/lib/api').then(m => {
        if (agent) {
          return m.agents.update(agent.id, body)
        } else {
          return m.agents.create(body)
        }
      })
    },
    onSuccess: () => onClose(),
  })

  // Fetch available models when provider changes
  useEffect(() => {
    if (!providerId) return
    
    setLoadingModels(true)
    import('@/lib/api')
      .then(api => api.providers.models(providerId))
      .then(models => {
        setAvailableModels(models)
        if (agent && agent.provider_id === providerId) {
          // Keep existing model if provider matches
          setModelName(agent.model_name ?? models[0] ?? '')
        } else {
          if (models.length > 0) setModelName(models[0])
        }
      })
      .catch(err => {
        console.error('Failed to fetch models:', err)
        setAvailableModels([])
      })
      .finally(() => setLoadingModels(false))
  }, [providerId, agent])

  const handleProviderChange = (newProviderId: string) => {
    setProviderId(newProviderId)
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-md max-h-[90vh] overflow-y-auto">
        <h2 className="text-lg font-semibold mb-4">{agent ? 'Edit Agent' : 'Create Agent'}</h2>
        <div className="space-y-3">
          <input
            placeholder="Agent name"
            value={name}
            onChange={e => setName(e.target.value)}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500"
          />
          
          <div>
            <label className="text-xs text-gray-400 mb-1 block">Provider</label>
            <select
              value={providerId}
              onChange={e => handleProviderChange(e.target.value)}
              className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none"
            >
              {providerList.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>

          {availableModels.length > 0 && (
            <div>
              <label className="text-xs text-gray-400 mb-1 block">Model</label>
              <select
                value={modelName}
                onChange={e => setModelName(e.target.value)}
                disabled={loadingModels}
                className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none"
              >
                {availableModels.map(m => <option key={m} value={m}>{m}</option>)}
              </select>
            </div>
          )}

          <div>
            <label className="text-xs text-gray-400 mb-1 block">Skills (select one or more)</label>
            <div className="space-y-1 max-h-48 overflow-y-auto border border-gray-700 rounded-lg p-2">
              {skillList.map(s => (
                <label key={s.id} className="flex items-center gap-2 text-sm hover:bg-gray-800 p-1 rounded cursor-pointer">
                  <input
                    type="checkbox"
                    checked={skillIds.includes(s.id)}
                    onChange={e => {
                      if (e.target.checked) setSkillIds(prev => [...prev, s.id])
                      else setSkillIds(prev => prev.filter(id => id !== s.id))
                    }}
                    className="rounded"
                  />
                  <span className="flex-1">{s.name}</span>
                </label>
              ))}
            </div>
          </div>

          <div>
            <label className="text-xs text-gray-400 mb-1 block">Tools</label>
            <div className="space-y-1 max-h-32 overflow-y-auto">
              {toolList.map(t => (
                <label key={t.id} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={toolIds.includes(t.id)}
                    onChange={e => {
                      if (e.target.checked) setToolIds(prev => [...prev, t.id])
                      else setToolIds(prev => prev.filter(id => id !== t.id))
                    }}
                  />
                  {t.name}
                </label>
              ))}
            </div>
          </div>

          <div className="flex gap-2">
            <label className="text-xs text-gray-400 mt-2">Temp: {temperature}</label>
            <input type="range" min={0} max={1} step={0.1} value={temperature}
              onChange={e => setTemperature(parseFloat(e.target.value))}
              className="flex-1"
            />
          </div>

          <div>
            <label className="text-xs text-gray-400 mb-1 block">Max Tokens</label>
            <input
              type="number"
              placeholder="e.g., 8192 (leave blank for default)"
              value={maxTokens ?? ''}
              onChange={e => {
                const val = parseInt(e.target.value)
                setMaxTokens(isNaN(val) ? undefined : val)
              }}
              className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500"
            />
          </div>
        </div>

        <div className="flex gap-2 mt-5">
          <button onClick={onClose} className="flex-1 px-4 py-2 bg-gray-800 hover:bg-gray-700 rounded-lg text-sm transition-colors">
            Cancel
          </button>
          <button
            onClick={() => {
              const payload = { 
                name, 
                provider_id: providerId, 
                skill_ids: skillIds, 
                model_name: modelName || undefined,
                tool_ids: toolIds, 
                temperature, 
                max_tokens: maxTokens,
                memory_type: memoryType 
              }
              console.log('Saving agent with payload:', payload)
              saveMut.mutate(payload)
            }}
            disabled={!name || !providerId || skillIds.length === 0}
            className="flex-1 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {agent ? 'Save' : 'Create'}
          </button>
        </div>
      </div>
    </div>
  )
}
