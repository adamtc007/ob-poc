import React from "react";
import type {
  NarrationPayload,
  NarrationGap,
  SuggestedAction,
  NarrationBlocker,
  SlotDelta,
} from "../../../types/chat";

interface NarrationPanelProps {
  narration: NarrationPayload;
  onSendMessage?: (utterance: string) => void;
}

/**
 * Proactive narration panel — goal-directed workflow guidance (ADR 043).
 *
 * Renders progress, slot deltas, gaps, suggested next steps, and blockers
 * after state-changing actions. Silent narration = no render.
 */
export const NarrationPanel: React.FC<NarrationPanelProps> = ({
  narration,
  onSendMessage,
}) => {
  if (narration.verbosity === "silent") return null;

  const hasGaps =
    narration.required_gaps.length > 0 || narration.optional_gaps.length > 0;
  const allComplete = !hasGaps && narration.verbosity === "full";

  return (
    <div
      style={{
        margin: "8px 0",
        padding: "12px 16px",
        borderRadius: "8px",
        border: allComplete
          ? "1px solid #4caf50"
          : "1px solid var(--border-color, #e0e0e0)",
        backgroundColor: allComplete
          ? "rgba(76, 175, 80, 0.06)"
          : "var(--card-bg, #fafafa)",
        fontSize: "13px",
        lineHeight: 1.5,
      }}
    >
      {/* Progress bar */}
      {narration.progress && (
        <div
          style={{
            fontWeight: 600,
            marginBottom: hasGaps ? "8px" : 0,
            color: allComplete ? "#2e7d32" : "inherit",
          }}
        >
          {allComplete ? "\u2714 " : "\u25B6 "}
          {narration.progress}
        </div>
      )}

      {/* Slot deltas (what just changed) */}
      {narration.delta.length > 0 && (
        <div style={{ marginBottom: "8px" }}>
          {narration.delta.map((d, i) => (
            <DeltaBadge key={i} delta={d} />
          ))}
        </div>
      )}

      {/* Required gaps */}
      {narration.verbosity !== "light" &&
        narration.required_gaps.length > 0 && (
          <GapSection
            title="Required"
            gaps={narration.required_gaps}
            critical
            onSendMessage={onSendMessage}
          />
        )}

      {/* Optional gaps (only in Full mode) */}
      {narration.verbosity === "full" &&
        narration.optional_gaps.length > 0 && (
          <GapSection
            title="Optional"
            gaps={narration.optional_gaps}
            onSendMessage={onSendMessage}
          />
        )}

      {/* Suggested next actions */}
      {narration.suggested_next.length > 0 &&
        narration.verbosity !== "light" && (
          <SuggestedActions
            actions={narration.suggested_next}
            onSendMessage={onSendMessage}
          />
        )}

      {/* Blockers */}
      {narration.blockers.length > 0 && (
        <BlockerList blockers={narration.blockers} />
      )}

      {/* Completion + workspace transition */}
      {allComplete && (
        <div
          style={{
            marginTop: "8px",
            color: "#2e7d32",
            fontWeight: 500,
          }}
        >
          All required slots filled — ready to proceed.
        </div>
      )}

      {/* Workspace transition suggestion */}
      {narration.workspace_transition && (
        <div
          style={{
            marginTop: "10px",
            padding: "8px 12px",
            borderRadius: "6px",
            border: "1px solid #1976d2",
            backgroundColor: "rgba(25, 118, 210, 0.06)",
            display: "flex",
            alignItems: "center",
            gap: "12px",
          }}
        >
          <span style={{ fontSize: "16px" }}>{"\u27A1"}</span>
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 600, color: "#1565c0" }}>
              Next: {narration.workspace_transition.target_label}
            </div>
            <div style={{ fontSize: "12px", color: "#757575" }}>
              {narration.workspace_transition.reason}
            </div>
          </div>
          {onSendMessage && (
            <button
              onClick={() =>
                onSendMessage(
                  narration.workspace_transition!.suggested_utterance,
                )
              }
              style={{
                padding: "6px 16px",
                fontSize: "12px",
                fontWeight: 600,
                borderRadius: "16px",
                border: "1px solid #1976d2",
                backgroundColor: "#1976d2",
                color: "#fff",
                cursor: "pointer",
              }}
            >
              Switch
            </button>
          )}
        </div>
      )}
    </div>
  );
};

// ── Sub-components ────────────────────────────────────────────────────

const DeltaBadge: React.FC<{ delta: SlotDelta }> = ({ delta }) => (
  <span
    style={{
      display: "inline-block",
      padding: "2px 8px",
      marginRight: "4px",
      borderRadius: "4px",
      fontSize: "12px",
      backgroundColor:
        delta.to_state === "filled"
          ? "rgba(76, 175, 80, 0.12)"
          : "rgba(255, 152, 0, 0.12)",
      color: delta.to_state === "filled" ? "#2e7d32" : "#e65100",
    }}
  >
    {delta.slot_label}: {delta.from_state} → {delta.to_state}
    {delta.entity_name && ` (${delta.entity_name})`}
  </span>
);

const GapSection: React.FC<{
  title: string;
  gaps: NarrationGap[];
  critical?: boolean;
  onSendMessage?: (utterance: string) => void;
}> = ({ title, gaps, critical, onSendMessage }) => (
  <div style={{ marginBottom: "6px" }}>
    <div
      style={{
        fontSize: "11px",
        fontWeight: 600,
        textTransform: "uppercase",
        color: critical ? "#c62828" : "#757575",
        marginBottom: "4px",
      }}
    >
      {title}
    </div>
    {gaps.map((gap, i) => (
      <div
        key={i}
        style={{
          display: "flex",
          alignItems: "center",
          gap: "8px",
          padding: "2px 0",
        }}
      >
        <span style={{ color: critical ? "#c62828" : "#757575" }}>
          {critical ? "\u25CF" : "\u25CB"}
        </span>
        <span>{gap.slot_label}</span>
        {gap.why_required && (
          <span style={{ fontSize: "11px", color: "#9e9e9e" }}>
            — {gap.why_required}
          </span>
        )}
        {onSendMessage && (
          <button
            onClick={() => onSendMessage(gap.suggested_utterance)}
            style={{
              marginLeft: "auto",
              padding: "2px 8px",
              fontSize: "11px",
              borderRadius: "4px",
              border: "1px solid var(--border-color, #ccc)",
              backgroundColor: "transparent",
              cursor: "pointer",
              color: "inherit",
            }}
            title={`Send: "${gap.suggested_utterance}"`}
          >
            {gap.suggested_utterance}
          </button>
        )}
      </div>
    ))}
  </div>
);

const SuggestedActions: React.FC<{
  actions: SuggestedAction[];
  onSendMessage?: (utterance: string) => void;
}> = ({ actions, onSendMessage }) => (
  <div style={{ marginTop: "8px" }}>
    <div
      style={{
        fontSize: "11px",
        fontWeight: 600,
        textTransform: "uppercase",
        color: "#757575",
        marginBottom: "4px",
      }}
    >
      Suggested Next
    </div>
    <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
      {actions.map((action, i) => (
        <button
          key={i}
          onClick={() => onSendMessage?.(action.utterance)}
          style={{
            padding: "4px 12px",
            fontSize: "12px",
            borderRadius: "16px",
            border:
              action.priority === "critical"
                ? "1px solid #c62828"
                : "1px solid var(--border-color, #ccc)",
            backgroundColor:
              action.priority === "critical"
                ? "rgba(198, 40, 40, 0.08)"
                : "transparent",
            cursor: onSendMessage ? "pointer" : "default",
            color: "inherit",
          }}
          title={action.reason}
        >
          {action.utterance}
        </button>
      ))}
    </div>
  </div>
);

const BlockerList: React.FC<{ blockers: NarrationBlocker[] }> = ({
  blockers,
}) => (
  <div style={{ marginTop: "8px" }}>
    <div
      style={{
        fontSize: "11px",
        fontWeight: 600,
        textTransform: "uppercase",
        color: "#e65100",
        marginBottom: "4px",
      }}
    >
      Blockers
    </div>
    {blockers.map((b, i) => (
      <div key={i} style={{ fontSize: "12px", padding: "2px 0" }}>
        <span style={{ color: "#e65100" }}>{"\u26A0"} </span>
        <strong>{b.blocked_verb}</strong>: {b.reason}
        <span style={{ color: "#9e9e9e" }}> — {b.unblock_hint}</span>
      </div>
    ))}
  </div>
);

export default NarrationPanel;
