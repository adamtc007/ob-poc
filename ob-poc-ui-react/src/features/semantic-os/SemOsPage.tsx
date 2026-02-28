/**
 * SemOsPage - Semantic OS agent interface
 *
 * Three-column layout reusing chat components for message display and input.
 * Sessions are created with workflow_focus="semantic-os" which triggers a
 * workflow selection decision packet on the backend.
 */

import { useParams } from "react-router-dom";
import { useQuery, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useCallback } from "react";
import { Loader2 } from "lucide-react";
import { chatApi } from "../../api/chat";
import { queryKeys, queryClient } from "../../lib/query";
import { useChatStore } from "../../stores/chat";
import { ChatMessage, ChatInput } from "../chat/components";
import { SemOsSidebar, SemOsContextPanel } from "./components";
import type { DecisionReply } from "../../types/chat";

export function SemOsPage() {
  const { sessionId } = useParams<{ sessionId?: string }>();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const {
    setCurrentSession,
    currentSession,
    addMessage,
    isStreaming,
    setStreaming,
  } = useChatStore();

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
      setCurrentSession(response.session);
      setStreaming(false);
    },
    onError: () => {
      setStreaming(false);
    },
  });

  // Decision reply mutation (for workflow selection and other decisions)
  const replyMutation = useMutation({
    mutationFn: (reply: DecisionReply) => {
      if (!sessionId) throw new Error("No session selected");
      return chatApi.replyToDecision(sessionId, reply);
    },
    onSuccess: (message) => {
      addMessage(message);
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
      {/* Left sidebar - Session list */}
      <SemOsSidebar className="w-64 flex-shrink-0 border-r border-[var(--border-primary)] bg-[var(--bg-secondary)]" />

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
                  Semantic OS
                </h2>
                <p className="mt-2 text-[var(--text-secondary)]">
                  Create a new session to start working with the semantic
                  registry.
                </p>
                <p className="mt-1 text-sm text-[var(--text-muted)]">
                  You&apos;ll choose a workflow focus: Onboarding, KYC, Data
                  Management, or Stewardship.
                </p>
              </div>
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

      {/* Right sidebar - Registry context */}
      <SemOsContextPanel className="w-64 flex-shrink-0 border-l border-[var(--border-primary)] bg-[var(--bg-secondary)]" />
    </div>
  );
}

export default SemOsPage;
