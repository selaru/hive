import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useState, useRef, useEffect } from 'react'
import { X, Check, XCircle, Trash2 } from 'lucide-react'
import { projects as pApi } from '@/lib/api'
import { useProjectEvents } from '@/lib/ws'
import type { AgentMessage, WsEvent } from '@/lib/types'

interface Props {
  projectId: string
  onClose:   () => void
}

export default function ProjectDetail({ projectId, onClose }: Props) {
  const qc = useQueryClient()
  const [liveTokens, setLiveTokens] = useState<Record<string, string>>({})
  const messagesEndRef = useRef<HTMLDivElement>(null)

  const { data, refetch } = useQuery({
    queryKey: ['project', projectId],
    queryFn: () => pApi.get(projectId),
  })
  const { data: messages = [], refetch: refetchMessages } = useQuery({
    queryKey: ['project-messages', projectId],
    queryFn: () => pApi.messages(projectId),
  })
  const { data: stories = [] } = useQuery({
    queryKey: ['project-stories', projectId],
    queryFn: () => pApi.stories(projectId),
  })

  const approveMut = useMutation({
    mutationFn: (comment: string) => pApi.approve(projectId, comment),
    onSuccess: () => { refetch(); qc.invalidateQueries({ queryKey: ['projects'] }) },
  })
  const rejectMut = useMutation({
    mutationFn: (comment: string) => pApi.reject(projectId, comment),
    onSuccess: () => { refetch(); qc.invalidateQueries({ queryKey: ['projects'] }) },
  })

  const deleteStoryMut = useMutation({
    mutationFn: (storyId: string) => pApi.deleteStory(projectId, storyId),
    onSuccess: () => {
      refetch()
      qc.invalidateQueries({ queryKey: ['project-stories', projectId] })
      qc.invalidateQueries({ queryKey: ['projects'] })
    },
  })

  const deleteProjectMut = useMutation({
    mutationFn: () => pApi.delete(projectId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['projects'] })
      onClose()
    },
  })

  // Live events
  useProjectEvents(projectId, (event: WsEvent) => {
    if (event.type === 'agent_token') {
      const { role, token } = event.data
      setLiveTokens(prev => ({ ...prev, [role]: (prev[role] ?? '') + token }))
    }
    if (event.type === 'message_created') {
      setLiveTokens(prev => {
        const next = { ...prev }
        delete next[event.data.message.from_role ?? '']
        return next
      })
      refetchMessages()
    }
    if (event.type === 'approval_required' || event.type === 'project_status_changed') {
      refetch()
    }
    if (event.type === 'story_updated' || event.type === 'story_deleted') {
      qc.invalidateQueries({ queryKey: ['project-stories', projectId] })
      refetch()
    }
  })

  // Scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages.length, Object.keys(liveTokens).length])

  const project = data?.project
  const approval = data?.pending_approval
  const [comment, setComment] = useState('')

  if (!project) return null

  return (
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4">
      <div className="bg-gray-900 rounded-xl border border-gray-700 w-full max-w-5xl h-[85vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-800 shrink-0">
          <div>
            <h2 className="font-semibold">{project.title}</h2>
            <span className="text-xs text-gray-500">{project.status}</span>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => {
                if (confirm('Are you sure you want to delete this project? This will stop all active agent work and delete all associated stories and data.')) {
                  deleteProjectMut.mutate()
                }
              }}
              title="Delete Project"
              className="p-1.5 text-gray-500 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-all"
            >
              <Trash2 className="w-4 h-4" />
            </button>
            <button onClick={onClose} className="p-1 text-gray-500 hover:text-gray-300">
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        <div className="flex-1 flex overflow-hidden">
          {/* Message stream (left) */}
          <div className="flex-1 overflow-y-auto p-4 space-y-3">
            {messages.map(msg => (
              <MessageBubble key={msg.id} msg={msg} />
            ))}
            {/* Live streaming tokens */}
            {Object.entries(liveTokens).map(([role, text]) => text && (
              <div key={role} className="flex gap-2">
                <div className="shrink-0 w-6 h-6 rounded-full bg-amber-500/20 flex items-center justify-center">
                  <span className="text-xs text-amber-400">{role[0].toUpperCase()}</span>
                </div>
                <div className="flex-1 bg-gray-800 rounded-xl p-3 text-sm text-gray-300">
                  <p className="text-xs text-amber-400 mb-1">{role} <span className="animate-pulse">▊</span></p>
                  <p className="whitespace-pre-wrap">{text}</p>
                </div>
              </div>
            ))}
            <div ref={messagesEndRef} />
          </div>

          {/* Right panel: approval + stories */}
          <div className="w-72 border-l border-gray-800 flex flex-col shrink-0">
            {/* Approval panel */}
            {approval && (
              <div className="p-4 border-b border-gray-800">
                <p className="text-xs font-medium text-amber-400 mb-2">
                  Awaiting your approval
                </p>
                <div className="bg-gray-950 rounded-lg p-3 text-xs text-gray-300 max-h-40 overflow-y-auto mb-3">
                  <pre className="whitespace-pre-wrap">
                    {JSON.stringify(approval.payload, null, 2)}
                  </pre>
                </div>
                <textarea
                  placeholder="Comment (optional)"
                  value={comment}
                  onChange={e => setComment(e.target.value)}
                  rows={2}
                  className="w-full px-2 py-1.5 bg-gray-800 rounded-lg text-xs border border-gray-700 focus:outline-none resize-none mb-2"
                />
                <div className="flex gap-2">
                  <button
                    onClick={() => { approveMut.mutate(comment); setComment('') }}
                    className="flex-1 flex items-center justify-center gap-1 py-1.5 bg-green-600 hover:bg-green-500 rounded-lg text-xs font-medium"
                  >
                    <Check className="w-3 h-3" /> Approve
                  </button>
                  <button
                    onClick={() => { rejectMut.mutate(comment); setComment('') }}
                    className="flex-1 flex items-center justify-center gap-1 py-1.5 bg-red-700 hover:bg-red-600 rounded-lg text-xs font-medium"
                  >
                    <XCircle className="w-3 h-3" /> Reject
                  </button>
                </div>
              </div>
            )}

            {/* Stories */}
            <div className="flex-1 overflow-y-auto p-4">
              {stories.length > 0 && (
                <>
                  <p className="text-xs font-medium text-gray-400 mb-2">Stories</p>
                  <div className="space-y-2">
                    {stories.map(s => (
                      <div key={s.id} className="bg-gray-800 rounded-lg p-2 flex items-start justify-between gap-2 group">
                        <div className="flex-1 min-w-0">
                          <p className="text-xs font-medium text-gray-200 line-clamp-2">{s.title}</p>
                          <div className="flex items-center gap-1 mt-1">
                            <span className={`w-1.5 h-1.5 rounded-full ${
                              s.status === 'done' ? 'bg-green-400' :
                              s.status === 'in_progress' ? 'bg-amber-400 animate-pulse' :
                              'bg-gray-600'
                            }`} />
                            <span className="text-xs text-gray-500">{s.status}</span>
                          </div>
                        </div>
                        <button
                          onClick={() => {
                            if (confirm('Are you sure you want to delete this story? This will interrupt any active agent work on it.')) {
                              deleteStoryMut.mutate(s.id)
                            }
                          }}
                          className="text-gray-500 hover:text-red-400 p-1 rounded transition shrink-0 self-center"
                          title="Delete Story"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    ))}
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function MessageBubble({ msg }: { msg: AgentMessage }) {
  const isUser = msg.from_role === 'user'
  return (
    <div className="flex gap-2">
      <div className="shrink-0 w-6 h-6 rounded-full bg-gray-800 flex items-center justify-center">
        <span className="text-xs text-gray-400">{(msg.from_role ?? '?')[0].toUpperCase()}</span>
      </div>
      <div className="flex-1 bg-gray-800 rounded-xl p-3">
        <p className="text-xs text-gray-500 mb-1">{msg.from_role}</p>
        <p className="text-sm text-gray-300 whitespace-pre-wrap">{msg.content}</p>
      </div>
    </div>
  )
}
