/**
 * VerbDisambiguationCard — "Did you mean?" with full context.
 *
 * Renders verb disambiguation options with:
 * - Suggested utterance (what to say)
 * - Differentiation (why this option is different)
 * - Entity/constellation context (where it operates)
 * - Verb kind badge (primitive/macro/query/workflow)
 */

import type {
  ChatMessage,
  VerbDisambiguationOption,
} from "../../../types/chat";

interface Props {
  message: ChatMessage;
  onSelect?: (utterance: string) => void;
}

const kindColors: Record<string, string> = {
  primitive: "bg-blue-100 text-blue-800",
  macro: "bg-purple-100 text-purple-800",
  query: "bg-green-100 text-green-800",
  workflow: "bg-orange-100 text-orange-800",
};

export function VerbDisambiguationCard({ message, onSelect }: Props) {
  const detail = message.verb_disambiguation_detail;
  if (!detail || !detail.options?.length) return null;

  return (
    <div className="mt-2 rounded-lg border border-amber-200 bg-amber-50 p-3">
      <div className="mb-2 text-sm font-medium text-amber-900">
        {detail.prompt || "Which operation did you mean?"}
      </div>
      <div className="space-y-2">
        {detail.options.map((opt, idx) => (
          <VerbOptionRow
            key={idx}
            option={opt}
            index={idx}
            onSelect={onSelect}
          />
        ))}
      </div>
      <div className="mt-2 text-xs text-amber-700">
        Click an option or type one of the phrases above.
      </div>
    </div>
  );
}

function VerbOptionRow({
  option,
  index,
  onSelect,
}: {
  option: VerbDisambiguationOption;
  index: number;
  onSelect?: (utterance: string) => void;
}) {
  const utterance = option.suggested_utterance || option.verb_fqn;
  const kindClass = option.verb_kind
    ? kindColors[option.verb_kind] || "bg-gray-100 text-gray-800"
    : "";

  return (
    <button
      onClick={() => onSelect?.(utterance)}
      className="w-full rounded-md border border-amber-200 bg-white p-2 text-left transition-colors hover:border-amber-400 hover:bg-amber-50"
    >
      <div className="flex items-start gap-2">
        <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-amber-200 text-xs font-bold text-amber-900">
          {index + 1}
        </span>
        <div className="min-w-0 flex-1">
          {/* Suggested utterance — what to say */}
          <div className="text-sm font-medium text-gray-900">
            &ldquo;{utterance}&rdquo;
          </div>

          {/* Differentiation — why this option is different */}
          {option.differentiation && (
            <div className="mt-0.5 text-xs text-gray-600">
              {option.differentiation}
            </div>
          )}

          {/* Entity/constellation context — where it operates */}
          {option.entity_context && (
            <div className="mt-0.5 text-xs text-gray-500 italic">
              {option.entity_context}
            </div>
          )}

          {/* Badges row */}
          <div className="mt-1 flex flex-wrap gap-1">
            {option.verb_kind && (
              <span
                className={`inline-block rounded px-1.5 py-0.5 text-[10px] font-medium ${kindClass}`}
              >
                {option.verb_kind}
              </span>
            )}
            {option.constellation_slot && (
              <span className="inline-block rounded bg-gray-100 px-1.5 py-0.5 text-[10px] text-gray-600">
                {option.constellation_slot}
              </span>
            )}
            {option.target_entity_kind && (
              <span className="inline-block rounded bg-gray-100 px-1.5 py-0.5 text-[10px] text-gray-600">
                {option.target_entity_kind}
              </span>
            )}
            {option.step_count != null && option.step_count > 0 && (
              <span className="inline-block rounded bg-purple-50 px-1.5 py-0.5 text-[10px] text-purple-700">
                {option.step_count} steps
              </span>
            )}
          </div>
        </div>
      </div>
    </button>
  );
}
