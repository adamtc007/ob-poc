/**
 * SemOsSidebar - Session list sidebar for Semantic OS
 */

import { useQuery, useMutation } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { Plus, MessageSquare, Trash2, Loader2 } from "lucide-react";
import { chatApi } from "../../../api/chat";
import { queryKeys, queryClient } from "../../../lib/query";
import { cn, formatDate, truncate } from "../../../lib/utils";
import type { ChatSessionSummary } from "../../../types/chat";

const STORAGE_KEY = "ob-poc-semos-sessions";

/** SemOS-specific session list (separate localStorage from chat) */
function listSemOsSessions(): Promise<ChatSessionSummary[]> {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored) {
    try {
      return Promise.resolve(JSON.parse(stored));
    } catch {
      return Promise.resolve([]);
    }
  }
  return Promise.resolve([]);
}

/** Store a new SemOS session in localStorage */
function storeSemOsSession(session: ChatSessionSummary) {
  const stored = localStorage.getItem(STORAGE_KEY);
  const sessions: ChatSessionSummary[] = stored ? JSON.parse(stored) : [];
  sessions.unshift(session);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(sessions.slice(0, 50)));
}

/** Remove a SemOS session from localStorage */
function removeSemOsSession(id: string) {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored) {
    const sessions: ChatSessionSummary[] = JSON.parse(stored);
    const filtered = sessions.filter((s) => s.id !== id);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(filtered));
  }
}

interface SemOsSidebarProps {
  className?: string;
}

export function SemOsSidebar({ className }: SemOsSidebarProps) {
  const navigate = useNavigate();
  const { sessionId } = useParams<{ sessionId?: string }>();

  // Fetch sessions from SemOS-specific localStorage
  const { data: sessions, isLoading } = useQuery({
    queryKey: queryKeys.semOs.sessions(),
    queryFn: listSemOsSessions,
  });

  // Create session mutation — passes workflow_focus="semantic-os"
  const createMutation = useMutation({
    mutationFn: async () => {
      const session = await chatApi.createSession(undefined, "semantic-os");
      // Store in SemOS-specific localStorage (not the chat one)
      storeSemOsSession({
        id: session.id,
        title: session.title,
        created_at: session.created_at,
        updated_at: session.updated_at,
        message_count: 0,
      });
      return session;
    },
    onSuccess: (newSession) => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.semOs.sessions(),
      });
      navigate(`/semantic-os/${newSession.id}`);
    },
  });

  // Delete session mutation
  const deleteMutation = useMutation({
    mutationFn: async (id: string) => {
      removeSemOsSession(id);
      try {
        await chatApi.deleteSession(id);
      } catch {
        // Best-effort backend cleanup
      }
    },
    onSuccess: (_data, deletedId) => {
      if (sessionId === deletedId) {
        navigate("/semantic-os");
      }
      queryClient.removeQueries({
        queryKey: queryKeys.semOs.session(deletedId),
      });
      queryClient.invalidateQueries({
        queryKey: queryKeys.semOs.sessions(),
      });
    },
  });

  const handleDelete = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (confirm("Delete this session?")) {
      deleteMutation.mutate(id);
    }
  };

  return (
    <div className={cn("flex flex-col", className)}>
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[var(--border-primary)] px-4 py-3">
        <h2 className="font-semibold text-[var(--text-primary)]">
          Semantic OS
        </h2>
        <button
          onClick={() => createMutation.mutate()}
          disabled={createMutation.isPending}
          className="rounded-lg p-1.5 text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50"
          title="New session"
        >
          {createMutation.isPending ? (
            <Loader2 size={18} className="animate-spin" />
          ) : (
            <Plus size={18} />
          )}
        </button>
      </div>

      {/* Session list */}
      <div className="flex-1 overflow-auto p-2">
        {isLoading ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="h-6 w-6 animate-spin text-[var(--text-muted)]" />
          </div>
        ) : sessions && sessions.length > 0 ? (
          <div className="space-y-1">
            {sessions.map((session: ChatSessionSummary) => (
              <button
                key={session.id}
                onClick={() => navigate(`/semantic-os/${session.id}`)}
                className={cn(
                  "group flex w-full items-start gap-2 rounded-lg px-3 py-2 text-left transition-colors",
                  sessionId === session.id
                    ? "bg-[var(--accent-blue)]/10 text-[var(--text-primary)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]",
                )}
              >
                <MessageSquare size={16} className="mt-0.5 flex-shrink-0" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">
                    {session.title || "Untitled Session"}
                  </p>
                  {session.last_message_preview && (
                    <p className="truncate text-xs text-[var(--text-muted)]">
                      {truncate(session.last_message_preview, 50)}
                    </p>
                  )}
                  <p className="text-xs text-[var(--text-muted)]">
                    {formatDate(session.updated_at)}
                  </p>
                </div>
                <button
                  onClick={(e) => handleDelete(e, session.id)}
                  className="rounded p-1 text-[var(--text-muted)] opacity-0 hover:bg-[var(--bg-tertiary)] hover:text-[var(--accent-red)] group-hover:opacity-100"
                  title="Delete session"
                >
                  <Trash2 size={14} />
                </button>
              </button>
            ))}
          </div>
        ) : (
          <div className="py-8 text-center text-sm text-[var(--text-muted)]">
            <p>No sessions yet</p>
            <button
              onClick={() => createMutation.mutate()}
              className="mt-2 text-[var(--accent-blue)] hover:underline"
            >
              Start a new session
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

export default SemOsSidebar;
