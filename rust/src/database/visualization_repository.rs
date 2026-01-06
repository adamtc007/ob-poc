//! Visualization Repository
//!
//! Provides read-only data access for visualization builders.
//! All SQL is centralized here - visualization layer does not know SQL dialect.
//!
//! This enables database portability (e.g., Postgres → Oracle migration).

use crate::graph::{NodeOffset, NodeSizeOverride};
use anyhow::Result;
use sqlx::types::Json;
use sqlx::PgPool;
use uuid::Uuid;

// =============================================================================
// VIEW MODELS (read-only structs for visualization)
// =============================================================================

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CbuView {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub commercial_client_entity_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct EntityView {
    pub entity_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub entity_type: String,
}

#[derive(Debug, Clone)]
pub struct OfficerView {
    pub entity_id: Uuid,
    pub name: String,
    pub nationality: Option<String>,
    pub roles: Vec<String>,
}

/// Entity with role information for CBU tree building
#[derive(Debug, Clone)]
pub struct EntityWithRoleView {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub role_name: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShareClassView {
    pub id: Uuid,
    pub name: String,
    pub currency: String,
    pub class_category: Option<String>,
    pub isin: Option<String>,
    pub nav_per_share: Option<String>,
    pub fund_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HoldingView {
    pub investor_entity_id: Uuid,
    pub share_class_id: Uuid,
    pub units: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ControlRelationshipView {
    pub controller_entity_id: Uuid,
    pub controlled_entity_id: Uuid,
    pub control_type: String,
}

#[derive(Debug, Clone)]
pub struct ServiceDeliveryView {
    pub delivery_id: Uuid,
    pub product_id: Uuid,
    pub product_name: String,
    pub service_id: Uuid,
    pub service_name: String,
    pub instance_id: Option<Uuid>,
    pub instance_name: Option<String>,
    pub resource_type_name: Option<String>,
    pub delivery_status: Option<String>,
}

/// Product view for graph building (via cbus.product_id)
#[derive(Debug, Clone)]
pub struct ProductView {
    pub product_id: Uuid,
    pub name: String,
    pub product_code: Option<String>,
    pub product_category: Option<String>,
    pub is_active: Option<bool>,
}

/// Service view for graph building (via product_services)
#[derive(Debug, Clone)]
pub struct ServiceView {
    pub service_id: Uuid,
    pub name: String,
    pub service_code: Option<String>,
    pub service_category: Option<String>,
    pub is_mandatory: Option<bool>,
}

/// Service resource type view for graph building (via service_resource_capabilities)
#[derive(Debug, Clone)]
pub struct ServiceResourceTypeView {
    pub resource_id: Uuid,
    pub name: String,
    pub resource_type: Option<String>,
    pub resource_code: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CbuSummaryView {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct EntityAttributeView {
    pub attribute_id: String,
    pub attribute_name: String,
    pub value_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocumentAttributeView {
    pub attribute_id: Uuid,
    pub attribute_name: String,
    pub value: serde_json::Value,
}

// =============================================================================
// TRADING VIEW MODELS
// =============================================================================

/// Trading profile view for graph building
#[derive(Debug, Clone)]
pub struct TradingProfileView {
    pub profile_id: Uuid,
    pub version: i32,
    pub status: String,
    pub activated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Universe entry view for graph building - represents tradeable instrument/market combinations
#[derive(Debug, Clone)]
pub struct UniverseEntryView {
    pub universe_id: Uuid,
    pub instrument_class_id: Uuid,
    pub class_code: String,
    pub class_name: String,
    pub market_id: Option<Uuid>,
    pub mic: Option<String>,
    pub market_name: Option<String>,
    pub counterparty_id: Option<Uuid>,
    pub counterparty_name: Option<String>,
    pub currencies: Vec<String>,
    pub is_otc: bool,
}

/// ISDA agreement view for graph building
#[derive(Debug, Clone)]
pub struct IsdaAgreementView {
    pub isda_id: Uuid,
    pub counterparty_entity_id: Uuid,
    pub counterparty_name: Option<String>,
    pub governing_law: Option<String>,
    pub agreement_date: Option<chrono::NaiveDate>,
}

/// CSA (Credit Support Annex) view for graph building
#[derive(Debug, Clone)]
pub struct CsaAgreementView {
    pub csa_id: Uuid,
    pub csa_type: String,
    pub threshold_amount: Option<f64>,
    pub threshold_currency: Option<String>,
}

/// Investment Manager assignment view for graph building
#[derive(Debug, Clone)]
pub struct InvestmentManagerView {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub can_trade: bool,
    pub can_settle: bool,
    pub scope_mics: Vec<String>,
    pub scope_classes: Vec<String>,
    pub scope_description: String,
}

// =============================================================================
// MCP VIEW MODELS
// =============================================================================

#[derive(Debug, Clone)]
pub struct CbuEntityView {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
}

#[derive(Debug, Clone)]
pub struct CbuRoleView {
    pub entity_id: Uuid,
    pub role_name: String,
}

#[derive(Debug, Clone)]
pub struct CbuDocumentView {
    pub doc_id: Uuid,
    pub document_type_code: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CbuScreeningView {
    pub screening_id: Uuid,
    pub entity_id: Uuid,
    pub screening_type: String,
    pub status: Option<String>,
    pub result: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EntityCbuView {
    pub cbu_id: Uuid,
    pub cbu_name: String,
}

#[derive(Debug, Clone)]
pub struct EntityRoleView {
    pub role_name: String,
    pub cbu_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct EntityScreeningView {
    pub screening_id: Uuid,
    pub screening_type: String,
    pub status: Option<String>,
    pub result: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EntityTypeView {
    pub type_code: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct RoleView {
    pub role_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct DocumentTypeView {
    pub type_code: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct CbuBasicView {
    pub cbu_id: Uuid,
    pub name: String,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub cbu_category: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EntityBasicView {
    pub entity_id: Uuid,
    pub name: String,
    pub type_code: Option<String>,
}

// =============================================================================
// GRAPH VIEW MODELS
// =============================================================================

#[derive(Debug, Clone)]
pub struct GraphEntityView {
    pub cbu_entity_role_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: String,
    pub entity_category: Option<String>,
    pub role_name: String,
    pub jurisdiction: Option<String>,
    pub roles: Vec<String>,
    pub role_categories: Vec<String>,
    pub primary_role: Option<String>,
    pub role_priority: Option<i32>,
    // Role Taxonomy V2 fields
    /// Primary role category from taxonomy (e.g., "OWNERSHIP_CHAIN", "CONTROL_CHAIN")
    pub primary_role_category: Option<String>,
    /// Layout behavior hint from taxonomy (e.g., "PYRAMID_UP", "OVERLAY")
    pub layout_category: Option<String>,
    /// UBO treatment code (e.g., "TERMINUS", "LOOK_THROUGH")
    pub ubo_treatment: Option<String>,
    /// KYC obligation level (e.g., "FULL_KYC", "SIMPLIFIED")
    pub kyc_obligation: Option<String>,
    /// Person state for proper_person entities: GHOST, IDENTIFIED, or VERIFIED
    /// Ghost entities have minimal info (name only) and render with dashed/faded style
    pub person_state: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UniverseView {
    pub universe_id: Uuid,
    pub instrument_class_id: Uuid,
    pub market_id: Option<Uuid>,
    pub currencies: Vec<String>,
    pub settlement_types: Vec<String>,
    pub is_active: Option<bool>,
    pub class_name: Option<String>,
    pub market_name: Option<String>,
    pub mic: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SsiView {
    pub ssi_id: Uuid,
    pub ssi_name: String,
    pub ssi_type: String,
    pub status: Option<String>,
    pub cash_currency: Option<String>,
    pub safekeeping_account: Option<String>,
    pub safekeeping_bic: Option<String>,
    pub cash_account: Option<String>,
    pub cash_account_bic: Option<String>,
    pub market_id: Option<Uuid>,
    pub mic: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BookingRuleView {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub priority: i32,
    pub ssi_id: Uuid,
    pub instrument_class_id: Option<Uuid>,
    pub market_id: Option<Uuid>,
    pub currency: Option<String>,
    pub is_active: Option<bool>,
    pub class_name: Option<String>,
    pub mic: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IsdaView {
    pub isda_id: Uuid,
    pub counterparty_entity_id: Uuid,
    pub governing_law: Option<String>,
    pub agreement_date: chrono::NaiveDate,
    pub is_active: Option<bool>,
    pub counterparty_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CsaView {
    pub csa_id: Uuid,
    pub csa_type: String,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct KycStatusView {
    pub status_id: Uuid,
    pub entity_id: Uuid,
    pub kyc_status: Option<String>,
    pub risk_rating: Option<String>,
    pub next_review_date: Option<chrono::NaiveDate>,
    pub entity_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocumentRequestView {
    pub request_id: Uuid,
    pub document_type: String,
    pub status: Option<String>,
    pub requested_from_entity_id: Option<Uuid>,
    pub entity_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScreeningView {
    pub screening_id: Uuid,
    pub entity_id: Uuid,
    pub screening_type: String,
    pub result: Option<String>,
    pub resolution: Option<String>,
    pub entity_name: Option<String>,
}

/// UBO edge from entity_relationships + cbu_relationship_verification tables
/// This is the new unified ownership/control model with separated structure and verification
#[derive(Debug, Clone)]
pub struct UboEdgeView {
    pub edge_id: Uuid,
    pub cbu_id: Uuid,
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub edge_type: String,
    pub percentage: Option<bigdecimal::BigDecimal>,
    pub control_role: Option<String>,
    pub trust_role: Option<String>,
    pub status: String,
    pub alleged_percentage: Option<bigdecimal::BigDecimal>,
    pub proven_percentage: Option<bigdecimal::BigDecimal>,
    pub from_name: String,
    pub to_name: String,
    pub from_type_code: Option<String>,
    pub to_type_code: Option<String>,
    pub from_category: Option<String>,
    pub to_category: Option<String>,
}

/// Fund structure edges from entity_parent_relationships table
/// Used for trading view: fund manager, umbrella fund, master fund relationships
#[derive(Debug, Clone)]
pub struct FundStructureEdgeView {
    pub relationship_id: Uuid,
    /// Child entity (the fund being managed, subfund, or feeder)
    pub child_entity_id: Uuid,
    pub child_name: String,
    pub child_type_code: Option<String>,
    /// Parent entity (manager, umbrella, or master fund)
    pub parent_entity_id: Option<Uuid>,
    pub parent_lei: Option<String>,
    pub parent_name: Option<String>,
    pub parent_type_code: Option<String>,
    /// Relationship type: FUND_MANAGER, UMBRELLA_FUND, MASTER_FUND
    pub relationship_type: String,
    pub relationship_status: Option<String>,
    pub source: Option<String>,
}

/// UBO registry entries (legacy - prefer UboEdgeView)
#[derive(Debug, Clone)]
pub struct UboView {
    pub ubo_id: Uuid,
    pub subject_entity_id: Uuid,
    pub ubo_proper_person_id: Uuid,
    pub relationship_type: String,
    pub ownership_percentage: Option<bigdecimal::BigDecimal>,
    pub control_type: Option<String>,
    pub verification_status: Option<String>,
    pub subject_name: Option<String>,
    pub ubo_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OwnershipView {
    pub ownership_id: Uuid,
    pub owner_entity_id: Uuid,
    pub owned_entity_id: Uuid,
    pub ownership_type: String,
    pub ownership_percent: bigdecimal::BigDecimal,
    pub owner_name: Option<String>,
    pub owned_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ControlView {
    pub control_id: Uuid,
    pub controller_entity_id: Uuid,
    pub controlled_entity_id: Uuid,
    pub control_type: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub controller_name: Option<String>,
    pub controlled_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResourceInstanceView {
    pub instance_id: Uuid,
    pub status: String,
    pub instance_name: Option<String>,
    pub type_name: String,
    pub category: Option<String>,
}

// =============================================================================
// KYC CASE VIEW MODELS
// =============================================================================

#[derive(Debug, Clone)]
pub struct CaseView {
    pub case_id: Uuid,
    pub cbu_id: Uuid,
    pub status: String,
    pub escalation_level: String,
    pub risk_rating: Option<String>,
    pub case_type: Option<String>,
    pub sla_deadline: Option<chrono::DateTime<chrono::Utc>>,
    pub opened_at: chrono::DateTime<chrono::Utc>,
    pub closed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct WorkstreamView {
    pub workstream_id: Uuid,
    pub case_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub status: String,
    pub risk_rating: Option<String>,
    pub is_ubo: bool,
    pub ownership_percentage: Option<f64>,
    pub requires_enhanced_dd: bool,
    pub discovery_reason: Option<String>,
    pub discovery_depth: i32,
    pub discovery_source_workstream_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct RedFlagView {
    pub red_flag_id: Uuid,
    pub case_id: Uuid,
    pub workstream_id: Option<Uuid>,
    pub flag_type: String,
    pub severity: String,
    pub status: String,
    pub description: String,
    pub source: Option<String>,
    pub raised_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct DocStatsView {
    pub pending: i64,
    pub received: i64,
    pub verified: i64,
    pub rejected: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ScreeningStatsView {
    pub clear: i64,
    pub pending_review: i64,
    pub confirmed_hits: i64,
}

// =============================================================================
// LAYOUT OVERRIDE VIEW MODEL
// =============================================================================

/// Layout overrides for CBU graph visualization (positions and sizes)
#[derive(Debug, Clone)]
pub struct LayoutOverrideView {
    pub positions: Vec<NodeOffset>,
    pub sizes: Vec<NodeSizeOverride>,
}

// =============================================================================
// REPOSITORY
// =============================================================================

/// Repository for visualization data access
///
/// All database queries for visualization are centralized here.
/// This is the ONLY place visualization code should get data from.
pub struct VisualizationRepository {
    pool: PgPool,
}

impl VisualizationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // CBU QUERIES
    // =========================================================================

    /// List all CBUs (for dropdown/selection)
    pub async fn list_cbus(&self) -> Result<Vec<CbuSummaryView>> {
        let cbus = sqlx::query_as!(
            CbuSummaryView,
            r#"SELECT cbu_id, name, jurisdiction, client_type, created_at, updated_at
               FROM "ob-poc".cbus
               ORDER BY name"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(cbus)
    }

    /// Get CBU by ID
    pub async fn get_cbu(&self, cbu_id: Uuid) -> Result<Option<CbuSummaryView>> {
        let cbu = sqlx::query_as!(
            CbuSummaryView,
            r#"SELECT cbu_id, name, jurisdiction, client_type, created_at, updated_at
               FROM "ob-poc".cbus
               WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(cbu)
    }

    /// Get CBU with commercial client info (for tree building)
    pub async fn get_cbu_for_tree(&self, cbu_id: Uuid) -> Result<CbuView> {
        let cbu = sqlx::query_as!(
            CbuView,
            r#"SELECT cbu_id, name, jurisdiction, client_type, commercial_client_entity_id
               FROM "ob-poc".cbus
               WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(cbu)
    }

    // =========================================================================
    // ENTITY QUERIES
    // =========================================================================

    /// Get entity by ID with jurisdiction from type-specific table
    pub async fn get_entity(&self, entity_id: Uuid) -> Result<EntityView> {
        let row = sqlx::query!(
            r#"SELECT e.entity_id, e.name,
                      COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
                      et.type_code as "entity_type!"
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               WHERE e.entity_id = $1"#,
            entity_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(EntityView {
            entity_id: row.entity_id,
            name: row.name,
            jurisdiction: row.jurisdiction,
            entity_type: row.entity_type,
        })
    }

    /// Get single entity by role for a CBU
    pub async fn get_entity_by_role(&self, cbu_id: Uuid, role: &str) -> Result<Option<EntityView>> {
        let row = sqlx::query!(
            r#"SELECT e.entity_id, e.name,
                      COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
                      et.type_code as "entity_type!"
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               WHERE cer.cbu_id = $1 AND r.name = $2
               LIMIT 1"#,
            cbu_id,
            role
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| EntityView {
            entity_id: r.entity_id,
            name: r.name,
            jurisdiction: r.jurisdiction,
            entity_type: r.entity_type,
        }))
    }

    /// Get all entities by role for a CBU
    pub async fn get_entities_by_role(&self, cbu_id: Uuid, role: &str) -> Result<Vec<EntityView>> {
        let rows = sqlx::query!(
            r#"SELECT e.entity_id, e.name,
                      COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) as jurisdiction,
                      et.type_code as "entity_type!"
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
               WHERE cer.cbu_id = $1 AND r.name = $2"#,
            cbu_id,
            role
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EntityView {
                entity_id: r.entity_id,
                name: r.name,
                jurisdiction: r.jurisdiction,
                entity_type: r.entity_type,
            })
            .collect())
    }

    /// Get ALL entities linked to a CBU via any role
    /// Returns each entity with their role (entity may appear multiple times if multiple roles)
    pub async fn get_all_linked_entities(&self, cbu_id: Uuid) -> Result<Vec<EntityWithRoleView>> {
        let rows = sqlx::query!(
            r#"SELECT e.entity_id, e.name,
                      et.type_code as "entity_type!",
                      COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) as jurisdiction,
                      r.name as "role_name!"
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
               WHERE cer.cbu_id = $1
               ORDER BY r.name, e.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EntityWithRoleView {
                entity_id: r.entity_id,
                name: r.name,
                entity_type: r.entity_type,
                jurisdiction: r.jurisdiction,
                role_name: r.role_name,
            })
            .collect())
    }

    /// Get officers (persons) for a CBU with their roles
    pub async fn get_officers(&self, cbu_id: Uuid) -> Result<Vec<OfficerView>> {
        let rows = sqlx::query!(
            r#"SELECT e.entity_id, e.name, pp.nationality, r.name as role_name
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
               WHERE cer.cbu_id = $1 AND et.type_code LIKE 'PROPER_PERSON%'
               ORDER BY e.name, r.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Group by person
        let mut officers: std::collections::HashMap<Uuid, OfficerView> =
            std::collections::HashMap::new();
        for row in rows {
            let entry = officers
                .entry(row.entity_id)
                .or_insert_with(|| OfficerView {
                    entity_id: row.entity_id,
                    name: row.name.clone(),
                    nationality: row.nationality.clone(),
                    roles: Vec::new(),
                });
            entry.roles.push(row.role_name);
        }

        Ok(officers.into_values().collect())
    }

    // =========================================================================
    // SHARE CLASS / HOLDING QUERIES
    // =========================================================================

    /// Get share classes for a CBU
    pub async fn get_share_classes(&self, cbu_id: Uuid) -> Result<Vec<ShareClassView>> {
        let classes = sqlx::query_as!(
            ShareClassView,
            r#"SELECT id, name, currency as "currency!", class_category, isin,
                      nav_per_share::text as nav_per_share, fund_type
               FROM kyc.share_classes
               WHERE cbu_id = $1
               ORDER BY class_category DESC, name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(classes)
    }

    /// Get active holdings for a CBU
    pub async fn get_holdings(&self, cbu_id: Uuid) -> Result<Vec<HoldingView>> {
        let rows = sqlx::query!(
            r#"SELECT h.investor_entity_id, h.share_class_id, h.units::text as "units!"
               FROM kyc.holdings h
               JOIN kyc.share_classes sc ON h.share_class_id = sc.id
               WHERE sc.cbu_id = $1 AND h.status = 'active'"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| HoldingView {
                investor_entity_id: r.investor_entity_id,
                share_class_id: r.share_class_id,
                units: r.units,
            })
            .collect())
    }

    // =========================================================================
    // ATTRIBUTE QUERIES (for display)
    // =========================================================================

    /// Get typed attribute values for an entity
    pub async fn get_entity_attributes(&self, entity_id: Uuid) -> Result<Vec<EntityAttributeView>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                av.attribute_id,
                av.value_text,
                ar.display_name as attribute_name
            FROM "ob-poc".attribute_values_typed av
            JOIN "ob-poc".attribute_registry ar ON ar.id = av.attribute_id
            WHERE av.entity_id = $1
            ORDER BY ar.display_name
            "#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EntityAttributeView {
                attribute_id: r.attribute_id,
                attribute_name: r.attribute_name,
                value_text: r.value_text,
            })
            .collect())
    }

    // =========================================================================
    // SERVICE DELIVERY QUERIES
    // =========================================================================

    /// Get service delivery records for a CBU
    /// Joins cbu_resource_instances via (cbu_id, product_id, service_id) to find resources
    pub async fn get_service_deliveries(&self, cbu_id: Uuid) -> Result<Vec<ServiceDeliveryView>> {
        let rows = sqlx::query!(
            r#"SELECT
                sdm.delivery_id,
                sdm.product_id,
                p.name as "product_name!",
                sdm.service_id,
                s.name as "service_name!",
                cri.instance_id as "instance_id?",
                cri.instance_name as "instance_name?",
                srt.name as "resource_type_name?",
                sdm.delivery_status as "delivery_status?"
               FROM "ob-poc".service_delivery_map sdm
               JOIN "ob-poc".products p ON p.product_id = sdm.product_id
               JOIN "ob-poc".services s ON s.service_id = sdm.service_id
               LEFT JOIN "ob-poc".cbu_resource_instances cri
                   ON cri.cbu_id = sdm.cbu_id
                   AND cri.product_id = sdm.product_id
                   AND cri.service_id = sdm.service_id
               LEFT JOIN "ob-poc".service_resource_types srt ON srt.resource_id = cri.resource_type_id
               WHERE sdm.cbu_id = $1
               ORDER BY p.name, s.name, srt.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ServiceDeliveryView {
                delivery_id: r.delivery_id,
                product_id: r.product_id,
                product_name: r.product_name,
                service_id: r.service_id,
                service_name: r.service_name,
                instance_id: r.instance_id,
                instance_name: r.instance_name,
                resource_type_name: r.resource_type_name,
                delivery_status: r.delivery_status,
            })
            .collect())
    }

    // =========================================================================
    // MCP QUERIES - CBU
    // =========================================================================

    /// Get basic CBU info
    pub async fn get_cbu_basic(&self, cbu_id: Uuid) -> Result<Option<CbuBasicView>> {
        let row = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction, client_type, cbu_category
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| CbuBasicView {
            cbu_id: r.cbu_id,
            name: r.name,
            client_type: r.client_type,
            jurisdiction: r.jurisdiction,
            cbu_category: r.cbu_category,
        }))
    }

    /// List CBUs with optional search filter
    pub async fn list_cbus_filtered(
        &self,
        search: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CbuBasicView>> {
        let rows = sqlx::query!(
            r#"SELECT cbu_id, name, client_type, jurisdiction, cbu_category
               FROM "ob-poc".cbus
               WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')
               ORDER BY name
               LIMIT $2"#,
            search,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CbuBasicView {
                cbu_id: r.cbu_id,
                name: r.name,
                client_type: r.client_type,
                jurisdiction: r.jurisdiction,
                cbu_category: r.cbu_category,
            })
            .collect())
    }

    /// Get entities linked to a CBU
    pub async fn get_cbu_entities(&self, cbu_id: Uuid) -> Result<Vec<CbuEntityView>> {
        let rows = sqlx::query!(
            r#"SELECT DISTINCT e.entity_id, e.name, et.type_code as "entity_type!"
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               WHERE cer.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CbuEntityView {
                entity_id: r.entity_id,
                name: r.name,
                entity_type: r.entity_type,
            })
            .collect())
    }

    /// Get roles for a CBU
    pub async fn get_cbu_roles(&self, cbu_id: Uuid) -> Result<Vec<CbuRoleView>> {
        let rows = sqlx::query!(
            r#"SELECT cer.entity_id, r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               WHERE cer.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CbuRoleView {
                entity_id: r.entity_id,
                role_name: r.role_name,
            })
            .collect())
    }

    /// Get documents for a CBU
    pub async fn get_cbu_documents(&self, cbu_id: Uuid) -> Result<Vec<CbuDocumentView>> {
        let rows = sqlx::query!(
            r#"SELECT dc.doc_id, dc.document_type_code, dc.status
               FROM "ob-poc".document_catalog dc
               WHERE dc.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CbuDocumentView {
                doc_id: r.doc_id,
                document_type_code: r.document_type_code,
                status: r.status,
            })
            .collect())
    }

    /// Get screenings for entities in a CBU (via workstreams in cases)
    pub async fn get_cbu_screenings(&self, cbu_id: Uuid) -> Result<Vec<CbuScreeningView>> {
        let rows = sqlx::query!(
            r#"SELECT s.screening_id, w.entity_id, s.screening_type, s.status, s.result_summary as result
               FROM kyc.screenings s
               JOIN kyc.entity_workstreams w ON w.workstream_id = s.workstream_id
               JOIN kyc.cases c ON c.case_id = w.case_id
               WHERE c.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CbuScreeningView {
                screening_id: r.screening_id,
                entity_id: r.entity_id,
                screening_type: r.screening_type,
                status: Some(r.status),
                result: r.result,
            })
            .collect())
    }

    // =========================================================================
    // MCP QUERIES - ENTITY
    // =========================================================================

    /// Get basic entity info
    pub async fn get_entity_basic(&self, entity_id: Uuid) -> Result<Option<EntityBasicView>> {
        let row = sqlx::query!(
            r#"SELECT e.entity_id, e.name, et.type_code
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               WHERE e.entity_id = $1"#,
            entity_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| EntityBasicView {
            entity_id: r.entity_id,
            name: r.name,
            type_code: r.type_code,
        }))
    }

    /// Get CBUs an entity belongs to
    pub async fn get_entity_cbus(&self, entity_id: Uuid) -> Result<Vec<EntityCbuView>> {
        let rows = sqlx::query!(
            r#"SELECT DISTINCT cer.cbu_id, c.name as cbu_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".cbus c ON cer.cbu_id = c.cbu_id
               WHERE cer.entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EntityCbuView {
                cbu_id: r.cbu_id,
                cbu_name: r.cbu_name,
            })
            .collect())
    }

    /// Get roles for an entity
    pub async fn get_entity_roles(&self, entity_id: Uuid) -> Result<Vec<EntityRoleView>> {
        let rows = sqlx::query!(
            r#"SELECT r.name as role_name, cer.cbu_id
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               WHERE cer.entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EntityRoleView {
                role_name: r.role_name,
                cbu_id: r.cbu_id,
            })
            .collect())
    }

    /// Get documents linked to an entity (placeholder - document_entity_links removed)
    pub async fn get_entity_documents(&self, _entity_id: Uuid) -> Result<Vec<CbuDocumentView>> {
        // document_entity_links table was removed in schema cleanup
        // Documents are now linked via workstream doc_requests in kyc schema
        Ok(Vec::new())
    }

    /// Get screenings for an entity (via workstreams)
    pub async fn get_entity_screenings(&self, entity_id: Uuid) -> Result<Vec<EntityScreeningView>> {
        let rows = sqlx::query!(
            r#"SELECT s.screening_id, s.screening_type, s.status, s.result_summary as result
               FROM kyc.screenings s
               JOIN kyc.entity_workstreams w ON w.workstream_id = s.workstream_id
               WHERE w.entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EntityScreeningView {
                screening_id: r.screening_id,
                screening_type: r.screening_type,
                status: Some(r.status),
                result: r.result,
            })
            .collect())
    }

    // =========================================================================
    // MCP QUERIES - SCHEMA INFO
    // =========================================================================

    /// Get all entity types
    pub async fn get_entity_types(&self) -> Result<Vec<EntityTypeView>> {
        let rows =
            sqlx::query!(r#"SELECT type_code, name FROM "ob-poc".entity_types ORDER BY type_code"#)
                .fetch_all(&self.pool)
                .await?;

        Ok(rows
            .into_iter()
            .map(|r| EntityTypeView {
                type_code: r.type_code,
                name: r.name,
            })
            .collect())
    }

    /// Get all roles
    pub async fn get_all_roles(&self) -> Result<Vec<RoleView>> {
        let rows = sqlx::query!(r#"SELECT role_id, name FROM "ob-poc".roles ORDER BY name"#)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|r| RoleView {
                role_id: r.role_id,
                name: r.name,
            })
            .collect())
    }

    /// Get all document types
    pub async fn get_document_types(&self) -> Result<Vec<DocumentTypeView>> {
        let rows = sqlx::query!(
            r#"SELECT type_code, display_name FROM "ob-poc".document_types ORDER BY type_code"#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DocumentTypeView {
                type_code: r.type_code,
                display_name: r.display_name,
            })
            .collect())
    }

    // =========================================================================
    // GRAPH QUERIES - CORE LAYER
    // =========================================================================

    /// Get entities linked to a CBU via cbu_entity_roles (for graph)
    /// Uses v_cbu_entity_with_roles view for aggregated role data
    pub async fn get_graph_entities(&self, cbu_id: Uuid) -> Result<Vec<GraphEntityView>> {
        // Use the view that aggregates roles and computes primary role
        // LEFT JOIN to entity_proper_persons to get person_state for ghost rendering
        let rows = sqlx::query!(
            r#"SELECT
                v.entity_id as "entity_id!",
                v.entity_name as "entity_name!",
                v.entity_type as "entity_type!",
                v.entity_category,
                v.jurisdiction,
                v.roles,
                v.role_categories,
                v.primary_role,
                v.max_role_priority as role_priority,
                v.primary_role_category,
                v.primary_layout_category,
                v.effective_ubo_treatment,
                v.effective_kyc_obligation,
                pp.person_state as "person_state?"
               FROM "ob-poc".v_cbu_entity_with_roles v
               LEFT JOIN "ob-poc".entity_proper_persons pp ON v.entity_id = pp.entity_id
               WHERE v.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| GraphEntityView {
                // Generate a synthetic role ID since we're grouping by entity
                cbu_entity_role_id: r.entity_id,
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                entity_type: r.entity_type,
                entity_category: r.entity_category,
                // Use primary_role as the main role_name
                role_name: r.primary_role.clone().unwrap_or_default(),
                jurisdiction: r.jurisdiction,
                roles: r.roles.unwrap_or_default(),
                role_categories: r.role_categories.unwrap_or_default(),
                primary_role: r.primary_role,
                role_priority: r.role_priority,
                // Role Taxonomy V2 fields
                primary_role_category: r.primary_role_category,
                layout_category: r.primary_layout_category,
                ubo_treatment: r.effective_ubo_treatment,
                kyc_obligation: r.effective_kyc_obligation,
                // Person state for ghost entity rendering (None for non-person entities)
                person_state: r.person_state,
            })
            .collect())
    }

    // =========================================================================
    // GRAPH QUERIES - CUSTODY LAYER
    // =========================================================================

    /// Get universe entries for a CBU
    pub async fn get_universes(&self, cbu_id: Uuid) -> Result<Vec<UniverseView>> {
        let rows = sqlx::query!(
            r#"SELECT
                u.universe_id,
                u.instrument_class_id,
                u.market_id,
                u.currencies,
                u.settlement_types,
                u.is_active,
                ic.name as "class_name?",
                m.name as "market_name?",
                m.mic as "mic?"
               FROM custody.cbu_instrument_universe u
               LEFT JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
               LEFT JOIN custody.markets m ON m.market_id = u.market_id
               WHERE u.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| UniverseView {
                universe_id: r.universe_id,
                instrument_class_id: r.instrument_class_id,
                market_id: r.market_id,
                currencies: r.currencies,
                settlement_types: r.settlement_types.unwrap_or_default(),
                is_active: r.is_active,
                class_name: r.class_name,
                market_name: r.market_name,
                mic: r.mic,
            })
            .collect())
    }

    /// Get SSIs for a CBU
    pub async fn get_ssis(&self, cbu_id: Uuid) -> Result<Vec<SsiView>> {
        let rows = sqlx::query!(
            r#"SELECT s.ssi_id, s.ssi_name, s.ssi_type, s.status, s.cash_currency,
                      s.safekeeping_account, s.safekeeping_bic, s.cash_account, s.cash_account_bic,
                      s.market_id,
                      m.mic as "mic?"
               FROM custody.cbu_ssi s
               LEFT JOIN custody.markets m ON m.market_id = s.market_id
               WHERE s.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SsiView {
                ssi_id: r.ssi_id,
                ssi_name: r.ssi_name,
                ssi_type: r.ssi_type,
                status: r.status,
                cash_currency: r.cash_currency,
                safekeeping_account: r.safekeeping_account,
                safekeeping_bic: r.safekeeping_bic,
                cash_account: r.cash_account,
                cash_account_bic: r.cash_account_bic,
                market_id: r.market_id,
                mic: r.mic,
            })
            .collect())
    }

    /// Get booking rules for a CBU
    pub async fn get_booking_rules(&self, cbu_id: Uuid) -> Result<Vec<BookingRuleView>> {
        let rows = sqlx::query!(
            r#"SELECT r.rule_id, r.rule_name, r.priority, r.ssi_id,
                      r.instrument_class_id, r.market_id, r.currency, r.is_active,
                      ic.name as "class_name?",
                      m.mic as "mic?"
               FROM custody.ssi_booking_rules r
               LEFT JOIN custody.instrument_classes ic ON ic.class_id = r.instrument_class_id
               LEFT JOIN custody.markets m ON m.market_id = r.market_id
               WHERE r.cbu_id = $1
               ORDER BY r.priority"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| BookingRuleView {
                rule_id: r.rule_id,
                rule_name: r.rule_name,
                priority: r.priority,
                ssi_id: r.ssi_id,
                instrument_class_id: r.instrument_class_id,
                market_id: r.market_id,
                currency: r.currency,
                is_active: r.is_active,
                class_name: r.class_name,
                mic: r.mic,
            })
            .collect())
    }

    /// Get ISDA agreements for a CBU
    pub async fn get_isdas(&self, cbu_id: Uuid) -> Result<Vec<IsdaView>> {
        let rows = sqlx::query!(
            r#"SELECT i.isda_id, i.counterparty_entity_id, i.governing_law,
                      i.agreement_date, i.is_active,
                      e.name as "counterparty_name?"
               FROM custody.isda_agreements i
               LEFT JOIN "ob-poc".entities e ON e.entity_id = i.counterparty_entity_id
               WHERE i.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| IsdaView {
                isda_id: r.isda_id,
                counterparty_entity_id: r.counterparty_entity_id,
                governing_law: r.governing_law,
                agreement_date: r.agreement_date,
                is_active: r.is_active,
                counterparty_name: r.counterparty_name,
            })
            .collect())
    }

    /// Get CSAs for an ISDA
    pub async fn get_csas(&self, isda_id: Uuid) -> Result<Vec<CsaView>> {
        let rows = sqlx::query!(
            r#"SELECT csa_id, csa_type, is_active
               FROM custody.csa_agreements
               WHERE isda_id = $1"#,
            isda_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CsaView {
                csa_id: r.csa_id,
                csa_type: r.csa_type,
                is_active: r.is_active,
            })
            .collect())
    }

    // =========================================================================
    // GRAPH QUERIES - KYC LAYER
    // =========================================================================

    /// Get KYC statuses for entities in a CBU (via workstreams)
    pub async fn get_kyc_statuses(&self, cbu_id: Uuid) -> Result<Vec<KycStatusView>> {
        let rows = sqlx::query!(
            r#"SELECT
                w.workstream_id as status_id,
                w.entity_id,
                w.status as kyc_status,
                w.risk_rating,
                NULL::date as next_review_date,
                e.name as "entity_name?"
               FROM kyc.entity_workstreams w
               JOIN kyc.cases c ON c.case_id = w.case_id
               JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
               WHERE c.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| KycStatusView {
                status_id: r.status_id,
                entity_id: r.entity_id,
                kyc_status: Some(r.kyc_status),
                risk_rating: r.risk_rating,
                next_review_date: r.next_review_date,
                entity_name: r.entity_name,
            })
            .collect())
    }

    /// Get document requests for a CBU (via workstreams)
    pub async fn get_document_requests(&self, cbu_id: Uuid) -> Result<Vec<DocumentRequestView>> {
        let rows = sqlx::query!(
            r#"SELECT
                dr.request_id,
                dr.doc_type as document_type,
                dr.status,
                w.entity_id as requested_from_entity_id,
                e.name as "entity_name?"
               FROM kyc.doc_requests dr
               JOIN kyc.entity_workstreams w ON w.workstream_id = dr.workstream_id
               JOIN kyc.cases c ON c.case_id = w.case_id
               LEFT JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
               WHERE c.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DocumentRequestView {
                request_id: r.request_id,
                document_type: r.document_type,
                status: Some(r.status),
                requested_from_entity_id: Some(r.requested_from_entity_id),
                entity_name: r.entity_name,
            })
            .collect())
    }

    /// Get screenings for entities in a CBU (for graph, via workstreams)
    pub async fn get_graph_screenings(&self, cbu_id: Uuid) -> Result<Vec<ScreeningView>> {
        let rows = sqlx::query!(
            r#"SELECT
                s.screening_id,
                w.entity_id,
                s.screening_type,
                s.result_summary as result,
                NULL::varchar as resolution,
                e.name as "entity_name?"
               FROM kyc.screenings s
               JOIN kyc.entity_workstreams w ON w.workstream_id = s.workstream_id
               JOIN kyc.cases c ON c.case_id = w.case_id
               JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
               WHERE c.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ScreeningView {
                screening_id: r.screening_id,
                entity_id: r.entity_id,
                screening_type: r.screening_type,
                result: r.result,
                resolution: r.resolution,
                entity_name: r.entity_name,
            })
            .collect())
    }

    // =========================================================================
    // GRAPH QUERIES - UBO LAYER
    // =========================================================================

    /// Get UBO edges for a CBU from entity_relationships + cbu_relationship_verification
    /// This is the primary source of truth for ownership/control chains with status workflow
    pub async fn get_ubo_edges(&self, cbu_id: Uuid) -> Result<Vec<UboEdgeView>> {
        let rows = sqlx::query!(
            r#"SELECT
                r.relationship_id as edge_id,
                v.cbu_id,
                r.from_entity_id,
                r.to_entity_id,
                r.relationship_type as edge_type,
                r.percentage,
                r.control_type as control_role,
                r.trust_role,
                v.status as "status!",
                v.alleged_percentage,
                v.observed_percentage as proven_percentage,
                from_e.name as "from_name!",
                to_e.name as "to_name!",
                from_et.type_code as "from_type_code?",
                to_et.type_code as "to_type_code?",
                from_et.entity_category as "from_category?",
                to_et.entity_category as "to_category?"
               FROM "ob-poc".entity_relationships r
               JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
               JOIN "ob-poc".entities from_e ON r.from_entity_id = from_e.entity_id
               JOIN "ob-poc".entities to_e ON r.to_entity_id = to_e.entity_id
               JOIN "ob-poc".entity_types from_et ON from_e.entity_type_id = from_et.entity_type_id
               JOIN "ob-poc".entity_types to_et ON to_e.entity_type_id = to_et.entity_type_id
               WHERE v.cbu_id = $1
               ORDER BY r.relationship_type, from_e.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| UboEdgeView {
                edge_id: r.edge_id,
                cbu_id: r.cbu_id,
                from_entity_id: r.from_entity_id,
                to_entity_id: r.to_entity_id,
                edge_type: r.edge_type,
                percentage: r.percentage,
                control_role: r.control_role,
                trust_role: r.trust_role,
                status: r.status,
                alleged_percentage: r.alleged_percentage,
                proven_percentage: r.proven_percentage,
                from_name: r.from_name,
                to_name: r.to_name,
                from_type_code: r.from_type_code,
                to_type_code: r.to_type_code,
                from_category: r.from_category,
                to_category: r.to_category,
            })
            .collect())
    }

    /// Get UBO registry entries for a CBU (legacy - prefer get_ubo_edges)
    pub async fn get_ubos(&self, cbu_id: Uuid) -> Result<Vec<UboView>> {
        let rows = sqlx::query!(
            r#"SELECT
                u.ubo_id,
                u.subject_entity_id,
                u.ubo_proper_person_id,
                u.relationship_type,
                u.ownership_percentage,
                u.control_type,
                u.verification_status,
                se.name as "subject_name?",
                pe.name as "ubo_name?"
               FROM "ob-poc".ubo_registry u
               LEFT JOIN "ob-poc".entities se ON se.entity_id = u.subject_entity_id
               LEFT JOIN "ob-poc".entities pe ON pe.entity_id = u.ubo_proper_person_id
               WHERE u.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| UboView {
                ubo_id: r.ubo_id,
                subject_entity_id: r.subject_entity_id,
                ubo_proper_person_id: r.ubo_proper_person_id,
                relationship_type: r.relationship_type,
                ownership_percentage: r.ownership_percentage,
                control_type: r.control_type,
                verification_status: r.verification_status,
                subject_name: r.subject_name,
                ubo_name: r.ubo_name,
            })
            .collect())
    }

    /// Get ownership relationships for a CBU from entity_relationships
    pub async fn get_ownerships(&self, cbu_id: Uuid) -> Result<Vec<OwnershipView>> {
        let rows = sqlx::query!(
            r#"SELECT
                r.relationship_id as "ownership_id!",
                r.from_entity_id as "owner_entity_id!",
                r.to_entity_id as "owned_entity_id!",
                COALESCE(r.source, 'direct') as "ownership_type!",
                COALESCE(r.percentage, 0) as "ownership_percent!",
                owner.name as "owner_name?",
                owned.name as "owned_name?"
               FROM "ob-poc".entity_relationships r
               LEFT JOIN "ob-poc".entities owner ON owner.entity_id = r.from_entity_id
               LEFT JOIN "ob-poc".entities owned ON owned.entity_id = r.to_entity_id
               WHERE r.relationship_type = 'ownership'
               AND (r.to_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )
               OR r.from_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               ))"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| OwnershipView {
                ownership_id: r.ownership_id,
                owner_entity_id: r.owner_entity_id,
                owned_entity_id: r.owned_entity_id,
                ownership_type: r.ownership_type,
                ownership_percent: r.ownership_percent,
                owner_name: r.owner_name,
                owned_name: r.owned_name,
            })
            .collect())
    }

    /// Get control relationships for a CBU from entity_relationships
    pub async fn get_graph_controls(&self, cbu_id: Uuid) -> Result<Vec<ControlView>> {
        let rows = sqlx::query!(
            r#"SELECT
                r.relationship_id as "control_id!",
                r.from_entity_id as "controller_entity_id!",
                r.to_entity_id as "controlled_entity_id!",
                COALESCE(r.control_type, 'control') as "control_type!",
                controller.name as "controller_name?",
                controlled.name as "controlled_name?"
               FROM "ob-poc".entity_relationships r
               LEFT JOIN "ob-poc".entities controller ON controller.entity_id = r.from_entity_id
               LEFT JOIN "ob-poc".entities controlled ON controlled.entity_id = r.to_entity_id
               WHERE r.relationship_type = 'control'
               AND (r.to_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )
               OR r.from_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               ))"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ControlView {
                control_id: r.control_id,
                controller_entity_id: r.controller_entity_id,
                controlled_entity_id: r.controlled_entity_id,
                control_type: r.control_type,
                description: None,
                is_active: Some(true),
                controller_name: r.controller_name,
                controlled_name: r.controlled_name,
            })
            .collect())
    }

    /// Get fund structure edges from entity_parent_relationships table
    /// These are GLEIF-sourced relationships: FUND_MANAGER, UMBRELLA_FUND, MASTER_FUND
    /// Used for Trading View to show operational structure
    pub async fn get_fund_structure_edges(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<FundStructureEdgeView>> {
        let rows = sqlx::query!(
            r#"SELECT
                epr.relationship_id,
                epr.child_entity_id,
                child_e.name as "child_name!",
                child_et.type_code as "child_type_code?",
                epr.parent_entity_id,
                epr.parent_lei,
                epr.parent_name,
                parent_et.type_code as "parent_type_code?",
                epr.relationship_type as "relationship_type!",
                epr.relationship_status,
                epr.source
               FROM "ob-poc".entity_parent_relationships epr
               JOIN "ob-poc".entities child_e ON epr.child_entity_id = child_e.entity_id
               JOIN "ob-poc".entity_types child_et ON child_e.entity_type_id = child_et.entity_type_id
               LEFT JOIN "ob-poc".entities parent_e ON epr.parent_entity_id = parent_e.entity_id
               LEFT JOIN "ob-poc".entity_types parent_et ON parent_e.entity_type_id = parent_et.entity_type_id
               WHERE epr.relationship_type IN ('FUND_MANAGER', 'UMBRELLA_FUND', 'MASTER_FUND')
               AND epr.child_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )
               ORDER BY epr.relationship_type, child_e.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| FundStructureEdgeView {
                relationship_id: r.relationship_id,
                child_entity_id: r.child_entity_id,
                child_name: r.child_name,
                child_type_code: r.child_type_code,
                parent_entity_id: r.parent_entity_id,
                parent_lei: r.parent_lei,
                parent_name: r.parent_name,
                parent_type_code: r.parent_type_code,
                relationship_type: r.relationship_type,
                relationship_status: r.relationship_status,
                source: r.source,
            })
            .collect())
    }

    // =========================================================================
    // GRAPH QUERIES - SERVICES LAYER
    // =========================================================================

    /// Get resource instances for a CBU
    pub async fn get_resource_instances(&self, cbu_id: Uuid) -> Result<Vec<ResourceInstanceView>> {
        let rows = sqlx::query!(
            r#"SELECT ri.instance_id, ri.status, ri.instance_name,
                      rt.name as type_name, rt.resource_type as category
               FROM "ob-poc".cbu_resource_instances ri
               JOIN "ob-poc".service_resource_types rt ON rt.resource_id = ri.resource_type_id
               WHERE ri.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ResourceInstanceView {
                instance_id: r.instance_id,
                status: r.status,
                instance_name: r.instance_name,
                type_name: r.type_name,
                category: r.category,
            })
            .collect())
    }

    /// Get products for a CBU via service_delivery_map (source of truth)
    /// A CBU can have 0..n products
    pub async fn get_cbu_products(&self, cbu_id: Uuid) -> Result<Vec<ProductView>> {
        let rows = sqlx::query!(
            r#"SELECT DISTINCT p.product_id, p.name, p.product_code, p.product_category, p.is_active
               FROM "ob-poc".products p
               JOIN "ob-poc".service_delivery_map sdm ON sdm.product_id = p.product_id
               WHERE sdm.cbu_id = $1
               ORDER BY p.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ProductView {
                product_id: r.product_id,
                name: r.name,
                product_code: r.product_code,
                product_category: r.product_category,
                is_active: r.is_active,
            })
            .collect())
    }

    /// Get services for a product via product_services
    pub async fn get_product_services(&self, product_id: Uuid) -> Result<Vec<ServiceView>> {
        let rows = sqlx::query!(
            r#"SELECT s.service_id, s.name, s.service_code, s.service_category, ps.is_mandatory
               FROM "ob-poc".services s
               JOIN "ob-poc".product_services ps ON ps.service_id = s.service_id
               WHERE ps.product_id = $1
               ORDER BY ps.display_order, s.name"#,
            product_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ServiceView {
                service_id: r.service_id,
                name: r.name,
                service_code: r.service_code,
                service_category: r.service_category,
                is_mandatory: r.is_mandatory,
            })
            .collect())
    }

    /// Get service resource types for a service via service_resource_capabilities
    pub async fn get_service_resource_types(
        &self,
        service_id: Uuid,
    ) -> Result<Vec<ServiceResourceTypeView>> {
        let rows = sqlx::query!(
            r#"SELECT rt.resource_id, rt.name, rt.resource_type, rt.resource_code, src.is_active
               FROM "ob-poc".service_resource_types rt
               JOIN "ob-poc".service_resource_capabilities src ON src.resource_id = rt.resource_id
               WHERE src.service_id = $1
               ORDER BY src.priority, rt.name"#,
            service_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ServiceResourceTypeView {
                resource_id: r.resource_id,
                name: r.name,
                resource_type: r.resource_type,
                resource_code: r.resource_code,
                is_active: r.is_active,
            })
            .collect())
    }

    // =========================================================================
    // KYC CASE QUERIES
    // =========================================================================

    /// Get a KYC case by ID
    pub async fn get_case(&self, case_id: Uuid) -> Result<CaseView> {
        let row = sqlx::query!(
            r#"SELECT
                case_id, cbu_id, status, escalation_level, risk_rating,
                case_type, sla_deadline, opened_at, closed_at
               FROM kyc.cases
               WHERE case_id = $1"#,
            case_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(CaseView {
            case_id: row.case_id,
            cbu_id: row.cbu_id,
            status: row.status,
            escalation_level: row.escalation_level,
            risk_rating: row.risk_rating,
            case_type: row.case_type,
            sla_deadline: row.sla_deadline,
            opened_at: row.opened_at,
            closed_at: row.closed_at,
        })
    }

    /// Get all workstreams for a case
    pub async fn get_case_workstreams(&self, case_id: Uuid) -> Result<Vec<WorkstreamView>> {
        let rows = sqlx::query!(
            r#"SELECT
                w.workstream_id,
                w.case_id,
                w.entity_id,
                e.name as entity_name,
                et.type_code as entity_type,
                COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
                w.status,
                w.risk_rating,
                w.is_ubo,
                w.ownership_percentage,
                w.requires_enhanced_dd,
                w.discovery_reason,
                w.discovery_depth,
                w.discovery_source_workstream_id
               FROM kyc.entity_workstreams w
               JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
               JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON p.entity_id = e.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON t.entity_id = e.entity_id
               WHERE w.case_id = $1
               ORDER BY w.discovery_depth, w.created_at"#,
            case_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| WorkstreamView {
                workstream_id: r.workstream_id,
                case_id: r.case_id,
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                entity_type: r.entity_type.unwrap_or_default(),
                jurisdiction: r.jurisdiction,
                status: r.status,
                risk_rating: r.risk_rating,
                is_ubo: r.is_ubo.unwrap_or(false),
                ownership_percentage: r
                    .ownership_percentage
                    .map(|d| d.to_string().parse().unwrap_or(0.0)),
                requires_enhanced_dd: r.requires_enhanced_dd.unwrap_or(false),
                discovery_reason: r.discovery_reason,
                discovery_depth: r.discovery_depth.unwrap_or(1),
                discovery_source_workstream_id: r.discovery_source_workstream_id,
            })
            .collect())
    }

    /// Get all red flags for a case
    pub async fn get_case_red_flags(&self, case_id: Uuid) -> Result<Vec<RedFlagView>> {
        let rows = sqlx::query!(
            r#"SELECT
                red_flag_id, case_id, workstream_id, flag_type, severity,
                status, description, source, raised_at
               FROM kyc.red_flags
               WHERE case_id = $1
               ORDER BY raised_at DESC"#,
            case_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| RedFlagView {
                red_flag_id: r.red_flag_id,
                case_id: r.case_id,
                workstream_id: r.workstream_id,
                flag_type: r.flag_type,
                severity: r.severity,
                status: r.status,
                description: r.description,
                source: r.source,
                raised_at: r.raised_at,
            })
            .collect())
    }

    /// Get document stats for a workstream
    pub async fn get_workstream_doc_stats(&self, workstream_id: Uuid) -> Result<DocStatsView> {
        let row = sqlx::query!(
            r#"SELECT
                COUNT(*) FILTER (WHERE status IN ('REQUIRED', 'REQUESTED')) as "pending!",
                COUNT(*) FILTER (WHERE status IN ('RECEIVED', 'UNDER_REVIEW')) as "received!",
                COUNT(*) FILTER (WHERE status = 'VERIFIED') as "verified!",
                COUNT(*) FILTER (WHERE status = 'REJECTED') as "rejected!"
               FROM kyc.doc_requests
               WHERE workstream_id = $1"#,
            workstream_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(DocStatsView {
            pending: row.pending,
            received: row.received,
            verified: row.verified,
            rejected: row.rejected,
        })
    }

    /// Get screening stats for a workstream
    pub async fn get_workstream_screening_stats(
        &self,
        workstream_id: Uuid,
    ) -> Result<ScreeningStatsView> {
        let row = sqlx::query!(
            r#"SELECT
                COUNT(*) FILTER (WHERE status = 'CLEAR') as "clear!",
                COUNT(*) FILTER (WHERE status = 'HIT_PENDING_REVIEW') as "pending_review!",
                COUNT(*) FILTER (WHERE status = 'HIT_CONFIRMED') as "confirmed_hits!"
               FROM kyc.screenings
               WHERE workstream_id = $1"#,
            workstream_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(ScreeningStatsView {
            clear: row.clear,
            pending_review: row.pending_review,
            confirmed_hits: row.confirmed_hits,
        })
    }
    // =====================================================================
    // LAYOUT OVERRIDE PERSISTENCE
    // =====================================================================

    /// Fetch saved layout overrides for a CBU/user/view_mode combo.
    pub async fn get_layout_override(
        &self,
        cbu_id: Uuid,
        user_id: Uuid,
        view_mode: &str,
    ) -> Result<Option<LayoutOverrideView>> {
        let row = sqlx::query!(
            r#"SELECT positions as "positions: Json<Vec<NodeOffset>>",
                      sizes as "sizes: Json<Vec<NodeSizeOverride>>"
               FROM "ob-poc".cbu_layout_overrides
               WHERE cbu_id = $1 AND user_id = $2 AND view_mode = $3"#,
            cbu_id,
            user_id,
            view_mode,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| LayoutOverrideView {
            positions: r.positions.0,
            sizes: r.sizes.0,
        }))
    }

    /// Upsert layout overrides for a CBU/user/view_mode combo.
    pub async fn upsert_layout_override(
        &self,
        cbu_id: Uuid,
        user_id: Uuid,
        view_mode: &str,
        overrides: LayoutOverrideView,
    ) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO "ob-poc".cbu_layout_overrides
                    (cbu_id, user_id, view_mode, positions, sizes, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW())
               ON CONFLICT (cbu_id, user_id, view_mode)
               DO UPDATE SET positions = EXCLUDED.positions,
                             sizes = EXCLUDED.sizes,
                             updated_at = NOW()"#,
            cbu_id,
            user_id,
            view_mode,
            serde_json::to_value(&overrides.positions).unwrap_or_default(),
            serde_json::to_value(&overrides.sizes).unwrap_or_default(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // =========================================================================
    // TRADING LAYER QUERIES
    // =========================================================================

    /// Get active trading profile for CBU
    pub async fn get_active_trading_profile(
        &self,
        cbu_id: Uuid,
    ) -> Result<Option<TradingProfileView>> {
        let row = sqlx::query!(
            r#"SELECT profile_id, version, status, activated_at
               FROM "ob-poc".cbu_trading_profiles
               WHERE cbu_id = $1 AND status = 'ACTIVE'
               ORDER BY version DESC
               LIMIT 1"#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| TradingProfileView {
            profile_id: r.profile_id,
            version: r.version,
            status: r.status,
            activated_at: r.activated_at,
        }))
    }

    /// Get current working trading profile for CBU (ACTIVE preferred, then DRAFT/VALIDATED/PENDING_REVIEW)
    /// This is used for visualization to show profiles that are being worked on
    pub async fn get_current_trading_profile(
        &self,
        cbu_id: Uuid,
    ) -> Result<Option<TradingProfileView>> {
        // First try to get ACTIVE profile
        if let Some(active) = self.get_active_trading_profile(cbu_id).await? {
            return Ok(Some(active));
        }

        // Fall back to most recent working version (DRAFT, VALIDATED, or PENDING_REVIEW)
        let row = sqlx::query!(
            r#"SELECT profile_id, version, status, activated_at
               FROM "ob-poc".cbu_trading_profiles
               WHERE cbu_id = $1 AND status IN ('DRAFT', 'VALIDATED', 'PENDING_REVIEW')
               ORDER BY version DESC
               LIMIT 1"#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| TradingProfileView {
            profile_id: r.profile_id,
            version: r.version,
            status: r.status,
            activated_at: r.activated_at,
        }))
    }

    /// Get instrument universe entries for CBU (from materialized custody tables)
    pub async fn get_cbu_instrument_universe(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<UniverseEntryView>> {
        let rows = sqlx::query!(
            r#"SELECT
                u.universe_id,
                u.instrument_class_id,
                ic.code as class_code,
                ic.name as class_name,
                u.market_id,
                m.mic as "mic?",
                m.name as "market_name?",
                u.counterparty_entity_id as counterparty_id,
                e.name as "counterparty_name?",
                u.currencies,
                COALESCE(ic.requires_isda, false) as "is_otc!"
               FROM custody.cbu_instrument_universe u
               JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
               LEFT JOIN custody.markets m ON m.market_id = u.market_id
               LEFT JOIN "ob-poc".entities e ON e.entity_id = u.counterparty_entity_id
               WHERE u.cbu_id = $1 AND u.is_active = true
               ORDER BY ic.code, m.mic, e.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| UniverseEntryView {
                universe_id: r.universe_id,
                instrument_class_id: r.instrument_class_id,
                class_code: r.class_code,
                class_name: r.class_name,
                market_id: r.market_id,
                mic: r.mic,
                market_name: r.market_name,
                counterparty_id: r.counterparty_id,
                counterparty_name: r.counterparty_name,
                currencies: r.currencies,
                is_otc: r.is_otc,
            })
            .collect())
    }

    /// Get ISDA agreements for CBU
    pub async fn get_cbu_isda_agreements(&self, cbu_id: Uuid) -> Result<Vec<IsdaAgreementView>> {
        let rows = sqlx::query!(
            r#"SELECT
                i.isda_id,
                i.counterparty_entity_id,
                e.name as counterparty_name,
                i.governing_law,
                i.agreement_date
               FROM custody.isda_agreements i
               JOIN "ob-poc".entities e ON e.entity_id = i.counterparty_entity_id
               WHERE i.cbu_id = $1 AND i.is_active = true
               ORDER BY e.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| IsdaAgreementView {
                isda_id: r.isda_id,
                counterparty_entity_id: r.counterparty_entity_id,
                counterparty_name: Some(r.counterparty_name),
                governing_law: r.governing_law,
                agreement_date: Some(r.agreement_date),
            })
            .collect())
    }

    /// Get CSA for ISDA agreement
    pub async fn get_isda_csa(&self, isda_id: Uuid) -> Result<Option<CsaAgreementView>> {
        let row = sqlx::query!(
            r#"SELECT
                csa_id,
                csa_type,
                threshold_amount::float8 as "threshold_amount: f64",
                threshold_currency
               FROM custody.csa_agreements
               WHERE isda_id = $1 AND is_active = true"#,
            isda_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| CsaAgreementView {
            csa_id: r.csa_id,
            csa_type: r.csa_type,
            threshold_amount: r.threshold_amount,
            threshold_currency: r.threshold_currency,
        }))
    }

    /// Get investment managers for CBU (entities with IM role assignment)
    pub async fn get_cbu_investment_managers(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<InvestmentManagerView>> {
        // Get entities assigned as investment managers via cbu_entity_roles
        let rows = sqlx::query!(
            r#"SELECT
                e.entity_id,
                e.name as entity_name,
                r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
               JOIN "ob-poc".roles r ON r.role_id = cer.role_id
               WHERE cer.cbu_id = $1
                 AND r.name IN ('INVESTMENT_MANAGER', 'DELEGATED_IM', 'SUB_ADVISOR')
               ORDER BY e.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| InvestmentManagerView {
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                can_trade: true, // Default - could be enhanced with scope data
                can_settle: true,
                scope_mics: vec![],
                scope_classes: vec![],
                scope_description: r.role_name,
            })
            .collect())
    }
}
