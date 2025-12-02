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
}
