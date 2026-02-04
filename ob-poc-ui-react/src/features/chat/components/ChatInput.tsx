/**
 * ChatInput - Message input component with streaming support
 */

import { useState, useRef, useEffect } from 'react';
import { Send, Square, Paperclip } from 'lucide-react';
import { cn } from '../../../lib/utils';

interface ChatInputProps {
  onSend: (message: string) => void;
  onCancel?: () => void;
  isStreaming?: boolean;
  disabled?: boolean;
  placeholder?: string;
  className?: string;
}

export function ChatInput({
  onSend,
  onCancel,
  isStreaming = false,
  disabled = false,
  placeholder = 'Type a message...',
  className,
}: ChatInputProps) {
  const [value, setValue] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 200)}px`;
    }
  }, [value]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (value.trim() && !disabled && !isStreaming) {
      onSend(value.trim());
      setValue('');
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Submit on Enter (without Shift)
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  return (
    <form
      onSubmit={handleSubmit}
      className={cn(
        'border-t border-[var(--border-primary)] bg-[var(--bg-secondary)] p-4',
        className
      )}
    >
      <div className="flex items-end gap-2">
        {/* Attachment button (placeholder for future) */}
        <button
          type="button"
          className="rounded-lg p-2 text-[var(--text-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-secondary)]"
          title="Attach file (coming soon)"
          disabled
        >
          <Paperclip size={18} />
        </button>

        {/* Textarea */}
        <div className="relative flex-1">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={placeholder}
            disabled={disabled}
            rows={1}
            className={cn(
              'w-full resize-none rounded-lg border border-[var(--border-primary)] bg-[var(--bg-tertiary)] px-4 py-2.5 pr-12 text-sm text-[var(--text-primary)] placeholder-[var(--text-muted)]',
              'focus:border-[var(--accent-blue)] focus:outline-none focus:ring-1 focus:ring-[var(--accent-blue)]',
              'disabled:opacity-50 disabled:cursor-not-allowed'
            )}
          />
        </div>

        {/* Send/Cancel button */}
        {isStreaming ? (
          <button
            type="button"
            onClick={onCancel}
            className="rounded-lg bg-[var(--accent-red)] p-2.5 text-white transition-colors hover:bg-[var(--accent-red)]/80"
            title="Stop generating"
          >
            <Square size={18} />
          </button>
        ) : (
          <button
            type="submit"
            disabled={!value.trim() || disabled}
            className={cn(
              'rounded-lg bg-[var(--accent-blue)] p-2.5 text-white transition-colors',
              'hover:bg-[var(--accent-blue)]/80',
              'disabled:opacity-50 disabled:cursor-not-allowed'
            )}
            title="Send message (Enter)"
          >
            <Send size={18} />
          </button>
        )}
      </div>

      {/* Keyboard hint */}
      <div className="mt-2 flex items-center justify-between text-xs text-[var(--text-muted)]">
        <span>Press Enter to send, Shift+Enter for new line</span>
        {isStreaming && (
          <span className="text-[var(--accent-yellow)]">Generating response...</span>
        )}
      </div>
    </form>
  );
}

export default ChatInput;
