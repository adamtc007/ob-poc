//! Tests for UUID migration functionality

use ob_poc::domains::attributes::{
    kyc::{FirstName, LastName},
    types::AttributeType,
    uuid_constants::{semantic_to_uuid, uuid_to_semantic},
};
use uuid::Uuid;

#[test]
fn test_attribute_has_uuid() {
    // Verify attributes have UUID constants
    let uuid_str = FirstName::UUID_STR;
    assert!(!uuid_str.is_empty());
    
    // Verify it's a valid UUID
    let uuid = Uuid::parse_str(uuid_str).expect("Should be valid UUID");
    assert_eq!(uuid, FirstName::uuid());
}

#[test]
fn test_uuid_dsl_token() {
    let token = FirstName::to_dsl_token();
    assert!(token.starts_with("@attr{"));
    assert!(token.ends_with("}"));
    assert!(token.contains(FirstName::UUID_STR));
}

#[test]
fn test_semantic_compatibility() {
    let semantic_token = FirstName::to_semantic_token();
    assert_eq!(semantic_token, "@attr.identity.first_name");
}

#[test]
fn test_uuid_resolution() {
    let semantic_id = "attr.identity.first_name";
    let uuid = semantic_to_uuid(semantic_id).expect("Should resolve");
    
    let resolved_semantic = uuid_to_semantic(&uuid).expect("Should reverse resolve");
    assert_eq!(resolved_semantic, semantic_id);
}

#[cfg(feature = "database")]
#[tokio::test]
async fn test_uuid_database_query() {
    use sqlx::PgPool;
    
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap()).await.unwrap();
    
    // Query using UUID
    let uuid = FirstName::uuid();
    let result = sqlx::query!(
        r#"
        SELECT id, display_name 
        FROM "ob-poc".attribute_registry 
        WHERE uuid = $1
        "#,
        uuid
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    
    assert!(result.is_some());
    let row = result.unwrap();
    assert_eq!(row.id, FirstName::ID);
}
