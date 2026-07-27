import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { teams as tApi, projects as pApi } from '@/lib/api'

interface Props {
  onClose: () => void
}

export function SubmitProjectForm({ onClose }: Props) {
  const qc = useQueryClient()
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [criteria, setCriteria] = useState('')
  const [teamId, setTeamId] = useState('')

  const { data: teamList = [] } = useQuery({ queryKey: ['teams'], queryFn: tApi.list })

  const createMut = useMutation({
    mutationFn: (body: unknown) => pApi.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['projects'] })
      onClose()
    },
  })

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-lg">
        <h2 className="text-lg font-semibold mb-4">Submit Project</h2>

        <div className="space-y-3">
          <input
            placeholder="Project title *"
            value={title}
            onChange={e => setTitle(e.target.value)}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500"
          />

          <textarea
            placeholder="Describe what you want built…"
            value={description}
            onChange={e => setDescription(e.target.value)}
            rows={4}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500 resize-none"
          />

          <textarea
            placeholder="Acceptance criteria (optional)…"
            value={criteria}
            onChange={e => setCriteria(e.target.value)}
            rows={3}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none focus:border-amber-500 resize-none"
          />

          <select
            value={teamId}
            onChange={e => setTeamId(e.target.value)}
            className="w-full px-3 py-2 bg-gray-800 rounded-lg text-sm border border-gray-700 focus:outline-none"
          >
            <option value="">Select team *</option>
            {teamList.map(t => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </div>

        <div className="flex gap-2 mt-5">
          <button onClick={onClose} className="flex-1 px-4 py-2 bg-gray-800 hover:bg-gray-700 rounded-lg text-sm">
            Cancel
          </button>
          <button
            onClick={() => createMut.mutate({ title, description, acceptance_criteria: criteria, team_id: teamId })}
            disabled={!title || !teamId || createMut.isPending}
            className="flex-1 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm disabled:opacity-50"
          >
            {createMut.isPending ? 'Submitting…' : 'Submit'}
          </button>
        </div>
      </div>
    </div>
  )
}
