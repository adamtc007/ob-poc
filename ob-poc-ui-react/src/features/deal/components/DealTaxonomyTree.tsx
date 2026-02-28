/**
 * Deal Taxonomy Tree - Navigation tree for deal hierarchy
 *
 * Displays the deal structure as an expandable tree:
 * Deal -> Products -> Rate Cards -> Lines
 *      -> Participants
 *      -> Contracts
 *      -> Onboarding Requests
 */

import { useState, useCallback, useMemo } from "react";
import {
  ChevronRight,
  ChevronDown,
  FileText,
  Package,
  CreditCard,
  Users,
  FileSignature,
  Rocket,
  CircleDot,
} from "lucide-react";
import { cn } from "../../../lib/utils";
import type {
  DealTaxonomyNode,
  DealGraphResponse,
  RateCardSummary,
} from "../../../types/deal";

interface DealTaxonomyTreeProps {
  graph: DealGraphResponse;
  selectedNodeId?: string;
  onSelectNode: (node: DealTaxonomyNode) => void;
}

/** Get icon for node type */
function getNodeIcon(type: DealTaxonomyNode["type"]) {
  switch (type) {
    case "deal":
      return FileText;
    case "product_list":
    case "product":
      return Package;
    case "rate_card_list":
    case "rate_card":
    case "line":
      return CreditCard;
    case "participant_list":
    case "participant":
      return Users;
    case "contract_list":
    case "contract":
      return FileSignature;
    case "onboarding_list":
    case "onboarding":
      return Rocket;
    default:
      return CircleDot;
  }
}

/** Build taxonomy tree from graph response */
function buildTree(graph: DealGraphResponse): DealTaxonomyNode {
  const { deal, products, participants, contracts, onboarding_requests } =
    graph;

  // Group rate cards by product
  const rateCardsByProduct = new Map<string, RateCardSummary[]>();
  for (const rc of graph.rate_cards) {
    const list = rateCardsByProduct.get(rc.deal_product_id) || [];
    list.push(rc);
    rateCardsByProduct.set(rc.deal_product_id, list);
  }

  // Build product nodes
  const productNodes: DealTaxonomyNode[] = products.map((product) => {
    const productRateCards =
      rateCardsByProduct.get(product.deal_product_id) || [];
    const rateCardNodes: DealTaxonomyNode[] = productRateCards.map((rc) => ({
      id: `rate_card:${rc.rate_card_id}`,
      type: "rate_card" as const,
      label: rc.rate_card_name,
      data: rc,
      childCount: rc.line_count,
    }));

    return {
      id: `product:${product.deal_product_id}`,
      type: "product" as const,
      label: product.product_name,
      data: product,
      children: rateCardNodes.length > 0 ? rateCardNodes : undefined,
      childCount: product.rate_card_count,
    };
  });

  // Build participant nodes
  const participantNodes: DealTaxonomyNode[] = participants.map((p) => ({
    id: `participant:${p.participant_id}`,
    type: "participant" as const,
    label: `${p.entity_name} (${p.role})`,
    data: p,
  }));

  // Build contract nodes
  const contractNodes: DealTaxonomyNode[] = contracts.map((c) => ({
    id: `contract:${c.contract_id}`,
    type: "contract" as const,
    label: c.contract_name,
    data: c,
  }));

  // Build onboarding request nodes
  const onboardingNodes: DealTaxonomyNode[] = onboarding_requests.map((r) => ({
    id: `onboarding:${r.request_id}`,
    type: "onboarding" as const,
    label: `${r.request_type} - ${r.status}`,
    data: r,
  }));

  // Build root deal node
  const dealChildren: DealTaxonomyNode[] = [];

  if (productNodes.length > 0) {
    dealChildren.push({
      id: `products:${deal.deal_id}`,
      type: "product_list",
      label: `Products (${products.length})`,
      children: productNodes,
      childCount: products.length,
    });
  }

  if (participantNodes.length > 0) {
    dealChildren.push({
      id: `participants:${deal.deal_id}`,
      type: "participant_list",
      label: `Participants (${participants.length})`,
      children: participantNodes,
      childCount: participants.length,
    });
  }

  if (contractNodes.length > 0) {
    dealChildren.push({
      id: `contracts:${deal.deal_id}`,
      type: "contract_list",
      label: `Contracts (${contracts.length})`,
      children: contractNodes,
      childCount: contracts.length,
    });
  }

  if (onboardingNodes.length > 0) {
    dealChildren.push({
      id: `onboarding:${deal.deal_id}`,
      type: "onboarding_list",
      label: `Onboarding Requests (${onboarding_requests.length})`,
      children: onboardingNodes,
      childCount: onboarding_requests.length,
    });
  }

  return {
    id: `deal:${deal.deal_id}`,
    type: "deal",
    label: deal.deal_name,
    data: deal,
    children: dealChildren,
    expanded: true,
  };
}

interface TreeNodeProps {
  node: DealTaxonomyNode;
  depth: number;
  selectedNodeId?: string;
  onSelectNode: (node: DealTaxonomyNode) => void;
  expandedNodes: Set<string>;
  toggleExpanded: (nodeId: string) => void;
}

function TreeNode({
  node,
  depth,
  selectedNodeId,
  onSelectNode,
  expandedNodes,
  toggleExpanded,
}: TreeNodeProps) {
  const hasChildren = node.children && node.children.length > 0;
  const isExpanded = expandedNodes.has(node.id);
  const isSelected = selectedNodeId === node.id;
  const nodeIcon = useMemo(
    () => getNodeIcon(node.type)({ size: 14, className: "flex-shrink-0 text-[var(--text-muted)]" }),
    [node.type],
  );

  const handleToggle = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      toggleExpanded(node.id);
    },
    [node.id, toggleExpanded],
  );

  const handleSelect = useCallback(() => {
    onSelectNode(node);
  }, [node, onSelectNode]);

  return (
    <div>
      <div
        className={cn(
          "flex items-center gap-1 px-2 py-1.5 cursor-pointer rounded-md transition-colors",
          isSelected
            ? "bg-[var(--accent-blue)]/20 text-[var(--accent-blue)]"
            : "hover:bg-[var(--bg-hover)] text-[var(--text-primary)]",
        )}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
        onClick={handleSelect}
      >
        {hasChildren ? (
          <button
            onClick={handleToggle}
            className="p-0.5 hover:bg-[var(--bg-tertiary)] rounded"
          >
            {isExpanded ? (
              <ChevronDown size={14} />
            ) : (
              <ChevronRight size={14} />
            )}
          </button>
        ) : (
          <span className="w-5" />
        )}
        {nodeIcon}
        <span className="text-sm truncate flex-1">{node.label}</span>
        {node.childCount !== undefined && node.childCount > 0 && (
          <span className="text-xs text-[var(--text-muted)] tabular-nums">
            {node.childCount}
          </span>
        )}
      </div>
      {hasChildren && isExpanded && (
        <div>
          {node.children!.map((child) => (
            <TreeNode
              key={child.id}
              node={child}
              depth={depth + 1}
              selectedNodeId={selectedNodeId}
              onSelectNode={onSelectNode}
              expandedNodes={expandedNodes}
              toggleExpanded={toggleExpanded}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export function DealTaxonomyTree({
  graph,
  selectedNodeId,
  onSelectNode,
}: DealTaxonomyTreeProps) {
  const [expandedNodes, setExpandedNodes] = useState<Set<string>>(() => {
    // Start with root deal node expanded
    return new Set([`deal:${graph.deal.deal_id}`]);
  });

  const toggleExpanded = useCallback((nodeId: string) => {
    setExpandedNodes((prev) => {
      const next = new Set(prev);
      if (next.has(nodeId)) {
        next.delete(nodeId);
      } else {
        next.add(nodeId);
      }
      return next;
    });
  }, []);

  const tree = buildTree(graph);

  return (
    <div className="py-2">
      <TreeNode
        node={tree}
        depth={0}
        selectedNodeId={selectedNodeId}
        onSelectNode={onSelectNode}
        expandedNodes={expandedNodes}
        toggleExpanded={toggleExpanded}
      />
    </div>
  );
}

export default DealTaxonomyTree;
