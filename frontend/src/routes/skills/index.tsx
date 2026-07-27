import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { BookOpen, Download, Trash2, Eye } from 'lucide-react'
import { skills as api } from '@/lib/api'
import type { Skill } from '@/lib/types'

export default function SkillsPage() {
  const qc = useQueryClient()
  const [showImport, setShowImport] = useState(false)
  const [preview, setPreview] = useState<Skill | null>(null)
  const { data: skillList = [] } = useQuery({ queryKey: ['skills'], queryFn: api.list })

  const deleteMut = useMutation({
    mutationFn: (id: string) => api.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['skills'] }),
  })

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">Skills</h1>
        <button
          onClick={() => setShowImport(true)}
          className="flex items-center gap-2 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm"
        >
          <Download className="w-4 h-4" /> Import Skill
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {skillList.map(skill => (
          <div key={skill.id} className="bg-gray-900 rounded-xl border border-gray-800 p-4">
            <div className="flex items-start justify-between mb-2">
              <div className="flex items-center gap-2">
                <BookOpen className="w-4 h-4 text-amber-400" />
                <span className="font-medium font-mono text-sm">{skill.name}</span>
              </div>
              <div className="flex gap-1">
                <button onClick={() => setPreview(skill)} className="p-1 text-gray-600 hover:text-gray-300">
                  <Eye className="w-4 h-4" />
                </button>
                <button onClick={() => deleteMut.mutate(skill.id)} className="p-1 text-gray-600 hover:text-red-400">
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            </div>
            <p className="text-sm text-gray-400 mb-3 line-clamp-2">{skill.description}</p>
            {skill.tool_hints.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {skill.tool_hints.map(h => (
                  <span key={h} className="text-xs px-2 py-0.5 bg-gray-800 text-gray-400 rounded-full">{h}</span>
                ))}
              </div>
            )}
            {skill.source_url && (
              <p className="text-xs text-gray-600 mt-2 truncate">{skill.source_url}</p>
            )}
          </div>
        ))}
      </div>

      {showImport && (
        <ImportModal onClose={() => { setShowImport(false); qc.invalidateQueries({ queryKey: ['skills'] }) }} />
      )}

      {preview && (
        <SkillPreview skill={preview} onClose={() => setPreview(null)} />
      )}
    </div>
  )
}

function ImportModal({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<'github' | 'paste'>('github')
  const [url, setUrl] = useState('')
  const [content, setContent] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  async function doImport() {
    setLoading(true); setError('')
    try {
      const body = tab === 'github'
        ? { source_type: 'github', url }
        : { source_type: 'local', content }
      await import('@/lib/api').then(m => m.skills.import(body))
      onClose()
    } catch (e: any) {
      setError(e.message)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-lg">
        <h2 className="text-lg font-semibold mb-4">Import Skill</h2>

        <div className="flex gap-2 mb-4">
          {(['github', 'paste'] as const).map(t => (
            <button key={t} onClick={() => setTab(t)}
              className={`px-3 py-1.5 rounded-lg text-sm transition-colors ${tab === t ? 'bg-amber-500 text-gray-950' : 'bg-gray-800 text-gray-400 hover:text-gray-200'}`}>
              {t === 'github' ? 'GitHub / skills.sh' : 'Paste SKILL.md'}
            </button>
          ))}
        </div>

        {tab === 'github' ? (
          <input
            placeholder="owner/repo  or  https://github.com/owner/repo"
            value={url}
            onChange={e => setUrl(e.target.value)}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500"
          />
        ) : (
          <textarea
            placeholder="Paste your SKILL.md content here..."
            value={content}
            onChange={e => setContent(e.target.value)}
            rows={10}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500 font-mono resize-none"
          />
        )}

        {error && <p className="text-red-400 text-sm mt-2">{error}</p>}

        <div className="flex gap-2 mt-5">
          <button onClick={onClose} className="flex-1 px-4 py-2 bg-gray-800 hover:bg-gray-700 rounded-lg text-sm">Cancel</button>
          <button
            onClick={doImport}
            disabled={loading || (tab === 'github' ? !url : !content)}
            className="flex-1 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm disabled:opacity-50"
          >
            {loading ? 'Importing…' : 'Import'}
          </button>
        </div>
      </div>
    </div>
  )
}

function SkillPreview({ skill, onClose }: { skill: Skill; onClose: () => void }) {
  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-2xl max-h-[80vh] flex flex-col">
        <div className="flex items-center justify-between mb-4">
          <h2 className="font-mono font-semibold">{skill.name}</h2>
          <button onClick={onClose} className="text-gray-500 hover:text-gray-300">✕</button>
        </div>
        <p className="text-sm text-gray-400 mb-3">{skill.description}</p>
        <pre className="flex-1 overflow-auto text-xs text-gray-300 bg-gray-950 rounded-lg p-4 font-mono whitespace-pre-wrap">
          {skill.system_prompt}
        </pre>
      </div>
    </div>
  )
}
