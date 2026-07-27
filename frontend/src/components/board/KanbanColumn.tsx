import type { Project, ProjectStatus } from '@/lib/types'
import { clsx } from 'clsx'
import { ProjectCard } from './ProjectCard'

interface Props {
  label:     string
  projects:  Project[]
  highlight?: boolean
}

export function KanbanColumn({ label, projects, highlight }: Props) {
  return (
    <div className={clsx(
      'w-56 flex flex-col rounded-xl border',
      highlight
        ? 'border-amber-500/40 bg-amber-500/5'
        : 'border-gray-800 bg-gray-900/50'
    )}>
      <div className={clsx(
        'px-3 py-2 flex items-center justify-between border-b rounded-t-xl',
        highlight ? 'border-amber-500/30' : 'border-gray-800'
      )}>
        <span className={clsx(
          'text-xs font-medium',
          highlight ? 'text-amber-400' : 'text-gray-400'
        )}>
          {label}
        </span>
        {projects.length > 0 && (
          <span className={clsx(
            'text-xs px-1.5 py-0.5 rounded-full',
            highlight ? 'bg-amber-500/20 text-amber-400' : 'bg-gray-800 text-gray-500'
          )}>
            {projects.length}
          </span>
        )}
      </div>
      <div className="flex-1 p-2 space-y-2 overflow-y-auto">
        {projects.map(p => <ProjectCard key={p.id} project={p} />)}
      </div>
    </div>
  )
}
