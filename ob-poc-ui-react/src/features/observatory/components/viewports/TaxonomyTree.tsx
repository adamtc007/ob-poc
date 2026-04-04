/**
 * TaxonomyTree — collapsible tree view from taxonomy data.
 *
 * Each node shows a label and membership indicators.
 * Uses nested <details> elements for collapsing.
 */

interface TaxonomyNode {
  label: string;
  id?: string;
  membership?: string[];
  children?: TaxonomyNode[];
}

interface Props {
  data: unknown;
}

export function TaxonomyTree({ data }: Props) {
  if (!data || typeof data !== "object") {
    return (
      <div className="text-xs text-[var(--text-secondary)]">
        No taxonomy data
      </div>
    );
  }

  const root = data as Record<string, unknown>;
  const nodes = (root.nodes as TaxonomyNode[]) ?? [];

  if (nodes.length === 0) {
    return (
      <div className="text-xs text-[var(--text-secondary)]">
        Empty taxonomy
      </div>
    );
  }

  return (
    <div className="space-y-1">
      {nodes.map((node, i) => (
        <TaxonomyNodeView key={node.id ?? i} node={node} depth={0} />
      ))}
    </div>
  );
}

function TaxonomyNodeView({
  node,
  depth,
}: {
  node: TaxonomyNode;
  depth: number;
}) {
  const hasChildren = node.children && node.children.length > 0;

  if (!hasChildren) {
    return (
      <div
        className="flex items-center gap-2 py-0.5"
        style={{ paddingLeft: `${depth * 12}px` }}
      >
        <span className="w-3 text-center text-[var(--text-muted)]">&bull;</span>
        <span className="text-xs text-[var(--text-primary)]">{node.label}</span>
        {node.membership && node.membership.length > 0 && (
          <MembershipBadges membership={node.membership} />
        )}
      </div>
    );
  }

  return (
    <details
      className="group"
      style={{ paddingLeft: `${depth * 12}px` }}
      open={depth < 2}
    >
      <summary className="flex items-center gap-2 py-0.5 cursor-pointer text-xs text-[var(--text-primary)] font-medium list-none">
        <span className="w-3 text-center text-[var(--text-muted)] group-open:rotate-90 transition-transform">
          &#9654;
        </span>
        {node.label}
        {node.membership && node.membership.length > 0 && (
          <MembershipBadges membership={node.membership} />
        )}
      </summary>
      <div className="mt-0.5">
        {node.children!.map((child, i) => (
          <TaxonomyNodeView
            key={child.id ?? i}
            node={child}
            depth={depth + 1}
          />
        ))}
      </div>
    </details>
  );
}

function MembershipBadges({ membership }: { membership: string[] }) {
  return (
    <span className="flex gap-1 ml-auto">
      {membership.map((m) => (
        <span
          key={m}
          className="px-1 py-0.5 rounded text-[9px] bg-[var(--bg-active)] text-[var(--text-secondary)]"
        >
          {m}
        </span>
      ))}
    </span>
  );
}

export default TaxonomyTree;
