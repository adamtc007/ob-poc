//! Context Discovery Service
//!
//! Queries linked contexts for a CBU to surface to agent and UI.
//! This enables the agent to understand "where we are" and generate
//! correctly scoped DSL.

use sqlx::PgPool;
use uuid::Uuid;

// ============================================================================
// Types (internal row types for SQLx queries)
// ============================================================================

/// CBU context row from database
#[derive(Debug, Clone)]
pub struct CbuContextRow {
    pub id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub cbu_category: Option<String>,
    pub entity_count: i64,
    pub role_count: i64,
}

/// Linked context row from database
#[derive(Debug, Clone)]
pub struct LinkedContextRow {
    pub id: Uuid,
    pub context_type: String,
    pub label: String,
    pub status: Option<String>,
    pub created_at: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

// ============================================================================
// Service
// ============================================================================

/// Context discovery service for surfacing linked contexts
pub struct ContextDiscoveryService {
    pool: PgPool,
}

impl ContextDiscoveryService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Discover all linked contexts for a CBU
    ///
    /// Returns the CBU context along with linked entities like:
    /// - KYC cases
    /// - Trading profiles
    /// - ISDA agreements
    /// - Product subscriptions
    pub async fn discover_for_cbu(&self, cbu_id: Uuid) -> Result<DiscoveredContext, sqlx::Error> {
        // Get CBU details with counts
        let cbu = self.get_cbu_context(cbu_id).await?;

        let Some(cbu) = cbu else {
            return Ok(DiscoveredContext::empty());
        };

        // Get linked contexts in parallel-ish (sequential for now, could be tokio::join!)
        let kyc_cases = self.get_kyc_cases(cbu_id).await.unwrap_or_default();
        let trading_profile = self.get_trading_profile(cbu_id).await.ok().flatten();
        let isda_agreements = self.get_isda_agreements(cbu_id).await.unwrap_or_default();
        let product_subscriptions = self
            .get_product_subscriptions(cbu_id)
            .await
            .unwrap_or_default();

        Ok(DiscoveredContext {
            cbu: Some(cbu),
            kyc_cases,
            trading_profile,
            isda_agreements,
            product_subscriptions,
        })
    }

    /// Get CBU context with entity/role counts
    async fn get_cbu_context(&self, cbu_id: Uuid) -> Result<Option<CbuContextRow>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                c.cbu_id,
                c.name,
                c.jurisdiction,
                c.client_type,
                c.cbu_category,
                (SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles cer WHERE cer.cbu_id = c.cbu_id) as "entity_count!",
                (SELECT COUNT(DISTINCT role_id) FROM "ob-poc".cbu_entity_roles cer WHERE cer.cbu_id = c.cbu_id) as "role_count!"
            FROM "ob-poc".cbus c
            WHERE c.cbu_id = $1
            "#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| CbuContextRow {
            id: r.cbu_id,
            name: r.name,
            jurisdiction: r.jurisdiction,
            client_type: r.client_type,
            cbu_category: r.cbu_category,
            entity_count: r.entity_count,
            role_count: r.role_count,
        }))
    }

    /// Get active KYC cases for CBU
    async fn get_kyc_cases(&self, cbu_id: Uuid) -> Result<Vec<LinkedContextRow>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                case_id as id,
                status,
                case_type,
                risk_rating,
                opened_at
            FROM kyc.cases
            WHERE cbu_id = $1
              AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN')
            ORDER BY opened_at DESC
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| LinkedContextRow {
                id: r.id,
                context_type: "kyc_case".to_string(),
                label: format!("{} Case", r.case_type.as_deref().unwrap_or("KYC")),
                status: Some(r.status),
                created_at: Some(r.opened_at.to_rfc3339()),
                metadata: Some(serde_json::json!({
                    "case_type": r.case_type,
                    "risk_rating": r.risk_rating
                })),
            })
            .collect())
    }

    /// Get active trading profile for CBU
    async fn get_trading_profile(
        &self,
        cbu_id: Uuid,
    ) -> Result<Option<LinkedContextRow>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                profile_id as id,
                version,
                status,
                created_at
            FROM "ob-poc".cbu_trading_profiles
            WHERE cbu_id = $1
              AND status = 'ACTIVE'
            ORDER BY version DESC
            LIMIT 1
            "#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| LinkedContextRow {
            id: r.id,
            context_type: "trading_profile".to_string(),
            label: format!("Trading Profile v{}", r.version),
            status: Some(r.status),
            created_at: Some(r.created_at.to_rfc3339()),
            metadata: Some(serde_json::json!({
                "version": r.version
            })),
        }))
    }

    /// Get active ISDA agreements for CBU
    async fn get_isda_agreements(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<LinkedContextRow>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                ia.isda_id as id,
                ia.governing_law,
                ia.agreement_date,
                ia.is_active,
                e.name as "counterparty_name?"
            FROM custody.isda_agreements ia
            LEFT JOIN "ob-poc".entities e ON e.entity_id = ia.counterparty_entity_id
            WHERE ia.cbu_id = $1
              AND ia.is_active = true
            ORDER BY ia.agreement_date DESC
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| LinkedContextRow {
                id: r.id,
                context_type: "isda_agreement".to_string(),
                label: r
                    .counterparty_name
                    .unwrap_or_else(|| "ISDA Agreement".to_string()),
                status: if r.is_active.unwrap_or(false) {
                    Some("ACTIVE".to_string())
                } else {
                    Some("INACTIVE".to_string())
                },
                created_at: Some(r.agreement_date.to_string()),
                metadata: Some(serde_json::json!({
                    "governing_law": r.governing_law
                })),
            })
            .collect())
    }

    /// Get active product subscriptions for CBU
    async fn get_product_subscriptions(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<LinkedContextRow>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                cps.subscription_id as id,
                p.product_id,
                p.name as product_name,
                p.product_code,
                cps.status,
                cps.created_at
            FROM "ob-poc".cbu_product_subscriptions cps
            JOIN "ob-poc".products p ON p.product_id = cps.product_id
            WHERE cps.cbu_id = $1
              AND cps.status = 'ACTIVE'
            ORDER BY p.name
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| LinkedContextRow {
                id: r.product_id,
                context_type: "product".to_string(),
                label: r.product_name,
                status: Some(r.status),
                created_at: r.created_at.map(|d| d.to_rfc3339()),
                metadata: Some(serde_json::json!({
                    "product_code": r.product_code,
                    "subscription_id": r.id
                })),
            })
            .collect())
    }
}

// ============================================================================
// Discovered Context (intermediate type)
// ============================================================================

/// Intermediate type holding discovered context data
/// This is converted to ob_poc_types::SessionContext at the API layer
#[derive(Debug, Clone, Default)]
pub struct DiscoveredContext {
    pub cbu: Option<CbuContextRow>,
    pub kyc_cases: Vec<LinkedContextRow>,
    pub trading_profile: Option<LinkedContextRow>,
    pub isda_agreements: Vec<LinkedContextRow>,
    pub product_subscriptions: Vec<LinkedContextRow>,
}

impl DiscoveredContext {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Check if any context was discovered
    pub fn has_context(&self) -> bool {
        self.cbu.is_some()
    }
}

// ============================================================================
// Conversion to API types
// ============================================================================

impl From<CbuContextRow> for ob_poc_types::CbuContext {
    fn from(row: CbuContextRow) -> Self {
        Self {
            id: row.id.to_string(),
            name: row.name,
            jurisdiction: row.jurisdiction,
            client_type: row.client_type,
            cbu_category: row.cbu_category,
            entity_count: row.entity_count as i32,
            role_count: row.role_count as i32,
            kyc_status: None,
            risk_rating: None,
        }
    }
}

impl From<LinkedContextRow> for ob_poc_types::LinkedContext {
    fn from(row: LinkedContextRow) -> Self {
        Self {
            id: row.id.to_string(),
            context_type: row.context_type,
            label: row.label,
            status: row.status,
            created_at: row.created_at,
            metadata: row.metadata,
        }
    }
}

impl From<DiscoveredContext> for ob_poc_types::SessionContext {
    fn from(ctx: DiscoveredContext) -> Self {
        Self {
            cbu: ctx.cbu.map(|c| c.into()),
            onboarding_request: None, // TODO: Add onboarding discovery if table exists
            kyc_cases: ctx.kyc_cases.into_iter().map(|c| c.into()).collect(),
            trading_matrix: ctx.trading_profile.map(|c| c.into()),
            isda_agreements: ctx.isda_agreements.into_iter().map(|c| c.into()).collect(),
            product_subscriptions: ctx
                .product_subscriptions
                .into_iter()
                .map(|c| c.into())
                .collect(),
            active_scope: None,
            symbols: std::collections::HashMap::new(),
            semantic_state: None, // Derived separately after context discovery
            stage_focus: None,    // Set from session context, not discovery
            viewport_state: None, // Set from session context, not discovery
            agent_state: None,    // Set from session context, not discovery
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_context_empty() {
        let ctx = DiscoveredContext::empty();
        assert!(!ctx.has_context());
    }

    #[test]
    fn test_cbu_context_conversion() {
        let row = CbuContextRow {
            id: Uuid::now_v7(),
            name: "Test Fund".to_string(),
            jurisdiction: Some("LU".to_string()),
            client_type: Some("FUND".to_string()),
            cbu_category: Some("SICAV".to_string()),
            entity_count: 5,
            role_count: 3,
        };

        let api_ctx: ob_poc_types::CbuContext = row.into();
        assert_eq!(api_ctx.name, "Test Fund");
        assert_eq!(api_ctx.jurisdiction, Some("LU".to_string()));
        assert_eq!(api_ctx.cbu_category, Some("SICAV".to_string()));
        assert_eq!(api_ctx.entity_count, 5);
    }

    #[test]
    fn test_linked_context_conversion() {
        let row = LinkedContextRow {
            id: Uuid::now_v7(),
            context_type: "kyc_case".to_string(),
            label: "NEW_CLIENT Case".to_string(),
            status: Some("INTAKE".to_string()),
            created_at: Some("2024-01-15T10:00:00Z".to_string()),
            metadata: Some(serde_json::json!({"case_type": "NEW_CLIENT"})),
        };

        let api_ctx: ob_poc_types::LinkedContext = row.into();
        assert_eq!(api_ctx.context_type, "kyc_case");
        assert_eq!(api_ctx.label, "NEW_CLIENT Case");
        assert_eq!(api_ctx.status, Some("INTAKE".to_string()));
    }
}
