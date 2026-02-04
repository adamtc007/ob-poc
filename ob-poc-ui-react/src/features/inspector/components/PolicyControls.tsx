/**
 * PolicyControls - LOD, depth, and chamber filter controls
 */

import { useInspectorStore } from '../../../stores/inspector';
import type { LodTier } from '../../../types/projection';
import { cn } from '../../../lib/utils';

const LOD_LABELS: Record<LodTier, string> = {
  0: 'Minimal',
  1: 'Summary',
  2: 'Standard',
  3: 'Full',
};

const CHAMBERS = [
  { id: 'cbu', label: 'CBU' },
  { id: 'entity', label: 'Entity' },
  { id: 'trading', label: 'Trading' },
  { id: 'kyc', label: 'KYC' },
  { id: 'custody', label: 'Custody' },
];

interface PolicyControlsProps {
  className?: string;
  onRegenerate?: () => void;
}

export function PolicyControls({ className, onRegenerate }: PolicyControlsProps) {
  const { policy, setLod, setMaxDepth, toggleChamber } = useInspectorStore();

  return (
    <div className={cn('space-y-4', className)}>
      {/* LOD Selector */}
      <div>
        <label className="block text-xs font-medium text-[var(--text-secondary)] mb-2">
          Level of Detail
        </label>
        <div className="flex gap-1">
          {([0, 1, 2, 3] as LodTier[]).map((lod) => (
            <button
              key={lod}
              onClick={() => setLod(lod)}
              className={cn(
                'flex-1 rounded px-2 py-1.5 text-xs transition-colors',
                policy.lod === lod
                  ? 'bg-[var(--accent-blue)] text-white'
                  : 'bg-[var(--bg-tertiary)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]'
              )}
              title={LOD_LABELS[lod]}
            >
              {lod}
            </button>
          ))}
        </div>
        <p className="mt-1 text-xs text-[var(--text-muted)]">
          {LOD_LABELS[policy.lod]}
        </p>
      </div>

      {/* Depth Slider */}
      <div>
        <label className="block text-xs font-medium text-[var(--text-secondary)] mb-2">
          Max Depth: {policy.max_depth}
        </label>
        <input
          type="range"
          min={1}
          max={10}
          value={policy.max_depth}
          onChange={(e) => setMaxDepth(parseInt(e.target.value, 10))}
          className="w-full h-1.5 bg-[var(--bg-tertiary)] rounded-lg appearance-none cursor-pointer accent-[var(--accent-blue)]"
        />
        <div className="flex justify-between text-xs text-[var(--text-muted)] mt-1">
          <span>1</span>
          <span>10</span>
        </div>
      </div>

      {/* Chamber Toggles */}
      <div>
        <label className="block text-xs font-medium text-[var(--text-secondary)] mb-2">
          Chambers
        </label>
        <div className="flex flex-wrap gap-1.5">
          {CHAMBERS.map(({ id, label }) => (
            <button
              key={id}
              onClick={() => toggleChamber(id)}
              className={cn(
                'rounded px-2 py-1 text-xs transition-colors',
                policy.chambers.includes(id)
                  ? 'bg-[var(--accent-blue)]/20 text-[var(--accent-blue)] border border-[var(--accent-blue)]/30'
                  : 'bg-[var(--bg-tertiary)] text-[var(--text-muted)] border border-transparent hover:bg-[var(--bg-hover)]'
              )}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Regenerate button */}
      {onRegenerate && (
        <button
          onClick={onRegenerate}
          className="w-full rounded-lg bg-[var(--accent-blue)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--accent-blue)]/80"
        >
          Regenerate Projection
        </button>
      )}
    </div>
  );
}

export default PolicyControls;
