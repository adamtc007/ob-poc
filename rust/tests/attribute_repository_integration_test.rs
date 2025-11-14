//! Integration tests for AttributeRepository
//!
//! These tests require a running PostgreSQL database with the attribute refactor migrations applied.
//! Run with: cargo test --features database --test attribute_repository_integration_test

#[cfg(feature = "database")]
mod tests {
    use ob_poc::database::AttributeRepository;
    use ob_poc::domains::attributes::kyc::*;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    async fn setup_test_pool() -> sqlx::PgPool {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");

        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to connect to database")
    }

    #[tokio::test]
    async fn test_set_and_get_string_attribute() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        // Set a first name
        let result = repo
            .set::<FirstName>(entity_id, "John".to_string(), Some("test"))
            .await;

        assert!(result.is_ok(), "Failed to set attribute: {:?}", result);

        // Get it back
        let retrieved = repo.get::<FirstName>(entity_id).await;
        assert!(
            retrieved.is_ok(),
            "Failed to get attribute: {:?}",
            retrieved
        );

        let value = retrieved.unwrap();
        assert_eq!(value, Some("John".to_string()));
    }

    #[tokio::test]
    async fn test_set_and_get_number_attribute() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        // Set ownership percentage
        let result = repo
            .set::<UboOwnershipPercentage>(entity_id, 25.5, Some("test"))
            .await;

        assert!(result.is_ok(), "Failed to set attribute: {:?}", result);

        // Get it back
        let retrieved = repo.get::<UboOwnershipPercentage>(entity_id).await;
        assert!(
            retrieved.is_ok(),
            "Failed to get attribute: {:?}",
            retrieved
        );

        let value = retrieved.unwrap();
        assert_eq!(value, Some(25.5));
    }

    #[tokio::test]
    async fn test_validation_failure() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        // Try to set an empty first name (should fail validation)
        let result = repo
            .set::<FirstName>(entity_id, "".to_string(), Some("test"))
            .await;

        assert!(result.is_err(), "Empty first name should fail validation");
    }

    #[tokio::test]
    async fn test_get_nonexistent_attribute() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        // Try to get an attribute that doesn't exist
        let retrieved = repo.get::<FirstName>(entity_id).await;

        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), None);
    }

    #[tokio::test]
    async fn test_update_attribute() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        // Set initial value
        repo.set::<FirstName>(entity_id, "John".to_string(), Some("test"))
            .await
            .expect("Failed to set initial value");

        // Update to new value
        repo.set::<FirstName>(entity_id, "Jane".to_string(), Some("test"))
            .await
            .expect("Failed to update value");

        // Get the current value
        let retrieved = repo.get::<FirstName>(entity_id).await.unwrap();
        assert_eq!(retrieved, Some("Jane".to_string()));
    }

    #[tokio::test]
    async fn test_get_history() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        // Set initial value
        repo.set::<FirstName>(entity_id, "John".to_string(), Some("test"))
            .await
            .expect("Failed to set initial value");

        // Update to new value
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        repo.set::<FirstName>(entity_id, "Jane".to_string(), Some("test"))
            .await
            .expect("Failed to update value");

        // Get history
        let history = repo.get_history::<FirstName>(entity_id, 10).await;
        assert!(history.is_ok(), "Failed to get history: {:?}", history);

        let history = history.unwrap();
        assert_eq!(history.len(), 2, "Should have 2 history entries");

        // Most recent should be first
        assert_eq!(history[0].value, "Jane");
        assert_eq!(history[1].value, "John");
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        // Set a value
        repo.set::<FirstName>(entity_id, "John".to_string(), Some("test"))
            .await
            .expect("Failed to set value");

        // First get - should hit database and cache
        let start = std::time::Instant::now();
        let _value1 = repo.get::<FirstName>(entity_id).await.unwrap();
        let first_duration = start.elapsed();

        // Second get - should hit cache (faster)
        let start = std::time::Instant::now();
        let _value2 = repo.get::<FirstName>(entity_id).await.unwrap();
        let second_duration = start.elapsed();

        println!(
            "First get: {:?}, Second get (cached): {:?}",
            first_duration, second_duration
        );

        // Cache stats
        let stats = repo.cache_stats().await;
        assert!(stats.entries > 0, "Cache should have entries");
    }

    #[tokio::test]
    async fn test_set_many_transactional() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        let attributes = vec![
            ("attr.identity.first_name", serde_json::json!("John")),
            ("attr.identity.last_name", serde_json::json!("Doe")),
        ];

        let result = repo
            .set_many_transactional(entity_id, attributes, Some("test"))
            .await;

        assert!(
            result.is_ok(),
            "Failed to set multiple attributes: {:?}",
            result
        );
        let ids = result.unwrap();
        assert_eq!(ids.len(), 2);

        // Verify both were set
        let first_name = repo.get::<FirstName>(entity_id).await.unwrap();
        let last_name = repo.get::<LastName>(entity_id).await.unwrap();

        assert_eq!(first_name, Some("John".to_string()));
        assert_eq!(last_name, Some("Doe".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_entity_types() {
        let pool = setup_test_pool().await;
        let repo = AttributeRepository::new(pool);
        let entity_id = Uuid::new_v4();

        // Set different types of attributes
        repo.set::<FirstName>(entity_id, "Alice".to_string(), Some("test"))
            .await
            .expect("Failed to set FirstName");

        repo.set::<Email>(entity_id, "alice@example.com".to_string(), Some("test"))
            .await
            .expect("Failed to set Email");

        repo.set::<DateOfBirth>(
            entity_id,
            chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
            Some("test"),
        )
        .await
        .expect("Failed to set DateOfBirth");

        // Retrieve all
        let first_name = repo.get::<FirstName>(entity_id).await.unwrap().unwrap();
        let email = repo.get::<Email>(entity_id).await.unwrap().unwrap();
        let dob = repo.get::<DateOfBirth>(entity_id).await.unwrap().unwrap();

        assert_eq!(first_name, "Alice");
        assert_eq!(email, "alice@example.com");
        assert_eq!(dob, chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap());
    }
}
