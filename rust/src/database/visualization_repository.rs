//! Visualization Repository
//!
//! Provides read-only data access for visualization builders.
//! All SQL is centralized here - visualization layer does not know SQL dialect.
//!
//! This enables database portability (e.g., Postgres → Oracle migration).

use anyhow::Result;
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
    pub role_name: String,
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
    // CONTROL RELATIONSHIP QUERIES
    // =========================================================================

    /// Get control relationships for entities in a CBU
    pub async fn get_control_relationships(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<ControlRelationshipView>> {
        let controls = sqlx::query_as!(
            ControlRelationshipView,
            r#"SELECT cr.controller_entity_id, cr.controlled_entity_id, cr.control_type
               FROM "ob-poc".control_relationships cr
               WHERE cr.is_active = true
               AND (cr.controller_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               ) OR cr.controlled_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               ))"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(controls)
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

    /// Get attributes extracted from a document
    pub async fn get_document_attributes(
        &self,
        doc_id: Uuid,
    ) -> Result<Vec<DocumentAttributeView>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                dm.attribute_id,
                dm.value,
                d.name as attribute_name
            FROM "ob-poc".document_metadata dm
            JOIN "ob-poc".dictionary d ON d.attribute_id = dm.attribute_id
            WHERE dm.doc_id = $1
            ORDER BY d.name
            "#,
            doc_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DocumentAttributeView {
                attribute_id: r.attribute_id,
                attribute_name: r.attribute_name,
                value: r.value,
            })
            .collect())
    }

    // =========================================================================
    // SERVICE DELIVERY QUERIES
    // =========================================================================

    /// Get service delivery records for a CBU
    pub async fn get_service_deliveries(&self, cbu_id: Uuid) -> Result<Vec<ServiceDeliveryView>> {
        let rows = sqlx::query!(
            r#"SELECT
                sdm.delivery_id,
                sdm.product_id,
                p.name as "product_name!",
                sdm.service_id,
                s.name as "service_name!",
                sdm.instance_id,
                cri.instance_name as "instance_name?",
                srt.name as "resource_type_name?",
                sdm.delivery_status as "delivery_status?"
               FROM "ob-poc".service_delivery_map sdm
               JOIN "ob-poc".products p ON p.product_id = sdm.product_id
               JOIN "ob-poc".services s ON s.service_id = sdm.service_id
               LEFT JOIN "ob-poc".cbu_resource_instances cri ON cri.instance_id = sdm.instance_id
               LEFT JOIN "ob-poc".service_resource_types srt ON srt.resource_id = cri.resource_type_id
               WHERE sdm.cbu_id = $1
               ORDER BY p.name, s.name"#,
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
            r#"SELECT cbu_id, name, jurisdiction, client_type
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
        }))
    }

    /// List CBUs with optional search filter
    pub async fn list_cbus_filtered(
        &self,
        search: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CbuBasicView>> {
        let rows = sqlx::query!(
            r#"SELECT cbu_id, name, client_type, jurisdiction
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

    /// Get screenings for entities in a CBU
    pub async fn get_cbu_screenings(&self, cbu_id: Uuid) -> Result<Vec<CbuScreeningView>> {
        let rows = sqlx::query!(
            r#"SELECT s.screening_id, s.entity_id, s.screening_type, s.status, s.result
               FROM "ob-poc".screenings s
               WHERE s.entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )"#,
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
                status: r.status,
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

    /// Get documents linked to an entity
    pub async fn get_entity_documents(&self, entity_id: Uuid) -> Result<Vec<CbuDocumentView>> {
        let rows = sqlx::query!(
            r#"SELECT del.doc_id, dc.document_type_code, dc.status
               FROM "ob-poc".document_entity_links del
               JOIN "ob-poc".document_catalog dc ON del.doc_id = dc.doc_id
               WHERE del.entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|r| CbuDocumentView {
                doc_id: r.doc_id,
                document_type_code: r.document_type_code,
                status: r.status,
            })
            .collect())
    }

    /// Get screenings for an entity
    pub async fn get_entity_screenings(&self, entity_id: Uuid) -> Result<Vec<EntityScreeningView>> {
        let rows = sqlx::query!(
            r#"SELECT screening_id, screening_type, status, result
               FROM "ob-poc".screenings WHERE entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EntityScreeningView {
                screening_id: r.screening_id,
                screening_type: r.screening_type,
                status: r.status,
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
    pub async fn get_graph_entities(&self, cbu_id: Uuid) -> Result<Vec<GraphEntityView>> {
        let rows = sqlx::query!(
            r#"SELECT
                cer.cbu_entity_role_id,
                cer.entity_id,
                e.name as entity_name,
                et.name as entity_type,
                r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
               JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
               JOIN "ob-poc".roles r ON r.role_id = cer.role_id
               WHERE cer.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| GraphEntityView {
                cbu_entity_role_id: r.cbu_entity_role_id,
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                entity_type: r.entity_type,
                role_name: r.role_name,
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

    /// Get KYC statuses for entities in a CBU
    pub async fn get_kyc_statuses(&self, cbu_id: Uuid) -> Result<Vec<KycStatusView>> {
        let rows = sqlx::query!(
            r#"SELECT
                ks.status_id,
                ks.entity_id,
                ks.kyc_status,
                ks.risk_rating,
                ks.next_review_date,
                e.name as "entity_name?"
               FROM "ob-poc".entity_kyc_status ks
               JOIN "ob-poc".entities e ON e.entity_id = ks.entity_id
               WHERE ks.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| KycStatusView {
                status_id: r.status_id,
                entity_id: r.entity_id,
                kyc_status: r.kyc_status,
                risk_rating: r.risk_rating,
                next_review_date: r.next_review_date,
                entity_name: r.entity_name,
            })
            .collect())
    }

    /// Get document requests for a CBU (via investigations)
    pub async fn get_document_requests(&self, cbu_id: Uuid) -> Result<Vec<DocumentRequestView>> {
        let rows = sqlx::query!(
            r#"SELECT
                dr.request_id,
                dr.document_type,
                dr.status,
                dr.requested_from_entity_id,
                e.name as "entity_name?"
               FROM "ob-poc".document_requests dr
               JOIN "ob-poc".kyc_investigations ki ON ki.investigation_id = dr.investigation_id
               LEFT JOIN "ob-poc".entities e ON e.entity_id = dr.requested_from_entity_id
               WHERE ki.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DocumentRequestView {
                request_id: r.request_id,
                document_type: r.document_type,
                status: r.status,
                requested_from_entity_id: r.requested_from_entity_id,
                entity_name: r.entity_name,
            })
            .collect())
    }

    /// Get screenings for entities in a CBU (for graph)
    pub async fn get_graph_screenings(&self, cbu_id: Uuid) -> Result<Vec<ScreeningView>> {
        let rows = sqlx::query!(
            r#"SELECT
                s.screening_id,
                s.entity_id,
                s.screening_type,
                s.result,
                s.resolution,
                e.name as "entity_name?"
               FROM "ob-poc".screenings s
               JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = s.entity_id
               JOIN "ob-poc".entities e ON e.entity_id = s.entity_id
               WHERE cer.cbu_id = $1"#,
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

    /// Get UBO registry entries for a CBU
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

    /// Get ownership relationships for a CBU
    pub async fn get_ownerships(&self, cbu_id: Uuid) -> Result<Vec<OwnershipView>> {
        let rows = sqlx::query!(
            r#"SELECT
                o.ownership_id,
                o.owner_entity_id,
                o.owned_entity_id,
                o.ownership_type,
                o.ownership_percent,
                owner.name as "owner_name?",
                owned.name as "owned_name?"
               FROM "ob-poc".ownership_relationships o
               LEFT JOIN "ob-poc".entities owner ON owner.entity_id = o.owner_entity_id
               LEFT JOIN "ob-poc".entities owned ON owned.entity_id = o.owned_entity_id
               WHERE o.owned_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )
               OR o.owner_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )"#,
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

    /// Get control relationships for a CBU (for graph)
    pub async fn get_graph_controls(&self, cbu_id: Uuid) -> Result<Vec<ControlView>> {
        let rows = sqlx::query!(
            r#"SELECT
                c.control_id,
                c.controller_entity_id,
                c.controlled_entity_id,
                c.control_type,
                c.description,
                c.is_active,
                controller.name as "controller_name?",
                controlled.name as "controlled_name?"
               FROM "ob-poc".control_relationships c
               LEFT JOIN "ob-poc".entities controller ON controller.entity_id = c.controller_entity_id
               LEFT JOIN "ob-poc".entities controlled ON controlled.entity_id = c.controlled_entity_id
               WHERE c.is_active = true
                 AND (c.controlled_entity_id IN (
                       SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
                     )
                  OR c.controller_entity_id IN (
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
                description: r.description,
                is_active: r.is_active,
                controller_name: r.controller_name,
                controlled_name: r.controlled_name,
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
}
