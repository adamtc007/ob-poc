/**
 * ChatMessage - Single message display component
 */

import { User, Bot, AlertCircle, CheckCircle, Loader2 } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type {
  ChatMessage as ChatMessageType,
  ToolCall,
} from "../../../types/chat";
import { cn, formatTime } from "../../../lib/utils";
import { DecisionCard } from "./DecisionCard";

interface ChatMessageProps {
  message: ChatMessageType;
  onDecisionReply?: (packetId: string, reply: unknown) => void;
}

function ToolCallDisplay({ toolCall }: { toolCall: ToolCall }) {
  return (
    <div className="mt-2 rounded border border-[var(--border-primary)] bg-[var(--bg-tertiary)] p-2 text-xs">
      <div className="flex items-center gap-2">
        {toolCall.status === "pending" && (
          <Loader2
            size={12}
            className="animate-spin text-[var(--text-muted)]"
          />
        )}
        {toolCall.status === "running" && (
          <Loader2
            size={12}
            className="animate-spin text-[var(--accent-blue)]"
          />
        )}
        {toolCall.status === "success" && (
          <CheckCircle size={12} className="text-[var(--accent-green)]" />
        )}
        {toolCall.status === "error" && (
          <AlertCircle size={12} className="text-[var(--accent-red)]" />
        )}
        <span className="font-mono text-[var(--text-secondary)]">
          {toolCall.name}
        </span>
      </div>
      {toolCall.result !== undefined && (
        <pre className="mt-1 overflow-auto text-[var(--text-muted)]">
          {JSON.stringify(toolCall.result, null, 2)}
        </pre>
      )}
    </div>
  );
}

function SageExplainCard({ message }: { message: ChatMessageType }) {
  if (!message.sage_explain) return null;
  return (
    <div className="mt-2 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-tertiary)] p-3 text-sm">
      <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
        Sage Understanding
      </div>
      <div className="mt-1 text-[var(--text-primary)]">
        {message.sage_explain.understanding}
      </div>
      <div className="mt-2 flex flex-wrap gap-2 text-xs text-[var(--text-secondary)]">
        <span className="rounded bg-[var(--bg-secondary)] px-2 py-1">
          mode: {message.sage_explain.mode}
        </span>
        <span className="rounded bg-[var(--bg-secondary)] px-2 py-1">
          confidence: {message.sage_explain.confidence}
        </span>
        {message.sage_explain.scope_summary && (
          <span className="rounded bg-[var(--bg-secondary)] px-2 py-1">
            scope: {message.sage_explain.scope_summary}
          </span>
        )}
      </div>
      {message.sage_explain.clarifications &&
        message.sage_explain.clarifications.length > 0 && (
          <ul className="mt-2 list-disc pl-5 text-xs text-[var(--text-secondary)]">
            {message.sage_explain.clarifications.map((item, index) => (
              <li key={`${item}-${index}`}>{item}</li>
            ))}
          </ul>
        )}
    </div>
  );
}

function CoderProposalCard({ message }: { message: ChatMessageType }) {
  if (!message.coder_proposal) return null;
  return (
    <div className="mt-2 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)] p-3 text-sm">
      <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
        Coder Proposal
      </div>
      <div className="mt-2 flex flex-wrap gap-2 text-xs text-[var(--text-secondary)]">
        {message.coder_proposal.verb_fqn && (
          <span className="rounded bg-[var(--bg-tertiary)] px-2 py-1 font-mono">
            {message.coder_proposal.verb_fqn}
          </span>
        )}
        <span className="rounded bg-[var(--bg-tertiary)] px-2 py-1">
          {message.coder_proposal.requires_confirmation
            ? "confirmation required"
            : "read-only / safe"}
        </span>
        <span className="rounded bg-[var(--bg-tertiary)] px-2 py-1">
          {message.coder_proposal.ready_to_execute
            ? "ready to execute"
            : "not executable yet"}
        </span>
      </div>
      {message.coder_proposal.change_summary &&
        message.coder_proposal.change_summary.length > 0 && (
          <ul className="mt-2 list-disc pl-5 text-xs text-[var(--text-secondary)]">
            {message.coder_proposal.change_summary.map((item, index) => (
              <li key={`${item}-${index}`}>{item}</li>
            ))}
          </ul>
        )}
      {message.coder_proposal.dsl && (
        <pre className="mt-2 overflow-auto rounded bg-[var(--bg-tertiary)] p-2 text-xs text-[var(--text-primary)]">
          <code>{message.coder_proposal.dsl}</code>
        </pre>
      )}
    </div>
  );
}

export function ChatMessage({ message, onDecisionReply }: ChatMessageProps) {
  const isUser = message.role === "user";
  const isSystem = message.role === "system";

  return (
    <div className={cn("flex gap-3", isUser && "flex-row-reverse")}>
      {/* Avatar */}
      <div
        className={cn(
          "flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full",
          isUser && "bg-[var(--accent-blue)]",
          !isUser && !isSystem && "bg-[var(--accent-purple)]",
          isSystem && "bg-[var(--bg-tertiary)]",
        )}
      >
        {isUser ? (
          <User size={16} className="text-white" />
        ) : (
          <Bot
            size={16}
            className={isSystem ? "text-[var(--text-muted)]" : "text-white"}
          />
        )}
      </div>

      {/* Content */}
      <div className={cn("flex-1 max-w-[80%]", isUser && "text-right")}>
        {/* Message bubble */}
        <div
          className={cn(
            "inline-block rounded-lg px-4 py-2 text-sm",
            isUser && "bg-[var(--accent-blue)] text-white",
            !isUser && "bg-[var(--bg-secondary)] text-[var(--text-primary)]",
            isSystem &&
              "bg-[var(--bg-tertiary)] text-[var(--text-muted)] italic",
          )}
        >
          {isUser ? (
            <p className="whitespace-pre-wrap">{message.content}</p>
          ) : (
            <div className="markdown-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {message.content}
              </ReactMarkdown>
            </div>
          )}
        </div>

        {/* Tool calls */}
        {message.tool_calls && message.tool_calls.length > 0 && (
          <div className="mt-2 space-y-1">
            {message.tool_calls.map((tc) => (
              <ToolCallDisplay key={tc.id} toolCall={tc} />
            ))}
          </div>
        )}

        {!isUser && (
          <>
            <SageExplainCard message={message} />
            <CoderProposalCard message={message} />
          </>
        )}

        {/* Decision packet */}
        {message.decision_packet && (
          <div className="mt-2">
            <DecisionCard
              packet={message.decision_packet}
              onReply={(reply) =>
                onDecisionReply?.(message.decision_packet!.id, reply)
              }
            />
          </div>
        )}

        {/* Timestamp */}
        <div
          className={cn(
            "mt-1 text-xs text-[var(--text-muted)]",
            isUser && "text-right",
          )}
        >
          {formatTime(message.timestamp)}
        </div>
      </div>
    </div>
  );
}

export default ChatMessage;
