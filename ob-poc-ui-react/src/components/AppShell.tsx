/**
 * App Shell - Main layout with sidebar navigation
 */

import { Outlet, NavLink } from 'react-router-dom';
import { Search, MessageSquare, Database, Settings, ChevronLeft, ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../lib/utils';

const navItems = [
  { to: '/inspector', icon: Search, label: 'Inspector' },
  { to: '/chat', icon: MessageSquare, label: 'Chat' },
  { to: '/semantic-os', icon: Database, label: 'Semantic OS' },
  { to: '/settings', icon: Settings, label: 'Settings' },
];

export function AppShell() {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="flex h-screen bg-[var(--bg-primary)]">
      {/* Sidebar */}
      <aside
        className={cn(
          'flex flex-col border-r border-[var(--border-primary)] bg-[var(--bg-secondary)] transition-all duration-200',
          collapsed ? 'w-16' : 'w-56'
        )}
      >
        {/* Logo */}
        <div className="flex h-14 items-center border-b border-[var(--border-primary)] px-4">
          {!collapsed && (
            <span className="text-lg font-semibold text-[var(--text-primary)]">
              OB-POC
            </span>
          )}
        </div>

        {/* Navigation */}
        <nav className="flex-1 space-y-1 p-2">
          {navItems.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                cn(
                  'flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors',
                  isActive
                    ? 'bg-[var(--accent-blue)]/10 text-[var(--accent-blue)]'
                    : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                )
              }
            >
              <Icon size={20} />
              {!collapsed && <span>{label}</span>}
            </NavLink>
          ))}
        </nav>

        {/* Collapse toggle */}
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex h-10 items-center justify-center border-t border-[var(--border-primary)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
        >
          {collapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
        </button>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}

export default AppShell;
