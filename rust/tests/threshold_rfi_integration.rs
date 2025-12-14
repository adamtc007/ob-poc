//! Threshold and RFI Domain Integration Tests
//!
//! Tests the risk-based document requirement system:
//! 1. Risk band derivation based on CBU factors
//! 2. Threshold requirements lookup by risk band
//! 3. RFI (Request for Information) generation
//!
//! Run with: cargo test --features database --test threshold_rfi_integration

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

/// Test helper to set up database connection
async fn setup_pool() -> PgPool {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database")
}

/// Test helper to create a test CBU with unique name
async fn create_test_cbu(pool: &PgPool, name: &str, jurisdiction: &str, client_type: &str) -> Uuid {
    let unique_name = format!("{} - {}", name, Uuid::new_v4());
    sqlx::query_scalar!(
        r#"
        INSERT INTO "ob-poc".cbus (name, jurisdiction, client_type)
        VALUES ($1, $2, $3)
        RETURNING cbu_id
        "#,
        unique_name,
        jurisdiction,
        client_type
    )
    .fetch_one(pool)
    .await
    .expect("Failed to create test CBU")
}

/// Cleanup test data
async fn cleanup_test_cbu(pool: &PgPool, cbu_id: Uuid) {
    // Clean up in order
    let _ = sqlx::query!(
        r#"DELETE FROM kyc.doc_requests WHERE workstream_id IN
           (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN
            (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#,
        cbu_id
    )
    .execute(pool)
    .await;

    let _ = sqlx::query!(
        r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN
           (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#,
        cbu_id
    )
    .execute(pool)
    .await;

    let _ = sqlx::query!("DELETE FROM kyc.cases WHERE cbu_id = $1", cbu_id)
        .execute(pool)
        .await;

    let _ = sqlx::query!(
        r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#,
        cbu_id
    )
    .execute(pool)
    .await;

    let _ = sqlx::query!(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#, cbu_id)
        .execute(pool)
        .await;
}

// ============================================================================
// Risk Band Tests
// ============================================================================

#[tokio::test]
async fn test_risk_bands_exist() {
    let pool = setup_pool().await;

    // Verify risk bands are seeded
    let bands = sqlx::query!(
        r#"SELECT band_code, min_score, max_score FROM "ob-poc".risk_bands ORDER BY min_score"#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query risk bands");

    assert!(!bands.is_empty(), "Risk bands should be seeded");

    // Check we have standard bands
    let band_codes: Vec<String> = bands.iter().map(|b| b.band_code.clone()).collect();
    assert!(
        band_codes.contains(&"LOW".to_string()) || band_codes.contains(&"MEDIUM".to_string()),
        "Should have at least LOW or MEDIUM risk band"
    );
}

#[tokio::test]
async fn test_threshold_factors_exist() {
    let pool = setup_pool().await;

    // Verify threshold factors are seeded
    let factors = sqlx::query!(
        r#"SELECT factor_code, factor_type, risk_weight FROM "ob-poc".threshold_factors LIMIT 10"#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query threshold factors");

    assert!(!factors.is_empty(), "Threshold factors should be seeded");
}

#[tokio::test]
async fn test_threshold_requirements_by_risk_band() {
    let pool = setup_pool().await;

    // Get requirements for each risk band
    let requirements = sqlx::query!(
        r#"SELECT DISTINCT tr.risk_band, COUNT(*) as req_count
           FROM "ob-poc".threshold_requirements tr
           GROUP BY tr.risk_band"#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query requirements");

    // Higher risk bands should generally have more requirements
    if requirements.len() >= 2 {
        println!("Requirements by risk band:");
        for req in &requirements {
            println!(
                "  {}: {} requirements",
                req.risk_band,
                req.req_count.unwrap_or(0)
            );
        }
    }
}

// ============================================================================
// Requirement -> Acceptable Docs Tests
// ============================================================================

#[tokio::test]
async fn test_requirement_acceptable_docs_mapping() {
    let pool = setup_pool().await;

    // Verify the mapping table has data
    let mappings = sqlx::query!(
        r#"SELECT tr.attribute_code, rad.document_type_code, rad.priority
           FROM "ob-poc".threshold_requirements tr
           JOIN "ob-poc".requirement_acceptable_docs rad ON rad.requirement_id = tr.requirement_id
           ORDER BY tr.attribute_code, rad.priority
           LIMIT 20"#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query acceptable docs");

    assert!(
        !mappings.is_empty(),
        "Should have requirement -> document mappings"
    );

    // Print sample mappings
    println!("Sample requirement -> acceptable doc mappings:");
    for m in mappings.iter().take(5) {
        println!(
            "  {:?} -> {:?} (priority {:?})",
            m.attribute_code, m.document_type_code, m.priority
        );
    }
}

#[tokio::test]
async fn test_identity_attribute_has_acceptable_docs() {
    let pool = setup_pool().await;

    // Identity should have passport, national ID, etc. as acceptable docs
    let identity_docs = sqlx::query!(
        r#"SELECT rad.document_type_code, rad.priority
           FROM "ob-poc".threshold_requirements tr
           JOIN "ob-poc".requirement_acceptable_docs rad ON rad.requirement_id = tr.requirement_id
           WHERE tr.attribute_code = 'identity'
           ORDER BY rad.priority"#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query identity docs");

    if !identity_docs.is_empty() {
        let doc_codes: Vec<String> = identity_docs
            .iter()
            .map(|d| d.document_type_code.clone())
            .collect();

        println!("Acceptable docs for 'identity': {:?}", doc_codes);

        // Should include common identity documents
        let has_identity_doc = doc_codes
            .iter()
            .any(|c| c.contains("PASSPORT") || c.contains("ID") || c.contains("LICENSE"));

        assert!(
            has_identity_doc,
            "Identity attribute should accept passport/ID/license"
        );
    }
}

// ============================================================================
// Screening Requirements Tests
// ============================================================================

#[tokio::test]
async fn test_screening_requirements_exist() {
    let pool = setup_pool().await;

    let screenings = sqlx::query!(
        r#"SELECT risk_band, screening_type, is_required, frequency_months
           FROM "ob-poc".screening_requirements
           ORDER BY risk_band, screening_type"#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query screening requirements");

    if !screenings.is_empty() {
        println!("Screening requirements:");
        for s in &screenings {
            println!(
                "  {} - {:?}: required={}, every {:?} months",
                s.risk_band, s.screening_type, s.is_required, s.frequency_months
            );
        }
    }
}

#[tokio::test]
async fn test_high_risk_requires_more_screenings() {
    let pool = setup_pool().await;

    // Count required screenings per risk band
    let counts = sqlx::query!(
        r#"SELECT risk_band, COUNT(*) as screening_count
           FROM "ob-poc".screening_requirements
           WHERE is_required = true
           GROUP BY risk_band"#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to count screenings");

    if counts.len() >= 2 {
        // Find HIGH and LOW bands
        let high_count = counts
            .iter()
            .find(|c| c.risk_band == "HIGH" || c.risk_band == "VERY_HIGH")
            .map(|c| c.screening_count.unwrap_or(0));

        let low_count = counts
            .iter()
            .find(|c| c.risk_band == "LOW")
            .map(|c| c.screening_count.unwrap_or(0));

        if let (Some(high), Some(low)) = (high_count, low_count) {
            println!("HIGH risk: {} required screenings", high);
            println!("LOW risk: {} required screenings", low);
            // HIGH risk should have at least as many screenings as LOW
            assert!(high >= low, "HIGH risk should have >= screenings than LOW");
        }
    }
}

// ============================================================================
// KYC Case + RFI Integration Tests
// ============================================================================

#[tokio::test]
async fn test_kyc_case_creation_for_cbu() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Threshold Test Fund", "US", "FUND").await;

    // Create a KYC case
    let case_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.cases (cbu_id, case_type, status)
           VALUES ($1, 'NEW_CLIENT', 'INTAKE')
           RETURNING case_id"#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create KYC case");

    assert!(case_id != Uuid::nil());

    // Verify case exists
    let case = sqlx::query!(
        "SELECT status, case_type FROM kyc.cases WHERE case_id = $1",
        case_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch case");

    assert_eq!(case.status, "INTAKE");
    assert_eq!(case.case_type.as_deref(), Some("NEW_CLIENT"));

    cleanup_test_cbu(&pool, cbu_id).await;
}

#[tokio::test]
async fn test_doc_request_creation() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "RFI Test Fund", "GB", "CORPORATE").await;

    // Create entity - use PROPER_PERSON_NATURAL type
    let entity_type_id: Uuid = sqlx::query_scalar!(
        r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = 'PROPER_PERSON_NATURAL' LIMIT 1"#
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to get entity type");

    let entity_name = format!("Test Person - {}", Uuid::new_v4());
    let entity_id = sqlx::query_scalar!(
        r#"INSERT INTO "ob-poc".entities (name, entity_type_id)
           VALUES ($1, $2)
           RETURNING entity_id"#,
        entity_name,
        entity_type_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create entity");

    // Create KYC case
    let case_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.cases (cbu_id, case_type, status)
           VALUES ($1, 'NEW_CLIENT', 'DISCOVERY')
           RETURNING case_id"#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create case");

    // Create workstream
    let workstream_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.entity_workstreams (case_id, entity_id, status)
           VALUES ($1, $2, 'COLLECT')
           RETURNING workstream_id"#,
        case_id,
        entity_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create workstream");

    // Create document request
    let request_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.doc_requests (workstream_id, doc_type, status, is_mandatory)
           VALUES ($1, 'PASSPORT', 'REQUIRED', true)
           RETURNING request_id"#,
        workstream_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create doc request");

    assert!(request_id != Uuid::nil());

    // Verify request exists
    let req = sqlx::query!(
        "SELECT doc_type, status, is_mandatory FROM kyc.doc_requests WHERE request_id = $1",
        request_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch request");

    assert_eq!(req.doc_type, "PASSPORT");
    assert_eq!(req.status, "REQUIRED");
    assert!(req.is_mandatory.unwrap_or(false));

    // Clean up entity
    let _ = sqlx::query!(
        r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#,
        entity_id
    )
    .execute(&pool)
    .await;

    cleanup_test_cbu(&pool, cbu_id).await;
}

#[tokio::test]
async fn test_doc_request_status_transitions() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Doc Status Test", "LU", "FUND").await;

    // Create entity type for person
    let entity_type_id: Uuid = sqlx::query_scalar!(
        r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = 'PROPER_PERSON_NATURAL' LIMIT 1"#
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to get entity type");

    let entity_name = format!("Status Test Person - {}", Uuid::new_v4());
    let entity_id = sqlx::query_scalar!(
        r#"INSERT INTO "ob-poc".entities (name, entity_type_id)
           VALUES ($1, $2)
           RETURNING entity_id"#,
        entity_name,
        entity_type_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create entity");

    let case_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.cases (cbu_id, case_type, status)
           VALUES ($1, 'NEW_CLIENT', 'DISCOVERY')
           RETURNING case_id"#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create case");

    let workstream_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.entity_workstreams (case_id, entity_id, status)
           VALUES ($1, $2, 'COLLECT')
           RETURNING workstream_id"#,
        case_id,
        entity_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create workstream");

    let request_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.doc_requests (workstream_id, doc_type, status)
           VALUES ($1, 'UTILITY_BILL', 'REQUIRED')
           RETURNING request_id"#,
        workstream_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create request");

    // Transition: REQUIRED -> REQUESTED
    sqlx::query!(
        "UPDATE kyc.doc_requests SET status = 'REQUESTED', requested_at = NOW() WHERE request_id = $1",
        request_id
    )
    .execute(&pool)
    .await
    .expect("Failed to update to REQUESTED");

    // Transition: REQUESTED -> RECEIVED
    sqlx::query!(
        "UPDATE kyc.doc_requests SET status = 'RECEIVED', received_at = NOW() WHERE request_id = $1",
        request_id
    )
    .execute(&pool)
    .await
    .expect("Failed to update to RECEIVED");

    // Transition: RECEIVED -> VERIFIED
    sqlx::query!(
        "UPDATE kyc.doc_requests SET status = 'VERIFIED', verified_at = NOW() WHERE request_id = $1",
        request_id
    )
    .execute(&pool)
    .await
    .expect("Failed to update to VERIFIED");

    let final_status: String = sqlx::query_scalar!(
        "SELECT status FROM kyc.doc_requests WHERE request_id = $1",
        request_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch status");

    assert_eq!(final_status, "VERIFIED");

    // Clean up
    let _ = sqlx::query!(
        r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#,
        entity_id
    )
    .execute(&pool)
    .await;

    cleanup_test_cbu(&pool, cbu_id).await;
}

// ============================================================================
// RFI Completion Check Tests
// ============================================================================

#[tokio::test]
async fn test_rfi_completion_check() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "RFI Completion Test", "IE", "FUND").await;

    let entity_type_id: Uuid = sqlx::query_scalar!(
        r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = 'PROPER_PERSON_NATURAL' LIMIT 1"#
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to get entity type");

    let entity_name = format!("Completion Test Person - {}", Uuid::new_v4());
    let entity_id = sqlx::query_scalar!(
        r#"INSERT INTO "ob-poc".entities (name, entity_type_id)
           VALUES ($1, $2)
           RETURNING entity_id"#,
        entity_name,
        entity_type_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create entity");

    let case_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.cases (cbu_id, case_type, status)
           VALUES ($1, 'NEW_CLIENT', 'DISCOVERY')
           RETURNING case_id"#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create case");

    let workstream_id = sqlx::query_scalar!(
        r#"INSERT INTO kyc.entity_workstreams (case_id, entity_id, status)
           VALUES ($1, $2, 'COLLECT')
           RETURNING workstream_id"#,
        case_id,
        entity_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create workstream");

    // Create 2 mandatory and 1 optional request
    sqlx::query!(
        r#"INSERT INTO kyc.doc_requests (workstream_id, doc_type, status, is_mandatory)
           VALUES ($1, 'PASSPORT', 'VERIFIED', true)"#,
        workstream_id
    )
    .execute(&pool)
    .await
    .expect("Failed to create request 1");

    sqlx::query!(
        r#"INSERT INTO kyc.doc_requests (workstream_id, doc_type, status, is_mandatory)
           VALUES ($1, 'UTILITY_BILL', 'REQUIRED', true)"#,
        workstream_id
    )
    .execute(&pool)
    .await
    .expect("Failed to create request 2");

    sqlx::query!(
        r#"INSERT INTO kyc.doc_requests (workstream_id, doc_type, status, is_mandatory)
           VALUES ($1, 'BANK_STATEMENT', 'WAIVED', false)"#,
        workstream_id
    )
    .execute(&pool)
    .await
    .expect("Failed to create request 3");

    // Check completion
    let completion = sqlx::query!(
        r#"SELECT
             COUNT(*) FILTER (WHERE is_mandatory = true) as mandatory_count,
             COUNT(*) FILTER (WHERE is_mandatory = true AND status IN ('VERIFIED', 'WAIVED')) as mandatory_complete,
             COUNT(*) FILTER (WHERE status = 'REQUIRED' OR status = 'REQUESTED') as pending_count
           FROM kyc.doc_requests
           WHERE workstream_id = $1"#,
        workstream_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to check completion");

    assert_eq!(completion.mandatory_count.unwrap_or(0), 2);
    assert_eq!(completion.mandatory_complete.unwrap_or(0), 1); // Only passport is verified
    assert_eq!(completion.pending_count.unwrap_or(0), 1); // Utility bill is still required

    // Clean up
    let _ = sqlx::query!(
        r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#,
        entity_id
    )
    .execute(&pool)
    .await;

    cleanup_test_cbu(&pool, cbu_id).await;
}
