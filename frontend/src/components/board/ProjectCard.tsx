import type { Project } from '@/lib/types'
import { useState } from 'react'
import clsx from 'clsx'
import { useQuery } from '@tanstack/react-query'
import { projects as pApi } from '@/lib/api'
import ProjectDetail from './ProjectDetail'

const APPROVAL_STATUSES = [
  'awaiting_approval_lead',
  'awaiting_approval_stories',
  'awaiting_final_approval',
]

const DEVELOPMENT_STATUSES = [
  'in_development',
  'in_review',
  'awaiting_final_approval',
]

export function ProjectCard({ project }: { project: Project }) {
  const [open, setOpen] = useState(false)
  const needsAction = APPROVAL_STATUSES.includes(project.status)
  const isDevPhase = DEVELOPMENT_STATUSES.includes(project.status)

  const { data: stories = [] } = useQuery({
    queryKey: ['project-stories', project.id],
    queryFn: () => pApi.stories(project.id),
    enabled: isDevPhase,
  })

  return (
    <>
      <div
        onClick={() => setOpen(true)}
        className={clsx(
          'p-3 rounded-lg border cursor-pointer transition-all',
          needsAction
            ? 'border-amber-500/50 bg-amber-500/10 hover:border-amber-400'
            : 'border-gray-800 bg-gray-900 hover:border-gray-700'
        )}
      >
        <p className="text-sm font-medium text-gray-200 line-clamp-2">{project.title}</p>
        <p className="text-xs text-gray-500 mt-1">
          {new Date(project.created_at).toLocaleDateString()}
        </p>
        {needsAction && (
          <div className="mt-2 flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
            <span className="text-xs text-amber-400">Action required</span>
          </div>
        )}

        {isDevPhase && stories.length > 0 && (
          <div className="mt-3 pt-3 border-t border-gray-800 space-y-1.5">
            <p className="text-[10px] font-semibold text-gray-500 uppercase tracking-wider">Sub-tasks</p>
            <div className="space-y-1.5 max-h-48 overflow-y-auto">
              {stories.map(story => (
                <div
                  key={story.id}
                  className={clsx(
                    "px-2 py-1.5 rounded text-[11px] border flex flex-col gap-0.5",
                    story.status === 'in_progress' && "bg-amber-500/5 border-amber-500/25 text-amber-200",
                    story.status === 'todo' && "bg-gray-800/40 border-gray-800 text-gray-400",
                    story.status === 'done' && "bg-green-500/5 border-green-500/25 text-green-200",
                    story.status === 'in_review' && "bg-purple-500/5 border-purple-500/25 text-purple-200",
                    story.status === 'blocked' && "bg-red-500/5 border-red-500/25 text-red-200",
                  )}
                >
                  <div className="flex items-center justify-between gap-1">
                    <span className="font-medium truncate max-w-[130px]" title={story.title}>{story.title}</span>
                    <span className={clsx(
                      "px-1 py-0.2 rounded-[3px] text-[8px] font-bold uppercase",
                      story.status === 'in_progress' && "bg-amber-500/10 text-amber-400",
                      story.status === 'todo' && "bg-gray-800 text-gray-500",
                      story.status === 'done' && "bg-green-500/10 text-green-400",
                      story.status === 'in_review' && "bg-purple-500/10 text-purple-400",
                      story.status === 'blocked' && "bg-red-500/10 text-red-400",
                    )}>
                      {story.status}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {open && <ProjectDetail projectId={project.id} onClose={() => setOpen(false)} />}
    </>
  )
}
