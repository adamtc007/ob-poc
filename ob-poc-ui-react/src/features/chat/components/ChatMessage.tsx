/**
 * ChatMessage - Single message display component
 */

import { useState } from "react";
import { User, Bot, AlertCircle, CheckCircle, Loader2 } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type {
  AcpTraceSummary,
  ChatMessage as ChatMessageType,
  DiscoverySelection,
  ToolCall,
} from "../../../types/chat";
import { cn, formatTime } from "../../../lib/utils";
import { DecisionCard } from "./DecisionCard";
import { VerbDisambiguationCard } from "./VerbDisambiguationCard";
import { NarrationPanel } from "./NarrationPanel";
import { OnboardingStateCard } from "./OnboardingStateCard";
import { FormioForm } from "../../forms/FormioForm";

interface ChatMessageProps {
  message: ChatMessageType;
  onDecisionReply?: (packetId: string, reply: unknown) => void;
  onDiscoverySelection?: (selection: DiscoverySelection) => void;
  onSendMessage?: (message: string) => void;
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

function AcpTraceCard({ trace }: { trace: AcpTraceSummary }) {
  const statusLabel = trace.outcome || trace.status;
  const provider = trace.state_anchor_provider;
  const detailItems = [
    trace.route ? `route: ${trace.route}` : null,
    trace.draft_source ? `draft: ${trace.draft_source}` : null,
    trace.requested_draft_source &&
    trace.requested_draft_source !== trace.draft_source
      ? `requested: ${trace.requested_draft_source}`
      : null,
    trace.outcome_layer ? `layer: ${trace.outcome_layer}` : null,
    trace.transition_ref ? `transition: ${trace.transition_ref}` : null,
    trace.refusal_code ? `refusal: ${trace.refusal_code}` : null,
    trace.pending_question_code
      ? `pending: ${trace.pending_question_code}`
      : null,
    trace.revision_count !== undefined
      ? `revisions: ${trace.revision_count}`
      : null,
    trace.performance?.total_ms !== undefined
      ? `total: ${trace.performance.total_ms}ms`
      : trace.route_latency_ms !== undefined
        ? `route: ${trace.route_latency_ms}ms`
        : null,
    trace.performance?.llm_draft_ms !== undefined &&
    trace.performance.llm_draft_ms > 0
      ? `llm: ${trace.performance.llm_draft_ms}ms`
      : null,
  ].filter((item): item is string => Boolean(item));
  const providerItems = provider
    ? [
        provider.task ? `task: ${provider.task}` : null,
        provider.status ? `provider: ${provider.status}` : null,
        provider.state_anchor_source
          ? `anchor: ${provider.state_anchor_source}`
          : null,
        provider.language_pack_generated !== undefined
          ? `language pack: ${provider.language_pack_generated ? "yes" : "no"}`
          : null,
        provider.dry_run_valid !== undefined
          ? `provider dry-run: ${provider.dry_run_valid ? "valid" : "not valid"}`
          : null,
        provider.no_mutation_authority ? "mutation: no authority" : null,
      ].filter((item): item is string => Boolean(item))
    : [];

  return (
    <div className="mt-2 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-tertiary)] p-3 text-sm">
      <div className="flex items-center justify-between gap-2">
        <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
          ACP Trace
        </div>
        <span className="rounded bg-[var(--bg-secondary)] px-2 py-1 text-xs font-mono text-[var(--text-secondary)]">
          {statusLabel}
        </span>
      </div>

      {trace.human_summary && (
        <div className="mt-2 text-xs text-[var(--text-secondary)]">
          {trace.human_summary}
        </div>
      )}

      {detailItems.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-2">
          {detailItems.map((item) => (
            <span
              key={item}
              className="rounded bg-[var(--bg-secondary)] px-2 py-1 text-xs text-[var(--text-secondary)]"
            >
              {item}
            </span>
          ))}
        </div>
      )}

      {provider && (
        <div className="mt-3 border-t border-[var(--border-primary)] pt-3">
          <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
            State Anchor Provider
          </div>
          <div className="mt-2 flex flex-wrap gap-2">
            {providerItems.map((item) => (
              <span
                key={item}
                className="rounded bg-[var(--bg-secondary)] px-2 py-1 text-xs text-[var(--text-secondary)]"
              >
                {item}
              </span>
            ))}
            {provider.provider_id && (
              <span className="rounded bg-[var(--bg-secondary)] px-2 py-1 font-mono text-xs text-[var(--text-secondary)]">
                {provider.provider_id}
              </span>
            )}
          </div>
          {provider.supported_tasks && provider.supported_tasks.length > 0 && (
            <div className="mt-2 text-xs text-[var(--text-secondary)]">
              supported: {provider.supported_tasks.join(", ")}
            </div>
          )}
          {provider.needed && provider.needed.length > 0 && (
            <div className="mt-1 text-xs text-[var(--text-secondary)]">
              needed:{" "}
              {provider.needed
                .map((need) => need.replaceAll("_", " "))
                .join(", ")}
            </div>
          )}
        </div>
      )}

      {trace.needed_from_user && trace.needed_from_user.length > 0 && (
        <div className="mt-3">
          <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
            Needed From User
          </div>
          <ul className="mt-1 list-disc pl-5 text-xs text-[var(--text-secondary)]">
            {trace.needed_from_user.map((need) => (
              <li key={need}>{need.replaceAll("_", " ")}</li>
            ))}
          </ul>
        </div>
      )}

      {trace.diagnostic_codes && trace.diagnostic_codes.length > 0 && (
        <div className="mt-3">
          <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
            Diagnostics
          </div>
          <div className="mt-1 flex flex-wrap gap-2">
            {trace.diagnostic_codes.map((code) => (
              <span
                key={code}
                className="rounded bg-[var(--bg-secondary)] px-2 py-1 font-mono text-xs text-[var(--text-secondary)]"
              >
                {code}
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="mt-3 flex flex-wrap gap-2 text-xs text-[var(--text-secondary)]">
        {trace.dry_run_valid !== undefined && (
          <span className="rounded bg-[var(--bg-secondary)] px-2 py-1">
            dry-run: {trace.dry_run_valid ? "valid" : "not valid"}
          </span>
        )}
        {trace.prose_only_failure !== undefined && (
          <span className="rounded bg-[var(--bg-secondary)] px-2 py-1">
            prose-only failure: {trace.prose_only_failure ? "yes" : "no"}
          </span>
        )}
        {trace.semantic_diff_uri && (
          <span className="rounded bg-[var(--bg-secondary)] px-2 py-1 font-mono">
            {trace.semantic_diff_uri}
          </span>
        )}
      </div>
    </div>
  );
}

function ParkedEntriesCard({ message }: { message: ChatMessageType }) {
  if (!message.parked_entries || message.parked_entries.length === 0)
    return null;

  return (
    <div className="mt-2 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)] p-3 text-sm">
      <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
        Execution Parked
      </div>
      <div className="mt-2 space-y-2">
        {message.parked_entries.map((entry) => (
          <div
            key={entry.step_id}
            className="rounded border border-[var(--border-primary)] bg-[var(--bg-tertiary)] p-3"
          >
            <div className="font-mono text-xs text-[var(--text-primary)]">
              {entry.verb}
            </div>
            <div className="mt-1 text-xs text-[var(--text-secondary)]">
              reason: {entry.park_reason.replaceAll("_", " ")}
            </div>
            {entry.message && (
              <div className="mt-1 text-xs text-[var(--text-secondary)]">
                {entry.message}
              </div>
            )}
            {entry.correlation_key && (
              <div className="mt-1 text-xs text-[var(--text-secondary)]">
                callback key: {entry.correlation_key}
              </div>
            )}
            {entry.resource && (
              <div className="mt-1 text-xs text-[var(--text-secondary)]">
                resource: {entry.resource}
              </div>
            )}
            {entry.gate_entry_id && (
              <div className="mt-1 text-xs text-[var(--text-secondary)]">
                gate entry: {entry.gate_entry_id}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function DiscoveryBootstrapCard({
  message,
  onDiscoverySelection,
}: {
  message: ChatMessageType;
  onDiscoverySelection?: (selection: DiscoverySelection) => void;
}) {
  const bootstrap = message.discovery_bootstrap;
  const [questionAnswers, setQuestionAnswers] = useState<
    Record<string, string>
  >({});
  if (!bootstrap) return null;

  return (
    <div className="mt-2 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-tertiary)] p-3 text-sm">
      <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
        Sage Bootstrap
      </div>
      <div className="mt-2 flex flex-wrap gap-2 text-xs text-[var(--text-secondary)]">
        <span className="rounded bg-[var(--bg-secondary)] px-2 py-1">
          readiness: {bootstrap.grounding_readiness.replaceAll("_", " ")}
        </span>
      </div>

      {bootstrap.entry_questions && bootstrap.entry_questions.length > 0 && (
        <div className="mt-3">
          <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
            Next Questions
          </div>
          <div className="mt-2 space-y-3">
            {bootstrap.entry_questions.map((item) => {
              const answer = questionAnswers[item.question_id] ?? "";
              return (
                <div
                  key={item.question_id}
                  className="rounded border border-[var(--border-primary)] bg-[var(--bg-secondary)] p-3"
                >
                  <div className="text-xs text-[var(--text-secondary)]">
                    {item.prompt}
                  </div>
                  <div className="mt-2 flex gap-2">
                    <input
                      type="text"
                      value={answer}
                      placeholder="Type your answer"
                      className="flex-1 rounded border border-[var(--border-primary)] bg-[var(--bg-primary)] px-2 py-1 text-xs text-[var(--text-primary)] outline-none placeholder:text-[var(--text-muted)]"
                      onChange={(event) =>
                        setQuestionAnswers((current) => ({
                          ...current,
                          [item.question_id]: event.target.value,
                        }))
                      }
                    />
                    <button
                      type="button"
                      disabled={!answer.trim()}
                      className="rounded bg-[var(--accent-blue)] px-2 py-1 text-xs text-white transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
                      onClick={() =>
                        onDiscoverySelection?.({
                          selection_kind: "question_answer",
                          selection_id: item.question_id,
                          label: item.prompt,
                          maps_to: item.maps_to,
                          value: answer.trim(),
                        })
                      }
                    >
                      Use Answer
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {bootstrap.matched_domains && bootstrap.matched_domains.length > 0 && (
        <div className="mt-3">
          <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
            Likely Work Areas
          </div>
          <div className="mt-2 flex flex-wrap gap-2">
            {bootstrap.matched_domains.map((item) => (
              <button
                key={item.domain_id}
                type="button"
                className="rounded bg-[var(--bg-secondary)] px-2 py-1 text-xs text-[var(--text-primary)] transition hover:bg-[var(--accent-blue)] hover:text-white"
                onClick={() =>
                  onDiscoverySelection?.({
                    selection_kind: "domain",
                    selection_id: item.domain_id,
                    label: item.label,
                  })
                }
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {bootstrap.matched_families && bootstrap.matched_families.length > 0 && (
        <div className="mt-3">
          <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
            Candidate Families
          </div>
          <div className="mt-2 flex flex-wrap gap-2">
            {bootstrap.matched_families.map((item) => (
              <button
                key={item.family_id}
                type="button"
                className="rounded bg-[var(--bg-secondary)] px-2 py-1 text-xs text-[var(--text-primary)] transition hover:bg-[var(--accent-blue)] hover:text-white"
                onClick={() =>
                  onDiscoverySelection?.({
                    selection_kind: "family",
                    selection_id: item.family_id,
                    label: item.label,
                  })
                }
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {bootstrap.matched_constellations &&
        bootstrap.matched_constellations.length > 0 && (
          <div className="mt-3">
            <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
              Candidate Constellations
            </div>
            <div className="mt-2 flex flex-wrap gap-2">
              {bootstrap.matched_constellations.map((item) => (
                <button
                  key={item.constellation_id}
                  type="button"
                  className="rounded bg-[var(--bg-secondary)] px-2 py-1 text-xs text-[var(--text-primary)] transition hover:bg-[var(--accent-blue)] hover:text-white"
                  onClick={() =>
                    onDiscoverySelection?.({
                      selection_kind: "constellation",
                      selection_id: item.constellation_id,
                      label: item.label,
                    })
                  }
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>
        )}

      {bootstrap.missing_inputs && bootstrap.missing_inputs.length > 0 && (
        <div className="mt-3">
          <div className="text-xs font-semibold uppercase tracking-wide text-[var(--text-muted)]">
            Still Needed
          </div>
          <ul className="mt-2 list-disc pl-5 text-xs text-[var(--text-secondary)]">
            {bootstrap.missing_inputs.map((item) => (
              <li key={item.key}>
                {item.label}
                {item.required ? " (required)" : ""}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

export function ChatMessage({
  message,
  onDecisionReply,
  onDiscoverySelection,
  onSendMessage,
}: ChatMessageProps) {
  const isUser = message.role === "user";
  const isSystem = message.role === "system";
  const decisionPacket = message.decision_packet;

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
            <DiscoveryBootstrapCard
              message={message}
              onDiscoverySelection={onDiscoverySelection}
            />
            <ParkedEntriesCard message={message} />
            <CoderProposalCard message={message} />
            {message.acp_trace && <AcpTraceCard trace={message.acp_trace} />}
            <OnboardingStateCard
              message={message}
              onVerbClick={onSendMessage}
            />
            {message.narration && (
              <NarrationPanel
                narration={message.narration}
                onSendMessage={onSendMessage}
              />
            )}
          </>
        )}

        {/* Verb disambiguation — rich "did you mean?" with context */}
        {message.verb_disambiguation_detail && (
          <VerbDisambiguationCard message={message} onSelect={onSendMessage} />
        )}

        {/* Decision packet (fallback for non-verb disambiguation) */}
        {decisionPacket && !message.verb_disambiguation_detail && (
          <div className="mt-2">
            <DecisionCard
              packet={decisionPacket}
              onReply={(reply) => onDecisionReply?.(decisionPacket.id, reply)}
            />
          </div>
        )}

        {/* dsl.form verb — Form.io human task */}
        {message.bpmn_form && (
          <div className="mt-2">
            <FormioForm
              formRef={message.bpmn_form.form_ref}
              prefillData={message.bpmn_form.prefill_data}
              mode={message.bpmn_form.mode}
              tokenId={message.bpmn_form.token_id}
              onComplete={() => {
                // Form submitted — the backend delivers HumanTaskComplete
                // and the session will receive an updated response on the
                // next poll/push cycle.
              }}
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
