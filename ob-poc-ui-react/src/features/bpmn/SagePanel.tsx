import { useState } from "react";
import { SageReasoningRecord } from "@/api/bpmn";

interface Props {
  records: SageReasoningRecord[];
}

function ConfidenceBar({ value }: { value: number }) {
  const pct = Math.round(value * 100);
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-1.5 bg-gray-700 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full ${pct >= 90 ? "bg-green-500" : pct >= 70 ? "bg-yellow-500" : "bg-red-500"}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-xs text-gray-400 font-mono">{pct}%</span>
    </div>
  );
}

function RecordCard({ record }: { record: SageReasoningRecord }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="border border-gray-700 rounded p-3 space-y-2 bg-gray-900">
      <div className="flex items-center justify-between">
        <span className="text-xs font-mono text-blue-300">{record.verb_fqn}</span>
        <span className="text-xs text-gray-500">{record.mode}</span>
      </div>

      <ConfidenceBar value={record.confidence} />

      <div className="text-xs text-gray-300">{record.rationale}</div>

      <button
        onClick={() => setExpanded(!expanded)}
        className="text-xs text-gray-500 hover:text-gray-300 transition-colors"
      >
        {expanded ? "▲ Hide options" : `▼ ${record.options_considered.length} options considered`}
      </button>

      {expanded && (
        <div className="space-y-1 mt-1">
          {record.options_considered.map((opt, i) => (
            <div key={i} className="flex items-start gap-2 text-xs">
              <span
                className={`font-mono shrink-0 ${
                  opt.verb === record.chosen ? "text-green-400" : "text-gray-500"
                }`}
              >
                {opt.verb === record.chosen ? "✓" : "○"} {opt.verb}
                <span className="ml-1 text-gray-600">({Math.round(opt.score * 100)}%)</span>
              </span>
            </div>
          ))}
          {record.options_considered.length > 0 && (
            <div className="mt-1 text-xs text-gray-500 italic">
              {record.options_considered.find((o) => o.verb === record.chosen)?.reason}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function SagePanel({ records }: Props) {
  if (records.length === 0) {
    return (
      <div className="p-4 text-xs text-gray-500 text-center">
        No Sage reasoning records yet.
        <div className="mt-1 text-gray-600">
          Sage observes each completed verb invocation.
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 space-y-3">
      <div className="text-xs text-gray-400">
        {records.length} reasoning record{records.length !== 1 ? "s" : ""} — actor: Sage, mode: observation
      </div>
      {[...records].reverse().map((r) => (
        <RecordCard key={r.id} record={r} />
      ))}
    </div>
  );
}
