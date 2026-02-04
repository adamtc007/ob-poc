/**
 * Settings Page
 */

import { Moon, Sun, Monitor } from 'lucide-react';
import { cn } from '../../lib/utils';
import { useState } from 'react';

type Theme = 'light' | 'dark' | 'system';

export function SettingsPage() {
  const [theme, setTheme] = useState<Theme>('dark');

  const themeOptions = [
    { value: 'light' as const, icon: Sun, label: 'Light' },
    { value: 'dark' as const, icon: Moon, label: 'Dark' },
    { value: 'system' as const, icon: Monitor, label: 'System' },
  ];

  return (
    <div className="h-full overflow-auto">
      <div className="mx-auto max-w-2xl p-6">
        <h1 className="text-2xl font-semibold text-[var(--text-primary)]">
          Settings
        </h1>

        {/* Theme Section */}
        <section className="mt-8">
          <h2 className="text-lg font-medium text-[var(--text-primary)]">
            Appearance
          </h2>
          <p className="mt-1 text-sm text-[var(--text-secondary)]">
            Customize how the application looks.
          </p>

          <div className="mt-4 flex gap-3">
            {themeOptions.map(({ value, icon: Icon, label }) => (
              <button
                key={value}
                onClick={() => setTheme(value)}
                className={cn(
                  'flex flex-col items-center gap-2 rounded-lg border p-4 transition-colors',
                  theme === value
                    ? 'border-[var(--accent-blue)] bg-[var(--accent-blue)]/10'
                    : 'border-[var(--border-primary)] bg-[var(--bg-secondary)] hover:border-[var(--border-secondary)]'
                )}
              >
                <Icon
                  size={24}
                  className={
                    theme === value
                      ? 'text-[var(--accent-blue)]'
                      : 'text-[var(--text-secondary)]'
                  }
                />
                <span
                  className={cn(
                    'text-sm',
                    theme === value
                      ? 'text-[var(--accent-blue)]'
                      : 'text-[var(--text-secondary)]'
                  )}
                >
                  {label}
                </span>
              </button>
            ))}
          </div>
        </section>

        {/* Inspector Defaults Section */}
        <section className="mt-8">
          <h2 className="text-lg font-medium text-[var(--text-primary)]">
            Inspector Defaults
          </h2>
          <p className="mt-1 text-sm text-[var(--text-secondary)]">
            Default settings for the projection inspector.
          </p>

          <div className="mt-4 space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-primary)]">
                Default LOD Level
              </label>
              <select className="mt-1 w-full rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)] px-3 py-2 text-[var(--text-primary)]">
                <option value="0">0 - Minimal</option>
                <option value="1">1 - Summary</option>
                <option value="2">2 - Standard</option>
                <option value="3">3 - Full Detail</option>
              </select>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--text-primary)]">
                Default Max Depth
              </label>
              <input
                type="range"
                min="1"
                max="10"
                defaultValue="3"
                className="mt-1 w-full"
              />
              <div className="mt-1 flex justify-between text-xs text-[var(--text-muted)]">
                <span>1</span>
                <span>10</span>
              </div>
            </div>
          </div>
        </section>

        {/* Keyboard Shortcuts Section */}
        <section className="mt-8">
          <h2 className="text-lg font-medium text-[var(--text-primary)]">
            Keyboard Shortcuts
          </h2>
          <p className="mt-1 text-sm text-[var(--text-secondary)]">
            Quick reference for keyboard navigation.
          </p>

          <div className="mt-4 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)]">
            <table className="w-full text-sm">
              <tbody className="divide-y divide-[var(--border-primary)]">
                {[
                  { key: '↑ / ↓', action: 'Navigate tree' },
                  { key: '← / →', action: 'Collapse / Expand node' },
                  { key: 'Enter', action: 'Focus selected node' },
                  { key: 'Backspace', action: 'Go back' },
                  { key: '/', action: 'Open search' },
                  { key: '1-4', action: 'Set LOD level' },
                ].map(({ key, action }) => (
                  <tr key={key}>
                    <td className="px-4 py-2">
                      <kbd className="rounded bg-[var(--bg-tertiary)] px-2 py-0.5 font-mono text-xs">
                        {key}
                      </kbd>
                    </td>
                    <td className="px-4 py-2 text-[var(--text-secondary)]">
                      {action}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        {/* API Configuration */}
        <section className="mt-8">
          <h2 className="text-lg font-medium text-[var(--text-primary)]">
            API Configuration
          </h2>
          <p className="mt-1 text-sm text-[var(--text-secondary)]">
            Backend connection settings.
          </p>

          <div className="mt-4">
            <label className="block text-sm font-medium text-[var(--text-primary)]">
              API Base URL
            </label>
            <input
              type="text"
              defaultValue="/api"
              className="mt-1 w-full rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)] px-3 py-2 text-[var(--text-primary)]"
            />
            <p className="mt-1 text-xs text-[var(--text-muted)]">
              Leave as /api for same-origin requests via proxy.
            </p>
          </div>
        </section>
      </div>
    </div>
  );
}

export default SettingsPage;
