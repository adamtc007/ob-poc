/**
 * Trading Matrix API client
 *
 * Fetches the trading matrix document for a CBU.
 * The trading matrix is a hierarchical tree structure containing:
 * - Trading Universe (instruments, markets, counterparties)
 * - Settlement Instructions (SSIs, booking rules)
 * - Settlement Chains
 * - Tax Configuration
 * - ISDA Agreements (CSAs, product coverage)
 * - Investment Managers
 * - Corporate Actions
 */

import { api } from "./client";

// =============================================================================
// TYPES (matching Rust types in ob-poc-types/src/trading_matrix.rs)
// =============================================================================

/** Path-based node identifier */
export type TradingMatrixNodeId = string[];

/** Visual status indicator */
export type StatusColor = "green" | "yellow" | "red" | "gray";

/** Node type discriminator with type-specific metadata */
export type TradingMatrixNodeType =
  | { type: "category"; name: string }
  | { type: "instrument_class"; class_code: string; cfi_prefix?: string; is_otc: boolean }
  | { type: "market"; mic: string; market_name: string; country_code: string }
  | { type: "counterparty"; entity_id: string; entity_name: string; lei?: string }
  | { type: "universe_entry"; universe_id: string; currencies: string[]; settlement_types: string[]; is_held: boolean; is_traded: boolean }
  | { type: "ssi"; ssi_id: string; ssi_name: string; ssi_type: string; status: string; safekeeping_account?: string; safekeeping_bic?: string; cash_account?: string; cash_bic?: string; pset_bic?: string; cash_currency?: string }
  | { type: "booking_rule"; rule_id: string; rule_name: string; priority: number; specificity_score: number; is_active: boolean; match_criteria?: BookingMatchCriteria }
  | { type: "settlement_chain"; chain_id: string; chain_name: string; hop_count: number; is_active: boolean; mic?: string; currency?: string }
  | { type: "settlement_hop"; hop_id: string; sequence: number; intermediary_bic?: string; intermediary_name?: string; role: string }
  | { type: "tax_jurisdiction"; jurisdiction_id: string; jurisdiction_code: string; jurisdiction_name: string; default_withholding_rate?: number; reclaim_available: boolean }
  | { type: "tax_config"; status_id: string; investor_type: string; tax_exempt: boolean; documentation_status?: string; treaty_rate?: number }
  | { type: "isda_agreement"; isda_id: string; counterparty_name: string; governing_law?: string; agreement_date?: string; counterparty_entity_id?: string; counterparty_lei?: string }
  | { type: "csa_agreement"; csa_id: string; csa_type: string; threshold_currency?: string; threshold_amount?: number; minimum_transfer_amount?: number; collateral_ssi_ref?: string }
  | { type: "product_coverage"; coverage_id: string; asset_class: string; base_products: string[] }
  | { type: "investment_manager_mandate"; mandate_id: string; manager_entity_id: string; manager_name: string; manager_lei?: string; priority: number; role: string; can_trade: boolean; can_settle: boolean }
  | { type: "pricing_rule"; rule_id: string; priority: number; source: string; fallback_source?: string; price_type?: string }
  | { type: "corporate_actions_policy"; enabled_count: number; has_custom_elections: boolean; has_cutoff_rules: boolean; elector?: string }
  | { type: "ca_event_type_config"; event_code: string; event_name: string; processing_mode: string; default_option?: string; is_elective: boolean }
  | { type: "ca_cutoff_rule_node"; rule_key: string; days_before: number; warning_days: number; escalation_days: number }
  | { type: "ca_proceeds_mapping_node"; proceeds_type: string; currency?: string; ssi_reference: string };

/** Booking rule match criteria */
export interface BookingMatchCriteria {
  instrument_class?: string;
  security_type?: string;
  mic?: string;
  currency?: string;
  settlement_type?: string;
  counterparty_entity_id?: string;
}

/** A node in the trading matrix tree */
export interface TradingMatrixNode {
  id: TradingMatrixNodeId;
  node_type: TradingMatrixNodeType;
  label: string;
  sublabel?: string;
  children: TradingMatrixNode[];
  status_color?: StatusColor;
  is_loaded: boolean;
  leaf_count: number;
}

/** Document status */
export type DocumentStatus = "DRAFT" | "VALIDATED" | "PENDING_REVIEW" | "ACTIVE" | "SUPERSEDED" | "ARCHIVED";

/** Document metadata */
export interface TradingMatrixMetadata {
  source?: string;
  source_ref?: string;
  modified_by?: string;
  notes?: string;
  regulatory_framework?: string;
}

/** Complete trading matrix document */
export interface TradingMatrixDocument {
  cbu_id: string;
  cbu_name: string;
  version: number;
  status: DocumentStatus;
  children: TradingMatrixNode[];
  total_leaf_count: number;
  metadata: TradingMatrixMetadata;
  created_at?: string;
  updated_at?: string;
}

/** API response for trading matrix */
export interface TradingMatrixResponse {
  cbu_id: string;
  cbu_name: string;
  children: TradingMatrixNode[];
  total_leaf_count: number;
}

// =============================================================================
// API FUNCTIONS
// =============================================================================

/**
 * Fetch the trading matrix document for a CBU
 */
export async function getTradingMatrix(cbuId: string): Promise<TradingMatrixResponse> {
  return api.get<TradingMatrixResponse>(`/cbu/${cbuId}/trading-matrix`);
}

/**
 * Get icon for a node type
 */
export function getNodeTypeIcon(nodeType: TradingMatrixNodeType): string {
  switch (nodeType.type) {
    case "category":
      return "📁";
    case "instrument_class":
      return nodeType.is_otc ? "🔄" : "📊";
    case "market":
      return "🏛️";
    case "counterparty":
      return "🏢";
    case "universe_entry":
      return "📈";
    case "ssi":
      return nodeType.status === "ACTIVE" ? "✅" : "⏳";
    case "booking_rule":
      return nodeType.is_active ? "📋" : "📋";
    case "settlement_chain":
      return "🔗";
    case "settlement_hop":
      return "➡️";
    case "tax_jurisdiction":
      return "🏴";
    case "tax_config":
      return "📄";
    case "isda_agreement":
      return "📝";
    case "csa_agreement":
      return "🛡️";
    case "product_coverage":
      return "📦";
    case "investment_manager_mandate":
      return "👔";
    case "pricing_rule":
      return "💰";
    case "corporate_actions_policy":
      return "📢";
    case "ca_event_type_config":
      return "📅";
    case "ca_cutoff_rule_node":
      return "⏰";
    case "ca_proceeds_mapping_node":
      return "💵";
    default:
      return "📄";
  }
}

/**
 * Get status color CSS class
 */
export function getStatusColorClass(color?: StatusColor): string {
  switch (color) {
    case "green":
      return "text-green-500";
    case "yellow":
      return "text-yellow-500";
    case "red":
      return "text-red-500";
    case "gray":
    default:
      return "text-gray-400";
  }
}

/**
 * Get a human-readable type label
 */
export function getNodeTypeLabel(nodeType: TradingMatrixNodeType): string {
  switch (nodeType.type) {
    case "category":
      return "Category";
    case "instrument_class":
      return nodeType.is_otc ? "OTC Instrument Class" : "Exchange Instrument Class";
    case "market":
      return "Market";
    case "counterparty":
      return "Counterparty";
    case "universe_entry":
      return "Universe Entry";
    case "ssi":
      return "SSI";
    case "booking_rule":
      return "Booking Rule";
    case "settlement_chain":
      return "Settlement Chain";
    case "settlement_hop":
      return "Settlement Hop";
    case "tax_jurisdiction":
      return "Tax Jurisdiction";
    case "tax_config":
      return "Tax Configuration";
    case "isda_agreement":
      return "ISDA Agreement";
    case "csa_agreement":
      return "CSA";
    case "product_coverage":
      return "Product Coverage";
    case "investment_manager_mandate":
      return "IM Mandate";
    case "pricing_rule":
      return "Pricing Rule";
    case "corporate_actions_policy":
      return "CA Policy";
    case "ca_event_type_config":
      return "CA Event Type";
    case "ca_cutoff_rule_node":
      return "CA Cutoff Rule";
    case "ca_proceeds_mapping_node":
      return "CA Proceeds Mapping";
    default:
      return "Node";
  }
}

// Standard category names
export const MATRIX_CATEGORIES = {
  UNIVERSE: "Trading Universe",
  SSI: "Standing Settlement Instructions",
  CHAINS: "Settlement Chains",
  TAX: "Tax Configuration",
  ISDA: "ISDA Agreements",
  PRICING: "Pricing Configuration",
  MANAGERS: "Investment Managers",
  CORPORATE_ACTIONS: "Corporate Actions",
} as const;
