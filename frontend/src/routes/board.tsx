import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { Plus, RefreshCw } from 'lucide-react'
import { projects as api } from '@/lib/api'
import { useBoardEvents } from '@/lib/ws'
import type { Project, ProjectStatus } from '@/lib/types'
import { SubmitProjectForm } from '@/components/board/SubmitProjectForm'
import { KanbanColumn } from '@/components/board/KanbanColumn'

const COLUMNS: { status: ProjectStatus; label: string; highlight?: boolean }[] = [
  { status: 'submitted',                 label: 'Submitted' },
  { status: 'lead_review',              label: 'Lead Review' },
  { status: 'awaiting_approval_lead',   label: 'Awaiting Approval', highlight: true },
  { status: 'architect_breakdown',      label: 'Architect' },
  { status: 'awaiting_approval_stories',label: 'Approve Stories',   highlight: true },
  { status: 'in_development',           label: 'In Development' },
  { status: 'in_review',               label: 'In Review' },
  { status: 'awaiting_final_approval',  label: 'Final Approval',    highlight: true },
  { status: 'done',                     label: 'Done' },
]

export default function BoardPage() {
  const qc = useQueryClient()
  const [showForm, setShowForm] = useState(false)
  const { data: all = [], refetch } = useQuery({ queryKey: ['projects'], queryFn: api.list })

  // Live updates from WebSocket
  useBoardEvents((event) => {
    if (event.type === 'project_status_changed') {
      qc.invalidateQueries({ queryKey: ['projects'] })
    }
    if (event.type === 'story_updated' || event.type === 'story_deleted') {
      qc.invalidateQueries({ queryKey: ['project-stories'] })
    }
  })

  const byStatus = (status: ProjectStatus) => all.filter(p => p.status === status)

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <header className="flex items-center justify-between px-6 py-4 border-b border-gray-800 shrink-0">
        <h1 className="text-xl font-semibold text-gray-100">Board</h1>
        <div className="flex gap-2">
          <button
            onClick={() => refetch()}
            className="p-2 rounded-lg text-gray-400 hover:text-gray-200 hover:bg-gray-800 transition-colors"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
          <button
            onClick={() => setShowForm(true)}
            className="flex items-center gap-2 px-4 py-2 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg transition-colors text-sm"
          >
            <Plus className="w-4 h-4" />
            Submit Project
          </button>
        </div>
      </header>

      {/* Kanban */}
      <div className="flex-1 overflow-x-auto overflow-y-hidden p-4">
        <div className="flex gap-3 h-full min-w-max">
          {COLUMNS.map(col => (
            <KanbanColumn
              key={col.status}
              label={col.label}
              projects={byStatus(col.status)}
              highlight={col.highlight}
            />
          ))}
        </div>
      </div>

      {/* Submit form modal */}
      {showForm && (
        <SubmitProjectForm onClose={() => setShowForm(false)} />
      )}
    </div>
  )
}
