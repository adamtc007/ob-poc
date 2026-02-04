/**
 * Breadcrumbs - Navigation trail component
 */

import { ChevronRight, ChevronLeft } from "lucide-react";
import { useInspectorStore } from "../../../stores/inspector";
import { cn } from "../../../lib/utils";

interface BreadcrumbsProps {
  className?: string;
}

export function Breadcrumbs({ className }: BreadcrumbsProps) {
  const {
    getBreadcrumbs,
    goBack,
    goForward,
    canGoBack,
    canGoForward,
    focusNode,
  } = useInspectorStore();

  const breadcrumbs = getBreadcrumbs();

  return (
    <div className={cn("flex items-center gap-2", className)}>
      {/* Back/Forward buttons */}
      <div className="flex items-center gap-1">
        <button
          onClick={goBack}
          disabled={!canGoBack()}
          className={cn(
            "rounded p-1 transition-colors",
            canGoBack()
              ? "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              : "text-[var(--text-muted)] cursor-not-allowed",
          )}
          title="Go back (Backspace)"
        >
          <ChevronLeft size={18} />
        </button>
        <button
          onClick={goForward}
          disabled={!canGoForward()}
          className={cn(
            "rounded p-1 transition-colors",
            canGoForward()
              ? "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              : "text-[var(--text-muted)] cursor-not-allowed",
          )}
          title="Go forward"
        >
          <ChevronRight size={18} />
        </button>
      </div>

      {/* Breadcrumb trail */}
      <div className="flex items-center gap-1 overflow-hidden">
        {breadcrumbs.map((crumb, index) => (
          <div key={crumb.nodeId} className="flex items-center">
            {index > 0 && (
              <ChevronRight
                size={14}
                className="mx-1 text-[var(--text-muted)]"
              />
            )}
            <button
              onClick={() => focusNode(crumb.nodeId, crumb.label)}
              className={cn(
                "rounded px-1.5 py-0.5 text-sm truncate max-w-[150px] transition-colors",
                index === breadcrumbs.length - 1
                  ? "text-[var(--text-primary)] font-medium"
                  : "text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]",
              )}
              title={crumb.label}
            >
              {crumb.label}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

export default Breadcrumbs;
