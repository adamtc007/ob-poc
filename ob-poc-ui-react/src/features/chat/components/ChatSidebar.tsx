/**
 * ChatSidebar - Session list sidebar
 */

import { useQuery, useMutation } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { Plus, MessageSquare, Trash2, Loader2 } from "lucide-react";
import { chatApi } from "../../../api/chat";
import { queryKeys, queryClient } from "../../../lib/query";
import { cn, formatDate, truncate } from "../../../lib/utils";
import type { ChatSessionSummary } from "../../../types/chat";

interface ChatSidebarProps {
  className?: string;
}

export function ChatSidebar({ className }: ChatSidebarProps) {
  const navigate = useNavigate();
  const { sessionId } = useParams<{ sessionId?: string }>();

  // Fetch sessions
  const { data: sessions, isLoading } = useQuery({
    queryKey: queryKeys.chat.sessions(),
    queryFn: chatApi.listSessions,
  });

  // Create session mutation
  const createMutation = useMutation({
    mutationFn: () => chatApi.createSession(),
    onSuccess: (newSession) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.chat.sessions() });
      navigate(`/chat/${newSession.id}`);
    },
  });

  // Delete session mutation
  const deleteMutation = useMutation({
    mutationFn: (id: string) => chatApi.deleteSession(id),
    onSuccess: (_data, deletedId) => {
      // Navigate away FIRST if we deleted the current session (prevents ChatPage refetch)
      if (sessionId === deletedId) {
        navigate("/chat");
      }
      // Remove the deleted session's query from cache so it can't be refetched
      queryClient.removeQueries({
        queryKey: queryKeys.chat.session(deletedId),
      });
      // Refresh the session list
      queryClient.invalidateQueries({ queryKey: queryKeys.chat.sessions() });
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
        <h2 className="font-semibold text-[var(--text-primary)]">Sessions</h2>
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
                onClick={() => navigate(`/chat/${session.id}`)}
                className={cn(
                  "group flex w-full items-start gap-2 rounded-lg px-3 py-2 text-left transition-colors",
                  sessionId === session.id
                    ? "bg-[var(--accent-blue)]/10 text-[var(--text-primary)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]",
                )}
              >
                <MessageSquare size={16} className="mt-0.5 flex-shrink-0" />
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">
                    {session.title || "Untitled Session"}
                  </p>
                  {session.last_message_preview && (
                    <p className="text-xs text-[var(--text-muted)] truncate">
                      {truncate(session.last_message_preview, 50)}
                    </p>
                  )}
                  <p className="text-xs text-[var(--text-muted)]">
                    {formatDate(session.updated_at)} · {session.message_count}{" "}
                    messages
                  </p>
                </div>
                <button
                  onClick={(e) => handleDelete(e, session.id)}
                  className="opacity-0 group-hover:opacity-100 rounded p-1 text-[var(--text-muted)] hover:bg-[var(--bg-tertiary)] hover:text-[var(--accent-red)]"
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
              Create your first session
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

export default ChatSidebar;
