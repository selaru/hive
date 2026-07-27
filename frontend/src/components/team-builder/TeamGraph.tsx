import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  addEdge,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  type Connection,
} from '@xyflow/react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useState } from 'react'
import { ArrowLeft, Save, LayoutGrid } from 'lucide-react'
import { teams as tApi, agents as aApi } from '@/lib/api'
import AgentNode from './AgentNode'
import HierarchyEdge from './HierarchyEdge'
import '@xyflow/react/dist/style.css'

const nodeTypes  = { agentNode: AgentNode }
const edgeTypes  = { hierarchyEdge: HierarchyEdge }

interface Props {
  teamId: string
  onBack: () => void
}

export default function TeamGraph({ teamId, onBack }: Props) {
  const qc = useQueryClient()
  const { data: team }        = useQuery({ queryKey: ['team', teamId],     queryFn: () => tApi.get(teamId) })
  const { data: graph }       = useQuery({ queryKey: ['hierarchy', teamId], queryFn: () => tApi.getHierarchy(teamId) })
  const { data: agentList = []} = useQuery({ queryKey: ['agents'],           queryFn: aApi.list })

  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([])
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([])

  // Load saved hierarchy into React Flow
  useEffect(() => {
    if (!graph) return
    setNodes(graph.nodes.map(n => ({
      id:       n.id,
      type:     'agentNode',
      position: n.position,
      data:     n.data,
    })))
    setEdges(graph.edges.map(e => ({
      id:     e.id,
      source: e.source,
      target: e.target,
      type:   'hierarchyEdge',
      data:   e.data,
    })))
  }, [graph])

  const onConnect = useCallback(
    (params: Connection) =>
      setEdges(eds => addEdge({ ...params, type: 'hierarchyEdge', data: { communication_mode: 'request_response' } }, eds)),
    [setEdges],
  )

  // Drag agent from sidebar onto canvas
  const onDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
  }, [])

  const onDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    const agentId = e.dataTransfer.getData('agentId')
    const agent   = agentList.find(a => a.id === agentId)
    if (!agent) return
    const rect = (e.target as HTMLElement).closest('.react-flow')?.getBoundingClientRect()
    const x = rect ? e.clientX - rect.left : e.clientX
    const y = rect ? e.clientY - rect.top  : e.clientY
    const newNode: Node = {
      id:       `member-${Date.now()}`,
      type:     'agentNode',
      position: { x, y },
      data:     { agent_id: agentId, agent_name: agent.name, role: 'developer', member_id: null },
    }
    setNodes(nds => [...nds, newNode])
  }, [agentList, setNodes])

  const saveMut = useMutation({
    mutationFn: () => tApi.saveHierarchy(teamId, { nodes, edges }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['hierarchy', teamId] }),
  })

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <header className="flex items-center gap-4 px-4 py-3 border-b border-gray-800 shrink-0">
        <button onClick={onBack} className="p-1.5 rounded-lg hover:bg-gray-800 text-gray-400 hover:text-gray-200">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <h1 className="text-base font-semibold flex-1">{team?.name ?? 'Team'}</h1>
        <button
          onClick={() => saveMut.mutate()}
          disabled={saveMut.isPending}
          className="flex items-center gap-2 px-3 py-1.5 bg-amber-500 hover:bg-amber-400 text-gray-950 font-medium rounded-lg text-sm"
        >
          <Save className="w-3.5 h-3.5" />
          {saveMut.isPending ? 'Saving…' : 'Save'}
        </button>
      </header>

      <div className="flex-1 flex overflow-hidden">
        {/* Agent sidebar */}
        <div className="w-48 border-r border-gray-800 p-3 overflow-y-auto shrink-0">
          <p className="text-xs text-gray-500 mb-2 font-medium uppercase tracking-wide">Drag agents</p>
          {agentList.map(agent => (
            <div
              key={agent.id}
              draggable
              onDragStart={e => e.dataTransfer.setData('agentId', agent.id)}
              className="px-3 py-2 mb-1 bg-gray-800 rounded-lg text-sm cursor-grab hover:bg-gray-700 transition-colors select-none"
            >
              {agent.name}
            </div>
          ))}
        </div>

        {/* Flow canvas */}
        <div className="flex-1" onDrop={onDrop} onDragOver={onDragOver}>
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            fitView
          >
            <Background color="#374151" gap={20} />
            <Controls />
            <MiniMap nodeColor="#d97706" />
          </ReactFlow>
        </div>
      </div>
    </div>
  )
}
