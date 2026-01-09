//! Viewport Service - Database queries for CBU viewport rendering
//!
//! This module provides optimized queries for the viewport focus system,
//! including CBU containers, entity members with confidence scores,
//! and instrument matrix data.
//!
//! ## Design Philosophy
//!
//! Queries are designed for lazy loading based on focus/enhance transitions:
//! - L0: Basic CBU info + jurisdiction flag
//! - L1: Category counts
//! - L2: Entity members with roles and confidence
//! - Matrix: Instrument types and configuration

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// ============================================================================
// CBU VIEWPORT TYPES
// ============================================================================

/// CBU container data for viewport rendering (L0-L1)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuViewportContainer {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Category counts for CBU at L1 enhance level
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CbuCategoryCounts {
    pub entity_count: i64,
    pub company_count: i64,
    pub person_count: i64,
    pub trust_count: i64,
    pub partnership_count: i64,
    pub product_count: i64,
    pub service_count: i64,
    pub document_count: i64,
}

/// Entity member with role and confidence for viewport rendering (L2+)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuEntityMember {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: String,
    pub entity_type_code: Option<String>,
    pub role_id: Uuid,
    pub role_name: String,
    pub jurisdiction: Option<String>,
    /// Confidence score 0.0-1.0 (defaults to 1.0 if not set)
    pub confidence: f64,
    pub created_at: Option<DateTime<Utc>>,
}

impl CbuEntityMember {
    /// Get confidence zone based on score
    pub fn confidence_zone(&self) -> ConfidenceZone {
        match self.confidence {
            x if x >= 0.95 => ConfidenceZone::Core,
            x if x >= 0.70 => ConfidenceZone::Shell,
            x if x >= 0.40 => ConfidenceZone::Penumbra,
            _ => ConfidenceZone::Speculative,
        }
    }
}

/// Confidence zone for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceZone {
    Core,        // >= 0.95
    Shell,       // >= 0.70
    Penumbra,    // >= 0.40
    Speculative, // < 0.40
}

impl ConfidenceZone {
    /// Create zone from confidence score
    pub fn from_score(score: f64) -> Self {
        match score {
            x if x >= 0.95 => ConfidenceZone::Core,
            x if x >= 0.70 => ConfidenceZone::Shell,
            x if x >= 0.40 => ConfidenceZone::Penumbra,
            _ => ConfidenceZone::Speculative,
        }
    }

    /// Get minimum confidence for this zone
    pub fn min_confidence(&self) -> f64 {
        match self {
            ConfidenceZone::Core => 0.95,
            ConfidenceZone::Shell => 0.70,
            ConfidenceZone::Penumbra => 0.40,
            ConfidenceZone::Speculative => 0.0,
        }
    }
}

// ============================================================================
// INSTRUMENT MATRIX TYPES
// ============================================================================

/// Instrument matrix summary for viewport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentMatrixSummary {
    pub profile_id: Uuid,
    pub cbu_id: Uuid,
    pub version: i32,
    pub status: String,
    pub instrument_type_count: i64,
    pub ssi_count: i64,
    pub booking_rule_count: i64,
}

/// Instrument type node for matrix rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentTypeNode {
    pub node_id: String,
    pub instrument_type: String,
    pub label: String,
    pub enabled: bool,
    pub mic_count: i64,
    pub bic_count: i64,
    pub has_pricing: bool,
    pub has_restrictions: bool,
}

// ============================================================================
// VIEWPORT SERVICE
// ============================================================================

/// Service for viewport database queries
#[derive(Clone, Debug)]
pub struct ViewportService {
    pool: PgPool,
}

impl ViewportService {
    /// Create a new viewport service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ========================================================================
    // CBU CONTAINER QUERIES (L0-L1)
    // ========================================================================

    /// Get CBU container data for viewport (L0)
    pub async fn get_cbu_container(&self, cbu_id: Uuid) -> Result<Option<CbuViewportContainer>> {
        let result = sqlx::query_as::<_, CbuViewportContainer>(
            r#"
            SELECT
                cbu_id,
                name,
                jurisdiction,
                client_type,
                description,
                created_at
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CBU container")?;

        Ok(result)
    }

    /// Get category counts for CBU (L1)
    pub async fn get_cbu_category_counts(&self, cbu_id: Uuid) -> Result<CbuCategoryCounts> {
        // Get entity counts by type
        let entity_counts = sqlx::query!(
            r#"
            SELECT
                COUNT(DISTINCT cer.entity_id) as total_entities,
                COUNT(DISTINCT CASE WHEN et.type_code = 'limited_company' THEN e.entity_id END) as companies,
                COUNT(DISTINCT CASE WHEN et.type_code = 'proper_person' THEN e.entity_id END) as persons,
                COUNT(DISTINCT CASE WHEN et.type_code = 'trust_discretionary' THEN e.entity_id END) as trusts,
                COUNT(DISTINCT CASE WHEN et.type_code = 'partnership_limited' THEN e.entity_id END) as partnerships
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE cer.cbu_id = $1
            "#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get entity counts")?;

        // Get product count
        let product_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM "ob-poc".cbu_product_subscriptions cp
            WHERE cp.cbu_id = $1 AND cp.status = 'ACTIVE'
            "#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        // Get service count
        let service_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM "ob-poc".service_delivery_map sdm
            WHERE sdm.cbu_id = $1
            "#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        // Get document count
        let document_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM "ob-poc".document_catalog dc
            WHERE dc.cbu_id = $1
            "#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        Ok(CbuCategoryCounts {
            entity_count: entity_counts.total_entities.unwrap_or(0),
            company_count: entity_counts.companies.unwrap_or(0),
            person_count: entity_counts.persons.unwrap_or(0),
            trust_count: entity_counts.trusts.unwrap_or(0),
            partnership_count: entity_counts.partnerships.unwrap_or(0),
            product_count,
            service_count,
            document_count,
        })
    }

    // ========================================================================
    // CBU ENTITY MEMBER QUERIES (L2+)
    // ========================================================================

    /// Get entity members for CBU with confidence filtering (L2+)
    ///
    /// Returns entities with their roles, ordered by confidence descending.
    /// Confidence defaults to 1.0 for entities without explicit scores.
    pub async fn get_cbu_entity_members(
        &self,
        cbu_id: Uuid,
        min_confidence: Option<f64>,
    ) -> Result<Vec<CbuEntityMember>> {
        let threshold = min_confidence.unwrap_or(0.0);

        let rows = sqlx::query!(
            r#"
            SELECT
                e.entity_id,
                e.name as entity_name,
                et.name as entity_type,
                et.type_code as entity_type_code,
                r.role_id,
                r.name as role_name,
                COALESCE(lc.jurisdiction, pp.nationality) as jurisdiction,
                COALESCE(cer.ownership_percentage::float / 100.0, 1.0) as "confidence!: f64",
                cer.created_at
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
            WHERE cer.cbu_id = $1
              AND COALESCE(cer.ownership_percentage::float / 100.0, 1.0) >= $2
            ORDER BY COALESCE(cer.ownership_percentage::float / 100.0, 1.0) DESC, r.name, e.name
            "#,
            cbu_id,
            threshold
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CBU entity members")?;

        Ok(rows
            .into_iter()
            .map(|row| CbuEntityMember {
                entity_id: row.entity_id,
                entity_name: row.entity_name,
                entity_type: row.entity_type,
                entity_type_code: row.entity_type_code,
                role_id: row.role_id,
                role_name: row.role_name,
                jurisdiction: row.jurisdiction,
                confidence: row.confidence,
                created_at: row.created_at,
            })
            .collect())
    }

    /// Get entity members by confidence zone
    pub async fn get_members_by_zone(
        &self,
        cbu_id: Uuid,
        zone: ConfidenceZone,
    ) -> Result<Vec<CbuEntityMember>> {
        let (min, max) = match zone {
            ConfidenceZone::Core => (0.95, 1.01),
            ConfidenceZone::Shell => (0.70, 0.95),
            ConfidenceZone::Penumbra => (0.40, 0.70),
            ConfidenceZone::Speculative => (0.0, 0.40),
        };

        let all_members = self.get_cbu_entity_members(cbu_id, Some(min)).await?;
        Ok(all_members
            .into_iter()
            .filter(|m| m.confidence < max)
            .collect())
    }

    // ========================================================================
    // INSTRUMENT MATRIX QUERIES
    // ========================================================================

    /// Get instrument matrix summary for CBU
    pub async fn get_instrument_matrix_summary(
        &self,
        cbu_id: Uuid,
    ) -> Result<Option<InstrumentMatrixSummary>> {
        // Get active trading profile
        let profile = sqlx::query!(
            r#"
            SELECT profile_id, cbu_id, version, status
            FROM "ob-poc".cbu_trading_profiles
            WHERE cbu_id = $1 AND status = 'ACTIVE'
            ORDER BY version DESC
            LIMIT 1
            "#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get trading profile")?;

        let Some(profile) = profile else {
            return Ok(None);
        };

        // Count instrument types from universe
        let instrument_type_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT instrument_class_id) as "count!"
            FROM custody.cbu_instrument_universe
            WHERE cbu_id = $1 AND is_active = true
            "#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        // Count SSIs
        let ssi_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM custody.cbu_ssi
            WHERE cbu_id = $1 AND status = 'ACTIVE'
            "#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        // Count booking rules
        let booking_rule_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM custody.ssi_booking_rules
            WHERE cbu_id = $1 AND is_active = true
            "#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        Ok(Some(InstrumentMatrixSummary {
            profile_id: profile.profile_id,
            cbu_id: profile.cbu_id,
            version: profile.version,
            status: profile.status,
            instrument_type_count,
            ssi_count,
            booking_rule_count,
        }))
    }

    /// Get instrument type nodes for matrix rendering
    pub async fn get_instrument_type_nodes(&self, cbu_id: Uuid) -> Result<Vec<InstrumentTypeNode>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                ic.class_id,
                ic.code,
                ic.name,
                COUNT(DISTINCT ciu.market_id) as "mic_count!: i64",
                COUNT(DISTINCT sbr.ssi_id) as "bic_count!: i64",
                bool_or(ciu.is_traded) as is_traded
            FROM custody.instrument_classes ic
            LEFT JOIN custody.cbu_instrument_universe ciu
                ON ic.class_id = ciu.instrument_class_id AND ciu.cbu_id = $1 AND ciu.is_active = true
            LEFT JOIN custody.ssi_booking_rules sbr
                ON ic.class_id = sbr.instrument_class_id AND sbr.cbu_id = $1 AND sbr.is_active = true
            GROUP BY ic.class_id, ic.code, ic.name
            HAVING COUNT(DISTINCT ciu.universe_id) > 0
            ORDER BY ic.code
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get instrument type nodes")?;

        Ok(rows
            .into_iter()
            .map(|row| InstrumentTypeNode {
                node_id: row.class_id.to_string(),
                instrument_type: row.code.clone(),
                label: row.name,
                enabled: row.is_traded.unwrap_or(false),
                mic_count: row.mic_count,
                bic_count: row.bic_count,
                has_pricing: false,      // TODO: Add pricing config check
                has_restrictions: false, // TODO: Add restrictions check
            })
            .collect())
    }

    // ========================================================================
    // ENTITY DETAIL QUERIES (L3+)
    // ========================================================================

    /// Get detailed entity info for focused entity
    pub async fn get_entity_detail(&self, entity_id: Uuid) -> Result<Option<EntityViewportDetail>> {
        let row = sqlx::query!(
            r#"
            SELECT
                e.entity_id,
                e.name,
                et.name as entity_type,
                et.type_code,
                lc.company_name as "company_name?",
                lc.registration_number as "registration_number?",
                lc.jurisdiction as "company_jurisdiction?",
                lc.incorporation_date as "incorporation_date?",
                pp.first_name as "first_name?",
                pp.last_name as "last_name?",
                pp.date_of_birth as "date_of_birth?",
                pp.nationality as "nationality?"
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
            WHERE e.entity_id = $1
            "#,
            entity_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get entity detail")?;

        let Some(row) = row else {
            return Ok(None);
        };

        Ok(Some(EntityViewportDetail {
            entity_id: row.entity_id,
            name: row.name,
            entity_type: row.entity_type,
            type_code: row.type_code,
            company_name: row.company_name,
            registration_number: row.registration_number,
            jurisdiction: row.company_jurisdiction.or(row.nationality.clone()),
            incorporation_date: row.incorporation_date,
            first_name: row.first_name,
            last_name: row.last_name,
            date_of_birth: row.date_of_birth,
            nationality: row.nationality,
        }))
    }

    /// Get entity relationships for 1-hop expansion
    pub async fn get_entity_relationships(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityRelationship>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                er.relationship_id,
                er.from_entity_id,
                e_from.name as from_name,
                er.to_entity_id,
                e_to.name as to_name,
                er.relationship_type,
                er.percentage
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".entities e_from ON er.from_entity_id = e_from.entity_id
            JOIN "ob-poc".entities e_to ON er.to_entity_id = e_to.entity_id
            WHERE er.from_entity_id = $1 OR er.to_entity_id = $1
              AND er.effective_to IS NULL
            ORDER BY er.relationship_type, er.percentage DESC NULLS LAST
            "#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get entity relationships")?;

        Ok(rows
            .into_iter()
            .map(|row| EntityRelationship {
                relationship_id: row.relationship_id,
                from_entity_id: row.from_entity_id,
                from_name: row.from_name,
                to_entity_id: row.to_entity_id,
                to_name: row.to_name,
                relationship_type: row.relationship_type,
                percentage: row.percentage.map(|p| p.to_string().parse().unwrap_or(0.0)),
            })
            .collect())
    }
}

// ============================================================================
// ADDITIONAL TYPES
// ============================================================================

/// Detailed entity information for focused view (L3+)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityViewportDetail {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub type_code: Option<String>,
    // Company-specific
    pub company_name: Option<String>,
    pub registration_number: Option<String>,
    pub jurisdiction: Option<String>,
    pub incorporation_date: Option<chrono::NaiveDate>,
    // Person-specific
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub nationality: Option<String>,
}

/// Entity relationship for graph expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRelationship {
    pub relationship_id: Uuid,
    pub from_entity_id: Uuid,
    pub from_name: String,
    pub to_entity_id: Uuid,
    pub to_name: String,
    pub relationship_type: String,
    pub percentage: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // CONFIDENCE ZONE TESTS
    // ========================================================================

    #[test]
    fn test_confidence_zone_from_score() {
        // Core zone: >= 0.95
        assert_eq!(ConfidenceZone::from_score(0.95), ConfidenceZone::Core);
        assert_eq!(ConfidenceZone::from_score(0.99), ConfidenceZone::Core);
        assert_eq!(ConfidenceZone::from_score(1.0), ConfidenceZone::Core);

        // Shell zone: >= 0.70, < 0.95
        assert_eq!(ConfidenceZone::from_score(0.70), ConfidenceZone::Shell);
        assert_eq!(ConfidenceZone::from_score(0.80), ConfidenceZone::Shell);
        assert_eq!(ConfidenceZone::from_score(0.94), ConfidenceZone::Shell);

        // Penumbra zone: >= 0.40, < 0.70
        assert_eq!(ConfidenceZone::from_score(0.40), ConfidenceZone::Penumbra);
        assert_eq!(ConfidenceZone::from_score(0.55), ConfidenceZone::Penumbra);
        assert_eq!(ConfidenceZone::from_score(0.69), ConfidenceZone::Penumbra);

        // Speculative zone: < 0.40
        assert_eq!(ConfidenceZone::from_score(0.0), ConfidenceZone::Speculative);
        assert_eq!(
            ConfidenceZone::from_score(0.20),
            ConfidenceZone::Speculative
        );
        assert_eq!(
            ConfidenceZone::from_score(0.39),
            ConfidenceZone::Speculative
        );
    }

    #[test]
    fn test_confidence_zone_min_confidence() {
        assert_eq!(ConfidenceZone::Core.min_confidence(), 0.95);
        assert_eq!(ConfidenceZone::Shell.min_confidence(), 0.70);
        assert_eq!(ConfidenceZone::Penumbra.min_confidence(), 0.40);
        assert_eq!(ConfidenceZone::Speculative.min_confidence(), 0.0);
    }

    #[test]
    fn test_confidence_zone_boundaries() {
        // Test exact boundary values
        assert_eq!(ConfidenceZone::from_score(0.949999), ConfidenceZone::Shell);
        assert_eq!(
            ConfidenceZone::from_score(0.699999),
            ConfidenceZone::Penumbra
        );
        assert_eq!(
            ConfidenceZone::from_score(0.399999),
            ConfidenceZone::Speculative
        );
    }

    // ========================================================================
    // ENTITY MEMBER CONFIDENCE ZONE TESTS
    // ========================================================================

    #[test]
    fn test_entity_member_confidence_zones() {
        let member = CbuEntityMember {
            entity_id: Uuid::new_v4(),
            entity_name: "Test".to_string(),
            entity_type: "Company".to_string(),
            entity_type_code: Some("limited_company".to_string()),
            role_id: Uuid::new_v4(),
            role_name: "Director".to_string(),
            jurisdiction: Some("US".to_string()),
            confidence: 0.98,
            created_at: None,
        };
        assert_eq!(member.confidence_zone(), ConfidenceZone::Core);

        let member2 = CbuEntityMember {
            confidence: 0.75,
            ..member.clone()
        };
        assert_eq!(member2.confidence_zone(), ConfidenceZone::Shell);

        let member3 = CbuEntityMember {
            confidence: 0.50,
            ..member.clone()
        };
        assert_eq!(member3.confidence_zone(), ConfidenceZone::Penumbra);

        let member4 = CbuEntityMember {
            confidence: 0.20,
            ..member
        };
        assert_eq!(member4.confidence_zone(), ConfidenceZone::Speculative);
    }

    // ========================================================================
    // TYPE CONSTRUCTION TESTS
    // ========================================================================

    #[test]
    fn test_cbu_container_creation() {
        let container = CbuViewportContainer {
            cbu_id: Uuid::new_v4(),
            name: "Luxembourg Growth Fund".to_string(),
            jurisdiction: Some("LU".to_string()),
            client_type: Some("FUND".to_string()),
            description: Some("A growth-focused investment fund".to_string()),
            created_at: Some(chrono::Utc::now()),
        };

        assert_eq!(container.jurisdiction, Some("LU".to_string()));
        assert_eq!(container.client_type, Some("FUND".to_string()));
        assert!(container.description.is_some());
    }

    #[test]
    fn test_category_counts_default() {
        let counts = CbuCategoryCounts {
            entity_count: 10,
            company_count: 3,
            person_count: 5,
            trust_count: 1,
            partnership_count: 1,
            product_count: 2,
            service_count: 8,
            document_count: 15,
        };

        assert_eq!(counts.entity_count, 10);
        assert_eq!(
            counts.company_count
                + counts.person_count
                + counts.trust_count
                + counts.partnership_count,
            10
        );
    }

    #[test]
    fn test_instrument_type_node() {
        let node = InstrumentTypeNode {
            node_id: "class-001".to_string(),
            instrument_type: "EQUITY".to_string(),
            label: "Equity Securities".to_string(),
            enabled: true,
            mic_count: 5,
            bic_count: 3,
            has_pricing: true,
            has_restrictions: false,
        };

        assert_eq!(node.instrument_type, "EQUITY");
        assert!(node.enabled);
        assert_eq!(node.mic_count, 5);
    }

    #[test]
    fn test_matrix_summary() {
        let summary = InstrumentMatrixSummary {
            profile_id: Uuid::new_v4(),
            cbu_id: Uuid::new_v4(),
            version: 3,
            status: "ACTIVE".to_string(),
            instrument_type_count: 5,
            ssi_count: 12,
            booking_rule_count: 18,
        };

        assert_eq!(summary.status, "ACTIVE");
        assert_eq!(summary.version, 3);
        assert!(summary.instrument_type_count > 0);
    }

    #[test]
    fn test_entity_detail_company() {
        let detail = EntityViewportDetail {
            entity_id: Uuid::new_v4(),
            name: "Acme Corp".to_string(),
            entity_type: "LIMITED_COMPANY".to_string(),
            type_code: Some("limited_company".to_string()),
            company_name: Some("Acme Corporation Ltd".to_string()),
            registration_number: Some("B123456".to_string()),
            jurisdiction: Some("LU".to_string()),
            incorporation_date: Some(chrono::NaiveDate::from_ymd_opt(2020, 1, 15).unwrap()),
            first_name: None,
            last_name: None,
            date_of_birth: None,
            nationality: None,
        };

        assert!(detail.company_name.is_some());
        assert!(detail.first_name.is_none());
        assert_eq!(detail.entity_type, "LIMITED_COMPANY");
    }

    #[test]
    fn test_entity_detail_person() {
        let detail = EntityViewportDetail {
            entity_id: Uuid::new_v4(),
            name: "John Smith".to_string(),
            entity_type: "PROPER_PERSON".to_string(),
            type_code: Some("proper_person".to_string()),
            company_name: None,
            registration_number: None,
            jurisdiction: Some("GB".to_string()),
            incorporation_date: None,
            first_name: Some("John".to_string()),
            last_name: Some("Smith".to_string()),
            date_of_birth: Some(chrono::NaiveDate::from_ymd_opt(1985, 6, 20).unwrap()),
            nationality: Some("GB".to_string()),
        };

        assert!(detail.first_name.is_some());
        assert!(detail.company_name.is_none());
        assert_eq!(detail.entity_type, "PROPER_PERSON");
    }
}
