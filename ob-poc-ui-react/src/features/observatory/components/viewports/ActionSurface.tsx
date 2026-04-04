/**
 * ActionSurface — table of valid and blocked actions with invoke support.
 *
 * Two sections: valid_actions (enabled, invoke button) and
 * blocked_actions (disabled, reason tooltip).
 */

import { observatoryApi } from "../../../../api/observatory";
import { queryClient } from "../../../../lib/query";

interface ActionEntry {
  verb_fqn: string;
  label: string;
  description?: string;
  reason?: string;
}

interface Props {
  data: unknown;
  sessionId?: string;
}

export function ActionSurface({ data, sessionId }: Props) {
  if (!data || typeof data !== "object") {
    return (
      <div className="text-xs text-[var(--text-secondary)]">
        No action data
      </div>
    );
  }

  const root = data as Record<string, unknown>;
  const validActions = (root.valid_actions as ActionEntry[]) ?? [];
  const blockedActions = (root.blocked_actions as ActionEntry[]) ?? [];

  const handleInvoke = async (verbFqn: string) => {
    if (!sessionId) return;
    try {
      await observatoryApi.navigate(sessionId, verbFqn, {});
      queryClient.invalidateQueries({
        queryKey: ["observatory"],
      });
    } catch (err) {
      console.error("Action invoke failed:", err);
    }
  };

  return (
    <div className="space-y-3">
      {validActions.length > 0 && (
        <div>
          <div className="text-[10px] font-semibold uppercase text-[var(--text-muted)] mb-1">
            Available
          </div>
          <table className="w-full text-xs">
            <tbody>
              {validActions.map((a) => (
                <tr
                  key={a.verb_fqn}
                  className="border-b border-[var(--border-secondary)] last:border-b-0"
                >
                  <td className="py-1 pr-2 text-[var(--text-primary)]">
                    <div className="font-medium">{a.label}</div>
                    {a.description && (
                      <div className="text-[10px] text-[var(--text-muted)]">
                        {a.description}
                      </div>
                    )}
                  </td>
                  <td className="py-1 text-right">
                    <button
                      onClick={() => handleInvoke(a.verb_fqn)}
                      disabled={!sessionId}
                      className="px-2 py-0.5 rounded text-[10px] font-medium bg-[var(--bg-active)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)] disabled:opacity-40 disabled:cursor-not-allowed"
                    >
                      Invoke
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {blockedActions.length > 0 && (
        <div>
          <div className="text-[10px] font-semibold uppercase text-[var(--text-muted)] mb-1">
            Blocked
          </div>
          <table className="w-full text-xs">
            <tbody>
              {blockedActions.map((a) => (
                <tr
                  key={a.verb_fqn}
                  className="border-b border-[var(--border-secondary)] last:border-b-0 opacity-50"
                >
                  <td className="py-1 pr-2 text-[var(--text-secondary)]">
                    <div>{a.label}</div>
                  </td>
                  <td className="py-1 text-right">
                    <span
                      className="px-2 py-0.5 rounded text-[10px] bg-[var(--bg-secondary)] text-[var(--text-muted)] cursor-help"
                      title={a.reason ?? "Blocked"}
                    >
                      Blocked
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {validActions.length === 0 && blockedActions.length === 0 && (
        <div className="text-xs text-[var(--text-secondary)]">No actions</div>
      )}
    </div>
  );
}

export default ActionSurface;
