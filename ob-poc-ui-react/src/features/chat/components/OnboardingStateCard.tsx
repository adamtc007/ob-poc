/**
 * OnboardingStateCard — "Where am I + what can I do" DAG view.
 *
 * Renders the onboarding state as a vertical timeline showing:
 * - 6 DAG layers with progress and state badges
 * - Forward verbs (advance state) as clickable utterances
 * - Revert verbs (back up state) as secondary actions
 * - Linked-CBU state cards with next action
 */

import type {
  ChatMessage,
  OnboardingLayer,
  SuggestedVerb,
  CbuStateCard as CbuStateCardType,
  LayerState,
} from "../../../types/chat";

interface Props {
  message: ChatMessage;
  onVerbClick?: (utterance: string) => void;
}

const stateColors: Record<LayerState, string> = {
  complete: "bg-green-100 text-green-800 border-green-300",
  in_progress: "bg-blue-100 text-blue-800 border-blue-300",
  not_started: "bg-gray-100 text-gray-600 border-gray-300",
  blocked: "bg-red-50 text-red-700 border-red-200",
};

const stateLabels: Record<LayerState, string> = {
  complete: "Complete",
  in_progress: "In Progress",
  not_started: "Not Started",
  blocked: "Blocked",
};

const dotColors: Record<LayerState, string> = {
  complete: "bg-green-500",
  in_progress: "bg-blue-500",
  not_started: "bg-gray-300",
  blocked: "bg-red-400",
};

export function OnboardingStateCard({ message, onVerbClick }: Props) {
  const state = message.onboarding_state;
  if (!state) return null;

  return (
    <div className="mt-2 rounded-lg border border-indigo-200 bg-indigo-50 p-3">
      {/* Header */}
      <div className="mb-3 flex items-center justify-between">
        <div>
          <div className="text-sm font-semibold text-indigo-900">
            {state.group_name || "Onboarding Progress"}
          </div>
          <div className="text-xs text-indigo-600">
            Overall: {state.overall_progress_pct}%
          </div>
        </div>
        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-indigo-200 text-xs font-bold text-indigo-900">
          {state.overall_progress_pct}%
        </div>
      </div>

      {/* DAG Timeline */}
      <div className="space-y-1">
        {state.layers.map((layer) => (
          <LayerRow
            key={layer.index}
            layer={layer}
            isActive={layer.index === state.active_layer_index}
            onVerbClick={onVerbClick}
          />
        ))}
      </div>

      {/* CBU Cards */}
      {state.cbu_cards.length > 0 && (
        <div className="mt-3 border-t border-indigo-200 pt-2">
          <div className="mb-1 text-xs font-medium text-indigo-700">
            Linked CBU Status
          </div>
          <div className="space-y-1">
            {state.cbu_cards.map((cbu) => (
              <CbuCard key={cbu.cbu_id} cbu={cbu} onVerbClick={onVerbClick} />
            ))}
          </div>
        </div>
      )}

      {/* Context reset hint */}
      {state.context_reset_hint && (
        <div className="mt-2 rounded bg-yellow-50 p-2 text-xs text-yellow-800">
          {state.context_reset_hint.message}
          <button
            onClick={() =>
              onVerbClick?.(state.context_reset_hint!.reset_utterance)
            }
            className="ml-1 font-medium underline"
          >
            {state.context_reset_hint.reset_utterance}
          </button>
        </div>
      )}
    </div>
  );
}

function LayerRow({
  layer,
  isActive,
  onVerbClick,
}: {
  layer: OnboardingLayer;
  isActive: boolean;
  onVerbClick?: (utterance: string) => void;
}) {
  return (
    <div
      className={`rounded-md border p-2 ${
        isActive
          ? "border-indigo-400 bg-white shadow-sm"
          : "border-transparent bg-transparent"
      }`}
    >
      <div className="flex items-center gap-2">
        {/* Timeline dot */}
        <div
          className={`h-2.5 w-2.5 shrink-0 rounded-full ${dotColors[layer.state]}`}
        />

        {/* Layer name + state */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-medium text-gray-900">
              {layer.name}
            </span>
            <span
              className={`rounded px-1 py-0.5 text-[10px] font-medium ${stateColors[layer.state]}`}
            >
              {stateLabels[layer.state]}
            </span>
            {layer.progress_pct > 0 && layer.progress_pct < 100 && (
              <span className="text-[10px] text-gray-500">
                {layer.progress_pct}%
              </span>
            )}
          </div>
          {layer.summary && (
            <div className="text-[11px] text-gray-500">{layer.summary}</div>
          )}
        </div>
      </div>

      {/* Forward verbs — advance state */}
      {isActive && layer.forward_verbs.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1 pl-4">
          {layer.forward_verbs.map((verb, idx) => (
            <VerbButton key={idx} verb={verb} onVerbClick={onVerbClick} />
          ))}
        </div>
      )}

      {/* Revert verbs — back up state */}
      {isActive && layer.revert_verbs.length > 0 && (
        <div className="mt-1 flex flex-wrap gap-1 pl-4">
          {layer.revert_verbs.map((verb, idx) => (
            <VerbButton
              key={idx}
              verb={verb}
              onVerbClick={onVerbClick}
              variant="revert"
            />
          ))}
        </div>
      )}
    </div>
  );
}

function VerbButton({
  verb,
  onVerbClick,
  variant = "forward",
}: {
  verb: SuggestedVerb;
  onVerbClick?: (utterance: string) => void;
  variant?: "forward" | "revert";
}) {
  const isRevert = variant === "revert";
  return (
    <button
      onClick={() => onVerbClick?.(verb.suggested_utterance)}
      title={verb.reason}
      className={`rounded px-2 py-0.5 text-[11px] font-medium transition-colors ${
        isRevert
          ? "border border-gray-300 bg-white text-gray-600 hover:bg-gray-50"
          : "bg-indigo-600 text-white hover:bg-indigo-700"
      }`}
    >
      {verb.label}
    </button>
  );
}

function CbuCard({
  cbu,
  onVerbClick,
}: {
  cbu: CbuStateCardType;
  onVerbClick?: (utterance: string) => void;
}) {
  return (
    <div className="flex items-center gap-2 rounded bg-white px-2 py-1.5">
      {/* Progress ring */}
      <div className="relative h-6 w-6 shrink-0">
        <svg className="h-6 w-6 -rotate-90" viewBox="0 0 24 24">
          <circle
            cx="12"
            cy="12"
            r="10"
            fill="none"
            stroke="#e5e7eb"
            strokeWidth="2"
          />
          <circle
            cx="12"
            cy="12"
            r="10"
            fill="none"
            stroke={cbu.progress_pct === 100 ? "#22c55e" : "#6366f1"}
            strokeWidth="2"
            strokeDasharray={`${(cbu.progress_pct / 100) * 62.83} 62.83`}
            strokeLinecap="round"
          />
        </svg>
      </div>

      {/* CBU info */}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1">
          <span className="truncate text-xs font-medium text-gray-900">
            {cbu.cbu_name || cbu.cbu_id}
          </span>
          {cbu.lifecycle_state && (
            <span className="rounded bg-gray-100 px-1 py-0.5 text-[10px] text-gray-600">
              {cbu.lifecycle_state}
            </span>
          )}
          {cbu.phases.case_status && (
            <span className="rounded bg-blue-50 px-1 py-0.5 text-[10px] text-blue-700">
              {cbu.phases.case_status}
            </span>
          )}
        </div>
      </div>

      {/* Next action */}
      {cbu.next_action && (
        <button
          onClick={() => onVerbClick?.(cbu.next_action!.suggested_utterance)}
          title={cbu.next_action.reason}
          className="shrink-0 rounded bg-indigo-600 px-2 py-0.5 text-[10px] font-medium text-white hover:bg-indigo-700"
        >
          {cbu.next_action.label}
        </button>
      )}

      {/* Revert action */}
      {cbu.revert_action && (
        <button
          onClick={() => onVerbClick?.(cbu.revert_action!.suggested_utterance)}
          title={cbu.revert_action.reason}
          className="shrink-0 rounded border border-gray-300 px-2 py-0.5 text-[10px] text-gray-500 hover:bg-gray-50"
        >
          {cbu.revert_action.label}
        </button>
      )}
    </div>
  );
}
