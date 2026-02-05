/**
 * TradingMatrixTree - Hierarchical tree view for trading matrix documents
 *
 * Features:
 * - Expandable/collapsible tree nodes
 * - Node type icons and status colors
 * - Click to select and view details
 * - Keyboard navigation
 * - Breadcrumb trail for current path
 */

import { useState, useCallback } from "react";
import { ChevronRight, ChevronDown, Loader2 } from "lucide-react";
import type {
  TradingMatrixNode,
  TradingMatrixNodeType,
  TradingMatrixResponse,
} from "../../../api/tradingMatrix";
import {
  getNodeTypeIcon,
  getStatusColorClass,
  getNodeTypeLabel,
} from "../../../api/tradingMatrix";

// =============================================================================
// TYPES
// =============================================================================

interface TradingMatrixTreeProps {
  data: TradingMatrixResponse | null;
  loading?: boolean;
  error?: string;
  onNodeSelect?: (node: TradingMatrixNode) => void;
  selectedNodeId?: string[];
}

interface TreeNodeProps {
  node: TradingMatrixNode;
  depth: number;
  expanded: Set<string>;
  onToggle: (nodeIdPath: string) => void;
  onSelect: (node: TradingMatrixNode) => void;
  selectedNodeId?: string[];
}

// =============================================================================
// HELPERS
// =============================================================================

/** Convert node ID array to string key for Set operations */
function nodeIdToKey(id: string[]): string {
  return id.join("/");
}

/** Get detail text for a node based on its type */
function getNodeDetail(nodeType: TradingMatrixNodeType): string | null {
  switch (nodeType.type) {
    case "instrument_class":
      return nodeType.cfi_prefix ? `CFI: ${nodeType.cfi_prefix}` : null;
    case "market":
      return `${nodeType.mic} • ${nodeType.country_code}`;
    case "counterparty":
      return nodeType.lei || null;
    case "universe_entry":
      return `${nodeType.currencies.join(", ")} • ${nodeType.settlement_types.join(", ")}`;
    case "ssi":
      return nodeType.status;
    case "booking_rule":
      return `Priority: ${nodeType.priority}`;
    case "settlement_chain":
      return `${nodeType.hop_count} hops`;
    case "settlement_hop":
      return `${nodeType.sequence}. ${nodeType.role}`;
    case "tax_jurisdiction":
      return nodeType.default_withholding_rate
        ? `WHT: ${nodeType.default_withholding_rate}%`
        : null;
    case "isda_agreement":
      return nodeType.governing_law ? `${nodeType.governing_law} Law` : null;
    case "csa_agreement":
      return nodeType.csa_type;
    case "product_coverage":
      return nodeType.asset_class;
    case "investment_manager_mandate":
      return nodeType.role;
    case "pricing_rule":
      return nodeType.source;
    case "corporate_actions_policy":
      return `${nodeType.enabled_count} event types`;
    case "ca_event_type_config":
      return nodeType.processing_mode;
    default:
      return null;
  }
}

// =============================================================================
// TREE NODE COMPONENT
// =============================================================================

function TreeNode({
  node,
  depth,
  expanded,
  onToggle,
  onSelect,
  selectedNodeId,
}: TreeNodeProps) {
  const nodeKey = nodeIdToKey(node.id);
  const isExpanded = expanded.has(nodeKey);
  const hasChildren = node.children.length > 0;
  const isSelected = selectedNodeId && nodeIdToKey(selectedNodeId) === nodeKey;

  const icon = getNodeTypeIcon(node.node_type);
  const detail = getNodeDetail(node.node_type);
  const statusClass = getStatusColorClass(node.status_color);

  const handleToggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (hasChildren) {
      onToggle(nodeKey);
    }
  };

  const handleSelect = () => {
    onSelect(node);
  };

  return (
    <div className="select-none">
      {/* Node row */}
      <div
        className={`
          flex items-center gap-1 py-1 px-2 rounded cursor-pointer
          hover:bg-gray-700/50 transition-colors
          ${isSelected ? "bg-blue-600/30 ring-1 ring-blue-500" : ""}
        `}
        style={{ paddingLeft: `${depth * 16 + 4}px` }}
        onClick={handleSelect}
        onDoubleClick={handleToggle}
      >
        {/* Expand/collapse toggle */}
        <button
          onClick={handleToggle}
          className={`
            w-5 h-5 flex items-center justify-center rounded
            hover:bg-gray-600/50 transition-colors
            ${hasChildren ? "" : "invisible"}
          `}
        >
          {isExpanded ? (
            <ChevronDown className="w-4 h-4 text-gray-400" />
          ) : (
            <ChevronRight className="w-4 h-4 text-gray-400" />
          )}
        </button>

        {/* Icon */}
        <span className="text-sm mr-1">{icon}</span>

        {/* Label */}
        <span className={`flex-1 truncate ${statusClass}`}>{node.label}</span>

        {/* Sublabel or detail */}
        {(node.sublabel || detail) && (
          <span className="text-xs text-gray-500 ml-2 truncate max-w-[150px]">
            {node.sublabel || detail}
          </span>
        )}

        {/* Leaf count badge */}
        {hasChildren && node.leaf_count > 0 && (
          <span className="text-xs text-gray-500 bg-gray-700 px-1.5 rounded">
            {node.leaf_count}
          </span>
        )}
      </div>

      {/* Children */}
      {isExpanded && hasChildren && (
        <div>
          {node.children.map((child, idx) => (
            <TreeNode
              key={nodeIdToKey(child.id) || idx}
              node={child}
              depth={depth + 1}
              expanded={expanded}
              onToggle={onToggle}
              onSelect={onSelect}
              selectedNodeId={selectedNodeId}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// =============================================================================
// NODE DETAIL PANEL
// =============================================================================

interface NodeDetailPanelProps {
  node: TradingMatrixNode | null;
}

function NodeDetailPanel({ node }: NodeDetailPanelProps) {
  if (!node) {
    return (
      <div className="p-4 text-center text-gray-500">
        <p>Select a node to view details</p>
      </div>
    );
  }

  const typeLabel = getNodeTypeLabel(node.node_type);
  const icon = getNodeTypeIcon(node.node_type);

  // Extract all fields from node_type for display
  const fields: { label: string; value: string }[] = [];
  const nodeType = node.node_type;

  switch (nodeType.type) {
    case "category":
      fields.push({ label: "Name", value: nodeType.name });
      break;
    case "instrument_class":
      fields.push({ label: "Class Code", value: nodeType.class_code });
      if (nodeType.cfi_prefix)
        fields.push({ label: "CFI Prefix", value: nodeType.cfi_prefix });
      fields.push({ label: "OTC", value: nodeType.is_otc ? "Yes" : "No" });
      break;
    case "market":
      fields.push({ label: "MIC", value: nodeType.mic });
      fields.push({ label: "Name", value: nodeType.market_name });
      fields.push({ label: "Country", value: nodeType.country_code });
      break;
    case "counterparty":
      fields.push({ label: "Entity ID", value: nodeType.entity_id });
      fields.push({ label: "Name", value: nodeType.entity_name });
      if (nodeType.lei) fields.push({ label: "LEI", value: nodeType.lei });
      break;
    case "universe_entry":
      fields.push({ label: "Universe ID", value: nodeType.universe_id });
      fields.push({
        label: "Currencies",
        value: nodeType.currencies.join(", "),
      });
      fields.push({
        label: "Settlement Types",
        value: nodeType.settlement_types.join(", "),
      });
      fields.push({ label: "Held", value: nodeType.is_held ? "Yes" : "No" });
      fields.push({
        label: "Traded",
        value: nodeType.is_traded ? "Yes" : "No",
      });
      break;
    case "ssi":
      fields.push({ label: "SSI ID", value: nodeType.ssi_id });
      fields.push({ label: "Name", value: nodeType.ssi_name });
      fields.push({ label: "Type", value: nodeType.ssi_type });
      fields.push({ label: "Status", value: nodeType.status });
      if (nodeType.safekeeping_account)
        fields.push({
          label: "Safekeeping Acct",
          value: nodeType.safekeeping_account,
        });
      if (nodeType.safekeeping_bic)
        fields.push({
          label: "Safekeeping BIC",
          value: nodeType.safekeeping_bic,
        });
      if (nodeType.cash_account)
        fields.push({ label: "Cash Acct", value: nodeType.cash_account });
      if (nodeType.cash_bic)
        fields.push({ label: "Cash BIC", value: nodeType.cash_bic });
      if (nodeType.pset_bic)
        fields.push({ label: "PSET BIC", value: nodeType.pset_bic });
      if (nodeType.cash_currency)
        fields.push({ label: "Currency", value: nodeType.cash_currency });
      break;
    case "booking_rule":
      fields.push({ label: "Rule ID", value: nodeType.rule_id });
      fields.push({ label: "Name", value: nodeType.rule_name });
      fields.push({ label: "Priority", value: nodeType.priority.toString() });
      fields.push({
        label: "Specificity",
        value: nodeType.specificity_score.toString(),
      });
      fields.push({
        label: "Active",
        value: nodeType.is_active ? "Yes" : "No",
      });
      if (nodeType.match_criteria) {
        const mc = nodeType.match_criteria;
        if (mc.instrument_class)
          fields.push({ label: "Match Class", value: mc.instrument_class });
        if (mc.mic) fields.push({ label: "Match MIC", value: mc.mic });
        if (mc.currency)
          fields.push({ label: "Match Currency", value: mc.currency });
        if (mc.settlement_type)
          fields.push({
            label: "Match Settle Type",
            value: mc.settlement_type,
          });
      }
      break;
    case "settlement_chain":
      fields.push({ label: "Chain ID", value: nodeType.chain_id });
      fields.push({ label: "Name", value: nodeType.chain_name });
      fields.push({ label: "Hops", value: nodeType.hop_count.toString() });
      fields.push({
        label: "Active",
        value: nodeType.is_active ? "Yes" : "No",
      });
      if (nodeType.mic) fields.push({ label: "MIC", value: nodeType.mic });
      if (nodeType.currency)
        fields.push({ label: "Currency", value: nodeType.currency });
      break;
    case "settlement_hop":
      fields.push({ label: "Hop ID", value: nodeType.hop_id });
      fields.push({ label: "Sequence", value: nodeType.sequence.toString() });
      fields.push({ label: "Role", value: nodeType.role });
      if (nodeType.intermediary_bic)
        fields.push({ label: "BIC", value: nodeType.intermediary_bic });
      if (nodeType.intermediary_name)
        fields.push({
          label: "Intermediary",
          value: nodeType.intermediary_name,
        });
      break;
    case "isda_agreement":
      fields.push({ label: "ISDA ID", value: nodeType.isda_id });
      fields.push({ label: "Counterparty", value: nodeType.counterparty_name });
      if (nodeType.governing_law)
        fields.push({ label: "Governing Law", value: nodeType.governing_law });
      if (nodeType.agreement_date)
        fields.push({ label: "Date", value: nodeType.agreement_date });
      if (nodeType.counterparty_lei)
        fields.push({ label: "LEI", value: nodeType.counterparty_lei });
      break;
    case "csa_agreement":
      fields.push({ label: "CSA ID", value: nodeType.csa_id });
      fields.push({ label: "Type", value: nodeType.csa_type });
      if (nodeType.threshold_currency)
        fields.push({
          label: "Threshold CCY",
          value: nodeType.threshold_currency,
        });
      if (nodeType.threshold_amount !== undefined)
        fields.push({
          label: "Threshold",
          value: nodeType.threshold_amount.toString(),
        });
      if (nodeType.minimum_transfer_amount !== undefined)
        fields.push({
          label: "MTA",
          value: nodeType.minimum_transfer_amount.toString(),
        });
      if (nodeType.collateral_ssi_ref)
        fields.push({
          label: "Collateral SSI",
          value: nodeType.collateral_ssi_ref,
        });
      break;
    case "investment_manager_mandate":
      fields.push({ label: "Mandate ID", value: nodeType.mandate_id });
      fields.push({ label: "Manager", value: nodeType.manager_name });
      fields.push({ label: "Role", value: nodeType.role });
      fields.push({ label: "Priority", value: nodeType.priority.toString() });
      fields.push({
        label: "Can Trade",
        value: nodeType.can_trade ? "Yes" : "No",
      });
      fields.push({
        label: "Can Settle",
        value: nodeType.can_settle ? "Yes" : "No",
      });
      break;
    default:
      // For other types, just show the type
      fields.push({ label: "Type", value: nodeType.type });
  }

  return (
    <div className="p-4 space-y-4">
      {/* Header */}
      <div className="flex items-center gap-2">
        <span className="text-2xl">{icon}</span>
        <div>
          <h3 className="text-lg font-semibold text-white">{node.label}</h3>
          <p className="text-sm text-gray-400">{typeLabel}</p>
        </div>
      </div>

      {/* Path */}
      <div className="text-xs text-gray-500 font-mono bg-gray-800 p-2 rounded overflow-x-auto">
        {node.id.join(" / ")}
      </div>

      {/* Fields */}
      <div className="space-y-2">
        {fields.map((field, idx) => (
          <div key={idx} className="flex gap-2">
            <span className="text-gray-500 min-w-[100px]">{field.label}:</span>
            <span className="text-white font-mono text-sm break-all">
              {field.value}
            </span>
          </div>
        ))}
      </div>

      {/* Children count */}
      {node.children.length > 0 && (
        <div className="pt-2 border-t border-gray-700">
          <span className="text-gray-400">
            {node.children.length} children • {node.leaf_count} total leaves
          </span>
        </div>
      )}
    </div>
  );
}

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export function TradingMatrixTree({
  data,
  loading,
  error,
  onNodeSelect,
  selectedNodeId,
}: TradingMatrixTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(() => {
    // Start with top-level categories expanded
    const initial = new Set<string>();
    if (data?.children) {
      data.children.forEach((child) => {
        initial.add(nodeIdToKey(child.id));
      });
    }
    return initial;
  });

  const [localSelectedNode, setLocalSelectedNode] =
    useState<TradingMatrixNode | null>(null);

  const handleToggle = useCallback((nodeKey: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(nodeKey)) {
        next.delete(nodeKey);
      } else {
        next.add(nodeKey);
      }
      return next;
    });
  }, []);

  const handleSelect = useCallback(
    (node: TradingMatrixNode) => {
      setLocalSelectedNode(node);
      onNodeSelect?.(node);
    },
    [onNodeSelect],
  );

  // Loading state
  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="w-8 h-8 animate-spin text-blue-500" />
        <span className="ml-2 text-gray-400">Loading trading matrix...</span>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="flex items-center justify-center h-full text-red-400">
        <p>{error}</p>
      </div>
    );
  }

  // No data state
  if (!data || !data.children.length) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500">
        <p>No trading matrix data available</p>
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Tree panel */}
      <div className="w-1/2 border-r border-gray-700 overflow-auto">
        {/* Header */}
        <div className="sticky top-0 bg-gray-900 border-b border-gray-700 p-3">
          <h2 className="text-lg font-semibold text-white">{data.cbu_name}</h2>
          <p className="text-sm text-gray-400">
            Trading Matrix • {data.total_leaf_count} items
          </p>
        </div>

        {/* Tree */}
        <div className="p-2">
          {data.children.map((child, idx) => (
            <TreeNode
              key={nodeIdToKey(child.id) || idx}
              node={child}
              depth={0}
              expanded={expanded}
              onToggle={handleToggle}
              onSelect={handleSelect}
              selectedNodeId={selectedNodeId || localSelectedNode?.id}
            />
          ))}
        </div>
      </div>

      {/* Detail panel */}
      <div className="w-1/2 overflow-auto bg-gray-850">
        <NodeDetailPanel node={localSelectedNode} />
      </div>
    </div>
  );
}

export default TradingMatrixTree;
