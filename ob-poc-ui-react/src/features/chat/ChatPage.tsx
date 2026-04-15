/**
 * Chat Page - Agent chat UI
 */

import { useNavigate, useParams } from "react-router-dom";
import { useQuery, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useCallback, useMemo, useState } from "react";
import { Loader2, BookOpen } from "lucide-react";
import { chatApi } from "../../api/chat";
import { scopeApi, type CbuSummary } from "../../api/scope";
import { observatoryApi } from "../../api/observatory";
import { FlightDeck } from "./components/FlightDeck";
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
import { NarrationPanel } from "./components/NarrationPanel";
import { RunbookPlanReview } from "./RunbookPlanReview";
import { runbookPlanApi } from "../../api/runbookPlan";
import { DealPanel } from "../deal/components";
import { ConstellationCanvas } from "../observatory/components/ConstellationCanvas";
import type { ObservatoryAction } from "../../types/observatory";
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

  // Observatory orientation for Flight Deck + canvas
  const { data: orientation } = useQuery({
    queryKey: queryKeys.observatory.orientation(sessionId!),
    queryFn: () => observatoryApi.getOrientation(sessionId!),
    enabled: !!sessionId,
    refetchInterval: 5000,
  });

  // Graph scene for embedded egui canvas
  const { data: graphScene } = useQuery({
    queryKey: queryKeys.observatory.graphScene(sessionId!),
    queryFn: () => observatoryApi.getGraphScene(sessionId!),
    enabled: !!sessionId,
    refetchInterval: 5000,
  });

  // Handle canvas interactions — route semantic actions through REPL input.
  // Workspace nodes (id starts with "workspace:") are sent as plain labels
  // so the orchestrator's ScopeGate/WorkspaceSelection handlers process them
  // directly — clicking "KYC" on the canvas = typing "KYC" in chat.
  const handleCanvasAction = useCallback(
    async (action: ObservatoryAction) => {
      if (!sessionId) return;

      // Workspace node click — send label as utterance for tollgate processing
      const nodeId = action.type === "drill" || action.type === "select_node" || action.type === "anchor_node"
        ? action.node_id : undefined;
      if (nodeId?.startsWith("workspace:") && (action.type === "drill" || action.type === "select_node")) {
        const label = nodeId.replace("workspace:", "");
        try {
          const response = await chatApi.sendMessage(sessionId, { message: label });
          addMessage(response.message);
          queryClient.invalidateQueries({ queryKey: queryKeys.observatory.all(sessionId) });
          queryClient.invalidateQueries({ queryKey: queryKeys.scope(sessionId) });
          queryClient.invalidateQueries({ queryKey: queryKeys.constellation.all });
        } catch (err) {
          console.error("Workspace selection failed:", err);
        }
        return;
      }

      let verb: string | null = null;
      let args: Record<string, unknown> = {};

      switch (action.type) {
        case "drill":
          verb = "nav.drill";
          args = { target_id: nodeId, target_level: action.target_level };
          break;
        case "semantic_zoom_out":
          verb = "nav.zoom-out";
          break;
        case "navigate_history":
          verb = action.direction === "back" ? "nav.history-back" : "nav.history-forward";
          break;
        case "select_node":
          verb = "nav.select";
          args = { target_id: nodeId };
          break;
        case "invoke_verb":
          verb = action.verb_fqn;
          break;
        default:
          return;
      }

      if (!verb) return;
      const message = args && Object.keys(args).length > 0
        ? `${verb} ${Object.values(args).join(" ")}`
        : verb;

      try {
        await chatApi.sendMessage(sessionId, { message });
        queryClient.invalidateQueries({ queryKey: queryKeys.observatory.all(sessionId) });
        queryClient.invalidateQueries({ queryKey: queryKeys.scope(sessionId) });
        queryClient.invalidateQueries({ queryKey: queryKeys.constellation.all });
      } catch (err) {
        console.error("Canvas navigation failed:", err);
      }
    },
    [sessionId, addMessage],
  );

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
      addMessage(response.message);

      // Detect session.start/resume — navigate to the new session
      try {
        const content = response.message?.content ?? "";
        if (content.includes("Session ID:")) {
          const match = content.match(/Session ID: ([0-9a-f-]{36})/);
          if (match?.[1] && match[1] !== sessionId) {
            queryClient.invalidateQueries({ queryKey: queryKeys.chat.sessions() });
            navigate(`/chat/${match[1]}`);
            setStreaming(false);
            return;
          }
        }
      } catch { /* ignore parse errors */ }
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
        // Cross-invalidate Observatory projections — both UIs project from the
        // same session DAG, so when a chat verb changes the DAG, Observatory
        // must refresh immediately (not wait for 5s poll).
        // Placed in onSuccess (not onMutate) because the server must have
        // committed the state change before we invalidate.
        queryClient.invalidateQueries({
          queryKey: queryKeys.observatory.all(sessionId),
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
      if (message === "New Session" || message === "session.start") {
        chatApi.createSession().then((newSession) => {
          queryClient.invalidateQueries({ queryKey: queryKeys.chat.sessions() });
          queryClient.setQueryData(queryKeys.chat.session(newSession.id), newSession);
          navigate(`/chat/${newSession.id}`);
        });
        return;
      }
      sendMutation.mutate(message);
    },
    [sendMutation, navigate],
  );

  const latestSessionFeedback = useMemo<SessionFeedback | undefined>(() => {
    const messages = currentSession?.messages ?? [];
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      const feedback = messages[index]?.session_feedback;
      if (feedback) {
        return feedback;
      }
    }
    return currentSession?.initial_session_feedback;
  }, [currentSession?.messages, currentSession?.initial_session_feedback]);

  const latestNarration = useMemo(() => {
    const messages = currentSession?.messages ?? [];
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      const narration = messages[index]?.narration;
      if (narration && narration.verbosity !== "silent") {
        return narration;
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
    <div className="flex h-screen bg-[var(--bg-primary)]">
      {/* Left: Session icons + Chat cockpit (cause) */}
      <ChatSidebar />

      {!sessionMissing && (
        <div className="w-[28rem] flex-shrink-0 border-r border-[var(--border-primary)] bg-[var(--bg-secondary)] flex flex-col overflow-hidden">
          {/* Chat messages (scrollable) */}
          <div className="flex-1 overflow-auto p-3 min-h-0">
            {isLoading ? (
              <div className="flex h-full items-center justify-center">
                <Loader2 className="h-6 w-6 animate-spin text-[var(--accent-blue)]" />
              </div>
            ) : error ? (
              <div className="flex h-full items-center justify-center">
                <p className="text-sm text-[var(--accent-red)]">
                  {error instanceof Error ? error.message : "Failed to load session"}
                </p>
              </div>
            ) : currentSession ? (
              <div className="space-y-3">
                {currentSession.messages.map((message) => (
                  <ChatMessage
                    key={message.id}
                    message={message}
                    onDecisionReply={handleDecisionReply}
                    onDiscoverySelection={handleDiscoverySelection}
                    onSendMessage={(msg) => sendMutation.mutate(msg)}
                  />
                ))}

                {isStreaming && (
                  <div className="flex items-center gap-2 text-[var(--text-muted)]">
                    <Loader2 size={14} className="animate-spin" />
                    <span className="text-xs">Thinking...</span>
                  </div>
                )}

                <div ref={messagesEndRef} />
              </div>
            ) : null}
          </div>

          {/* Chat input */}
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

          {/* Panels below input (scrollable) */}
          <div className="border-t border-[var(--border-primary)] overflow-auto" style={{ maxHeight: "40%" }}>
            {sessionId && <DealPanel sessionId={sessionId} />}

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

            {latestNarration && (
              <div className="border-t border-[var(--border-primary)]">
                <NarrationPanel
                  narration={latestNarration}
                  onSendMessage={(msg) => sendMutation.mutate(msg)}
                />
              </div>
            )}

            {sessionId && (
              <div className="flex items-center gap-2 border-t border-[var(--border-primary)] px-3 py-2">
                <button
                  onClick={showRunbookPlan ? () => setShowRunbookPlan(false) : handleCompileRunbook}
                  className="flex items-center gap-1.5 rounded px-2.5 py-1.5 text-xs font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] border border-[var(--border-secondary)]"
                >
                  <BookOpen size={14} />
                  {showRunbookPlan ? "Hide Plan" : "Compile Plan"}
                </button>
              </div>
            )}

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

            <VerbBrowser
              className="border-t border-[var(--border-primary)]"
              onVerbSubmit={handleVerbSubmit}
            />
          </div>
        </div>
      )}

      {/* Right: Observatory canvas (effect/visualization) */}
      <div className="flex flex-1 flex-col min-w-0">
        {sessionId && (
          <FlightDeck
            orientation={orientation ?? null}
            onSendMessage={(msg) => sendMutation.mutate(msg)}
          />
        )}

        <div className="flex-1 min-h-0">
          {sessionId ? (
            <ConstellationCanvas
              graphScene={graphScene ?? null}
              viewLevel={orientation?.view_level ?? "system"}
              onAction={handleCanvasAction}
            />
          ) : (
            <div className="flex h-full items-center justify-center bg-[var(--bg-primary)]">
              <div className="text-center">
                <h2 className="text-xl font-semibold text-[var(--text-primary)]">
                  Start a Conversation
                </h2>
                <p className="mt-2 text-[var(--text-secondary)]">
                  Select a session from the sidebar or create a new one.
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default ChatPage;
