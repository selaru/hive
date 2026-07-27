import { Handle, Position, type NodeProps } from '@xyflow/react'
import { useState } from 'react'
import { Bot } from 'lucide-react'

const ROLES = ['lead', 'architect', 'developer', 'reviewer']

export default function AgentNode({ data, selected }: NodeProps) {
  const [role, setRole] = useState(data.role as string)

  return (
    <div className={`
      min-w-36 bg-gray-900 rounded-xl border p-3 shadow-lg transition-colors
      ${selected ? 'border-amber-500' : 'border-gray-700'}
    `}>
      <Handle type="target" position={Position.Top}    className="!bg-amber-500 !border-amber-700" />

      <div className="flex items-center gap-2 mb-2">
        <Bot className="w-4 h-4 text-amber-400 shrink-0" />
        <span className="text-sm font-medium truncate">{data.agent_name as string}</span>
      </div>

      <select
        value={role}
        onChange={e => { setRole(e.target.value); data.role = e.target.value }}
        onClick={e => e.stopPropagation()}
        className="w-full text-xs px-2 py-1 bg-gray-800 rounded border border-gray-700 focus:outline-none focus:border-amber-500 text-gray-300"
      >
        {ROLES.map(r => <option key={r} value={r}>{r}</option>)}
      </select>

      <Handle type="source" position={Position.Bottom} className="!bg-amber-500 !border-amber-700" />
    </div>
  )
}
