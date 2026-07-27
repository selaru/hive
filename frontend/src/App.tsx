import { Link, useLocation } from '@tanstack/react-router'
import { LayoutGrid, Bot, Users, BookOpen, Settings, Hexagon } from 'lucide-react'
import BoardPage from './routes/board'
import AgentsPage from './routes/agents/index'
import TeamsPage from './routes/teams/index'
import SkillsPage from './routes/skills/index'
import ProvidersPage from './routes/settings/providers'
import { useState } from 'react'
import clsx from 'clsx'

type Route = 'board' | 'agents' | 'teams' | 'skills' | 'settings'

const NAV = [
  { id: 'board',    label: 'Board',     icon: LayoutGrid },
  { id: 'agents',   label: 'Agents',    icon: Bot },
  { id: 'teams',    label: 'Teams',     icon: Users },
  { id: 'skills',   label: 'Skills',    icon: BookOpen },
  { id: 'settings', label: 'Settings',  icon: Settings },
] as const

export default function App() {
  const [route, setRoute] = useState<Route>('board')

  const Page = {
    board:    BoardPage,
    agents:   AgentsPage,
    teams:    TeamsPage,
    skills:   SkillsPage,
    settings: ProvidersPage,
  }[route]

  return (
    <div className="flex h-screen overflow-hidden">
      {/* Sidebar */}
      <nav className="w-16 bg-gray-900 border-r border-gray-800 flex flex-col items-center py-4 gap-1 shrink-0">
        <div className="mb-6">
          <Hexagon className="w-8 h-8 text-amber-400" />
        </div>
        {NAV.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setRoute(id)}
            title={label}
            className={clsx(
              'w-12 h-12 rounded-xl flex items-center justify-center transition-colors',
              route === id
                ? 'bg-amber-500/20 text-amber-400'
                : 'text-gray-500 hover:text-gray-300 hover:bg-gray-800'
            )}
          >
            <Icon className="w-5 h-5" />
          </button>
        ))}
      </nav>

      {/* Main content */}
      <main className="flex-1 overflow-auto">
        <Page />
      </main>
    </div>
  )
}
