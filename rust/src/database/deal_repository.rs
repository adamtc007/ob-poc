//! Deal Repository - Database access layer for deal taxonomy
//!
//! Provides all database queries for the Deal Taxonomy Builder.
//! Follows the repository pattern used by other domain modules.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::deal_types::{
    DealContractSummary, DealFilters, DealParticipantSummary, DealProductSummary, DealSummary,
    OnboardingRequestSummary, RateCardLineSummary, RateCardSummary,
};

/// Repository for deal-related database operations
pub struct DealRepository;

impl DealRepository {
    // ========================================================================
    // Deal Summary Queries
    // ========================================================================

    /// Get a deal summary by ID with computed counts
    pub async fn get_deal_summary(pool: &PgPool, deal_id: Uuid) -> Result<Option<DealSummary>> {
        let row = sqlx::query_as::<_, DealSummaryRow>(
            r#"
            SELECT
                d.deal_id,
                d.deal_name,
                d.deal_reference,
                d.deal_status,
                d.primary_client_group_id,
                cg.canonical_name as client_group_name,
                d.sales_owner,
                d.sales_team,
                d.estimated_revenue,
                d.currency_code,
                d.opened_at,
                d.qualified_at,
                d.contracted_at,
                d.active_at,
                d.closed_at,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_products WHERE deal_id = d.deal_id), 0)::int as product_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = d.deal_id), 0)::int as rate_card_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_participants WHERE deal_id = d.deal_id), 0)::int as participant_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_contracts WHERE deal_id = d.deal_id), 0)::int as contract_count,
                0::int as onboarding_request_count
            FROM "ob-poc".deals d
            LEFT JOIN "ob-poc".client_group cg ON cg.id = d.primary_client_group_id
            WHERE d.deal_id = $1
            "#,
        )
        .bind(deal_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch deal summary")?;

        Ok(row.map(Into::into))
    }

    /// Search for deals by name (fuzzy match)
    pub async fn search_deals_by_name(
        pool: &PgPool,
        name_query: &str,
        limit: i32,
    ) -> Result<Vec<DealSummary>> {
        let rows = sqlx::query_as::<_, DealSummaryRow>(
            r#"
            SELECT
                d.deal_id,
                d.deal_name,
                d.deal_reference,
                d.deal_status,
                d.primary_client_group_id,
                cg.canonical_name as client_group_name,
                d.sales_owner,
                d.sales_team,
                d.estimated_revenue,
                d.currency_code,
                d.opened_at,
                d.qualified_at,
                d.contracted_at,
                d.active_at,
                d.closed_at,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_products WHERE deal_id = d.deal_id), 0)::int as product_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = d.deal_id), 0)::int as rate_card_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_participants WHERE deal_id = d.deal_id), 0)::int as participant_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_contracts WHERE deal_id = d.deal_id), 0)::int as contract_count,
                0::int as onboarding_request_count
            FROM "ob-poc".deals d
            LEFT JOIN "ob-poc".client_group cg ON cg.id = d.primary_client_group_id
            WHERE d.deal_name ILIKE '%' || $1 || '%'
               OR d.deal_reference ILIKE '%' || $1 || '%'
            ORDER BY
                CASE WHEN d.deal_name ILIKE $1 THEN 0
                     WHEN d.deal_name ILIKE $1 || '%' THEN 1
                     ELSE 2 END,
                d.deal_name
            LIMIT $2
            "#,
        )
        .bind(name_query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("Failed to search deals by name")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// List deals with filters
    pub async fn list_deals(pool: &PgPool, filters: &DealFilters) -> Result<Vec<DealSummary>> {
        let limit = filters.limit.unwrap_or(50);
        let offset = filters.offset.unwrap_or(0);

        let rows = sqlx::query_as::<_, DealSummaryRow>(
            r#"
            SELECT
                d.deal_id,
                d.deal_name,
                d.deal_reference,
                d.deal_status,
                d.primary_client_group_id,
                cg.canonical_name as client_group_name,
                d.sales_owner,
                d.sales_team,
                d.estimated_revenue,
                d.currency_code,
                d.opened_at,
                d.qualified_at,
                d.contracted_at,
                d.active_at,
                d.closed_at,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_products WHERE deal_id = d.deal_id), 0)::int as product_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = d.deal_id), 0)::int as rate_card_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_participants WHERE deal_id = d.deal_id), 0)::int as participant_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_contracts WHERE deal_id = d.deal_id), 0)::int as contract_count,
                0::int as onboarding_request_count
            FROM "ob-poc".deals d
            LEFT JOIN "ob-poc".client_group cg ON cg.id = d.primary_client_group_id
            WHERE ($1::uuid IS NULL OR d.primary_client_group_id = $1)
              AND ($2::text IS NULL OR d.deal_status = $2)
              AND ($3::text IS NULL OR d.sales_owner = $3)
              AND ($4::text IS NULL OR d.sales_team = $4)
              AND ($5::timestamptz IS NULL OR d.opened_at >= $5)
              AND ($6::timestamptz IS NULL OR d.opened_at <= $6)
            ORDER BY d.opened_at DESC
            LIMIT $7 OFFSET $8
            "#,
        )
        .bind(filters.client_group_id)
        .bind(&filters.deal_status)
        .bind(&filters.sales_owner)
        .bind(&filters.sales_team)
        .bind(filters.opened_after)
        .bind(filters.opened_before)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list deals")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Count deals matching filters
    pub async fn count_deals(pool: &PgPool, filters: &DealFilters) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".deals d
            WHERE ($1::uuid IS NULL OR d.primary_client_group_id = $1)
              AND ($2::text IS NULL OR d.deal_status = $2)
              AND ($3::text IS NULL OR d.sales_owner = $3)
              AND ($4::text IS NULL OR d.sales_team = $4)
              AND ($5::timestamptz IS NULL OR d.opened_at >= $5)
              AND ($6::timestamptz IS NULL OR d.opened_at <= $6)
            "#,
        )
        .bind(filters.client_group_id)
        .bind(&filters.deal_status)
        .bind(&filters.sales_owner)
        .bind(&filters.sales_team)
        .bind(filters.opened_after)
        .bind(filters.opened_before)
        .fetch_one(pool)
        .await
        .context("Failed to count deals")?;

        Ok(count.0)
    }

    /// Get deals for a client group (convenience method for session context)
    /// Returns active deals (excludes CANCELLED, OFFBOARDED)
    pub async fn get_deals_for_client_group(
        pool: &PgPool,
        client_group_id: Uuid,
    ) -> Result<Vec<DealSummary>> {
        let rows = sqlx::query_as::<_, DealSummaryRow>(
            r#"
            SELECT
                d.deal_id,
                d.deal_name,
                d.deal_reference,
                d.deal_status,
                d.primary_client_group_id,
                cg.canonical_name as client_group_name,
                d.sales_owner,
                d.sales_team,
                d.estimated_revenue,
                d.currency_code,
                d.opened_at,
                d.qualified_at,
                d.contracted_at,
                d.active_at,
                d.closed_at,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_products WHERE deal_id = d.deal_id), 0)::int as product_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = d.deal_id), 0)::int as rate_card_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_participants WHERE deal_id = d.deal_id), 0)::int as participant_count,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_contracts WHERE deal_id = d.deal_id), 0)::int as contract_count,
                0::int as onboarding_request_count
            FROM "ob-poc".deals d
            LEFT JOIN "ob-poc".client_group cg ON cg.id = d.primary_client_group_id
            WHERE d.primary_client_group_id = $1
              AND d.deal_status NOT IN ('CANCELLED', 'OFFBOARDED')
            ORDER BY d.opened_at DESC
            LIMIT 20
            "#,
        )
        .bind(client_group_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch deals for client group")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ========================================================================
    // Deal Products Queries
    // ========================================================================

    /// Get products for a deal
    pub async fn get_deal_products(
        pool: &PgPool,
        deal_id: Uuid,
    ) -> Result<Vec<DealProductSummary>> {
        let rows = sqlx::query_as::<_, DealProductRow>(
            r#"
            SELECT
                dp.deal_product_id,
                dp.deal_id,
                dp.product_id,
                p.name as product_name,
                p.product_code,
                p.product_category,
                dp.product_status,
                dp.indicative_revenue,
                dp.currency_code,
                dp.notes,
                dp.added_at,
                dp.agreed_at,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = dp.deal_id AND product_id = dp.product_id), 0)::int as rate_card_count
            FROM "ob-poc".deal_products dp
            JOIN "ob-poc".products p ON p.product_id = dp.product_id
            WHERE dp.deal_id = $1
            ORDER BY dp.added_at DESC
            "#,
        )
        .bind(deal_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch deal products")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ========================================================================
    // Rate Card Queries
    // ========================================================================

    /// Get rate cards for a deal
    pub async fn get_deal_rate_cards(pool: &PgPool, deal_id: Uuid) -> Result<Vec<RateCardSummary>> {
        let rows = sqlx::query_as::<_, RateCardRow>(
            r#"
            SELECT
                rc.rate_card_id,
                rc.deal_id,
                rc.contract_id,
                rc.product_id,
                p.name as product_name,
                rc.rate_card_name,
                rc.effective_from,
                rc.effective_to,
                rc.status,
                rc.negotiation_round,
                rc.superseded_by,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_card_lines WHERE rate_card_id = rc.rate_card_id), 0)::int as line_count,
                (rc.superseded_by IS NULL
                 AND rc.effective_from <= CURRENT_DATE
                 AND (rc.effective_to IS NULL OR rc.effective_to >= CURRENT_DATE)) as is_active
            FROM "ob-poc".deal_rate_cards rc
            LEFT JOIN "ob-poc".products p ON p.product_id = rc.product_id
            WHERE rc.deal_id = $1
            ORDER BY rc.effective_from DESC, rc.negotiation_round DESC NULLS LAST
            "#,
        )
        .bind(deal_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch deal rate cards")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get rate cards for a specific product
    pub async fn get_product_rate_cards(
        pool: &PgPool,
        deal_id: Uuid,
        product_id: Uuid,
    ) -> Result<Vec<RateCardSummary>> {
        let rows = sqlx::query_as::<_, RateCardRow>(
            r#"
            SELECT
                rc.rate_card_id,
                rc.deal_id,
                rc.contract_id,
                rc.product_id,
                p.name as product_name,
                rc.rate_card_name,
                rc.effective_from,
                rc.effective_to,
                rc.status,
                rc.negotiation_round,
                rc.superseded_by,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_card_lines WHERE rate_card_id = rc.rate_card_id), 0)::int as line_count,
                (rc.superseded_by IS NULL
                 AND rc.effective_from <= CURRENT_DATE
                 AND (rc.effective_to IS NULL OR rc.effective_to >= CURRENT_DATE)) as is_active
            FROM "ob-poc".deal_rate_cards rc
            LEFT JOIN "ob-poc".products p ON p.product_id = rc.product_id
            WHERE rc.deal_id = $1 AND rc.product_id = $2
            ORDER BY rc.effective_from DESC, rc.negotiation_round DESC NULLS LAST
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch product rate cards")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get rate card lines for a rate card
    pub async fn get_rate_card_lines(
        pool: &PgPool,
        rate_card_id: Uuid,
    ) -> Result<Vec<RateCardLineSummary>> {
        let rows = sqlx::query_as::<_, RateCardLineRow>(
            r#"
            SELECT
                line_id,
                rate_card_id,
                fee_type,
                fee_subtype,
                pricing_model,
                rate_value,
                minimum_fee,
                maximum_fee,
                currency_code,
                tier_brackets,
                fee_basis,
                description,
                sequence_order
            FROM "ob-poc".deal_rate_card_lines
            WHERE rate_card_id = $1
            ORDER BY sequence_order NULLS LAST, fee_type, fee_subtype
            "#,
        )
        .bind(rate_card_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch rate card lines")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get rate card supersession history (follow superseded_by chain)
    pub async fn get_rate_card_history(
        pool: &PgPool,
        rate_card_id: Uuid,
    ) -> Result<Vec<RateCardSummary>> {
        // Use recursive CTE to follow the supersession chain
        let rows = sqlx::query_as::<_, RateCardRow>(
            r#"
            WITH RECURSIVE history AS (
                -- Start with the current rate card
                SELECT
                    rc.rate_card_id,
                    rc.deal_id,
                    rc.contract_id,
                    rc.product_id,
                    rc.rate_card_name,
                    rc.effective_from,
                    rc.effective_to,
                    rc.status,
                    rc.negotiation_round,
                    rc.superseded_by,
                    0 as depth
                FROM "ob-poc".deal_rate_cards rc
                WHERE rc.rate_card_id = $1

                UNION ALL

                -- Find rate cards that were superseded by cards in the history
                SELECT
                    rc.rate_card_id,
                    rc.deal_id,
                    rc.contract_id,
                    rc.product_id,
                    rc.rate_card_name,
                    rc.effective_from,
                    rc.effective_to,
                    rc.status,
                    rc.negotiation_round,
                    rc.superseded_by,
                    h.depth + 1
                FROM "ob-poc".deal_rate_cards rc
                JOIN history h ON rc.superseded_by = h.rate_card_id
                WHERE h.depth < 10
            )
            SELECT
                h.rate_card_id,
                h.deal_id,
                h.contract_id,
                h.product_id,
                p.name as product_name,
                h.rate_card_name,
                h.effective_from,
                h.effective_to,
                h.status,
                h.negotiation_round,
                h.superseded_by,
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_card_lines WHERE rate_card_id = h.rate_card_id), 0)::int as line_count,
                (h.superseded_by IS NULL
                 AND h.effective_from <= CURRENT_DATE
                 AND (h.effective_to IS NULL OR h.effective_to >= CURRENT_DATE)) as is_active
            FROM history h
            LEFT JOIN "ob-poc".products p ON p.product_id = h.product_id
            ORDER BY h.depth ASC
            "#,
        )
        .bind(rate_card_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch rate card history")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ========================================================================
    // Participant Queries
    // ========================================================================

    /// Get participants for a deal
    pub async fn get_deal_participants(
        pool: &PgPool,
        deal_id: Uuid,
    ) -> Result<Vec<DealParticipantSummary>> {
        let rows = sqlx::query_as::<_, DealParticipantRow>(
            r#"
            SELECT
                dp.deal_participant_id,
                dp.deal_id,
                dp.entity_id,
                e.name as entity_name,
                dp.participant_role,
                dp.lei,
                COALESCE(dp.is_primary, false) as is_primary,
                dp.created_at
            FROM "ob-poc".deal_participants dp
            JOIN "ob-poc".entities e ON e.entity_id = dp.entity_id
            WHERE dp.deal_id = $1
            ORDER BY dp.is_primary DESC NULLS LAST, dp.participant_role, e.name
            "#,
        )
        .bind(deal_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch deal participants")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ========================================================================
    // Contract Queries
    // ========================================================================

    /// Get contracts linked to a deal
    pub async fn get_deal_contracts(
        pool: &PgPool,
        deal_id: Uuid,
    ) -> Result<Vec<DealContractSummary>> {
        let rows = sqlx::query_as::<_, DealContractRow>(
            r#"
            SELECT
                lc.contract_id,
                dc.deal_id,
                dc.contract_role,
                dc.sequence_order,
                lc.client_label,
                lc.contract_reference,
                lc.effective_date,
                lc.termination_date,
                lc.status
            FROM "ob-poc".deal_contracts dc
            JOIN "ob-poc".legal_contracts lc ON lc.contract_id = dc.contract_id
            WHERE dc.deal_id = $1
            ORDER BY dc.sequence_order, lc.effective_date DESC
            "#,
        )
        .bind(deal_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch deal contracts")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ========================================================================
    // Onboarding Request Queries
    // ========================================================================

    /// Get onboarding requests linked to a deal (via CBUs tied to deal products)
    pub async fn get_deal_onboarding_requests(
        pool: &PgPool,
        deal_id: Uuid,
    ) -> Result<Vec<OnboardingRequestSummary>> {
        let rows = sqlx::query_as::<_, OnboardingRequestRow>(
            r#"
            SELECT DISTINCT
                orq.request_id,
                orq.cbu_id,
                cbu.name as cbu_name,
                orq.request_state,
                orq.current_phase,
                orq.created_by,
                orq.created_at,
                orq.completed_at,
                dp.deal_product_id
            FROM "ob-poc".onboarding_requests orq
            JOIN "ob-poc".cbus cbu ON cbu.cbu_id = orq.cbu_id
            LEFT JOIN "ob-poc".deal_products dp ON dp.deal_id = $1
            WHERE EXISTS (
                -- CBU is linked to the deal via participants or products
                SELECT 1 FROM "ob-poc".deal_participants part
                WHERE part.deal_id = $1
                AND EXISTS (
                    SELECT 1 FROM "ob-poc".cbu_entity_roles cer
                    WHERE cer.cbu_id = orq.cbu_id
                    AND cer.entity_id = part.entity_id
                )
            )
            ORDER BY orq.created_at DESC
            "#,
        )
        .bind(deal_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch deal onboarding requests")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Internal Row Types (for sqlx mapping)
// ============================================================================

#[derive(Debug, sqlx::FromRow)]
struct DealSummaryRow {
    deal_id: Uuid,
    deal_name: String,
    deal_reference: Option<String>,
    deal_status: String,
    primary_client_group_id: Uuid,
    client_group_name: Option<String>,
    sales_owner: Option<String>,
    sales_team: Option<String>,
    estimated_revenue: Option<Decimal>,
    currency_code: Option<String>,
    opened_at: DateTime<Utc>,
    qualified_at: Option<DateTime<Utc>>,
    contracted_at: Option<DateTime<Utc>>,
    active_at: Option<DateTime<Utc>>,
    closed_at: Option<DateTime<Utc>>,
    product_count: i32,
    rate_card_count: i32,
    participant_count: i32,
    contract_count: i32,
    onboarding_request_count: i32,
}

impl From<DealSummaryRow> for DealSummary {
    fn from(row: DealSummaryRow) -> Self {
        Self {
            deal_id: row.deal_id,
            deal_name: row.deal_name,
            deal_reference: row.deal_reference,
            deal_status: row.deal_status,
            primary_client_group_id: row.primary_client_group_id,
            client_group_name: row.client_group_name,
            sales_owner: row.sales_owner,
            sales_team: row.sales_team,
            estimated_revenue: row.estimated_revenue,
            currency_code: row.currency_code,
            opened_at: row.opened_at,
            qualified_at: row.qualified_at,
            contracted_at: row.contracted_at,
            active_at: row.active_at,
            closed_at: row.closed_at,
            product_count: row.product_count,
            rate_card_count: row.rate_card_count,
            participant_count: row.participant_count,
            contract_count: row.contract_count,
            onboarding_request_count: row.onboarding_request_count,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DealProductRow {
    deal_product_id: Uuid,
    deal_id: Uuid,
    product_id: Uuid,
    product_name: String,
    product_code: Option<String>,
    product_category: Option<String>,
    product_status: String,
    indicative_revenue: Option<Decimal>,
    currency_code: Option<String>,
    notes: Option<String>,
    added_at: Option<DateTime<Utc>>,
    agreed_at: Option<DateTime<Utc>>,
    rate_card_count: i32,
}

impl From<DealProductRow> for DealProductSummary {
    fn from(row: DealProductRow) -> Self {
        Self {
            deal_product_id: row.deal_product_id,
            deal_id: row.deal_id,
            product_id: row.product_id,
            product_name: row.product_name,
            product_code: row.product_code,
            product_category: row.product_category,
            product_status: row.product_status,
            indicative_revenue: row.indicative_revenue,
            currency_code: row.currency_code,
            notes: row.notes,
            added_at: row.added_at,
            agreed_at: row.agreed_at,
            rate_card_count: row.rate_card_count,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct RateCardRow {
    rate_card_id: Uuid,
    deal_id: Uuid,
    contract_id: Uuid,
    product_id: Uuid,
    product_name: Option<String>,
    rate_card_name: Option<String>,
    effective_from: NaiveDate,
    effective_to: Option<NaiveDate>,
    status: Option<String>,
    negotiation_round: Option<i32>,
    superseded_by: Option<Uuid>,
    line_count: i32,
    is_active: bool,
}

impl From<RateCardRow> for RateCardSummary {
    fn from(row: RateCardRow) -> Self {
        Self {
            rate_card_id: row.rate_card_id,
            deal_id: row.deal_id,
            contract_id: row.contract_id,
            product_id: row.product_id,
            product_name: row.product_name,
            rate_card_name: row.rate_card_name,
            effective_from: row.effective_from,
            effective_to: row.effective_to,
            status: row.status,
            negotiation_round: row.negotiation_round,
            superseded_by: row.superseded_by,
            line_count: row.line_count,
            is_active: row.is_active,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct RateCardLineRow {
    line_id: Uuid,
    rate_card_id: Uuid,
    fee_type: String,
    fee_subtype: String,
    pricing_model: String,
    rate_value: Option<Decimal>,
    minimum_fee: Option<Decimal>,
    maximum_fee: Option<Decimal>,
    currency_code: Option<String>,
    tier_brackets: Option<serde_json::Value>,
    fee_basis: Option<String>,
    description: Option<String>,
    sequence_order: Option<i32>,
}

impl From<RateCardLineRow> for RateCardLineSummary {
    fn from(row: RateCardLineRow) -> Self {
        Self {
            line_id: row.line_id,
            rate_card_id: row.rate_card_id,
            fee_type: row.fee_type,
            fee_subtype: row.fee_subtype,
            pricing_model: row.pricing_model,
            rate_value: row.rate_value,
            minimum_fee: row.minimum_fee,
            maximum_fee: row.maximum_fee,
            currency_code: row.currency_code,
            tier_brackets: row.tier_brackets,
            fee_basis: row.fee_basis,
            description: row.description,
            sequence_order: row.sequence_order,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DealParticipantRow {
    deal_participant_id: Uuid,
    deal_id: Uuid,
    entity_id: Uuid,
    entity_name: String,
    participant_role: String,
    lei: Option<String>,
    is_primary: bool,
    created_at: Option<DateTime<Utc>>,
}

impl From<DealParticipantRow> for DealParticipantSummary {
    fn from(row: DealParticipantRow) -> Self {
        Self {
            deal_participant_id: row.deal_participant_id,
            deal_id: row.deal_id,
            entity_id: row.entity_id,
            entity_name: row.entity_name,
            participant_role: row.participant_role,
            lei: row.lei,
            is_primary: row.is_primary,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DealContractRow {
    contract_id: Uuid,
    deal_id: Uuid,
    contract_role: Option<String>,
    sequence_order: i32,
    client_label: Option<String>,
    contract_reference: Option<String>,
    effective_date: Option<NaiveDate>,
    termination_date: Option<NaiveDate>,
    status: Option<String>,
}

impl From<DealContractRow> for DealContractSummary {
    fn from(row: DealContractRow) -> Self {
        Self {
            contract_id: row.contract_id,
            deal_id: row.deal_id,
            contract_role: row.contract_role,
            sequence_order: row.sequence_order,
            client_label: row.client_label,
            contract_reference: row.contract_reference,
            effective_date: row.effective_date,
            termination_date: row.termination_date,
            status: row.status,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct OnboardingRequestRow {
    request_id: Uuid,
    cbu_id: Uuid,
    cbu_name: Option<String>,
    request_state: String,
    current_phase: Option<String>,
    created_by: Option<String>,
    created_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    deal_product_id: Option<Uuid>,
}

impl From<OnboardingRequestRow> for OnboardingRequestSummary {
    fn from(row: OnboardingRequestRow) -> Self {
        Self {
            request_id: row.request_id,
            cbu_id: row.cbu_id,
            cbu_name: row.cbu_name,
            request_state: row.request_state,
            current_phase: row.current_phase,
            created_by: row.created_by,
            created_at: row.created_at,
            completed_at: row.completed_at,
            deal_product_id: row.deal_product_id,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_deal_view_mode_display() {
        use crate::api::deal_types::DealViewMode;
        assert_eq!(DealViewMode::Commercial.to_string(), "COMMERCIAL");
        assert_eq!(DealViewMode::Financial.to_string(), "FINANCIAL");
        assert_eq!(DealViewMode::Status.to_string(), "STATUS");
    }

    #[test]
    fn test_deal_view_mode_parse() {
        use crate::api::deal_types::DealViewMode;
        use std::str::FromStr;
        assert_eq!(
            DealViewMode::from_str("commercial").unwrap(),
            DealViewMode::Commercial
        );
        assert_eq!(
            DealViewMode::from_str("FINANCIAL").unwrap(),
            DealViewMode::Financial
        );
        assert!(DealViewMode::from_str("invalid").is_err());
    }
}
