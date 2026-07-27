import { useEffect, useRef, useCallback } from 'react'
import type { WsEvent } from './types'

const WS_BASE = import.meta.env.VITE_WS_URL ?? 'ws://localhost/ws'

/**
 * Subscribe to real-time events for a specific project.
 * Re-connects automatically on disconnect.
 */
export function useProjectEvents(
  projectId: string | null,
  onEvent: (event: WsEvent) => void,
) {
  const ws  = useRef<WebSocket | null>(null)
  const cbRef = useRef(onEvent)
  cbRef.current = onEvent

  useEffect(() => {
    if (!projectId) return

    let active = true
    let retryTimeout: ReturnType<typeof setTimeout>

    function connect() {
      if (!active) return
      const socket = new WebSocket(`${WS_BASE}/projects/${projectId}`)
      ws.current = socket

      socket.onmessage = (e) => {
        try {
          const event: WsEvent = JSON.parse(e.data)
          cbRef.current(event)
        } catch {
          // ignore malformed events
        }
      }

      socket.onclose = () => {
        if (active) {
          retryTimeout = setTimeout(connect, 2000)
        }
      }

      socket.onerror = () => {
        socket.close()
      }
    }

    connect()

    return () => {
      active = false
      clearTimeout(retryTimeout)
      ws.current?.close()
    }
  }, [projectId])
}

/**
 * Subscribe to board-level events (project status changes, new projects).
 */
export function useBoardEvents(onEvent: (event: WsEvent) => void) {
  const cbRef = useRef(onEvent)
  cbRef.current = onEvent

  useEffect(() => {
    let active = true
    let retryTimeout: ReturnType<typeof setTimeout>

    function connect() {
      if (!active) return
      const socket = new WebSocket(`${WS_BASE}/board`)

      socket.onmessage = (e) => {
        try {
          const event: WsEvent = JSON.parse(e.data)
          cbRef.current(event)
        } catch {}
      }

      socket.onclose = () => {
        if (active) retryTimeout = setTimeout(connect, 2000)
      }
      socket.onerror = () => socket.close()
    }

    connect()
    return () => {
      active = false
      clearTimeout(retryTimeout)
    }
  }, [])
}
