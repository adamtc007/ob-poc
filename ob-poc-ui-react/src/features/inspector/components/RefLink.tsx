/**
 * RefLink - Clickable $ref link component
 *
 * Renders node references as clickable links that navigate to the target node.
 */

import { ExternalLink } from 'lucide-react';
import { useInspectorStore } from '../../../stores/inspector';
import type { NodeRef } from '../../../types/projection';
import { cn } from '../../../lib/utils';

interface RefLinkProps {
  nodeRef: NodeRef;
  className?: string;
}

export function RefLink({ nodeRef, className }: RefLinkProps) {
  const { focusNode, getNodeById } = useInspectorStore();

  // Extract node ID from $ref (format: "#/nodes/{id}")
  const refPath = nodeRef.$ref;
  const nodeId = refPath.split('/').pop() || refPath;

  // Try to get node info for display
  const targetNode = getNodeById(nodeId);
  const displayText = targetNode?.label || nodeId;

  const handleClick = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (targetNode) {
      focusNode(nodeId, targetNode.label);
    }
  };

  return (
    <button
      onClick={handleClick}
      className={cn(
        'inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-sm',
        'bg-[var(--accent-blue)]/10 text-[var(--accent-blue)]',
        'hover:bg-[var(--accent-blue)]/20 transition-colors',
        className
      )}
      title={`Go to: ${displayText}`}
    >
      <span className="truncate max-w-[200px]">{displayText}</span>
      <ExternalLink size={12} />
    </button>
  );
}

export default RefLink;
