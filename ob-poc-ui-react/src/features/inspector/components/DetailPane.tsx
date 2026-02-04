/**
 * DetailPane - Right panel showing focused node details
 */

import { useInspectorStore } from '../../../stores/inspector';
import { NodeCard } from './NodeCard';
import { cn } from '../../../lib/utils';

interface DetailPaneProps {
  className?: string;
}

export function DetailPane({ className }: DetailPaneProps) {
  const { focusedNodeId, getNodeById, projection } = useInspectorStore();

  // Get the focused node (or root if none focused)
  const node = focusedNodeId
    ? getNodeById(focusedNodeId)
    : projection?.root;

  if (!node) {
    return (
      <div className={cn('flex items-center justify-center p-8', className)}>
        <p className="text-[var(--text-muted)]">
          Select a node to view details
        </p>
      </div>
    );
  }

  return (
    <div className={cn('p-4 space-y-4', className)}>
      <NodeCard node={node} showChildren />
    </div>
  );
}

export default DetailPane;
