/**
 * Chat Page - Agent chat UI
 */

import { useNavigate, useParams } from "react-router-dom";
import { useQuery, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useCallback, useMemo, useState } from "react";
import { Loader2, BookOpen, Telescope } from "lucide-react";
import { chatApi } from "../../api/chat";
import { scopeApi, type CbuSummary } from "../../api/scope";
import { isSessionMissingError } from "../../api/sessionStorage";
import { queryKeys, queryClient } from "../../lib/query";
import { useChatStore } from "../../stores/chat";
import {
  ChatSidebar,
  ChatMessage,
  ChatInput,
  ConstellationPanel,
  ScopePanel,
  VerbBrowser,
} from "./components";
import { RunbookPlanReview } from "./RunbookPlanReview";
import { runbookPlanApi } from "../../api/runbookPlan";
import { DealPanel } from "../deal/components";
import type { DecisionReply, DiscoverySelection } from "../../types/chat";
import type { SessionFeedback } from "../../api/replV2";

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId?: string }>();
  const navigate = useNavigate();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const [selectedCbu, setSelectedCbu] = useState<CbuSummary | null>(null);
  const [showRunbookPlan, setShowRunbookPlan] = useState(false);
  const {
    setCurrentSession,
    currentSession,
    addMessage,
    isStreaming,
    setStreaming,
    setAvailableVerbs,
  } = useChatStore();

  // Fetch session if ID is provided
  const { data, isLoading, error } = useQuery({
    queryKey: queryKeys.chat.session(sessionId || ""),
    queryFn: () => chatApi.getSession(sessionId!),
    enabled: !!sessionId,
  });
  const sessionMissing = isSessionMissingError(error);

  const { data: scopeData } = useQuery({
    queryKey: queryKeys.scope(sessionId || ""),
    queryFn: () => scopeApi.getScope(sessionId!),
    enabled: !!sessionId,
    retry: (failureCount, err) =>
      !isSessionMissingError(err) && failureCount < 2,
  });

  useEffect(() => {
    if (!sessionId || !sessionMissing) return;

    queryClient.removeQueries({
      queryKey: queryKeys.chat.session(sessionId),
    });
    queryClient.invalidateQueries({
      queryKey: queryKeys.chat.sessions(),
    });
    navigate("/chat", { replace: true });
  }, [navigate, sessionId, sessionMissing]);

  // Update store when data changes
  useEffect(() => {
    if (data) {
      setCurrentSession(data);
    } else if (!sessionId) {
      setCurrentSession(null);
    }
  }, [data, sessionId, setCurrentSession]);

  useEffect(() => {
    if (!scopeData?.cbus?.length) {
      setSelectedCbu(null);
      return;
    }

    setSelectedCbu((current) => {
      if (current && scopeData.cbus.some((cbu) => cbu.id === current.id)) {
        return current;
      }
      return scopeData.cbus[0] ?? null;
    });
  }, [scopeData]);

  // Fetch verb surface when session loads
  useEffect(() => {
    if (!sessionId) return;
    chatApi
      .getVerbSurface(sessionId)
      .then((result) => {
        if (result.verbs.length > 0) {
          setAvailableVerbs(
            result.verbs,
            result.surface_fingerprint
              ? {
                  fingerprint: result.surface_fingerprint,
                  totalRegistry: result.totalRegistry ?? 0,
                  finalCount: result.verbs.length,
                }
              : undefined,
          );
        }
      })
      .catch((err) => {
        console.warn("[ChatPage] getVerbSurface failed:", err);
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
      // Optimistically add user message
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
            ? {
                fingerprint: response.surface_fingerprint,
                totalRegistry: 0,
                finalCount: response.available_verbs.length,
              }
            : undefined,
        );
      }
      if (sessionId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.scope(sessionId) });
        queryClient.invalidateQueries({
          queryKey: queryKeys.constellation.all,
        });
      }
      setStreaming(false);
    },
    onError: (err) => {
      console.error("[ChatPage] sendMessage failed:", err);
      setStreaming(false);
    },
  });

  // Decision reply mutation
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
            ? {
                fingerprint: response.surface_fingerprint,
                totalRegistry: 0,
                finalCount: response.available_verbs.length,
              }
            : undefined,
        );
      }
      queryClient.invalidateQueries({
        queryKey: queryKeys.chat.session(sessionId!),
      });
      queryClient.invalidateQueries({
        queryKey: queryKeys.scope(sessionId!),
      });
      queryClient.invalidateQueries({
        queryKey: queryKeys.constellation.all,
      });
    },
  });

  const discoverySelectionMutation = useMutation({
    mutationFn: (selection: DiscoverySelection) => {
      if (!sessionId) throw new Error("No session selected");
      return chatApi.sendDiscoverySelection(sessionId, selection);
    },
    onMutate: (selection) => {
      addMessage({
        id: `temp-discovery-${Date.now()}`,
        role: "user",
        content: selection.label || selection.value || selection.selection_id,
        timestamp: new Date().toISOString(),
      });
      setStreaming(true);
    },
    onSuccess: (response) => {
      addMessage(response.message);
      if (response.available_verbs?.length) {
        setAvailableVerbs(
          response.available_verbs,
          response.surface_fingerprint
            ? {
                fingerprint: response.surface_fingerprint,
                totalRegistry: 0,
                finalCount: response.available_verbs.length,
              }
            : undefined,
        );
      }
      if (sessionId) {
        queryClient.invalidateQueries({
          queryKey: queryKeys.chat.session(sessionId),
        });
        queryClient.invalidateQueries({ queryKey: queryKeys.scope(sessionId) });
        queryClient.invalidateQueries({
          queryKey: queryKeys.constellation.all,
        });
      }
      setStreaming(false);
    },
    onError: (err) => {
      console.error("[ChatPage] sendDiscoverySelection failed:", err);
      setStreaming(false);
    },
  });

  const handleSend = useCallback(
    (message: string) => {
      sendMutation.mutate(message);
    },
    [sendMutation],
  );

  const latestSessionFeedback = useMemo<SessionFeedback | undefined>(() => {
    const messages = currentSession?.messages ?? [];
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      const feedback = messages[index]?.session_feedback;
      if (feedback) {
        return feedback;
      }
    }
    return undefined;
  }, [currentSession?.messages]);

  const handleVerbSubmit = useCallback(
    (sexpr: string) => {
      sendMutation.mutate(sexpr);
    },
    [sendMutation],
  );

  const handleCancel = useCallback(() => {
    // TODO: Implement streaming cancellation
    setStreaming(false);
  }, [setStreaming]);

  const handleCompileRunbook = useCallback(async () => {
    if (!sessionId) return;
    try {
      await runbookPlanApi.compileRunbookPlan(sessionId);
      setShowRunbookPlan(true);
    } catch (e) {
      console.warn("[ChatPage] compileRunbookPlan failed:", e);
      // Still show the panel so the user can see the error
      setShowRunbookPlan(true);
    }
  }, [sessionId]);

  const handleDecisionReply = useCallback(
    (packetId: string, reply: unknown) => {
      replyMutation.mutate({
        packet_id: packetId,
        ...(reply as object),
      });
    },
    [replyMutation],
  );

  const handleDiscoverySelection = useCallback(
    (selection: DiscoverySelection) => {
      discoverySelectionMutation.mutate(selection);
    },
    [discoverySelectionMutation],
  );

  return (
    <div className="flex h-full">
      {/* Left sidebar - Session list */}
      <ChatSidebar className="w-64 flex-shrink-0 border-r border-[var(--border-primary)] bg-[var(--bg-secondary)]" />

      {/* Main chat area */}
      <div className="flex flex-1 flex-col">
        {/* Messages */}
        <div className="flex-1 overflow-auto p-4">
          {isLoading ? (
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
              <div className="text-center">
                <h2 className="text-xl font-semibold text-[var(--text-primary)]">
                  Start a Conversation
                </h2>
                <p className="mt-2 text-[var(--text-secondary)]">
                  Select a session from the sidebar or create a new one.
                </p>
              </div>
            </div>
          ) : (
            <div className="space-y-4 max-w-3xl mx-auto">
              {currentSession.messages.length === 0 ? (
                <div className="py-12 text-center">
                  <h3 className="text-lg font-medium text-[var(--text-primary)]">
                    Loading...
                  </h3>
                  <p className="mt-2 text-sm text-[var(--text-secondary)]">
                    Setting up your session.
                  </p>
                </div>
              ) : (
                currentSession.messages.map((message) => (
                  <ChatMessage
                    key={message.id}
                    message={message}
                    onDecisionReply={handleDecisionReply}
                    onDiscoverySelection={handleDiscoverySelection}
                    onSendMessage={(msg) => sendMutation.mutate(msg)}
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
          disabled={
            !sessionId ||
            sendMutation.isPending ||
            discoverySelectionMutation.isPending
          }
          placeholder={
            sessionId ? "Type a message..." : "Select or create a session first"
          }
        />
      </div>

      {/* Right sidebar - Scope, Verbs, and Deal panels */}
      {!sessionMissing && (
        <div className="w-[32rem] flex-shrink-0 border-l border-[var(--border-primary)] bg-[var(--bg-secondary)] flex flex-col overflow-hidden">
          {/* Deal context panel */}
          {sessionId && <DealPanel sessionId={sessionId} />}

          {/* Scope panel showing loaded CBUs */}
          <ScopePanel
            sessionId={sessionId}
            selectedCbuId={selectedCbu?.id ?? null}
            onSelectCbu={setSelectedCbu}
          />

          <ConstellationPanel
            selectedCbu={selectedCbu}
            sessionFeedback={latestSessionFeedback}
            className="min-h-0"
            onPromptAgent={handleSend}
          />

          {/* Runbook plan + Observatory controls */}
          {sessionId && (
            <div className="flex items-center gap-2 border-t border-[var(--border-primary)] px-3 py-2">
              <button
                onClick={showRunbookPlan ? () => setShowRunbookPlan(false) : handleCompileRunbook}
                className="flex items-center gap-1.5 rounded px-2.5 py-1.5 text-xs font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] border border-[var(--border-secondary)]"
              >
                <BookOpen size={14} />
                {showRunbookPlan ? "Hide Plan" : "Compile Plan"}
              </button>
              <button
                onClick={() => navigate(`/observatory/${sessionId}`)}
                className="flex items-center gap-1.5 rounded px-2.5 py-1.5 text-xs font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] border border-[var(--border-secondary)]"
                title="Open Observatory"
              >
                <Telescope size={14} />
                Observatory
              </button>
            </div>
          )}

          {/* Runbook plan review panel */}
          {sessionId && showRunbookPlan && (
            <div className="border-t border-[var(--border-primary)] overflow-auto max-h-[40vh] p-3">
              <RunbookPlanReview
                sessionId={sessionId}
                onApproved={() => {
                  if (sessionId) {
                    queryClient.invalidateQueries({ queryKey: queryKeys.chat.session(sessionId) });
                    queryClient.invalidateQueries({ queryKey: queryKeys.constellation.all });
                  }
                }}
                onCancelled={() => setShowRunbookPlan(false)}
                onCompleted={() => {
                  if (sessionId) {
                    queryClient.invalidateQueries({ queryKey: queryKeys.chat.session(sessionId) });
                    queryClient.invalidateQueries({ queryKey: queryKeys.constellation.all });
                    queryClient.invalidateQueries({ queryKey: queryKeys.scope(sessionId) });
                  }
                }}
              />
            </div>
          )}

          {/* Available commands / verb browser */}
          <VerbBrowser
            className="border-t border-[var(--border-primary)]"
            onVerbSubmit={handleVerbSubmit}
          />
        </div>
      )}
    </div>
  );
}

export default ChatPage;
