/**
 * Deal Taxonomy Types
 *
 * These types match the DealGraphResponse and related types from the Rust backend.
 * Used for deal visualization with drill-down capabilities.
 */

/** Deal summary information */
export interface DealSummary {
  deal_id: string;
  deal_name: string;
  deal_status: string;
  client_group_id?: string;
  client_group_name?: string;
  product_count: number;
  rate_card_count: number;
  participant_count: number;
  contract_count: number;
  onboarding_request_count: number;
  created_at?: string;
  updated_at?: string;
}

/** Product within a deal */
export interface DealProductSummary {
  deal_product_id: string;
  deal_id: string;
  product_name: string;
  product_code?: string;
  product_category?: string;
  product_status: string;
  rate_card_count: number;
}

/** Rate card summary */
export interface RateCardSummary {
  rate_card_id: string;
  rate_card_name: string;
  deal_product_id: string;
  effective_from: string;
  effective_to?: string;
  status?: string;
  line_count: number;
  superseded_by_id?: string;
}

/** Rate card line item */
export interface RateCardLineSummary {
  line_id: string;
  rate_card_id: string;
  fee_type: string;
  fee_subtype: string;
  pricing_model: string;
  rate_value?: string;
  min_fee?: string;
  max_fee?: string;
  currency?: string;
  tier_from?: number;
  tier_to?: number;
}

/** Participant in a deal */
export interface DealParticipantSummary {
  participant_id: string;
  deal_id: string;
  entity_id: string;
  entity_name: string;
  role: string;
  jurisdiction?: string;
  lei?: string;
}

/** Contract within a deal */
export interface DealContractSummary {
  contract_id: string;
  deal_id: string;
  contract_name: string;
  contract_type: string;
  effective_date?: string;
  termination_date?: string;
  status: string;
}

/** Onboarding request linked to a deal */
export interface OnboardingRequestSummary {
  request_id: string;
  deal_id: string;
  request_type: string;
  status: string;
  cbu_id?: string;
  cbu_name?: string;
  submitted_at?: string;
  completed_at?: string;
}

/** View mode for deal visualization */
export type DealViewMode = "COMMERCIAL" | "FINANCIAL" | "STATUS";

/** Complete deal graph response */
export interface DealGraphResponse {
  deal: DealSummary;
  products: DealProductSummary[];
  rate_cards: RateCardSummary[];
  participants: DealParticipantSummary[];
  contracts: DealContractSummary[];
  onboarding_requests: OnboardingRequestSummary[];
  view_mode: DealViewMode;
}

/** Session deal context */
export interface SessionDealContext {
  deal_id?: string;
  deal_name?: string;
  deal_status?: string;
}

/** Deal search/filter parameters */
export interface DealFilters {
  search?: string;
  status?: string;
  client_group_id?: string;
  limit?: number;
  offset?: number;
}

/** Deal list response */
export interface DealListResponse {
  deals: DealSummary[];
  total: number;
  limit: number;
  offset: number;
}

/** Deal taxonomy tree node for navigation */
export interface DealTaxonomyNode {
  id: string;
  type:
    | "deal"
    | "product_list"
    | "product"
    | "rate_card_list"
    | "rate_card"
    | "line"
    | "participant_list"
    | "participant"
    | "contract_list"
    | "contract"
    | "onboarding_list"
    | "onboarding";
  label: string;
  children?: DealTaxonomyNode[];
  data?: DealSummary | DealProductSummary | RateCardSummary | RateCardLineSummary | DealParticipantSummary | DealContractSummary | OnboardingRequestSummary;
  expanded?: boolean;
  childCount?: number;
}

/** Rate card history entry */
export interface RateCardHistoryEntry extends RateCardSummary {
  superseded_at?: string;
}
