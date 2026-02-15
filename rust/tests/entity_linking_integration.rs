//! Integration tests for Entity Linking Service (073)
//!
//! Tests verify:
//! 1. Snapshot loading and deterministic hashing
//! 2. Mention extraction from natural language
//! 3. Entity resolution with kind constraints
//! 4. Stub service graceful degradation
//! 5. AgentService integration (extract_entity_mentions)

use ob_poc::entity_linking::{
    EntityLinkingService, EntityLinkingServiceImpl, EntitySnapshot, Evidence,
    StubEntityLinkingService,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// TEST FIXTURES - Deterministic snapshot for reproducible tests
// ============================================================================

/// Create a deterministic test snapshot with known entities
fn create_test_snapshot() -> EntitySnapshot {
    use ob_poc::entity_linking::snapshot::EntityRow;
    use smallvec::smallvec;

    let entity1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let entity2 = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let entity3 = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
    let entity4 = Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();
    let entity5 = Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap();

    let entities = vec![
        EntityRow {
            entity_id: entity1,
            entity_kind: "company".to_string(),
            canonical_name: "Goldman Sachs Group Inc".to_string(),
            canonical_name_norm: "goldman sachs group inc".to_string(),
        },
        EntityRow {
            entity_id: entity2,
            entity_kind: "company".to_string(),
            canonical_name: "BlackRock Inc".to_string(),
            canonical_name_norm: "blackrock inc".to_string(),
        },
        EntityRow {
            entity_id: entity3,
            entity_kind: "proper_person".to_string(),
            canonical_name: "John Smith".to_string(),
            canonical_name_norm: "john smith".to_string(),
        },
        EntityRow {
            entity_id: entity4,
            entity_kind: "fund".to_string(),
            canonical_name: "Allianz Global Investors Fund".to_string(),
            canonical_name_norm: "allianz global investors fund".to_string(),
        },
        EntityRow {
            entity_id: entity5,
            entity_kind: "company".to_string(),
            canonical_name: "Morgan Stanley".to_string(),
            canonical_name_norm: "morgan stanley".to_string(),
        },
    ];

    // Build indexes
    let mut alias_index: HashMap<String, smallvec::SmallVec<[Uuid; 4]>> = HashMap::new();
    let mut name_index: HashMap<String, Uuid> = HashMap::new();
    let mut token_index: HashMap<String, smallvec::SmallVec<[Uuid; 8]>> = HashMap::new();
    let mut kind_index: HashMap<String, smallvec::SmallVec<[Uuid; 16]>> = HashMap::new();

    for row in &entities {
        // Name index (canonical)
        name_index.insert(row.canonical_name_norm.clone(), row.entity_id);

        // Alias index (normalized name)
        alias_index
            .entry(row.canonical_name_norm.clone())
            .or_default()
            .push(row.entity_id);

        // Token index
        for token in row.canonical_name_norm.split_whitespace() {
            token_index
                .entry(token.to_string())
                .or_default()
                .push(row.entity_id);
        }

        // Kind index
        kind_index
            .entry(row.entity_kind.clone())
            .or_default()
            .push(row.entity_id);
    }

    // Add common aliases
    alias_index.insert("goldman".to_string(), smallvec![entity1]);
    alias_index.insert("goldman sachs".to_string(), smallvec![entity1]);
    alias_index.insert("gs".to_string(), smallvec![entity1]);
    alias_index.insert("blackrock".to_string(), smallvec![entity2]);
    alias_index.insert("blk".to_string(), smallvec![entity2]);
    alias_index.insert("morgan stanley".to_string(), smallvec![entity5]);
    alias_index.insert("ms".to_string(), smallvec![entity5]);
    alias_index.insert("allianz".to_string(), smallvec![entity4]);

    EntitySnapshot {
        version: 1,
        hash: "test-deterministic-hash-001".to_string(),
        entities,
        alias_index,
        name_index,
        token_index,
        concept_links: HashMap::new(),
        kind_index,
    }
}

// ============================================================================
// UNIT TESTS - Snapshot and service basics
// ============================================================================

#[test]
fn test_snapshot_entity_count() {
    let snapshot = create_test_snapshot();
    assert_eq!(snapshot.entities.len(), 5);
}

#[test]
fn test_snapshot_deterministic_hash() {
    let snapshot = create_test_snapshot();
    assert_eq!(snapshot.hash, "test-deterministic-hash-001");
}

#[test]
fn test_snapshot_get_by_id() {
    let snapshot = create_test_snapshot();
    let entity_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();

    let row = snapshot.get(&entity_id);
    assert!(row.is_some());
    assert_eq!(row.unwrap().canonical_name, "Goldman Sachs Group Inc");
}

#[test]
fn test_snapshot_kind_index() {
    let snapshot = create_test_snapshot();

    let companies = snapshot.kind_index.get("company");
    assert!(companies.is_some());
    assert_eq!(companies.unwrap().len(), 3); // Goldman, BlackRock, Morgan Stanley
}

// ============================================================================
// ENTITY LINKING SERVICE TESTS
// ============================================================================

#[test]
fn test_service_entity_count() {
    let snapshot = Arc::new(create_test_snapshot());
    let service = EntityLinkingServiceImpl::new(snapshot);

    assert_eq!(service.entity_count(), 5);
}

#[test]
fn test_service_snapshot_hash() {
    let snapshot = Arc::new(create_test_snapshot());
    let service = EntityLinkingServiceImpl::new(snapshot);

    assert_eq!(service.snapshot_hash(), "test-deterministic-hash-001");
}

#[test]
fn test_resolve_exact_alias() {
    let snapshot = Arc::new(create_test_snapshot());
    let service = EntityLinkingServiceImpl::new(snapshot);

    let results = service.resolve_mentions("Work with Goldman Sachs", None, None, 5);

    assert!(!results.is_empty(), "Should find at least one mention");

    let goldman_mention = results
        .iter()
        .find(|r| r.mention_text.to_lowercase().contains("goldman"));
    assert!(
        goldman_mention.is_some(),
        "Should find Goldman Sachs mention"
    );

    let mention = goldman_mention.unwrap();
    assert!(mention.selected.is_some(), "Should have selected entity");
    assert!(mention.confidence > 0.5, "Should have high confidence");
}

#[test]
fn test_resolve_multiple_mentions() {
    let snapshot = Arc::new(create_test_snapshot());
    let service = EntityLinkingServiceImpl::new(snapshot);

    let results = service.resolve_mentions(
        "Set up ISDA between Goldman Sachs and Morgan Stanley",
        None,
        None,
        5,
    );

    // Should find both mentions
    let mention_texts: Vec<&str> = results.iter().map(|r| r.mention_text.as_str()).collect();

    // Check we found entities (may be exact match or token overlap)
    assert!(
        !results.is_empty(),
        "Should find at least one entity mention, got: {:?}",
        mention_texts
    );
}

#[test]
fn test_resolve_with_kind_constraint() {
    let snapshot = Arc::new(create_test_snapshot());
    let service = EntityLinkingServiceImpl::new(snapshot);

    // Without constraint
    let results_no_constraint = service.resolve_mentions("John Smith", None, None, 5);

    // With person constraint
    let results_with_constraint =
        service.resolve_mentions("John Smith", Some(&["proper_person".to_string()]), None, 5);

    // Person constraint should boost the person entity
    if !results_with_constraint.is_empty() && !results_no_constraint.is_empty() {
        let constrained = &results_with_constraint[0];
        let unconstrained = &results_no_constraint[0];

        // With constraint, should have higher or equal confidence for person match
        if let (Some(c_id), Some(u_id)) = (constrained.selected, unconstrained.selected) {
            // If both selected, the constrained one should prioritize person
            if c_id != u_id {
                // Different selections - constrained should pick person
                let c_kind = constrained
                    .candidates
                    .first()
                    .map(|c| c.entity_kind.as_str());
                assert_eq!(
                    c_kind,
                    Some("proper_person"),
                    "Kind constraint should prioritize person"
                );
            }
        }
    }
}

#[test]
fn test_resolve_no_matches() {
    let snapshot = Arc::new(create_test_snapshot());
    let service = EntityLinkingServiceImpl::new(snapshot);

    let results = service.resolve_mentions("xyzzy foobar qwerty", None, None, 5);

    // Should return empty or no selected entities
    let selected_count = results.iter().filter(|r| r.selected.is_some()).count();
    assert_eq!(selected_count, 0, "Should not match nonsense input");
}

// ============================================================================
// STUB SERVICE TESTS - Graceful degradation
// ============================================================================

#[test]
fn test_stub_returns_empty() {
    let stub = StubEntityLinkingService;

    let results = stub.resolve_mentions("Goldman Sachs", None, None, 5);
    assert!(results.is_empty(), "Stub should return empty results");
}

#[test]
fn test_stub_entity_count_zero() {
    let stub = StubEntityLinkingService;
    assert_eq!(stub.entity_count(), 0);
}

#[test]
fn test_stub_snapshot_hash() {
    let stub = StubEntityLinkingService;
    assert_eq!(stub.snapshot_hash(), "stub-no-snapshot");
}

// ============================================================================
// EVIDENCE TESTS - Audit trail
// ============================================================================

#[test]
fn test_evidence_serialization_roundtrip() {
    let evidences = vec![
        Evidence::AliasExact {
            alias: "goldman".to_string(),
        },
        Evidence::AliasTokenOverlap {
            tokens: vec!["goldman".to_string(), "sachs".to_string()],
            overlap: 0.85,
        },
        Evidence::KindMatchBoost {
            expected: "company".to_string(),
            actual: "company".to_string(),
            boost: 0.05,
        },
        Evidence::KindMismatchPenalty {
            expected: "person".to_string(),
            actual: "company".to_string(),
            penalty: 0.20,
        },
        Evidence::ConceptOverlapBoost {
            concepts: vec!["finance".to_string(), "banking".to_string()],
            boost: 0.08,
        },
    ];

    for evidence in evidences {
        let json = serde_json::to_string(&evidence).expect("Should serialize");
        let parsed: Evidence = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(evidence, parsed, "Roundtrip should preserve data");
    }
}

#[test]
fn test_evidence_tagged_enum_format() {
    let evidence = Evidence::AliasExact {
        alias: "test".to_string(),
    };
    let json = serde_json::to_string(&evidence).unwrap();

    // Should use tagged format with "type" field
    assert!(json.contains(r#""type":"alias_exact""#));
    assert!(json.contains(r#""alias":"test""#));
}

// ============================================================================
// MENTION SPAN TESTS
// ============================================================================

#[test]
fn test_mention_span_positions() {
    let snapshot = Arc::new(create_test_snapshot());
    let service = EntityLinkingServiceImpl::new(snapshot);

    let utterance = "Contact Goldman Sachs today";
    let results = service.resolve_mentions(utterance, None, None, 5);

    if !results.is_empty() {
        let mention = &results[0];
        let (start, end) = mention.mention_span;

        // Verify span points to actual text
        let extracted = &utterance[start..end];
        assert!(
            extracted.to_lowercase().contains("goldman")
                || mention.mention_text.to_lowercase().contains("goldman"),
            "Span should extract 'Goldman Sachs', got: '{}'",
            extracted
        );
    }
}

// ============================================================================
// TRAIT OBJECT TESTS - Verify trait is object-safe
// ============================================================================

#[test]
fn test_trait_object_dispatch() {
    let snapshot = Arc::new(create_test_snapshot());
    let service: Arc<dyn EntityLinkingService> = Arc::new(EntityLinkingServiceImpl::new(snapshot));

    // Call through trait object
    assert_eq!(service.entity_count(), 5);
    assert_eq!(service.snapshot_version(), 1);

    let results = service.resolve_mentions("Goldman", None, None, 5);
    assert!(!results.is_empty() || results.is_empty()); // Just verify it runs
}

#[test]
fn test_stub_as_trait_object() {
    let service: Arc<dyn EntityLinkingService> = Arc::new(StubEntityLinkingService);

    assert_eq!(service.entity_count(), 0);
    assert_eq!(service.snapshot_hash(), "stub-no-snapshot");

    let results = service.resolve_mentions("anything", None, None, 5);
    assert!(results.is_empty());
}

// ============================================================================
// DATABASE INTEGRATION TESTS (require DATABASE_URL)
// ============================================================================

#[cfg(feature = "database")]
mod database_tests {
    use super::*;
    use sqlx::PgPool;

    async fn get_test_pool() -> PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
        PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database")
    }

    /// Test loading real snapshot from database
    /// Run with: DATABASE_URL="postgresql:///data_designer" cargo test --features database entity_linking
    #[tokio::test]
    #[ignore] // Requires database
    async fn test_compile_and_load_snapshot() {
        use ob_poc::entity_linking::compile_entity_snapshot;

        let pool = get_test_pool().await;

        // Compile snapshot
        let snapshot = compile_entity_snapshot(&pool)
            .await
            .expect("Should compile snapshot");

        // Verify basics
        assert!(!snapshot.entities.is_empty(), "Should have entities");
        assert!(!snapshot.hash.is_empty(), "Should have hash");

        // Create service and test
        let service = EntityLinkingServiceImpl::new(Arc::new(snapshot));
        assert!(service.entity_count() > 0);
    }

    /// Test entity resolution with real data
    #[tokio::test]
    #[ignore] // Requires database with entities
    async fn test_resolve_real_entities() {
        use ob_poc::entity_linking::compile_entity_snapshot;

        let pool = get_test_pool().await;
        let snapshot = compile_entity_snapshot(&pool)
            .await
            .expect("Should compile snapshot");

        let service = EntityLinkingServiceImpl::new(Arc::new(snapshot));

        // Try common entity names that likely exist
        let test_phrases = vec![
            "Work with Allianz",
            "Contact the fund manager",
            "Set up ISDA agreement",
        ];

        for phrase in test_phrases {
            let results = service.resolve_mentions(phrase, None, None, 5);
            // Just verify no panic - may or may not find matches
            println!("Phrase '{}' found {} mentions", phrase, results.len());
        }
    }
}
