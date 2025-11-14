//! Integration Tests for Phase 1-3 Attribute System
//!
//! These tests verify the complete integration of:
//! - Phase 1: Type-safe domain attributes
//! - Phase 2: DSL validation and parsing
//! - Phase 3: Database persistence
//!
//! Run with: cargo test --test attribute_integration_test --features database

#[cfg(feature = "database")]
mod integration_tests {
    use ob_poc::database::attribute_repository::AttributeRepository;
    use ob_poc::domains::attributes::kyc::{FirstName, LastName, LegalEntityName};
    use ob_poc::domains::attributes::types::{AttributeMetadata, AttributeType};
    use ob_poc::domains::attributes::validator::AttributeValidator;
    use ob_poc::services::attribute_service::AttributeService;
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn setup_test_service() -> (AttributeService, PgPool) {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/ob_poc".to_string());

        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database");

        let mut validator = AttributeValidator::new();
        validator.register(FirstName::metadata());
        validator.register(LastName::metadata());
        validator.register(LegalEntityName::metadata());

        let repository = AttributeRepository::new(pool.clone());
        let service = AttributeService::new(repository, validator);

        (service, pool)
    }

    #[tokio::test]
    async fn test_end_to_end_attribute_storage_and_retrieval() {
        let (service, _pool) = setup_test_service().await;
        let entity_id = Uuid::new_v4();

        // Store attribute
        let value = "John".to_string();
        let id = service
            .set_attribute::<FirstName>(entity_id, value.clone(), Some("test"))
            .await
            .expect("Failed to set attribute");

        assert!(id > 0, "Should return valid ID");

        // Retrieve attribute
        let retrieved = service
            .get_attribute::<FirstName>(entity_id)
            .await
            .expect("Failed to get attribute");

        assert_eq!(
            retrieved,
            Some(value),
            "Retrieved value should match stored value"
        );
    }

    #[tokio::test]
    async fn test_multiple_attributes_same_entity() {
        let (service, _pool) = setup_test_service().await;
        let entity_id = Uuid::new_v4();

        // Store multiple attributes
        service
            .set_attribute::<FirstName>(entity_id, "Jane".to_string(), Some("test"))
            .await
            .expect("Failed to set first name");

        service
            .set_attribute::<LastName>(entity_id, "Smith".to_string(), Some("test"))
            .await
            .expect("Failed to set last name");

        // Retrieve both
        let first = service
            .get_attribute::<FirstName>(entity_id)
            .await
            .expect("Failed to get first name");

        let last = service
            .get_attribute::<LastName>(entity_id)
            .await
            .expect("Failed to get last name");

        assert_eq!(first, Some("Jane".to_string()));
        assert_eq!(last, Some("Smith".to_string()));
    }

    #[tokio::test]
    async fn test_attribute_update_creates_history() {
        let (service, _pool) = setup_test_service().await;
        let entity_id = Uuid::new_v4();

        // Initial value
        service
            .set_attribute::<FirstName>(entity_id, "Alice".to_string(), Some("user1"))
            .await
            .expect("Failed to set initial value");

        // Update value
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        service
            .set_attribute::<FirstName>(entity_id, "Alicia".to_string(), Some("user2"))
            .await
            .expect("Failed to update value");

        // Get current value
        let current = service
            .get_attribute::<FirstName>(entity_id)
            .await
            .expect("Failed to get current value");

        assert_eq!(current, Some("Alicia".to_string()));

        // Get history
        let history = service
            .get_attribute_history::<FirstName>(entity_id, 10)
            .await
            .expect("Failed to get history");

        assert!(history.len() >= 2, "Should have at least 2 history entries");
        assert_eq!(
            history[0].value,
            "Alicia".to_string(),
            "Latest value should be first"
        );
    }

    #[tokio::test]
    async fn test_dsl_processing_with_persistence() {
        let (service, _pool) = setup_test_service().await;
        let entity_id = Uuid::new_v4();

        let dsl = format!(
            r#"(entity.register
                :entity-id "{}"
                :first-name @attr.identity.first_name
                :last-name @attr.identity.last_name
            )"#,
            entity_id
        );

        let result = service
            .process_attribute_dsl(entity_id, &dsl, Some("test"))
            .await
            .expect("Failed to process DSL");

        assert!(result.validation_passed, "Validation should pass");
        assert_eq!(result.forms_processed, 1, "Should process 1 form");
        assert!(
            result.attributes_extracted >= 2,
            "Should extract at least 2 attributes"
        );
    }

    #[tokio::test]
    async fn test_batch_attribute_retrieval() {
        let (service, _pool) = setup_test_service().await;
        let entity_id = Uuid::new_v4();

        // Store multiple attributes
        service
            .set_attribute::<FirstName>(entity_id, "Bob".to_string(), Some("test"))
            .await
            .expect("Failed to set first name");

        service
            .set_attribute::<LastName>(entity_id, "Jones".to_string(), Some("test"))
            .await
            .expect("Failed to set last name");

        // Batch retrieval
        let attr_ids = vec![FirstName::ID, LastName::ID];
        let attributes = service
            .get_many_attributes(entity_id, &attr_ids)
            .await
            .expect("Failed to get multiple attributes");

        assert_eq!(attributes.len(), 2, "Should retrieve 2 attributes");
        assert!(attributes.contains_key(FirstName::ID));
        assert!(attributes.contains_key(LastName::ID));
    }

    #[tokio::test]
    async fn test_validation_rejects_unknown_attributes() {
        let (service, _pool) = setup_test_service().await;
        let entity_id = Uuid::new_v4();

        let invalid_dsl = format!(
            r#"(entity.register
                :entity-id "{}"
                :unknown-field @attr.unknown.field
            )"#,
            entity_id
        );

        let result = service
            .process_attribute_dsl(entity_id, &invalid_dsl, Some("test"))
            .await;

        assert!(result.is_err(), "Should reject unknown attribute");
    }

    #[tokio::test]
    async fn test_dsl_generation() {
        let (service, _pool) = setup_test_service().await;
        let entity_id = Uuid::new_v4();

        // Generate GET DSL
        let get_dsl = service
            .generate_get_attribute_dsl::<FirstName>(entity_id)
            .expect("Failed to generate GET DSL");

        assert!(get_dsl.contains("entity.get-attribute"));
        assert!(get_dsl.contains(&entity_id.to_string()));
        assert!(get_dsl.contains(FirstName::ID));

        // Generate SET DSL
        let set_dsl = service
            .generate_set_attribute_dsl::<FirstName>(entity_id, &"Test".to_string())
            .expect("Failed to generate SET DSL");

        assert!(set_dsl.contains("entity.set-attribute"));
        assert!(set_dsl.contains(&entity_id.to_string()));
        assert!(set_dsl.contains(FirstName::ID));
    }

    #[tokio::test]
    async fn test_concurrent_attribute_updates() {
        let (service, _pool) = setup_test_service().await;
        let entity_id = Uuid::new_v4();

        // Perform concurrent updates
        let service1 = service.clone();
        let service2 = service.clone();
        let service3 = service.clone();

        let entity_id1 = entity_id;
        let entity_id2 = entity_id;
        let entity_id3 = entity_id;

        let handle1 = tokio::spawn(async move {
            service1
                .set_attribute::<FirstName>(entity_id1, "Concurrent1".to_string(), Some("user1"))
                .await
        });

        let handle2 = tokio::spawn(async move {
            service2
                .set_attribute::<FirstName>(entity_id2, "Concurrent2".to_string(), Some("user2"))
                .await
        });

        let handle3 = tokio::spawn(async move {
            service3
                .set_attribute::<FirstName>(entity_id3, "Concurrent3".to_string(), Some("user3"))
                .await
        });

        // Wait for all updates
        let r1 = handle1.await.expect("Task 1 panicked");
        let r2 = handle2.await.expect("Task 2 panicked");
        let r3 = handle3.await.expect("Task 3 panicked");

        assert!(r1.is_ok(), "Update 1 should succeed");
        assert!(r2.is_ok(), "Update 2 should succeed");
        assert!(r3.is_ok(), "Update 3 should succeed");

        // Verify final state exists
        let final_value = service
            .get_attribute::<FirstName>(entity_id)
            .await
            .expect("Failed to get final value");

        assert!(final_value.is_some(), "Should have a final value");

        // Verify history captured all updates
        let history = service
            .get_attribute_history::<FirstName>(entity_id, 10)
            .await
            .expect("Failed to get history");

        assert!(history.len() >= 3, "Should have at least 3 history entries");
    }
}
