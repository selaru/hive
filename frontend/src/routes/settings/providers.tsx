import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { Plus, Trash2, Check, X, RefreshCw, Server } from 'lucide-react'
import { providers as pApi, mcpServers as mcpApi } from '@/lib/api'
import type { Provider } from '@/lib/types'

export default function ProvidersPage() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [testResults, setTestResults] = useState<Record<string, { ok: boolean; models: string[] }>>({})
  const { data: providerList = [] } = useQuery({ queryKey: ['providers'], queryFn: pApi.list })
  const { data: mcpList = [] } = useQuery({ queryKey: ['mcp-servers'], queryFn: mcpApi.list })

  const deleteMut = useMutation({
    mutationFn: (id: string) => pApi.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['providers'] }),
  })

  const testMut = useMutation({
    mutationFn: (id: string) => pApi.test(id),
    onSuccess: (data, id) => setTestResults(prev => ({ ...prev, [id]: data })),
    onError: (_, id) => setTestResults(prev => ({ ...prev, [id]: { ok: false, models: [] } })),
  })

  const deleteMcpMut = useMutation({
    mutationFn: (id: string) => mcpApi.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['mcp-servers'] }),
  })

  const [syncStatus, setSyncStatus] = useState<Record<string, { ok: boolean; message: string }>>({})

  const syncMcpMut = useMutation({
    mutationFn: (id: string) => mcpApi.sync(id),
    onSuccess: (_, id) => {
      setSyncStatus(prev => ({ ...prev, [id]: { ok: true, message: 'Successfully synced tools!' } }))
      qc.invalidateQueries({ queryKey: ['mcp-servers'] })
    },
    onError: (e: any, id) => {
      setSyncStatus(prev => ({ ...prev, [id]: { ok: false, message: e.message ?? 'Sync failed.' } }))
    },
  })

  return (
    <div className="p-6 max-w-4xl">
      {/* Providers section */}
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-xl font-semibold">Providers</h1>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm"
        >
          <Plus className="w-4 h-4" /> Add Provider
        </button>
      </div>

      <div className="space-y-3 mb-10">
        {providerList.map(p => {
          const result = testResults[p.id]
          return (
            <div key={p.id} className="bg-gray-900 rounded-xl border border-gray-800 p-4">
              <div className="flex items-center justify-between">
                <div>
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{p.name}</span>
                    <span className="text-xs px-2 py-0.5 bg-gray-800 text-gray-400 rounded-full">{p.type}</span>
                  </div>
                  <p className="text-sm text-gray-500 mt-0.5">
                    {p.base_url ?? p.aws_profile ?? '—'} · {p.model_name ?? 'no model'}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => testMut.mutate(p.id)}
                    disabled={testMut.isPending}
                    className="flex items-center gap-1 text-xs px-3 py-1.5 bg-gray-800 hover:bg-gray-700 rounded-lg transition-colors"
                  >
                    <RefreshCw className="w-3 h-3" /> Test
                  </button>
                  <button onClick={() => deleteMut.mutate(p.id)} className="p-1 text-gray-600 hover:text-red-400">
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
              {result && (
                <div className={`mt-2 flex items-center gap-2 text-xs ${result.ok ? 'text-green-400' : 'text-red-400'}`}>
                  {result.ok ? <Check className="w-3 h-3" /> : <X className="w-3 h-3" />}
                  {result.ok ? `Connected · ${result.models.length} models` : 'Connection failed'}
                  {result.ok && result.models.length > 0 && (
                    <span className="text-gray-500">({result.models.slice(0, 3).join(', ')})</span>
                  )}
                </div>
              )}
            </div>
          )
        })}
      </div>

      {/* MCP Servers section */}
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold">MCP Servers</h2>
        <AddMcpButton onAdded={() => qc.invalidateQueries({ queryKey: ['mcp-servers'] })} />
      </div>

      <div className="space-y-3">
        {mcpList.map(s => {
          const syncResult = syncStatus[s.id]
          const isSyncing = syncMcpMut.isPending && syncMcpMut.variables === s.id
          return (
            <div key={s.id} className="bg-gray-900 rounded-xl border border-gray-800 p-4">
              <div className="flex items-center justify-between">
                <div>
                  <div className="flex items-center gap-2">
                    <Server className="w-4 h-4 text-blue-400" />
                    <span className="font-medium">{s.name}</span>
                  </div>
                  <p className="text-sm text-gray-500 mt-0.5">{s.url}</p>
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={() => {
                      setSyncStatus(prev => {
                        const next = { ...prev }
                        delete next[s.id]
                        return next
                      })
                      syncMcpMut.mutate(s.id)
                    }}
                    disabled={isSyncing}
                    className="text-xs px-3 py-1.5 bg-gray-800 hover:bg-gray-700 rounded-lg disabled:opacity-50 transition-colors"
                  >
                    {isSyncing ? 'Syncing...' : 'Sync Tools'}
                  </button>
                  <button onClick={() => deleteMcpMut.mutate(s.id)} className="p-1 text-gray-600 hover:text-red-400 transition-colors">
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
              {syncResult && (
                <div className={`mt-2 text-xs p-2 rounded-lg font-mono whitespace-pre-wrap ${syncResult.ok ? 'bg-green-500/10 border border-green-500/20 text-green-400' : 'bg-red-500/10 border border-red-500/20 text-red-400'}`}>
                  {syncResult.ok ? '✓ ' : 'Error: '}{syncResult.message}
                </div>
              )}
            </div>
          )
        })}
      </div>

      {showCreate && (
        <ProviderForm onClose={() => { setShowCreate(false); qc.invalidateQueries({ queryKey: ['providers'] }) }} />
      )}
    </div>
  )
}

function AddMcpButton({ onAdded }: { onAdded: () => void }) {
  const [show, setShow] = useState(false)
  const [name, setName] = useState('')
  const [url, setUrl] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [pending, setPending] = useState(false)

  async function add() {
    setError(null)
    setPending(true)
    try {
      const api = await import('@/lib/api')
      await api.mcpServers.create({ name, url })
      setShow(false)
      setName('')
      setUrl('')
      onAdded()
    } catch (e: any) {
      console.error(e)
      setError(e.message ?? 'Failed to connect to MCP server during discovery.')
    } finally {
      setPending(false)
    }
  }

  if (!show) return (
    <button onClick={() => { setShow(true); setError(null) }} className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white font-medium rounded-lg text-sm">
      <Plus className="w-4 h-4" /> Add MCP Server
    </button>
  )

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <input placeholder="Name" value={name} onChange={e => setName(e.target.value)} disabled={pending}
          className="px-3 py-1.5 bg-gray-800 border border-gray-700 rounded-lg text-sm focus:outline-none focus:border-blue-500 w-36" />
        <input placeholder="http://host.docker.internal:3000/sse" value={url} onChange={e => setUrl(e.target.value)} disabled={pending}
          className="px-3 py-1.5 bg-gray-800 border border-gray-700 rounded-lg text-sm focus:outline-none focus:border-blue-500 w-80" />
        <button onClick={add} disabled={!name || !url || pending} className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm disabled:opacity-50">
          {pending ? 'Connecting...' : 'Add'}
        </button>
        <button onClick={() => setShow(false)} disabled={pending} className="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 rounded-lg text-sm">Cancel</button>
      </div>
      {error && (
        <p className="text-xs text-red-400 mt-1 max-w-lg bg-red-500/10 border border-red-500/20 rounded-lg p-2 font-mono whitespace-pre-wrap">
          Error: {error}
        </p>
      )}
    </div>
  )
}

function ProviderForm({ onClose }: { onClose: () => void }) {
  const [type, setType] = useState<'openai_compat' | 'aws_bedrock'>('openai_compat')
  const [name, setName] = useState('')
  const [baseUrl, setBaseUrl] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [awsProfile, setAwsProfile] = useState('')
  const [awsRegion, setAwsRegion] = useState('us-east-1')
  const [modelName, setModelName] = useState('')

  async function create() {
    const body = { name, type,
      ...(type === 'openai_compat' ? { base_url: baseUrl, api_key: apiKey } : {}),
      ...(type === 'aws_bedrock' ? { aws_profile: awsProfile, aws_region: awsRegion } : {}),
      model_name: modelName || undefined,
    }
    await import('@/lib/api').then(m => m.providers.create(body))
    onClose()
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-md">
        <h2 className="text-lg font-semibold mb-4">Add Provider</h2>
        <div className="space-y-3">
          <input placeholder="Display name" value={name} onChange={e => setName(e.target.value)}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500" />

          <select value={type} onChange={e => setType(e.target.value as any)}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none">
            <option value="openai_compat">OpenAI-compatible (MLX LM, Gemini, Ollama…)</option>
            <option value="aws_bedrock">AWS Bedrock (SSO)</option>
          </select>

          {type === 'openai_compat' && (<>
            <input placeholder="Base URL  e.g. http://host.docker.internal:8080" value={baseUrl} onChange={e => setBaseUrl(e.target.value)}
              className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500" />
            <input placeholder="API key (leave empty for MLX LM)" value={apiKey} onChange={e => setApiKey(e.target.value)} type="password"
              className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500" />
          </>)}

          {type === 'aws_bedrock' && (<>
            <input placeholder="AWS SSO profile name" value={awsProfile} onChange={e => setAwsProfile(e.target.value)}
              className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500" />
            <input placeholder="AWS region  e.g. us-east-1" value={awsRegion} onChange={e => setAwsRegion(e.target.value)}
              className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500" />
          </>)}

          <input placeholder="Default model name" value={modelName} onChange={e => setModelName(e.target.value)}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500" />
        </div>
        <div className="flex gap-2 mt-5">
          <button onClick={onClose} className="flex-1 px-4 py-2 bg-gray-800 hover:bg-gray-700 rounded-lg text-sm">Cancel</button>
          <button onClick={create} disabled={!name} className="flex-1 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm disabled:opacity-50">
            Save
          </button>
        </div>
      </div>
    </div>
  )
}
