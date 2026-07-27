import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { Plus, Trash2, Network, Edit3 } from 'lucide-react'
import { teams as api } from '@/lib/api'
import TeamGraph from '@/components/team-builder/TeamGraph'

export default function TeamsPage() {
  const qc = useQueryClient()
  const [selected, setSelected] = useState<string | null>(null)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editName, setEditName] = useState('')
  const [editDesc, setEditDesc] = useState('')

  const { data: teamList = [] } = useQuery({ queryKey: ['teams'], queryFn: api.list })

  const createMut = useMutation({
    mutationFn: () => api.create({ name: 'New Team' }),
    onSuccess: (team) => {
      qc.invalidateQueries({ queryKey: ['teams'] })
      setSelected(team.id)
    },
  })

  const updateMut = useMutation({
    mutationFn: ({ id, name, description }: { id: string, name: string, description: string }) =>
      api.update(id, { name, description }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['teams'] })
      setEditingId(null)
    },
  })

  const deleteMut = useMutation({
    mutationFn: (id: string) => api.delete(id),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['teams'] }); setSelected(null) },
  })

  if (selected) {
    return (
      <TeamGraph
        teamId={selected}
        onBack={() => setSelected(null)}
      />
    )
  }

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">Teams</h1>
        <button
          onClick={() => createMut.mutate()}
          className="flex items-center gap-2 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm"
        >
          <Plus className="w-4 h-4" /> New Team
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {teamList.map(team => {
          const isEditing = editingId === team.id

          if (isEditing) {
            return (
              <div key={team.id} className="bg-gray-900 rounded-xl border border-amber-500/50 p-4" onClick={e => e.stopPropagation()}>
                <div className="space-y-3">
                  <div>
                    <label className="text-xs font-medium text-gray-400">Team Name</label>
                    <input
                      type="text"
                      value={editName}
                      onChange={e => setEditName(e.target.value)}
                      className="w-full px-2 py-1.5 bg-gray-800 rounded-lg text-xs text-gray-200 border border-gray-700 focus:outline-none focus:border-amber-500 mt-1"
                    />
                  </div>
                  <div>
                    <label className="text-xs font-medium text-gray-400">Description</label>
                    <textarea
                      value={editDesc}
                      onChange={e => setEditDesc(e.target.value)}
                      rows={2}
                      className="w-full px-2 py-1.5 bg-gray-800 rounded-lg text-xs text-gray-200 border border-gray-700 focus:outline-none focus:border-amber-500 mt-1 resize-none"
                    />
                  </div>
                  <div className="flex justify-end gap-2 mt-2">
                    <button
                      onClick={() => setEditingId(null)}
                      className="px-3 py-1 bg-gray-800 hover:bg-gray-700 text-gray-300 rounded text-xs font-medium transition-colors"
                    >
                      Cancel
                    </button>
                    <button
                      onClick={() => updateMut.mutate({ id: team.id, name: editName, description: editDesc })}
                      className="px-3 py-1 bg-amber-500 hover:bg-amber-400 text-gray-950 rounded text-xs font-medium transition-colors"
                    >
                      Save
                    </button>
                  </div>
                </div>
              </div>
            )
          }

          return (
            <div key={team.id} className="bg-gray-900 rounded-xl border border-gray-800 p-4 cursor-pointer hover:border-amber-500/50 transition-colors group"
              onClick={() => setSelected(team.id)}>
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-2">
                  <Network className="w-4 h-4 text-amber-400" />
                  <span className="font-medium">{team.name}</span>
                </div>
                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    onClick={e => {
                      e.stopPropagation()
                      setEditingId(team.id)
                      setEditName(team.name)
                      setEditDesc(team.description ?? '')
                    }}
                    className="p-1 text-gray-600 hover:text-amber-400 transition-colors"
                    title="Edit Team Name"
                  >
                    <Edit3 className="w-4 h-4" />
                  </button>
                  <button
                    onClick={e => {
                      e.stopPropagation()
                      if (confirm(`Are you sure you want to delete "${team.name}"? This will delete all projects associated with it.`)) {
                        deleteMut.mutate(team.id)
                      }
                    }}
                    className="p-1 text-gray-600 hover:text-red-400 transition-colors"
                    title="Delete Team"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
              {team.description && (
                <p className="text-sm text-gray-400 mt-2">{team.description}</p>
              )}
              <p className="text-xs text-gray-600 mt-3">Click to open hierarchy editor →</p>
            </div>
          )
        })}
      </div>
    </div>
  )
}
