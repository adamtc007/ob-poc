/**
 * LoadingSpinner - Reusable loading indicator
 */

import { Loader2 } from 'lucide-react';
import { cn } from '../lib/utils';

interface LoadingSpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  text?: string;
  className?: string;
}

const sizeMap = {
  sm: 16,
  md: 24,
  lg: 32,
};

export function LoadingSpinner({ size = 'md', text, className }: LoadingSpinnerProps) {
  return (
    <div className={cn('flex items-center justify-center gap-2', className)}>
      <Loader2
        size={sizeMap[size]}
        className="animate-spin text-[var(--accent-blue)]"
      />
      {text && (
        <span className="text-sm text-[var(--text-muted)]">{text}</span>
      )}
    </div>
  );
}

export function FullPageLoader({ text = 'Loading...' }: { text?: string }) {
  return (
    <div className="flex h-full items-center justify-center">
      <LoadingSpinner size="lg" text={text} />
    </div>
  );
}

export default LoadingSpinner;
