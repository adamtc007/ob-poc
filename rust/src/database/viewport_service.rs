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
pub(crate) struct CbuViewportContainer {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Category counts for CBU at L1 enhance level
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct CbuCategoryCounts {
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
pub(crate) struct CbuEntityMember {
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
    pub(crate) fn confidence_zone(&self) -> ConfidenceZone {
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
pub(crate) enum ConfidenceZone {
    Core,        // >= 0.95
    Shell,       // >= 0.70
    Penumbra,    // >= 0.40
    Speculative, // < 0.40
}

impl ConfidenceZone {
    /// Create zone from confidence score
    pub(crate) fn from_score(score: f64) -> Self {
        match score {
            x if x >= 0.95 => ConfidenceZone::Core,
            x if x >= 0.70 => ConfidenceZone::Shell,
            x if x >= 0.40 => ConfidenceZone::Penumbra,
            _ => ConfidenceZone::Speculative,
        }
    }

    /// Get minimum confidence for this zone
    pub(crate) fn min_confidence(&self) -> f64 {
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
pub(crate) struct InstrumentMatrixSummary {
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
pub(crate) struct InstrumentTypeNode {
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
pub(crate) struct ViewportService {
    pool: PgPool,
}

impl ViewportService {
    /// Create a new viewport service
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ========================================================================
    // CBU CONTAINER QUERIES (L0-L1)
    // ========================================================================



    // ========================================================================
    // CBU ENTITY MEMBER QUERIES (L2+)
    // ========================================================================

    /// Get entity members for CBU with confidence filtering (L2+)
    ///
    /// Returns entities with their roles, ordered by confidence descending.
    /// Confidence defaults to 1.0 for entities without explicit scores.
    pub(crate) async fn get_cbu_entity_members(
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
              AND e.deleted_at IS NULL
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


    // ========================================================================
    // INSTRUMENT MATRIX QUERIES
    // ========================================================================



    // ========================================================================
    // ENTITY DETAIL QUERIES (L3+)
    // ========================================================================


}

// ============================================================================
// ADDITIONAL TYPES
// ============================================================================

/// Detailed entity information for focused view (L3+)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EntityViewportDetail {
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
pub(crate) struct EntityRelationship {
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
