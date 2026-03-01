/**
 * SemOsPage - Semantic OS agent interface
 *
 * Two-column layout reusing chat components for message display and input.
 * Auto-creates a session with workflow_focus="semantic-os" on mount,
 * which triggers a workflow selection decision packet (domain chooser)
 * on the backend.
 */

import { useParams, useNavigate } from "react-router-dom";
import { useQuery, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useCallback } from "react";
import { Loader2 } from "lucide-react";
import { chatApi } from "../../api/chat";
import { queryKeys, queryClient } from "../../lib/query";
import { useChatStore } from "../../stores/chat";
import { ChatMessage, ChatInput, VerbBrowser } from "../chat/components";
import { SemOsContextPanel } from "./components";
import type { DecisionReply } from "../../types/chat";

export function SemOsPage() {
  const { sessionId } = useParams<{ sessionId?: string }>();
  const navigate = useNavigate();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const {
    setCurrentSession,
    currentSession,
    addMessage,
    isStreaming,
    setStreaming,
    setAvailableVerbs,
  } = useChatStore();

  // Auto-create session when visiting /semantic-os with no sessionId
  const createMutation = useMutation({
    mutationFn: () => chatApi.createSession(undefined, "semantic-os"),
    onSuccess: (newSession) => {
      navigate(`/semantic-os/${newSession.id}`, { replace: true });
    },
    onError: (err) => {
      console.error("[SemOsPage] createSession failed:", err);
    },
  });

  useEffect(() => {
    if (!sessionId && !createMutation.isPending && !createMutation.isSuccess) {
      createMutation.mutate();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  // Fetch session if ID is provided (reuses chat session endpoints)
  const { data, isLoading, error } = useQuery({
    queryKey: queryKeys.semOs.session(sessionId || ""),
    queryFn: () => chatApi.getSession(sessionId!),
    enabled: !!sessionId,
  });

  // Update store when data changes
  useEffect(() => {
    if (data) {
      setCurrentSession(data);
    } else if (!sessionId) {
      setCurrentSession(null);
    }
  }, [data, sessionId, setCurrentSession]);

  // Fetch verb surface when session loads
  useEffect(() => {
    if (!sessionId) return;
    chatApi.getVerbSurface(sessionId).then((result) => {
      if (result.verbs.length > 0) {
        setAvailableVerbs(
          result.verbs,
          result.surface_fingerprint
            ? { fingerprint: result.surface_fingerprint, totalRegistry: result.totalRegistry ?? 0, finalCount: result.verbs.length }
            : undefined,
        );
      }
    }).catch((err) => {
      console.warn("[SemOsPage] getVerbSurface failed:", err);
    });
  }, [sessionId, setAvailableVerbs]);

  // Scroll to bottom when messages change
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [currentSession?.messages]);

  // Send message mutation
  const sendMutation = useMutation({
    mutationFn: (message: string) => {
      if (!sessionId) throw new Error("No session selected");
      return chatApi.sendMessage(sessionId, { message });
    },
    onMutate: (message) => {
      addMessage({
        id: `temp-${Date.now()}`,
        role: "user",
        content: message,
        timestamp: new Date().toISOString(),
      });
      setStreaming(true);
    },
    onSuccess: (response) => {
      // Add the assistant response message to the current session.
      // We don't call setCurrentSession(response.session) because the backend
      // may not persist all messages (e.g. /commands) into the session history,
      // which would overwrite our optimistically-added user message and lose
      // the assistant response entirely.
      addMessage(response.message);
      if (response.available_verbs?.length) {
        setAvailableVerbs(
          response.available_verbs,
          response.surface_fingerprint
            ? { fingerprint: response.surface_fingerprint, totalRegistry: 0, finalCount: response.available_verbs.length }
            : undefined,
        );
      }
      setStreaming(false);
    },
    onError: (err) => {
      console.error("[SemOsPage] sendMessage failed:", err);
      setStreaming(false);
    },
  });

  // Decision reply mutation (for workflow selection and other decisions)
  const replyMutation = useMutation({
    mutationFn: (reply: DecisionReply) => {
      if (!sessionId) throw new Error("No session selected");
      return chatApi.replyToDecision(sessionId, reply);
    },
    onSuccess: (response) => {
      addMessage(response.message);
      if (response.available_verbs?.length) {
        setAvailableVerbs(
          response.available_verbs,
          response.surface_fingerprint
            ? { fingerprint: response.surface_fingerprint, totalRegistry: 0, finalCount: response.available_verbs.length }
            : undefined,
        );
      }
      queryClient.invalidateQueries({
        queryKey: queryKeys.semOs.session(sessionId!),
      });
    },
  });

  const handleSend = useCallback(
    (message: string) => {
      sendMutation.mutate(message);
    },
    [sendMutation],
  );

  const handleVerbSubmit = useCallback(
    (sexpr: string) => {
      sendMutation.mutate(sexpr);
    },
    [sendMutation],
  );

  const handleCancel = useCallback(() => {
    setStreaming(false);
  }, [setStreaming]);

  const handleDecisionReply = useCallback(
    (packetId: string, reply: unknown) => {
      replyMutation.mutate({
        packet_id: packetId,
        ...(reply as object),
      });
    },
    [replyMutation],
  );

  return (
    <div className="flex h-full">
      {/* Main chat area */}
      <div className="flex flex-1 flex-col">
        {/* Messages */}
        <div className="flex-1 overflow-auto p-4">
          {!sessionId || createMutation.isPending ? (
            <div className="flex h-full items-center justify-center">
              <div className="text-center">
                <Loader2 className="mx-auto h-8 w-8 animate-spin text-[var(--accent-blue)]" />
                <p className="mt-3 text-sm text-[var(--text-secondary)]">
                  Starting Semantic OS session...
                </p>
              </div>
            </div>
          ) : createMutation.isError ? (
            <div className="flex h-full items-center justify-center">
              <p className="text-[var(--accent-red)]">
                Failed to create session. Please refresh and try again.
              </p>
            </div>
          ) : isLoading ? (
            <div className="flex h-full items-center justify-center">
              <Loader2 className="h-8 w-8 animate-spin text-[var(--accent-blue)]" />
            </div>
          ) : error ? (
            <div className="flex h-full items-center justify-center">
              <p className="text-[var(--accent-red)]">
                {error instanceof Error
                  ? error.message
                  : "Failed to load session"}
              </p>
            </div>
          ) : !currentSession ? (
            <div className="flex h-full items-center justify-center">
              <Loader2 className="h-8 w-8 animate-spin text-[var(--accent-blue)]" />
            </div>
          ) : (
            <div className="mx-auto max-w-3xl space-y-4">
              {currentSession.messages.length === 0 ? (
                <div className="py-12 text-center">
                  <h3 className="text-lg font-medium text-[var(--text-primary)]">
                    Loading...
                  </h3>
                  <p className="mt-2 text-sm text-[var(--text-secondary)]">
                    Setting up your Semantic OS session.
                  </p>
                </div>
              ) : (
                currentSession.messages.map((message) => (
                  <ChatMessage
                    key={message.id}
                    message={message}
                    onDecisionReply={handleDecisionReply}
                  />
                ))
              )}

              {/* Streaming indicator */}
              {isStreaming && (
                <div className="flex items-center gap-2 text-[var(--text-muted)]">
                  <Loader2 size={16} className="animate-spin" />
                  <span className="text-sm">Thinking...</span>
                </div>
              )}

              <div ref={messagesEndRef} />
            </div>
          )}
        </div>

        {/* Input area */}
        <ChatInput
          onSend={handleSend}
          onCancel={handleCancel}
          isStreaming={isStreaming}
          disabled={!sessionId || sendMutation.isPending}
          placeholder={
            sessionId
              ? "Type a message..."
              : "Select or create a session first"
          }
        />
      </div>

      {/* Right sidebar - Registry context + Verb browser */}
      <div className="w-64 flex-shrink-0 border-l border-[var(--border-primary)] bg-[var(--bg-secondary)] flex flex-col overflow-hidden">
        <SemOsContextPanel className="" />
        <VerbBrowser className="border-t border-[var(--border-primary)]" onVerbSubmit={handleVerbSubmit} />
      </div>
    </div>
  );
}

export default SemOsPage;
