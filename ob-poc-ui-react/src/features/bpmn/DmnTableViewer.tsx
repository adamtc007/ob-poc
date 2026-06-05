import { useEffect, useState } from "react";
import { bpmnApi } from "@/api/bpmn";
import type { DmnSchemaDto } from "@/api/bpmn";

export function DmnTableViewer({ decisionId }: { decisionId: string }) {
  const [schema, setSchema] = useState<DmnSchemaDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setSchema(null);
    setError(null);
    bpmnApi
      .getDmnDecision(decisionId)
      .then((data) => {
        if (active) setSchema(data);
      })
      .catch((err) => {
        if (active) {
          console.error(err);
          setError(err.message || "Failed to load DMN decision table");
        }
      });
    return () => {
      active = false;
    };
  }, [decisionId]);

  if (error) {
    return <div className="p-4 text-xs text-red-400 font-mono">Error: {error}</div>;
  }

  if (!schema) {
    return (
      <div className="p-4 text-xs text-gray-500 font-mono animate-pulse">
        Loading Decision Rules for {decisionId}...
      </div>
    );
  }

  return (
    <div className="p-4 bg-gray-900 border-t border-gray-800">
      <div className="flex justify-between items-center mb-3">
        <div className="text-xs font-semibold text-cyan-400 font-mono">
          DMN Decision Table: {schema.decision_name}
        </div>
        <div className="text-[10px] bg-cyan-950 text-cyan-300 px-2 py-0.5 rounded font-mono">
          Hit Policy: {schema.hit_policy}
        </div>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-left border-collapse border border-gray-800 text-[11px] font-mono">
          <thead>
            <tr className="bg-gray-800 text-gray-400 border-b border-gray-800">
              <th className="p-2 border-r border-gray-800 w-8">#</th>
              {schema.inputs.map((i) => (
                <th key={i.name} className="p-2 border-r border-gray-800">
                  Input: {i.name} ({i.type})
                </th>
              ))}
              {schema.outputs.map((o) => (
                <th key={o.name} className="p-2">
                  Output: {o.name} ({o.type})
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {schema.rules.map((rule, idx) => (
              <tr
                key={rule.id}
                className="border-b border-gray-800 hover:bg-gray-800/30 transition-colors"
              >
                <td className="p-2 border-r border-gray-800 text-gray-500">{idx + 1}</td>
                {rule.inputs.map((cell, i) => (
                  <td key={i} className="p-2 border-r border-gray-800 text-amber-300">
                    {cell.op} "{cell.value}"
                  </td>
                ))}
                {rule.outputs.map((val, i) => (
                  <td key={i} className="p-2 text-green-400 font-semibold">
                    {val}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
