import { useEffect, useRef, useState } from "react";
import { bpmnApi } from "@/api/bpmn";

interface LiveEvent {
  id: string;
  execution_id: string;
  node_id: string;
  outcome: string;
  occurred_at: string;
}

interface Props {
  instanceId: string;
}

export function PlanFeedPanel({ instanceId }: Props) {
  const [events, setEvents] = useState<LiveEvent[]>([]);
  const esRef = useRef<EventSource | null>(null);

  useEffect(() => {
    esRef.current = bpmnApi.subscribeToEvents(instanceId, (data) => {
      const e = data as LiveEvent;
      if (e.execution_id) {
        setEvents((prev) => [
          { ...e, id: e.execution_id },
          ...prev.slice(0, 49), // keep last 50
        ]);
      }
    });
    return () => esRef.current?.close();
  }, [instanceId]);

  if (events.length === 0) {
    return (
      <div className="p-4 text-xs text-gray-500 text-center">
        No events yet. Complete a step to see the lifecycle event stream.
      </div>
    );
  }

  return (
    <div className="p-4 space-y-2">
      <div className="text-xs text-gray-400">{events.length} event{events.length !== 1 ? "s" : ""}</div>
      {events.map((ev) => (
        <div
          key={ev.id}
          className="text-xs font-mono border border-gray-700 rounded p-2 bg-gray-900 space-y-1"
        >
          <div className="flex items-center justify-between">
            <span className="text-blue-300">{ev.node_id || "<lifecycle>"}</span>
            <span
              className={`px-1 rounded ${
                ev.outcome?.includes("Committed") ? "text-green-400" : "text-red-400"
              }`}
            >
              {ev.outcome?.replace("Committed", "✓ Committed") ?? "—"}
            </span>
          </div>
          <div className="text-gray-600 truncate">
            exec: {ev.execution_id?.slice(0, 8)}…
          </div>
        </div>
      ))}
    </div>
  );
}
