// Add this to your types.rs file or replace the trait definition

use crate::domains::attributes::uuid_constants;

/// Updated trait with UUID support
pub trait AttributeType: Send + Sync + 'static {
    /// The Rust type that represents this attribute's value
    type Value: Serialize + for<'de> Deserialize<'de> + Clone + Debug + Send + Sync;

    /// Static attribute identifier (e.g., "attr.identity.first_name")
    const ID: &'static str;
    
    /// UUID for this attribute (NEW)
    const UUID_STR: &'static str;

    /// Human-readable display name
    const DISPLAY_NAME: &'static str;

    /// Category this attribute belongs to
    const CATEGORY: AttributeCategory;

    /// Data type for storage and validation
    const DATA_TYPE: DataType;

    /// Get UUID as parsed type
    fn uuid() -> Uuid {
        Uuid::parse_str(Self::UUID_STR).expect("Invalid UUID constant")
    }

    /// Validation rules for this attribute
    fn validation_rules() -> ValidationRules;

    /// Validate a value for this attribute
    fn validate(value: &Self::Value) -> Result<(), ValidationError>;

    /// Convert to DSL token representation (UUID-based)
    fn to_dsl_token() -> String {
        format!("@attr{{{}}}", Self::UUID_STR)
    }
    
    /// Convert to semantic DSL token (for backward compat)
    fn to_semantic_token() -> String {
        format!("@{}", Self::ID)
    }

    /// Get attribute metadata
    fn metadata() -> AttributeMetadata {
        AttributeMetadata {
            id: Self::ID.to_string(),
            uuid: Self::uuid(),
            display_name: Self::DISPLAY_NAME.to_string(),
            category: Self::CATEGORY,
            data_type: Self::DATA_TYPE,
            validation: Self::validation_rules(),
        }
    }
}
