/**
 * Utility functions
 */

import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** Merge Tailwind classes with clsx */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** Format a date string for display */
export function formatDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/** Format a timestamp for display */
export function formatTime(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** Format a datetime for display */
export function formatDateTime(dateString: string): string {
  return `${formatDate(dateString)} ${formatTime(dateString)}`;
}

/** Truncate a string with ellipsis */
export function truncate(str: string, maxLength: number): string {
  if (str.length <= maxLength) return str;
  return str.slice(0, maxLength - 3) + '...';
}

/** Generate a unique ID */
export function generateId(): string {
  return Math.random().toString(36).substring(2, 11);
}

/** Debounce a function */
export function debounce<T extends (...args: unknown[]) => unknown>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout>;
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn(...args), delay);
  };
}

/** Get entity type color */
export function getEntityColor(entityType: string): string {
  const colors: Record<string, string> = {
    cbu: 'var(--entity-cbu)',
    fund: 'var(--entity-fund)',
    person: 'var(--entity-person)',
    company: 'var(--entity-company)',
    document: 'var(--entity-document)',
  };
  return colors[entityType.toLowerCase()] || 'var(--text-secondary)';
}

/** Get kind icon name (for Lucide) */
export function getKindIcon(kind: string): string {
  const icons: Record<string, string> = {
    cbu: 'building-2',
    fund: 'wallet',
    entity: 'user',
    person: 'user',
    company: 'building',
    document: 'file-text',
    holding: 'coins',
    control: 'git-branch',
    trading_profile: 'line-chart',
    isda: 'file-signature',
  };
  return icons[kind.toLowerCase()] || 'circle';
}
