/**
 * ChatSidebar - Session list sidebar
 */

import { useQuery, useMutation } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { Plus, MessageSquare, Trash2, Loader2, PanelLeftOpen, PanelLeftClose } from "lucide-react";
import { useState } from "react";
import { chatApi } from "../../../api/chat";
import { queryKeys, queryClient } from "../../../lib/query";
import { cn, formatDate } from "../../../lib/utils";
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
      queryClient.setQueryData(
        queryKeys.chat.session(newSession.id),
        newSession,
      );
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

  // Resume session mutation — creates a new session with the old session's scope
  const resumeMutation = useMutation({
    mutationFn: (oldSession: ChatSessionSummary) =>
      chatApi.resumeSession(oldSession),
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.chat.sessions() });
      navigate(`/chat/${result.session.id}`);
    },
  });

  const handleSessionClick = (session: ChatSessionSummary) => {
    if (session.client_group_name || session.workspace) {
      // Has saved context — resume by creating a fresh session with same scope
      resumeMutation.mutate(session);
    } else {
      // No saved context — just navigate to it (will show initial prompt)
      navigate(`/chat/${session.id}`);
    }
  };

  const handleDelete = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (confirm("Delete this session?")) {
      deleteMutation.mutate(id);
    }
  };

  const [expanded, setExpanded] = useState(false);

  if (!expanded) {
    return (
      <div className={cn("flex flex-col w-10 flex-shrink-0 border-r border-[var(--border-primary)] bg-[var(--bg-secondary)] items-center py-2 gap-1", className)}>
        <button
          onClick={() => setExpanded(true)}
          className="rounded p-1.5 text-[var(--text-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          title="Show sessions"
        >
          <PanelLeftOpen size={16} />
        </button>
        <button
          onClick={() => createMutation.mutate()}
          disabled={createMutation.isPending}
          className="rounded p-1.5 text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50"
          title="New session"
        >
          {createMutation.isPending ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <Plus size={14} />
          )}
        </button>
        <div className="flex-1 overflow-auto flex flex-col items-center gap-1 mt-1">
          {(sessions ?? []).map((session: ChatSessionSummary) => (
            <button
              key={session.id}
              onClick={() => handleSessionClick(session)}
              className={cn(
                "rounded p-1.5 transition-colors",
                sessionId === session.id
                  ? "bg-[var(--accent-blue)]/10 text-[var(--accent-blue)]"
                  : "text-[var(--text-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]",
              )}
              title={session.client_group_name || session.title || `Session ${session.id.slice(0, 8)}`}
            >
              <MessageSquare size={14} />
            </button>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className={cn("flex flex-col w-56 flex-shrink-0 border-r border-[var(--border-primary)] bg-[var(--bg-secondary)]", className)}>
      <div className="flex items-center justify-between border-b border-[var(--border-primary)] px-3 py-2">
        <h2 className="text-sm font-semibold text-[var(--text-primary)]">Sessions</h2>
        <div className="flex items-center gap-1">
          <button
            onClick={() => createMutation.mutate()}
            disabled={createMutation.isPending}
            className="rounded p-1 text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50"
            title="New session"
          >
            {createMutation.isPending ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <Plus size={14} />
            )}
          </button>
          <button
            onClick={() => setExpanded(false)}
            className="rounded p-1 text-[var(--text-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            title="Collapse"
          >
            <PanelLeftClose size={14} />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-1.5">
        {isLoading ? (
          <div className="flex items-center justify-center py-6">
            <Loader2 className="h-5 w-5 animate-spin text-[var(--text-muted)]" />
          </div>
        ) : sessions && sessions.length > 0 ? (
          <div className="space-y-0.5">
            {sessions.map((session: ChatSessionSummary) => (
              <button
                key={session.id}
                onClick={() => handleSessionClick(session)}
                className={cn(
                  "group flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left transition-colors",
                  sessionId === session.id
                    ? "bg-[var(--accent-blue)]/10 text-[var(--text-primary)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]",
                )}
              >
                <MessageSquare size={14} className="mt-0.5 flex-shrink-0" />
                <div className="flex-1 min-w-0">
                  <p className="text-xs font-medium truncate">
                    {session.client_group_name
                      ? `${session.client_group_name}${session.workspace ? ` / ${session.workspace}` : ""}`
                      : session.title || `Session ${session.id.slice(0, 8)}`}
                  </p>
                  <p className="text-[10px] text-[var(--text-muted)]">
                    {formatDate(session.updated_at)} · {session.message_count} msgs
                  </p>
                </div>
                <button
                  onClick={(e) => handleDelete(e, session.id)}
                  className="opacity-0 group-hover:opacity-100 rounded p-0.5 text-[var(--text-muted)] hover:bg-[var(--bg-tertiary)] hover:text-[var(--accent-red)]"
                  title="Delete session"
                >
                  <Trash2 size={12} />
                </button>
              </button>
            ))}
          </div>
        ) : (
          <div className="py-6 text-center text-xs text-[var(--text-muted)]">
            <p>No sessions yet</p>
            <button
              onClick={() => createMutation.mutate()}
              className="mt-1 text-[var(--accent-blue)] hover:underline"
            >
              Create first session
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

export default ChatSidebar;
