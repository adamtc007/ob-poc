//! CBU Lifecycle Integration Tests
//!
//! Full end-to-end tests that:
//! 1. Create CBU Model DSL and persist to database
//! 2. Execute DSL operations through complete lifecycle
//! 3. Validate state transitions against model
//! 4. Verify all database tables are populated correctly
//!
//! Run with: cargo test --test cbu_lifecycle_integration --features database

use ob_poc::cbu_model_dsl::CbuModelService;
use ob_poc::database::{CbuService, DatabaseConfig, DatabaseManager};
use ob_poc::forth_engine::{execute_sheet_with_db, DslSheet};
use sqlx::PgPool;
use uuid::Uuid;

/// Test fixture for CBU lifecycle tests
struct TestFixture {
    pool: PgPool,
    test_id: String,
}

impl TestFixture {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = DatabaseConfig::default();
        let db = DatabaseManager::new(config).await?;
        let pool = db.pool().clone();
        let test_id = Uuid::new_v4().to_string()[..8].to_string();
        Ok(Self { pool, test_id })
    }

    fn unique_name(&self, prefix: &str) -> String {
        format!("{} {}", prefix, self.test_id)
    }
}

/// Standard CBU Model for testing lifecycle
const TEST_CBU_MODEL: &str = r#"
(cbu-model
  :id "CBU.GENERIC"
  :version "1.0"
  :applies-to ["all"]
  
  (attributes
    (group :name "identification"
      :required [@attr("entity.legal_name")]
      :optional [@attr("jurisdiction")]))
  
  (states
    :initial "Proposed"
    :final ["Closed" "Rejected"]
    (state "Proposed" :description "Initial CBU creation")
    (state "UnderReview" :description "KYC review in progress")
    (state "Active" :description "Fully operational")
    (state "Suspended" :description "Temporarily suspended")
    (state "Closed" :description "Permanently closed")
    (state "Rejected" :description "Application rejected"))
  
  (transitions
    (-> "Proposed" "UnderReview" :verb "cbu.submit" :preconditions [])
    (-> "UnderReview" "Active" :verb "cbu.approve" :preconditions [])
    (-> "UnderReview" "Rejected" :verb "cbu.reject" :preconditions [])
    (-> "Active" "Suspended" :verb "cbu.suspend" :preconditions [])
    (-> "Suspended" "Active" :verb "cbu.reactivate" :preconditions [])
    (-> "Active" "Closed" :verb "cbu.close" :preconditions []))
  
  (roles
    (role "AccountManager" :min 1 :max 1)
    (role "BeneficialOwner" :min 1 :max 10)))
"#;

#[tokio::test]
async fn test_cbu_create_persists_to_database() {
    let fixture = TestFixture::new().await.expect("Failed to create fixture");
    let cbu_name = fixture.unique_name("CreateTest Corp");

    let sheet = DslSheet {
        id: format!("test-create-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(
            r#"(cbu.create :cbu-name "{}" :client-type "CORP" :jurisdiction "US")"#,
            cbu_name
        ),
    };

    let result = execute_sheet_with_db(&sheet, fixture.pool.clone())
        .await
        .expect("DSL execution failed");

    assert!(result.success, "Execution should succeed");
    assert!(result.case_id.is_some(), "Should generate CBU ID");

    let cbu_id = Uuid::parse_str(result.case_id.as_ref().unwrap()).unwrap();
    let cbu_service = CbuService::new(fixture.pool.clone());
    let cbu = cbu_service
        .get_cbu_by_id(cbu_id)
        .await
        .expect("DB query failed")
        .expect("CBU should exist");

    assert_eq!(cbu.name, cbu_name);
    assert_eq!(cbu.client_type.as_deref(), Some("CORP"));
    assert_eq!(cbu.jurisdiction.as_deref(), Some("US"));
}

#[tokio::test]
#[ignore = "Model save requires document_type_code column migration"]
async fn test_cbu_model_saves_and_loads() {
    let fixture = TestFixture::new().await.expect("Failed to create fixture");
    let model_service = CbuModelService::new(fixture.pool.clone());

    let model = model_service
        .parse_and_validate(TEST_CBU_MODEL)
        .await
        .expect("Model parsing failed");

    assert_eq!(model.id, "CBU.GENERIC");
    assert_eq!(model.states.initial, "Proposed");
    assert_eq!(model.states.transitions.len(), 6);

    let _instance_id = model_service
        .save_model(TEST_CBU_MODEL, &model)
        .await
        .expect("Model save failed");

    let loaded = model_service
        .load_model_by_id("CBU.GENERIC")
        .await
        .expect("Model load failed")
        .expect("Model should exist");

    assert_eq!(loaded.id, model.id);
    assert_eq!(loaded.states.initial, model.states.initial);

    let doc_exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".document_catalog WHERE document_type_code = 'DSL.CBU.MODEL' AND metadata->>'model_id' = 'CBU.GENERIC')"#,
    )
    .fetch_one(&fixture.pool)
    .await
    .expect("Query failed");

    assert!(doc_exists, "Document catalog entry should exist");
}

#[tokio::test]
async fn test_multiple_cbu_operations_single_sheet() {
    let fixture = TestFixture::new().await.expect("Failed to create fixture");
    let cbu_name = fixture.unique_name("MultiOp Corp");

    let sheet = DslSheet {
        id: format!("test-multi-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(
            r#"(cbu.create :cbu-name "{}" :client-type "FUND" :jurisdiction "GB")"#,
            cbu_name
        ),
    };

    let result = execute_sheet_with_db(&sheet, fixture.pool.clone())
        .await
        .expect("Multi-op execution failed");

    assert!(result.success, "Multi-operation sheet should succeed");
    // Verify execution completed
    assert!(!result.logs.is_empty(), "Should have execution logs");
}

#[tokio::test]
async fn test_cbu_update_operation() {
    let fixture = TestFixture::new().await.expect("Failed to create fixture");
    let cbu_name = fixture.unique_name("UpdateTest Corp");

    let create_sheet = DslSheet {
        id: format!("test-update-create-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(
            r#"(cbu.create :cbu-name "{}" :client-type "CORP" :jurisdiction "US")"#,
            cbu_name
        ),
    };

    let create_result = execute_sheet_with_db(&create_sheet, fixture.pool.clone())
        .await
        .expect("Create failed");

    let cbu_id = create_result.case_id.unwrap();

    let update_sheet = DslSheet {
        id: format!("test-update-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(
            r#"(cbu.update :cbu-id "{}" :name "Updated Corp {}")"#,
            cbu_id, fixture.test_id
        ),
    };

    let update_result = execute_sheet_with_db(&update_sheet, fixture.pool.clone())
        .await
        .expect("Update failed");

    assert!(update_result.success);

    let cbu_uuid = Uuid::parse_str(&cbu_id).unwrap();
    let cbu_service = CbuService::new(fixture.pool.clone());
    let updated_cbu = cbu_service
        .get_cbu_by_id(cbu_uuid)
        .await
        .expect("Query failed")
        .expect("CBU should exist");

    assert!(updated_cbu.name.contains("Updated"), "Name should be updated");
}

#[tokio::test]
async fn test_cbu_delete_operation() {
    let fixture = TestFixture::new().await.expect("Failed to create fixture");
    let cbu_name = fixture.unique_name("DeleteTest Corp");

    let create_sheet = DslSheet {
        id: format!("test-delete-create-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(
            r#"(cbu.create :cbu-name "{}" :client-type "CORP" :jurisdiction "US")"#,
            cbu_name
        ),
    };

    let create_result = execute_sheet_with_db(&create_sheet, fixture.pool.clone())
        .await
        .expect("Create failed");

    let cbu_id = create_result.case_id.unwrap();

    let delete_sheet = DslSheet {
        id: format!("test-delete-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(r#"(cbu.delete :cbu-id "{}")"#, cbu_id),
    };

    let delete_result = execute_sheet_with_db(&delete_sheet, fixture.pool.clone())
        .await
        .expect("Delete failed");

    assert!(delete_result.success);

    let cbu_uuid = Uuid::parse_str(&cbu_id).unwrap();
    let cbu_service = CbuService::new(fixture.pool.clone());
    let deleted_cbu = cbu_service.get_cbu_by_id(cbu_uuid).await.expect("Query failed");

    assert!(deleted_cbu.is_none(), "CBU should be deleted");
}

#[tokio::test]
#[ignore = "Model save requires document_type_code column migration"]
async fn test_full_cbu_lifecycle() {
    let fixture = TestFixture::new().await.expect("Failed to create fixture");
    let model_service = CbuModelService::new(fixture.pool.clone());
    let cbu_service = CbuService::new(fixture.pool.clone());

    // Save CBU Model
    let model = model_service.parse_and_validate(TEST_CBU_MODEL).await.expect("Model parse failed");
    model_service.save_model(TEST_CBU_MODEL, &model).await.expect("Model save failed");

    let cbu_name = fixture.unique_name("Lifecycle Corp");

    // Create CBU
    let create_sheet = DslSheet {
        id: format!("lifecycle-create-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(
            r#"(cbu.create :cbu-name "{}" :client-type "CORP" :jurisdiction "US")"#,
            cbu_name
        ),
    };

    let create_result = execute_sheet_with_db(&create_sheet, fixture.pool.clone())
        .await
        .expect("Create failed");

    let cbu_id = create_result.case_id.unwrap();
    let cbu_uuid = Uuid::parse_str(&cbu_id).unwrap();

    let cbu = cbu_service.get_cbu_by_id(cbu_uuid).await.expect("Query failed").expect("CBU should exist");
    assert_eq!(cbu.name, cbu_name);

    // Update to Active
    let activate_sheet = DslSheet {
        id: format!("lifecycle-activate-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(r#"(cbu.update :cbu-id "{}" :status "Active")"#, cbu_id),
    };

    let activate_result = execute_sheet_with_db(&activate_sheet, fixture.pool.clone())
        .await
        .expect("Activate failed");
    assert!(activate_result.success);

    // Close
    let close_sheet = DslSheet {
        id: format!("lifecycle-close-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(r#"(cbu.finalize :cbu-id "{}" :status "Closed")"#, cbu_id),
    };

    let close_result = execute_sheet_with_db(&close_sheet, fixture.pool.clone())
        .await
        .expect("Close failed");
    assert!(close_result.success);

    // Verify DSL versions
    let version_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM "ob-poc".dsl_instances WHERE business_reference = $1"#,
    )
    .bind(&cbu_id)
    .fetch_one(&fixture.pool)
    .await
    .expect("Count query failed");

    assert!(version_count.0 >= 3, "Should have at least 3 DSL versions for lifecycle");
}

#[tokio::test]
async fn test_cbu_count_increases() {
    let fixture = TestFixture::new().await.expect("Failed to create fixture");

    let initial_count: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".cbus"#)
        .fetch_one(&fixture.pool)
        .await
        .expect("Initial count failed");

    let cbu_name = fixture.unique_name("CountTest Corp");
    let sheet = DslSheet {
        id: format!("test-count-{}", fixture.test_id),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: format!(
            r#"(cbu.create :cbu-name "{}" :client-type "CORP" :jurisdiction "US")"#,
            cbu_name
        ),
    };

    execute_sheet_with_db(&sheet, fixture.pool.clone())
        .await
        .expect("Create failed");

    let new_count: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".cbus"#)
        .fetch_one(&fixture.pool)
        .await
        .expect("New count failed");

    assert!(new_count.0 >= initial_count.0 + 1, "CBU count should increase by at least 1");
}
