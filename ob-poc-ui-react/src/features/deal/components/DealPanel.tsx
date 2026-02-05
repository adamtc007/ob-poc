/**
 * Deal Panel - Shows current deal context in ChatPage sidebar
 *
 * Displays the currently loaded deal from session context.
 * Clicking navigates to the full DealPage.
 */

import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { FileText, ChevronRight, Loader2, X } from "lucide-react";
import { dealApi } from "../../../api/deal";
import { queryKeys } from "../../../lib/query";

interface DealPanelProps {
  sessionId: string;
  onUnloadDeal?: () => void;
}

export function DealPanel({ sessionId, onUnloadDeal }: DealPanelProps) {
  const navigate = useNavigate();

  const { data: dealContext, isLoading } = useQuery({
    queryKey: queryKeys.deals.sessionContext(sessionId),
    queryFn: () => dealApi.getSessionDealContext(sessionId),
    refetchInterval: 5000, // Refresh every 5 seconds
  });

  // Don't render if no deal context
  if (isLoading) {
    return (
      <div className="p-3 border-b border-[var(--border-primary)]">
        <div className="flex items-center gap-2 text-[var(--text-muted)]">
          <Loader2 size={14} className="animate-spin" />
          <span className="text-sm">Loading deal context...</span>
        </div>
      </div>
    );
  }

  if (!dealContext?.deal_id) {
    return null;
  }

  const handleNavigate = () => {
    navigate(`/deal/${dealContext.deal_id}`);
  };

  return (
    <div className="p-3 border-b border-[var(--border-primary)] bg-[var(--bg-secondary)]">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
          Deal Context
        </span>
        {onUnloadDeal && (
          <button
            onClick={onUnloadDeal}
            className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
            title="Unload deal"
          >
            <X size={12} />
          </button>
        )}
      </div>
      <button
        onClick={handleNavigate}
        className="w-full flex items-center gap-2 p-2 rounded-md bg-[var(--bg-tertiary)] hover:bg-[var(--bg-hover)] transition-colors group"
      >
        <FileText
          size={16}
          className="text-[var(--accent-blue)] flex-shrink-0"
        />
        <div className="flex-1 text-left min-w-0">
          <div className="text-sm font-medium text-[var(--text-primary)] truncate">
            {dealContext.deal_name}
          </div>
          {dealContext.deal_status && (
            <div className="text-xs text-[var(--text-muted)]">
              {dealContext.deal_status}
            </div>
          )}
        </div>
        <ChevronRight
          size={14}
          className="text-[var(--text-muted)] group-hover:text-[var(--text-primary)] flex-shrink-0"
        />
      </button>
    </div>
  );
}

export default DealPanel;
