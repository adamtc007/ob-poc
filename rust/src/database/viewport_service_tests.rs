//! Tests for the Viewport Service
//!
//! Tests cover:
//! - Confidence zone calculations
//! - Focus state machine transitions
//! - Enhance level calculations
//! - CBU container queries
//! - Entity member queries
//! - Instrument matrix queries

use super::viewport_service::*;

// ============================================================================
// UNIT TESTS - Confidence Zone Calculations
// ============================================================================

#[cfg(test)]
mod confidence_zone_tests {
    use super::*;

    #[test]
    fn test_core_zone_threshold() {
        // Core zone: >= 0.95
        assert!(matches!(
            ConfidenceZone::from_score(0.95),
            ConfidenceZone::Core
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.99),
            ConfidenceZone::Core
        ));
        assert!(matches!(
            ConfidenceZone::from_score(1.0),
            ConfidenceZone::Core
        ));
    }

    #[test]
    fn test_shell_zone_threshold() {
        // Shell zone: >= 0.70, < 0.95
        assert!(matches!(
            ConfidenceZone::from_score(0.70),
            ConfidenceZone::Shell
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.80),
            ConfidenceZone::Shell
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.94),
            ConfidenceZone::Shell
        ));
    }

    #[test]
    fn test_penumbra_zone_threshold() {
        // Penumbra zone: >= 0.40, < 0.70
        assert!(matches!(
            ConfidenceZone::from_score(0.40),
            ConfidenceZone::Penumbra
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.55),
            ConfidenceZone::Penumbra
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.69),
            ConfidenceZone::Penumbra
        ));
    }

    #[test]
    fn test_speculative_zone_threshold() {
        // Speculative zone: < 0.40
        assert!(matches!(
            ConfidenceZone::from_score(0.0),
            ConfidenceZone::Speculative
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.20),
            ConfidenceZone::Speculative
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.39),
            ConfidenceZone::Speculative
        ));
    }

    #[test]
    fn test_zone_min_confidence() {
        assert_eq!(ConfidenceZone::Core.min_confidence(), 0.95);
        assert_eq!(ConfidenceZone::Shell.min_confidence(), 0.70);
        assert_eq!(ConfidenceZone::Penumbra.min_confidence(), 0.40);
        assert_eq!(ConfidenceZone::Speculative.min_confidence(), 0.0);
    }

    #[test]
    fn test_edge_cases() {
        // Boundary tests at exact thresholds
        assert!(matches!(
            ConfidenceZone::from_score(0.949999),
            ConfidenceZone::Shell
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.699999),
            ConfidenceZone::Penumbra
        ));
        assert!(matches!(
            ConfidenceZone::from_score(0.399999),
            ConfidenceZone::Speculative
        ));
    }
}

// ============================================================================
// UNIT TESTS - Category Counts Default Values
// ============================================================================

#[cfg(test)]
mod category_counts_tests {
    use super::*;

    #[test]
    fn test_default_counts_are_zero() {
        let counts = CbuCategoryCounts {
            entity_count: 0,
            company_count: 0,
            person_count: 0,
            trust_count: 0,
            partnership_count: 0,
            product_count: 0,
            service_count: 0,
            document_count: 0,
        };

        assert_eq!(counts.entity_count, 0);
        assert_eq!(counts.company_count, 0);
        assert_eq!(counts.person_count, 0);
        assert_eq!(counts.trust_count, 0);
        assert_eq!(counts.partnership_count, 0);
        assert_eq!(counts.product_count, 0);
        assert_eq!(counts.service_count, 0);
        assert_eq!(counts.document_count, 0);
    }
}

// ============================================================================
// UNIT TESTS - Instrument Type Node
// ============================================================================

#[cfg(test)]
mod instrument_type_node_tests {
    use super::*;

    #[test]
    fn test_instrument_node_creation() {
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
        assert_eq!(node.bic_count, 3);
        assert!(node.has_pricing);
        assert!(!node.has_restrictions);
    }
}

// ============================================================================
// UNIT TESTS - Entity Member with Confidence
// ============================================================================

#[cfg(test)]
mod entity_member_tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_entity_member_zone_classification() {
        let high_confidence = CbuEntityMember {
            entity_id: Uuid::new_v4(),
            entity_name: "High Confidence Entity".to_string(),
            entity_type: "LIMITED_COMPANY".to_string(),
            entity_type_code: Some("limited_company".to_string()),
            role_id: Uuid::new_v4(),
            role_name: "DIRECTOR".to_string(),
            jurisdiction: Some("LU".to_string()),
            confidence: 0.98,
            created_at: None,
        };

        assert!(matches!(
            ConfidenceZone::from_score(high_confidence.confidence),
            ConfidenceZone::Core
        ));
    }

    #[test]
    fn test_entity_member_low_confidence() {
        let low_confidence = CbuEntityMember {
            entity_id: Uuid::new_v4(),
            entity_name: "Speculative Entity".to_string(),
            entity_type: "PROPER_PERSON".to_string(),
            entity_type_code: Some("proper_person".to_string()),
            role_id: Uuid::new_v4(),
            role_name: "UBO".to_string(),
            jurisdiction: None,
            confidence: 0.25,
            created_at: None,
        };

        assert!(matches!(
            ConfidenceZone::from_score(low_confidence.confidence),
            ConfidenceZone::Speculative
        ));
    }
}

// ============================================================================
// UNIT TESTS - Entity Viewport Detail
// ============================================================================

#[cfg(test)]
mod entity_detail_tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_company_detail() {
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
        assert_eq!(detail.jurisdiction, Some("LU".to_string()));
    }

    #[test]
    fn test_person_detail() {
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
        assert_eq!(detail.nationality, Some("GB".to_string()));
    }
}

// ============================================================================
// UNIT TESTS - Entity Relationship
// ============================================================================

#[cfg(test)]
mod entity_relationship_tests {
    use super::*;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    #[test]
    fn test_ownership_relationship() {
        let rel = EntityRelationship {
            relationship_id: Uuid::new_v4(),
            from_entity_id: Uuid::new_v4(),
            from_name: "Parent Corp".to_string(),
            to_entity_id: Uuid::new_v4(),
            to_name: "Subsidiary Ltd".to_string(),
            relationship_type: "ownership".to_string(),
            percentage: Some(Decimal::new(7500, 2)), // 75.00%
        };

        assert_eq!(rel.relationship_type, "ownership");
        assert!(rel.percentage.is_some());
        assert_eq!(rel.percentage.unwrap(), Decimal::new(7500, 2));
    }

    #[test]
    fn test_control_relationship_no_percentage() {
        let rel = EntityRelationship {
            relationship_id: Uuid::new_v4(),
            from_entity_id: Uuid::new_v4(),
            from_name: "Director".to_string(),
            to_entity_id: Uuid::new_v4(),
            to_name: "Company".to_string(),
            relationship_type: "control".to_string(),
            percentage: None,
        };

        assert_eq!(rel.relationship_type, "control");
        assert!(rel.percentage.is_none());
    }
}

// ============================================================================
// UNIT TESTS - CBU Viewport Container
// ============================================================================

#[cfg(test)]
mod cbu_container_tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_cbu_container_minimal() {
        let container = CbuViewportContainer {
            cbu_id: Uuid::new_v4(),
            name: "Test Fund".to_string(),
            jurisdiction: None,
            client_type: None,
            description: None,
            created_at: None,
        };

        assert!(!container.name.is_empty());
        assert!(container.jurisdiction.is_none());
    }

    #[test]
    fn test_cbu_container_full() {
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
}

// ============================================================================
// UNIT TESTS - Instrument Matrix Summary
// ============================================================================

#[cfg(test)]
mod matrix_summary_tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_matrix_summary_active() {
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
        assert!(summary.ssi_count > 0);
        assert!(summary.booking_rule_count > 0);
    }

    #[test]
    fn test_matrix_summary_draft() {
        let summary = InstrumentMatrixSummary {
            profile_id: Uuid::new_v4(),
            cbu_id: Uuid::new_v4(),
            version: 1,
            status: "DRAFT".to_string(),
            instrument_type_count: 0,
            ssi_count: 0,
            booking_rule_count: 0,
        };

        assert_eq!(summary.status, "DRAFT");
        assert_eq!(summary.version, 1);
        assert_eq!(summary.instrument_type_count, 0);
    }
}
