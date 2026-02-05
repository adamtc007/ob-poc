/**
 * NodeCard - Generic node display card
 */

import type {
  ProjectionNode,
  FieldValue,
  NodeRef,
} from "../../../types/projection";
import { isNodeRef } from "../../../types/projection";
import { RefLink } from "./RefLink";
import { cn, getKindIcon, getEntityColor } from "../../../lib/utils";
import * as LucideIcons from "lucide-react";

/** Icon map for kind icons */
const iconMap: Record<string, React.ComponentType<LucideIcons.LucideProps>> = {
  "building-2": LucideIcons.Building2,
  wallet: LucideIcons.Wallet,
  user: LucideIcons.User,
  building: LucideIcons.Building,
  "file-text": LucideIcons.FileText,
  coins: LucideIcons.Coins,
  "git-branch": LucideIcons.GitBranch,
  "line-chart": LucideIcons.LineChart,
  "file-signature": LucideIcons.FileSignature,
  circle: LucideIcons.Circle,
};

/** Dynamic icon component that renders based on icon name */
function DynamicIcon({
  name,
  size,
  color,
}: {
  name: string;
  size: number;
  color: string;
}) {
  const Icon = iconMap[name] || LucideIcons.Circle;
  return <Icon size={size} color={color} />;
}

/** Render a field value */
function FieldValueRenderer({
  value,
  depth = 0,
}: {
  value: FieldValue;
  depth?: number;
}) {
  // Handle null
  if (value === null) {
    return <span className="text-[var(--text-muted)] italic">null</span>;
  }

  // Handle primitives
  if (typeof value === "string") {
    return <span className="text-[var(--accent-green)]">"{value}"</span>;
  }
  if (typeof value === "number") {
    return <span className="text-[var(--accent-yellow)]">{value}</span>;
  }
  if (typeof value === "boolean") {
    return (
      <span className="text-[var(--accent-purple)]">
        {value ? "true" : "false"}
      </span>
    );
  }

  // Handle $ref
  if (isNodeRef(value)) {
    return <RefLink nodeRef={value as NodeRef} />;
  }

  // Handle arrays
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return <span className="text-[var(--text-muted)]">[]</span>;
    }
    return (
      <div className="space-y-1">
        {value.map((item, i) => (
          <div key={i} className="flex items-start gap-2">
            <span className="text-[var(--text-muted)]">{i}:</span>
            <FieldValueRenderer value={item} depth={depth + 1} />
          </div>
        ))}
      </div>
    );
  }

  // Handle objects
  if (typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) {
      return <span className="text-[var(--text-muted)]">{"{}"}</span>;
    }
    return (
      <div
        className={cn(
          "space-y-1",
          depth > 0 && "pl-4 border-l border-[var(--border-primary)]",
        )}
      >
        {entries.map(([key, val]) => (
          <div key={key} className="flex items-start gap-2">
            <span className="text-[var(--text-secondary)] font-medium">
              {key}:
            </span>
            <FieldValueRenderer value={val} depth={depth + 1} />
          </div>
        ))}
      </div>
    );
  }

  return null;
}

interface NodeCardProps {
  node: ProjectionNode;
  className?: string;
  showChildren?: boolean;
}

export function NodeCard({
  node,
  className,
  showChildren = false,
}: NodeCardProps) {
  const iconName = getKindIcon(node.kind);
  const color = getEntityColor(node.meta.entity_type || node.kind);

  return (
    <div
      className={cn(
        "rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)]",
        className,
      )}
    >
      {/* Header */}
      <div className="flex items-center gap-3 border-b border-[var(--border-primary)] px-4 py-3">
        <DynamicIcon name={iconName} size={20} color={color} />
        <div className="flex-1 min-w-0">
          <h3 className="font-medium text-[var(--text-primary)] truncate">
            {node.label}
          </h3>
          {node.label_full && node.label_full !== node.label && (
            <p className="text-sm text-[var(--text-secondary)] truncate">
              {node.label_full}
            </p>
          )}
        </div>
        <span className="rounded bg-[var(--bg-tertiary)] px-2 py-0.5 text-xs text-[var(--text-muted)]">
          {node.kind}
        </span>
      </div>

      {/* Metadata */}
      <div className="border-b border-[var(--border-primary)] px-4 py-2">
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-[var(--text-muted)]">
          <span>Chamber: {node.meta.chamber}</span>
          {node.meta.entity_type && <span>Type: {node.meta.entity_type}</span>}
          <span>LOD: {node.meta.lod_generated}</span>
          {node.meta.child_count !== undefined && (
            <span>Children: {node.meta.child_count}</span>
          )}
          {node.meta.truncated && (
            <span className="text-[var(--accent-yellow)]">Truncated</span>
          )}
        </div>
      </div>

      {/* Fields */}
      <div className="p-4">
        <h4 className="mb-2 text-sm font-medium text-[var(--text-secondary)]">
          Fields
        </h4>
        <div className="space-y-2 text-sm">
          {Object.entries(node.fields).map(([key, value]) => (
            <div key={key} className="flex items-start gap-2">
              <span className="text-[var(--text-secondary)] font-medium min-w-[100px]">
                {key}:
              </span>
              <div className="flex-1 overflow-hidden">
                <FieldValueRenderer value={value} />
              </div>
            </div>
          ))}
          {Object.keys(node.fields).length === 0 && (
            <p className="text-[var(--text-muted)] italic">No fields</p>
          )}
        </div>
      </div>

      {/* Children preview */}
      {showChildren && node.children && node.children.length > 0 && (
        <div className="border-t border-[var(--border-primary)] p-4">
          <h4 className="mb-2 text-sm font-medium text-[var(--text-secondary)]">
            Children ({node.children.length})
          </h4>
          <div className="space-y-1">
            {node.children.slice(0, 5).map((child) => (
              <div
                key={child.id}
                className="flex items-center gap-2 rounded px-2 py-1 text-sm text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
              >
                <span className="truncate">{child.label}</span>
                <span className="text-xs text-[var(--text-muted)]">
                  {child.kind}
                </span>
              </div>
            ))}
            {node.children.length > 5 && (
              <p className="text-xs text-[var(--text-muted)] pl-2">
                ...and {node.children.length - 5} more
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default NodeCard;
