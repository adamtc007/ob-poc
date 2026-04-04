/**
 * ViewportRenderer — dispatches ShowPacket viewports to typed sub-components.
 *
 * Renders focus, object, diff, and gates viewport cards, plus the action palette.
 */

import type {
  OrientationContract,
  ActionDescriptor,
} from "../../../types/observatory";
import { TaxonomyTree } from "./viewports/TaxonomyTree";
import { ImpactGraph } from "./viewports/ImpactGraph";
import { ActionSurface } from "./viewports/ActionSurface";
import { CoverageMap } from "./viewports/CoverageMap";

interface Props {
  showPacket: unknown;
  orientation: OrientationContract | null;
  sessionId?: string;
}

export function ViewportRenderer({ showPacket, orientation, sessionId }: Props) {
  if (!showPacket) {
    return (
      <div className="p-3 text-xs text-[var(--text-secondary)]">
        Loading viewports...
      </div>
    );
  }

  const packet = showPacket as Record<string, unknown>;
  const viewports = (packet.viewports as Array<Record<string, unknown>>) ?? [];
  const actions = orientation?.available_actions ?? [];

  return (
    <div className="flex flex-col gap-2 p-3">
      {viewports.map((vp, i) => (
        <ViewportCard key={i} viewport={vp} sessionId={sessionId} />
      ))}
      {actions.length > 0 && <ActionPalette actions={actions} />}
    </div>
  );
}

function ViewportCard({
  viewport,
  sessionId,
}: {
  viewport: Record<string, unknown>;
  sessionId?: string;
}) {
  const kind = (viewport.kind as string) ?? "unknown";
  const title =
    kind.charAt(0).toUpperCase() + kind.slice(1).replace(/_/g, " ");
  const data = viewport.data as Record<string, unknown> | undefined;

  const KNOWN_KINDS = [
    "focus",
    "object",
    "diff",
    "gates",
    "taxonomy",
    "impact",
    "action_surface",
    "coverage",
  ];

  return (
    <div className="rounded border border-[var(--border-secondary)] bg-[var(--bg-secondary)]">
      <div className="px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] border-b border-[var(--border-secondary)]">
        {title}
      </div>
      <div className="p-3 text-xs text-[var(--text-secondary)]">
        {kind === "focus" && <FocusView data={data} />}
        {kind === "object" && <ObjectView data={data} />}
        {kind === "diff" && <DiffView data={data} />}
        {kind === "gates" && <GatesView data={data} />}
        {kind === "taxonomy" && <TaxonomyTree data={data} />}
        {kind === "impact" && <ImpactGraph data={data} />}
        {kind === "action_surface" && (
          <ActionSurface data={data} sessionId={sessionId} />
        )}
        {kind === "coverage" && <CoverageMap data={data} />}
        {!KNOWN_KINDS.includes(kind) && (
          <pre className="whitespace-pre-wrap text-[10px]">
            {JSON.stringify(data, null, 2)}
          </pre>
        )}
      </div>
    </div>
  );
}

function FocusView({ data }: { data: Record<string, unknown> | undefined }) {
  if (!data) return null;
  const objects =
    (data.objects as Array<Record<string, unknown>>) ?? [];
  return (
    <div className="space-y-1.5">
      {objects.map((obj, i) => (
        <div key={i} className="flex items-center gap-2">
          <span className="px-1.5 py-0.5 rounded bg-[var(--bg-active)] text-[10px] font-mono">
            {String(obj.object_type)}
          </span>
          <span className="text-[var(--text-primary)]">{String(obj.fqn)}</span>
        </div>
      ))}
      {objects.length === 0 && <span>No objects in focus</span>}
    </div>
  );
}

function ObjectView({ data }: { data: Record<string, unknown> | undefined }) {
  if (!data?.objects) return <span>No objects</span>;
  const objects = data.objects as Array<Record<string, unknown>>;
  return (
    <div className="space-y-2">
      {objects.map((obj, i) => (
        <details key={i} className="group">
          <summary className="cursor-pointer font-medium text-[var(--text-primary)]">
            {String(obj.fqn ?? obj.object_type ?? `Object ${i}`)}
          </summary>
          <div className="mt-1 ml-2 space-y-0.5">
            {obj.definition &&
              Object.entries(
                obj.definition as Record<string, unknown>,
              ).map(([k, v]) => (
                <div key={k} className="flex gap-2">
                  <span className="text-[var(--text-muted)] shrink-0">
                    {k}:
                  </span>
                  <span className="text-[var(--text-primary)]">
                    {String(v)}
                  </span>
                </div>
              ))}
          </div>
        </details>
      ))}
    </div>
  );
}

function DiffView({ data }: { data: Record<string, unknown> | undefined }) {
  if (!data?.fields) return <span>No diff data</span>;
  const fields = data.fields as Array<Record<string, unknown>>;
  return (
    <div className="grid grid-cols-3 gap-1 text-[10px]">
      <span className="font-medium">Field</span>
      <span className="font-medium">Active</span>
      <span className="font-medium">Draft</span>
      {fields.map((f, i) => (
        <div key={i} className="contents">
          <span className="font-mono">{String(f.field)}</span>
          <span
            className={
              f.change_type === "removed" ? "text-red-400 line-through" : ""
            }
          >
            {String(f.active ?? "\u2014")}
          </span>
          <span
            className={
              f.change_type === "added" ? "text-green-400" : "font-medium"
            }
          >
            {String(f.draft ?? "\u2014")}
          </span>
        </div>
      ))}
    </div>
  );
}

function GatesView({ data }: { data: Record<string, unknown> | undefined }) {
  if (!data?.results) return <span>No gate results</span>;
  const results = data.results as Array<Record<string, unknown>>;
  const severityColor: Record<string, string> = {
    block: "border-red-500/40 bg-red-500/10",
    warning: "border-amber-500/40 bg-amber-500/10",
    advisory: "border-[var(--border-secondary)]",
  };
  return (
    <div className="space-y-1.5">
      {results.map((g, i) => (
        <div
          key={i}
          className={`rounded border p-2 ${severityColor[String(g.severity)] ?? ""}`}
        >
          <div className="font-medium">{String(g.guardrail_id)}</div>
          <div className="text-[var(--text-secondary)]">
            {String(g.message)}
          </div>
          {g.remediation && (
            <div className="mt-1 text-[var(--text-muted)]">
              Fix: {String(g.remediation)}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

function ActionPalette({ actions }: { actions: ActionDescriptor[] }) {
  const enabled = actions.filter((a) => a.enabled).slice(0, 12);
  if (enabled.length === 0) return null;

  return (
    <div className="rounded border border-[var(--border-secondary)] bg-[var(--bg-secondary)]">
      <div className="px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] border-b border-[var(--border-secondary)]">
        Actions
      </div>
      <div className="p-2 flex flex-wrap gap-1">
        {enabled.map((a) => (
          <button
            key={a.action_id}
            className="px-2 py-1 text-[10px] rounded bg-[var(--bg-active)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
            title={a.action_id}
          >
            {a.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export default ViewportRenderer;
