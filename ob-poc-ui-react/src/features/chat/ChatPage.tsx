/**
 * Chat Page - Agent chat UI
 */

import { useParams } from "react-router-dom";
import { useQuery, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useCallback } from "react";
import { Loader2 } from "lucide-react";
import { chatApi } from "../../api/chat";
import { queryKeys, queryClient } from "../../lib/query";
import { useChatStore } from "../../stores/chat";
import { ChatSidebar, ChatMessage, ChatInput, ScopePanel, VerbBrowser } from "./components";
import { DealPanel } from "../deal/components";
import type { DecisionReply } from "../../types/chat";

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId?: string }>();
  const messagesEndRef = useRef<HTMLDivElement>(null);
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
      // Replace temp message with real one and add assistant response
      setCurrentSession(response.session);
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
    onError: () => {
      setStreaming(false);
    },
  });

  // Decision reply mutation
  const replyMutation = useMutation({
    mutationFn: (reply: DecisionReply) => {
      if (!sessionId) throw new Error("No session selected");
      return chatApi.replyToDecision(sessionId, reply);
    },
    onSuccess: (message) => {
      addMessage(message);
      queryClient.invalidateQueries({
        queryKey: queryKeys.chat.session(sessionId!),
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
    // TODO: Implement streaming cancellation
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
            sessionId ? "Type a message..." : "Select or create a session first"
          }
        />
      </div>

      {/* Right sidebar - Scope, Verbs, and Deal panels */}
      <div className="w-72 flex-shrink-0 border-l border-[var(--border-primary)] bg-[var(--bg-secondary)] flex flex-col overflow-hidden">
        {/* Deal context panel */}
        {sessionId && <DealPanel sessionId={sessionId} />}

        {/* Scope panel showing loaded CBUs */}
        <ScopePanel sessionId={sessionId} />

        {/* Available commands / verb browser */}
        <VerbBrowser className="border-t border-[var(--border-primary)]" />
      </div>
    </div>
  );
}

export default ChatPage;
