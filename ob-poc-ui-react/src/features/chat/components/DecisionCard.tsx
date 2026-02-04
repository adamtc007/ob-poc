/**
 * DecisionCard - Renders DecisionPacket interactions
 */

import { useState } from 'react';
import { HelpCircle, ListChecks, AlertTriangle, CheckCircle, XCircle } from 'lucide-react';
import type {
  DecisionPacket,
  ClarificationPayload,
  ProposalPayload,
  ConfirmationPayload,
  ResultPayload,
  ErrorPayload,
} from '../../../types/chat';
import { cn } from '../../../lib/utils';

interface DecisionCardProps {
  packet: DecisionPacket;
  onReply?: (reply: unknown) => void;
}

/** Clarification card - presents options to choose from */
function ClarificationCard({
  payload,
  onReply
}: {
  payload: ClarificationPayload;
  onReply?: (reply: { selected_option: string } | { freeform_response: string }) => void;
}) {
  const [freeformValue, setFreeformValue] = useState('');

  return (
    <div className="space-y-3">
      <div className="flex items-start gap-2">
        <HelpCircle size={18} className="mt-0.5 text-[var(--accent-blue)]" />
        <div>
          <p className="font-medium text-[var(--text-primary)]">{payload.question}</p>
          {payload.context && (
            <p className="mt-1 text-sm text-[var(--text-secondary)]">{payload.context}</p>
          )}
        </div>
      </div>

      {/* Options */}
      <div className="space-y-2 pl-6">
        {payload.options.map((option) => (
          <button
            key={option.id}
            onClick={() => onReply?.({ selected_option: option.id })}
            className="block w-full rounded-lg border border-[var(--border-primary)] bg-[var(--bg-tertiary)] px-3 py-2 text-left text-sm transition-colors hover:border-[var(--accent-blue)] hover:bg-[var(--accent-blue)]/10"
          >
            <span className="font-medium text-[var(--text-primary)]">{option.label}</span>
            {option.description && (
              <span className="block text-xs text-[var(--text-muted)]">{option.description}</span>
            )}
          </button>
        ))}
      </div>

      {/* Freeform input */}
      {payload.allow_freeform && (
        <div className="pl-6">
          <div className="flex gap-2">
            <input
              type="text"
              value={freeformValue}
              onChange={(e) => setFreeformValue(e.target.value)}
              placeholder="Or type your own response..."
              className="flex-1 rounded border border-[var(--border-primary)] bg-[var(--bg-tertiary)] px-3 py-1.5 text-sm text-[var(--text-primary)] placeholder-[var(--text-muted)]"
            />
            <button
              onClick={() => onReply?.({ freeform_response: freeformValue })}
              disabled={!freeformValue.trim()}
              className="rounded bg-[var(--accent-blue)] px-3 py-1.5 text-sm text-white disabled:opacity-50"
            >
              Send
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/** Proposal card - shows planned actions for approval */
function ProposalCard({
  payload,
  onReply
}: {
  payload: ProposalPayload;
  onReply?: (reply: { confirmed: boolean }) => void;
}) {
  return (
    <div className="space-y-3">
      <div className="flex items-start gap-2">
        <ListChecks size={18} className="mt-0.5 text-[var(--accent-purple)]" />
        <div>
          <p className="font-medium text-[var(--text-primary)]">{payload.summary}</p>
          {payload.estimated_impact && (
            <p className="mt-1 text-sm text-[var(--text-secondary)]">
              Impact: {payload.estimated_impact}
            </p>
          )}
        </div>
      </div>

      {/* Actions list */}
      <div className="space-y-1 pl-6">
        {payload.actions.map((action, i) => (
          <div
            key={action.id}
            className="flex items-center gap-2 rounded bg-[var(--bg-tertiary)] px-3 py-2 text-sm"
          >
            <span className="text-[var(--text-muted)]">{i + 1}.</span>
            <span className="text-[var(--text-primary)]">{action.description}</span>
            <span className="ml-auto text-xs font-mono text-[var(--text-muted)]">
              {action.verb}
            </span>
          </div>
        ))}
      </div>

      {/* Confirm/Cancel */}
      {payload.requires_confirmation && (
        <div className="flex gap-2 pl-6">
          <button
            onClick={() => onReply?.({ confirmed: true })}
            className="flex-1 rounded bg-[var(--accent-green)] px-4 py-2 text-sm font-medium text-white hover:bg-[var(--accent-green)]/80"
          >
            Approve
          </button>
          <button
            onClick={() => onReply?.({ confirmed: false })}
            className="flex-1 rounded border border-[var(--border-secondary)] px-4 py-2 text-sm text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  );
}

/** Confirmation card - yes/no prompt */
function ConfirmationCard({
  payload,
  onReply
}: {
  payload: ConfirmationPayload;
  onReply?: (reply: { confirmed: boolean }) => void;
}) {
  return (
    <div className="space-y-3">
      <div className="flex items-start gap-2">
        <AlertTriangle size={18} className="mt-0.5 text-[var(--accent-yellow)]" />
        <div>
          <p className="font-medium text-[var(--text-primary)]">{payload.message}</p>
          <p className="mt-1 text-sm text-[var(--text-secondary)]">{payload.action_summary}</p>
        </div>
      </div>

      <div className="flex gap-2 pl-6">
        <button
          onClick={() => onReply?.({ confirmed: true })}
          className="flex-1 rounded bg-[var(--accent-blue)] px-4 py-2 text-sm font-medium text-white hover:bg-[var(--accent-blue)]/80"
        >
          {payload.confirm_button || 'Confirm'}
        </button>
        <button
          onClick={() => onReply?.({ confirmed: false })}
          className="flex-1 rounded border border-[var(--border-secondary)] px-4 py-2 text-sm text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
        >
          {payload.cancel_button || 'Cancel'}
        </button>
      </div>
    </div>
  );
}

/** Result card - shows operation outcome */
function ResultCard({ payload }: { payload: ResultPayload }) {
  return (
    <div className="space-y-2">
      <div className="flex items-start gap-2">
        {payload.success ? (
          <CheckCircle size={18} className="mt-0.5 text-[var(--accent-green)]" />
        ) : (
          <XCircle size={18} className="mt-0.5 text-[var(--accent-red)]" />
        )}
        <p className={cn(
          'font-medium',
          payload.success ? 'text-[var(--accent-green)]' : 'text-[var(--accent-red)]'
        )}>
          {payload.message}
        </p>
      </div>

      {payload.next_steps && payload.next_steps.length > 0 && (
        <div className="pl-6">
          <p className="text-xs text-[var(--text-muted)]">Next steps:</p>
          <ul className="mt-1 space-y-1 text-sm text-[var(--text-secondary)]">
            {payload.next_steps.map((step, i) => (
              <li key={i}>• {step}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

/** Error card - shows error with recovery suggestions */
function ErrorCard({ payload }: { payload: ErrorPayload }) {
  return (
    <div className="space-y-2">
      <div className="flex items-start gap-2">
        <XCircle size={18} className="mt-0.5 text-[var(--accent-red)]" />
        <div>
          <p className="font-medium text-[var(--accent-red)]">{payload.error}</p>
          {payload.code && (
            <p className="text-xs text-[var(--text-muted)]">Code: {payload.code}</p>
          )}
        </div>
      </div>

      {payload.suggestions && payload.suggestions.length > 0 && (
        <div className="pl-6">
          <p className="text-xs text-[var(--text-muted)]">Suggestions:</p>
          <ul className="mt-1 space-y-1 text-sm text-[var(--text-secondary)]">
            {payload.suggestions.map((suggestion, i) => (
              <li key={i}>• {suggestion}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

export function DecisionCard({ packet, onReply }: DecisionCardProps) {
  return (
    <div className="rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)] p-4">
      {packet.kind === 'clarification' && (
        <ClarificationCard
          payload={packet.payload as ClarificationPayload}
          onReply={onReply}
        />
      )}
      {packet.kind === 'proposal' && (
        <ProposalCard
          payload={packet.payload as ProposalPayload}
          onReply={onReply}
        />
      )}
      {packet.kind === 'confirmation' && (
        <ConfirmationCard
          payload={packet.payload as ConfirmationPayload}
          onReply={onReply}
        />
      )}
      {packet.kind === 'result' && (
        <ResultCard payload={packet.payload as ResultPayload} />
      )}
      {packet.kind === 'error' && (
        <ErrorCard payload={packet.payload as ErrorPayload} />
      )}
    </div>
  );
}

export default DecisionCard;
