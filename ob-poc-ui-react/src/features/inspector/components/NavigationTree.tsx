/**
 * NavigationTree - Left panel tree view using react-arborist
 */

import { Tree, type NodeRendererProps } from "react-arborist";
import { ChevronRight, ChevronDown } from "lucide-react";
import { useInspectorStore } from "../../../stores/inspector";
import type { ProjectionNode } from "../../../types/projection";
import { cn, getKindIcon, getEntityColor } from "../../../lib/utils";
import { useMemo, useCallback } from "react";
import * as LucideIcons from "lucide-react";

/** Tree data node shape for react-arborist */
interface TreeNode {
  id: string;
  name: string;
  kind: string;
  entityType?: string;
  children?: TreeNode[];
  data: ProjectionNode;
}

/** Convert projection tree to react-arborist format */
function toTreeData(node: ProjectionNode): TreeNode {
  return {
    id: node.id,
    name: node.label,
    kind: node.kind,
    entityType: node.meta.entity_type,
    children: node.children?.map(toTreeData),
    data: node,
  };
}

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

/** Individual tree node renderer */
function NodeRenderer({
  node,
  style,
  dragHandle,
}: NodeRendererProps<TreeNode>) {
  const { selectNode, focusNode, selectedNodeId } = useInspectorStore();
  const isSelected = selectedNodeId === node.data.id;

  const iconName = getKindIcon(node.data.kind);
  const color = getEntityColor(node.data.entityType || node.data.kind);

  const handleClick = () => {
    selectNode(node.data.id);
  };

  const handleDoubleClick = () => {
    focusNode(node.data.id, node.data.name);
  };

  return (
    <div
      ref={dragHandle}
      style={style}
      className={cn(
        "flex items-center gap-1.5 rounded px-2 py-1 cursor-pointer text-sm",
        isSelected
          ? "bg-[var(--accent-blue)]/20 text-[var(--text-primary)]"
          : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]",
      )}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
    >
      {/* Expand/collapse toggle */}
      <span
        className="flex h-4 w-4 items-center justify-center"
        onClick={(e) => {
          e.stopPropagation();
          node.toggle();
        }}
      >
        {node.data.children && node.data.children.length > 0 ? (
          node.isOpen ? (
            <ChevronDown size={14} className="text-[var(--text-muted)]" />
          ) : (
            <ChevronRight size={14} className="text-[var(--text-muted)]" />
          )
        ) : null}
      </span>

      {/* Kind icon */}
      <DynamicIcon name={iconName} size={14} color={color} />

      {/* Label */}
      <span className="truncate">{node.data.name}</span>

      {/* Child count badge */}
      {node.data.children && node.data.children.length > 0 && !node.isOpen && (
        <span className="ml-auto text-xs text-[var(--text-muted)]">
          {node.data.children.length}
        </span>
      )}
    </div>
  );
}

interface NavigationTreeProps {
  className?: string;
}

export function NavigationTree({ className }: NavigationTreeProps) {
  const { projection, expandedNodes, toggleExpanded } = useInspectorStore();

  const treeData = useMemo(() => {
    if (!projection) return [];
    return [toTreeData(projection.root)];
  }, [projection]);

  const initialOpenState = useMemo(() => {
    const state: Record<string, boolean> = {};
    expandedNodes.forEach((id) => {
      state[id] = true;
    });
    return state;
  }, [expandedNodes]);

  const handleToggle = useCallback(
    (id: string) => {
      toggleExpanded(id);
    },
    [toggleExpanded],
  );

  if (!projection) {
    return (
      <div className={cn("p-4 text-sm text-[var(--text-muted)]", className)}>
        No projection loaded
      </div>
    );
  }

  return (
    <div className={cn("h-full", className)}>
      <Tree
        data={treeData}
        openByDefault={false}
        initialOpenState={initialOpenState}
        onToggle={handleToggle}
        indent={16}
        rowHeight={28}
        overscanCount={5}
        width="100%"
        height={600}
      >
        {NodeRenderer}
      </Tree>
    </div>
  );
}

export default NavigationTree;
