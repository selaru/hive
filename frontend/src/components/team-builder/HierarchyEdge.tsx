import { BaseEdge, EdgeLabelRenderer, getStraightPath, type EdgeProps } from '@xyflow/react'

const MODES = ['request_response', 'broadcast', 'parallel']

export default function HierarchyEdge({
  id, sourceX, sourceY, targetX, targetY, data,
}: EdgeProps) {
  const [edgePath, labelX, labelY] = getStraightPath({ sourceX, sourceY, targetX, targetY })

  return (
    <>
      <BaseEdge id={id} path={edgePath} style={{ stroke: '#d97706', strokeWidth: 1.5 }} markerEnd="url(#arrow)" />
      <EdgeLabelRenderer>
        <div
          style={{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)` }}
          className="absolute nodrag nopan"
        >
          <select
            value={(data?.communication_mode as string) ?? 'request_response'}
            onChange={e => { if (data) data.communication_mode = e.target.value }}
            className="text-xs px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-gray-400 focus:outline-none focus:border-amber-500"
          >
            {MODES.map(m => <option key={m} value={m}>{m}</option>)}
          </select>
        </div>
      </EdgeLabelRenderer>
    </>
  )
}
