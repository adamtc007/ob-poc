/**
 * Flight Deck — Observatory control panel in the Sage session.
 *
 * Compact React panel that sits in the chat page, giving the user direct
 * manipulation controls for Observatory navigation. Every interaction fires
 * a verb through the existing chat/utterance pipeline — the flight deck is
 * a UI skin over nav.drill, nav.zoom-out, nav.set-lens, and available actions.
 */

import { useState } from "react";
import {
  ChevronDown,
  ChevronUp,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
} from "lucide-react";
import type {
  OrientationContract,
  ViewLevel,
  ActionDescriptor,
} from "../../../types/observatory";

const VIEW_LEVELS: ViewLevel[] = [
  "universe",
  "cluster",
  "system",
  "planet",
  "surface",
  "core",
];

const LEVEL_LABELS: Record<ViewLevel, string> = {
  universe: "Universe",
  cluster: "Cluster",
  system: "System",
  planet: "Planet",
  surface: "Surface",
  core: "Core",
};

const DEPTH_COLORS: Record<ViewLevel, string> = {
  universe: "bg-slate-600",
  cluster: "bg-indigo-600",
  system: "bg-violet-600",
  planet: "bg-purple-600",
  surface: "bg-fuchsia-600",
  core: "bg-pink-600",
};

const MODE_STYLES: Record<string, { bg: string; text: string }> = {
  governed: { bg: "bg-emerald-500/20", text: "text-emerald-400" },
  research: { bg: "bg-blue-500/20", text: "text-blue-400" },
  maintenance: { bg: "bg-amber-500/20", text: "text-amber-400" },
};

interface FlightDeckProps {
  orientation: OrientationContract | null;
  onSendMessage: (message: string) => void;
}

export function FlightDeck({ orientation, onSendMessage }: FlightDeckProps) {
  const [collapsed, setCollapsed] = useState(false);

  if (!orientation) return null;

  const currentLevel = orientation.view_level;
  const currentIdx = VIEW_LEVELS.indexOf(currentLevel);
  const enabledActions = orientation.available_actions.filter((a) => a.enabled);
  const modeStyle = MODE_STYLES[orientation.session_mode] ?? {
    bg: "bg-slate-500/20",
    text: "text-slate-400",
  };

  // Collapsed single-line summary
  if (collapsed) {
    return (
      <button
        onClick={() => setCollapsed(false)}
        className="flex items-center gap-2 px-3 py-1.5 text-xs border-b border-[var(--border-primary)] bg-[var(--bg-secondary)] hover:bg-[var(--bg-hover)] w-full text-left"
      >
        <ChevronDown size={12} className="text-[var(--text-muted)]" />
        <span className="font-medium text-[var(--text-primary)] capitalize">
          {currentLevel}
        </span>
        <span className="text-[var(--text-muted)]">·</span>
        <span className="text-[var(--text-secondary)] truncate">
          {orientation.focus_identity.business_label}
        </span>
        <span className="text-[var(--text-muted)]">·</span>
        <span className={`${modeStyle.text} uppercase text-[10px] font-semibold`}>
          {orientation.session_mode}
        </span>
        <span className="text-[var(--text-muted)] ml-auto">
          {enabledActions.length} actions
        </span>
      </button>
    );
  }

  return (
    <div className="border-b border-[var(--border-primary)] bg-[var(--bg-secondary)]">
      {/* Collapse toggle */}
      <button
        onClick={() => setCollapsed(true)}
        className="flex items-center gap-1 px-3 py-1 text-[10px] text-[var(--text-muted)] hover:text-[var(--text-secondary)] w-full"
      >
        <ChevronUp size={10} />
        Flight Deck
      </button>

      <div className="flex gap-0 px-2 pb-2">
        {/* 1. Altitude ladder (left) */}
        <AltitudeLadder
          currentLevel={currentLevel}
          currentIdx={currentIdx}
          onSendMessage={onSendMessage}
        />

        {/* Right side: focus + instruments + actions */}
        <div className="flex-1 min-w-0 flex flex-col gap-1.5 pl-2">
          {/* 2. Focus + mode + history */}
          <FocusBar orientation={orientation} onSendMessage={onSendMessage} />

          {/* 3. Instruments (lens controls) */}
          <Instruments orientation={orientation} onSendMessage={onSendMessage} />

          {/* 4. Actions */}
          <Actions
            actions={orientation.available_actions}
            onSendMessage={onSendMessage}
          />
        </div>
      </div>
    </div>
  );
}

// ── Altitude Ladder ──────────────────────────────────────────

function AltitudeLadder({
  currentLevel,
  currentIdx,
  onSendMessage,
}: {
  currentLevel: ViewLevel;
  currentIdx: number;
  onSendMessage: (msg: string) => void;
}) {
  return (
    <div className="flex flex-col gap-0.5 w-[110px] shrink-0">
      {VIEW_LEVELS.map((level, i) => {
        const active = level === currentLevel;
        const above = i < currentIdx;
        const below = i > currentIdx;

        return (
          <button
            key={level}
            onClick={() => {
              if (active) return;
              if (above) onSendMessage("nav.zoom-out");
              else if (below) onSendMessage(`nav.drill ${level}`);
            }}
            className={`flex items-center gap-1.5 px-2 py-0.5 rounded text-[11px] text-left transition-colors ${
              active
                ? "bg-[var(--bg-active)] text-[var(--text-primary)] font-semibold"
                : "text-[var(--text-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-secondary)]"
            }`}
          >
            <span
              className={`w-2 h-2 rounded-full shrink-0 ${
                active ? DEPTH_COLORS[level] : "bg-[var(--text-muted)]/30"
              }`}
            />
            <span className="truncate">{LEVEL_LABELS[level]}</span>
            {active && (
              <span className="ml-auto text-[var(--text-muted)]">◂</span>
            )}
          </button>
        );
      })}
    </div>
  );
}

// ── Focus Bar ────────────────────────────────────────────────

function FocusBar({
  orientation,
  onSendMessage,
}: {
  orientation: OrientationContract;
  onSendMessage: (msg: string) => void;
}) {
  const modeStyle = MODE_STYLES[orientation.session_mode] ?? {
    bg: "bg-slate-500/20",
    text: "text-slate-400",
  };

  return (
    <div className="flex items-center gap-2 text-[11px]">
      {/* Focus label */}
      <span className="text-[var(--text-primary)] font-medium truncate max-w-[200px]">
        {orientation.focus_identity.business_label || "—"}
      </span>

      {/* Mode badge */}
      <span
        className={`px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase ${modeStyle.bg} ${modeStyle.text}`}
      >
        {orientation.session_mode}
      </span>

      {/* Nav controls */}
      <div className="flex items-center gap-0.5 ml-auto">
        <NavButton
          icon={<ArrowLeft size={12} />}
          title="Back"
          onClick={() => onSendMessage("nav.history-back")}
        />
        <NavButton
          icon={<ArrowRight size={12} />}
          title="Forward"
          onClick={() => onSendMessage("nav.history-forward")}
        />
        <NavButton
          icon={<ArrowUp size={12} />}
          title="Zoom out"
          onClick={() => onSendMessage("nav.zoom-out")}
        />
      </div>
    </div>
  );
}

function NavButton({
  icon,
  title,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className="p-1 rounded text-[var(--text-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-secondary)]"
    >
      {icon}
    </button>
  );
}

// ── Instruments ──────────────────────────────────────────────

function Instruments({
  orientation,
  onSendMessage,
}: {
  orientation: OrientationContract;
  onSendMessage: (msg: string) => void;
}) {
  const overlay =
    orientation.lens.overlay.mode === "active_only"
      ? "active_only"
      : "draft_overlay";
  const depthProbe = orientation.lens.depth_probe;
  const clusterMode = orientation.lens.cluster_mode;

  return (
    <div className="flex flex-wrap items-center gap-1">
      {/* Overlay */}
      <Chip
        label="Active"
        active={overlay === "active_only"}
        onClick={() =>
          onSendMessage("nav.set-lens overlay active_only")
        }
      />
      <Chip
        label="Drafts"
        active={overlay === "draft_overlay"}
        onClick={() =>
          onSendMessage("nav.set-lens overlay draft_overlay")
        }
      />

      <Divider />

      {/* Depth probe */}
      {(["ownership", "control", "services", "documents"] as const).map(
        (probe) => (
          <Chip
            key={probe}
            label={probe.charAt(0).toUpperCase() + probe.slice(1)}
            active={depthProbe === probe}
            onClick={() =>
              onSendMessage(
                depthProbe === probe
                  ? "nav.set-lens"
                  : `nav.set-lens depth_probe ${probe}`
              )
            }
          />
        )
      )}

      <Divider />

      {/* Cluster mode */}
      {(["jurisdiction", "client", "risk", "product"] as const).map((mode) => (
        <Chip
          key={mode}
          label={mode.charAt(0).toUpperCase() + mode.slice(1)}
          active={clusterMode === mode}
          onClick={() => onSendMessage(`nav.set-cluster-type ${mode}`)}
        />
      ))}
    </div>
  );
}

function Chip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors ${
        active
          ? "bg-[var(--accent-blue)]/20 text-[var(--accent-blue)] border border-[var(--accent-blue)]/30"
          : "text-[var(--text-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-secondary)] border border-transparent"
      }`}
    >
      {label}
    </button>
  );
}

function Divider() {
  return (
    <span className="w-px h-3 bg-[var(--border-secondary)] mx-0.5" />
  );
}

// ── Actions ──────────────────────────────────────────────────

function Actions({
  actions,
  onSendMessage,
}: {
  actions: ActionDescriptor[];
  onSendMessage: (msg: string) => void;
}) {
  const sorted = [...actions].sort((a, b) => b.rank_score - a.rank_score);
  const top = sorted.slice(0, 6);

  if (top.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-1">
      {top.map((action) => (
        <button
          key={action.action_id}
          onClick={() => {
            if (action.enabled) onSendMessage(action.action_id);
          }}
          disabled={!action.enabled}
          title={
            action.enabled
              ? action.action_id
              : action.disabled_reason ?? "Not available"
          }
          className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] border transition-colors ${
            action.enabled
              ? "text-[var(--text-primary)] border-[var(--border-secondary)] hover:bg-[var(--bg-hover)] cursor-pointer"
              : "text-[var(--text-muted)] border-transparent opacity-50 cursor-not-allowed"
          }`}
        >
          <span>{action.label}</span>
          <span className="text-[var(--text-muted)] text-[9px]">
            {action.rank_score.toFixed(2)}
          </span>
        </button>
      ))}
    </div>
  );
}
