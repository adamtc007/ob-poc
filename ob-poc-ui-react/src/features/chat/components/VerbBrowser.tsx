/**
 * VerbBrowser - Progressive disclosure verb panel.
 *
 * Stage 1: Show domain categories as clickable cards (auto-expanded on first load).
 * Stage 2: After clicking a domain, show that domain's verbs with search.
 * Clicking a verb inserts its s-expression into the chat input.
 */

import { useState, useMemo, useEffect, useRef } from 'react';
import {
  ArrowLeft,
  ChevronDown,
  ChevronRight,
  Code2,
  Layers,
  Search,
  Terminal,
} from 'lucide-react';
import { useChatStore } from '../../../stores/chat';
import type { VerbProfile } from '../../../types/chat';

interface VerbBrowserProps {
  className?: string;
}

function VerbItem({
  verb,
  onSelect,
}: {
  verb: VerbProfile;
  onSelect: (verb: VerbProfile) => void;
}) {
  const [showArgs, setShowArgs] = useState(false);

  return (
    <div className="group">
      <button
        onClick={() => onSelect(verb)}
        onContextMenu={(e) => {
          e.preventDefault();
          setShowArgs(!showArgs);
        }}
        className="w-full text-left px-2 py-1.5 rounded text-xs hover:bg-[var(--bg-tertiary)] transition-colors"
        title={`Click to insert: ${verb.sexpr}\nRight-click for args`}
      >
        <div className="flex items-center gap-1.5">
          <Code2 size={11} className="flex-shrink-0 text-[var(--accent-blue)]" />
          <span className="font-mono text-[var(--text-primary)] truncate">
            {verb.fqn}
          </span>
        </div>
        <div className="text-[10px] text-[var(--text-muted)] mt-0.5 truncate pl-4">
          {verb.description}
        </div>
      </button>

      {showArgs && verb.args.length > 0 && (
        <div className="ml-4 pl-2 border-l border-[var(--border-primary)] mb-1">
          <div className="font-mono text-[10px] text-[var(--text-secondary)] py-0.5">
            {verb.sexpr}
          </div>
          {verb.args.map((arg) => (
            <div
              key={arg.name}
              className="flex items-center gap-1 text-[10px] py-0.5"
            >
              <span
                className={`font-mono ${arg.required ? 'text-[var(--accent-orange)]' : 'text-[var(--text-muted)]'}`}
              >
                :{arg.name}
              </span>
              <span className="text-[var(--text-muted)]">&lt;{arg.arg_type}&gt;</span>
              {arg.required && (
                <span className="text-[var(--accent-red)] text-[9px]">*</span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Domain card for Stage 1 */
function DomainCard({
  domain,
  count,
  onClick,
}: {
  domain: string;
  count: number;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-center gap-2 px-3 py-2 rounded text-sm text-left hover:bg-[var(--bg-tertiary)] transition-colors"
    >
      <Layers size={14} className="flex-shrink-0 text-[var(--accent-purple)]" />
      <div className="flex-1 min-w-0">
        <div className="font-medium text-[var(--text-primary)] truncate">{domain}</div>
      </div>
      <span className="text-xs text-[var(--text-muted)]">{count}</span>
      <ChevronRight size={14} className="text-[var(--text-muted)]" />
    </button>
  );
}

/** Domain verb list for Stage 2 */
function DomainVerbList({
  domain,
  verbs,
  onSelect,
  onBack,
}: {
  domain: string;
  verbs: VerbProfile[];
  onSelect: (verb: VerbProfile) => void;
  onBack: () => void;
}) {
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    if (!search) return verbs;
    const lower = search.toLowerCase();
    return verbs.filter(
      (v) =>
        v.fqn.toLowerCase().includes(lower) ||
        v.description.toLowerCase().includes(lower),
    );
  }, [verbs, search]);

  return (
    <div className="flex flex-col max-h-[60vh]">
      {/* Header with back */}
      <div className="flex items-center gap-2 px-2 py-1.5 border-b border-[var(--border-primary)]">
        <button
          onClick={onBack}
          className="p-1 hover:bg-[var(--bg-tertiary)] rounded"
        >
          <ArrowLeft size={14} className="text-[var(--text-muted)]" />
        </button>
        <span className="text-xs font-medium text-[var(--text-primary)] flex-1 truncate">
          {domain}
        </span>
        <span className="text-[10px] text-[var(--text-muted)]">{verbs.length} verbs</span>
      </div>

      {/* Search */}
      <div className="px-2 py-1.5 border-b border-[var(--border-primary)]">
        <div className="relative">
          <Search
            size={12}
            className="absolute left-2 top-1/2 -translate-y-1/2 text-[var(--text-muted)]"
          />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Filter verbs..."
            className="w-full pl-6 pr-2 py-1 text-xs bg-[var(--bg-tertiary)] border border-[var(--border-primary)] rounded text-[var(--text-primary)] placeholder:text-[var(--text-muted)] focus:outline-none focus:border-[var(--accent-blue)]"
          />
        </div>
        {search && (
          <div className="text-[10px] text-[var(--text-muted)] mt-1 px-1">
            {filtered.length} of {verbs.length}
          </div>
        )}
      </div>

      {/* Verb list */}
      <div className="flex-1 overflow-auto p-1 space-y-0.5">
        {filtered.length === 0 ? (
          <div className="text-xs text-[var(--text-muted)] text-center py-4">
            No verbs match &ldquo;{search}&rdquo;
          </div>
        ) : (
          filtered.map((verb) => (
            <VerbItem key={verb.fqn} verb={verb} onSelect={onSelect} />
          ))
        )}
      </div>
    </div>
  );
}

export function VerbBrowser({ className = '' }: VerbBrowserProps) {
  const { availableVerbs, setInputValue } = useChatStore();
  const [isExpanded, setIsExpanded] = useState(false);
  const [selectedDomain, setSelectedDomain] = useState<string | null>(null);
  const [domainSearch, setDomainSearch] = useState('');
  const prevCountRef = useRef(0);

  // Auto-expand when verbs first arrive
  useEffect(() => {
    if (availableVerbs.length > 0 && prevCountRef.current === 0) {
      setIsExpanded(true);
    }
    prevCountRef.current = availableVerbs.length;
  }, [availableVerbs.length]);

  // Group verbs by domain
  const { domains, totalCount } = useMemo(() => {
    const grouped = new Map<string, VerbProfile[]>();
    for (const verb of availableVerbs) {
      const existing = grouped.get(verb.domain);
      if (existing) {
        existing.push(verb);
      } else {
        grouped.set(verb.domain, [verb]);
      }
    }

    // Sort domains alphabetically
    const sorted = [...grouped.entries()].sort(([a], [b]) =>
      a.localeCompare(b),
    );

    return {
      domains: sorted,
      totalCount: availableVerbs.length,
    };
  }, [availableVerbs]);

  // Filter domains by search
  const filteredDomains = useMemo(() => {
    if (!domainSearch) return domains;
    const lower = domainSearch.toLowerCase();
    return domains.filter(([name]) => name.toLowerCase().includes(lower));
  }, [domains, domainSearch]);

  const handleSelectVerb = (verb: VerbProfile) => {
    setInputValue(verb.sexpr);
  };

  if (totalCount === 0) {
    return null;
  }

  // Get verbs for selected domain
  const selectedVerbs = selectedDomain
    ? domains.find(([d]) => d === selectedDomain)?.[1] ?? []
    : [];

  return (
    <div className={className}>
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-[var(--border-primary)]">
        <button
          onClick={() => {
            setIsExpanded(!isExpanded);
            if (!isExpanded) setSelectedDomain(null);
          }}
          className="flex items-center gap-2 hover:bg-[var(--bg-tertiary)] rounded px-1 py-0.5 -ml-1"
        >
          {isExpanded ? (
            <ChevronDown size={16} className="text-[var(--text-muted)]" />
          ) : (
            <ChevronRight size={16} className="text-[var(--text-muted)]" />
          )}
          <Terminal size={14} className="text-[var(--accent-purple)]" />
          <span className="text-sm font-medium text-[var(--text-primary)]">
            Commands
          </span>
          <span className="text-xs px-1.5 py-0.5 rounded bg-[var(--accent-purple)] text-white">
            {domains.length}
          </span>
        </button>
      </div>

      {/* Content */}
      {isExpanded && (
        selectedDomain ? (
          /* Stage 2: Domain verb list */
          <DomainVerbList
            domain={selectedDomain}
            verbs={selectedVerbs}
            onSelect={handleSelectVerb}
            onBack={() => setSelectedDomain(null)}
          />
        ) : (
          /* Stage 1: Domain cards */
          <div className="flex flex-col max-h-[60vh]">
            {/* Domain search (only if many domains) */}
            {domains.length > 8 && (
              <div className="px-2 py-1.5 border-b border-[var(--border-primary)]">
                <div className="relative">
                  <Search
                    size={12}
                    className="absolute left-2 top-1/2 -translate-y-1/2 text-[var(--text-muted)]"
                  />
                  <input
                    type="text"
                    value={domainSearch}
                    onChange={(e) => setDomainSearch(e.target.value)}
                    placeholder="Search domains..."
                    className="w-full pl-6 pr-2 py-1 text-xs bg-[var(--bg-tertiary)] border border-[var(--border-primary)] rounded text-[var(--text-primary)] placeholder:text-[var(--text-muted)] focus:outline-none focus:border-[var(--accent-blue)]"
                  />
                </div>
              </div>
            )}

            {/* Domain list */}
            <div className="flex-1 overflow-auto p-1 space-y-0.5">
              {filteredDomains.length === 0 ? (
                <div className="text-xs text-[var(--text-muted)] text-center py-4">
                  No domains match &ldquo;{domainSearch}&rdquo;
                </div>
              ) : (
                filteredDomains.map(([domain, verbs]) => (
                  <DomainCard
                    key={domain}
                    domain={domain}
                    count={verbs.length}
                    onClick={() => setSelectedDomain(domain)}
                  />
                ))
              )}
            </div>

            {/* Footer */}
            <div className="px-3 py-1.5 border-t border-[var(--border-primary)] text-[10px] text-[var(--text-muted)]">
              {totalCount} verbs across {domains.length} domains
            </div>
          </div>
        )
      )}
    </div>
  );
}

export default VerbBrowser;
