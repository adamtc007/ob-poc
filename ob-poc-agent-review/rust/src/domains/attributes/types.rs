//! Type-Safe Attribute System
//!
//! This module implements the AttributeID-as-Type pattern, providing compile-time
//! type safety for attributes used throughout the DSL and database operations.
//!
//! ## Design Philosophy
//! - **Type-First**: Every attribute has a strongly-typed Rust representation
//! - **DSL Native**: Attributes are first-class citizens in the DSL
//! - **Compile-Time Safety**: Invalid attribute usage caught during compilation
//! - **Zero-Cost Abstractions**: Type safety without runtime overhead
//!
//! ## Architecture
//! Instead of `AttributeId(UUID)`, we use typed enums and traits that provide:
//! - Semantic meaning in code
//! - Validation rules at compile time
//! - Proper DSL integration
//! - Database schema alignment

use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Display};
use std::marker::PhantomData;
use uuid::Uuid;

/// Core trait that all typed attributes must implement
///
/// This trait provides the contract for type-safe attributes, ensuring that
/// each attribute has a well-defined value type, validation rules, and metadata.
pub trait AttributeType: Send + Sync + 'static {
    /// The Rust type that represents this attribute's value
    type Value: Serialize + for<'de> Deserialize<'de> + Clone + Debug + Send + Sync;

    /// Static attribute identifier (e.g., "attr.identity.first_name")
    const ID: &'static str;

    /// UUID for this attribute (for DSL embedding)
    const UUID_STR: &'static str;

    /// Human-readable display name
    const DISPLAY_NAME: &'static str;

    /// Category this attribute belongs to
    const CATEGORY: AttributeCategory;

    /// Data type for storage and validation
    const DATA_TYPE: DataType;

    /// Validation rules for this attribute
    fn validation_rules() -> ValidationRules;

    /// Validate a value for this attribute
    fn validate(value: &Self::Value) -> Result<(), ValidationError>;

    /// Get UUID as parsed type
    fn uuid() -> Uuid {
        Uuid::parse_str(Self::UUID_STR).expect("Invalid UUID constant")
    }

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

/// Categories for organizing attributes by business domain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttributeCategory {
    /// Identity attributes (names, DOB, nationality, etc.)
    Identity,
    /// Financial information (income, net worth, etc.)
    Financial,
    /// Compliance and regulatory data
    Compliance,
    /// Document metadata and references
    Document,
    /// Risk assessment data
    Risk,
    /// Contact information
    Contact,
    /// Address information
    Address,
    /// Tax-related information
    Tax,
    /// Employment information
    Employment,
    /// Product and service data
    Product,
    /// Entity relationship data
    Entity,
    /// UBO-specific attributes
    UBO,
    /// ISDA and derivatives
    ISDA,
}

impl Display for AttributeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttributeCategory::Identity => write!(f, "identity"),
            AttributeCategory::Financial => write!(f, "financial"),
            AttributeCategory::Compliance => write!(f, "compliance"),
            AttributeCategory::Document => write!(f, "document"),
            AttributeCategory::Risk => write!(f, "risk"),
            AttributeCategory::Contact => write!(f, "contact"),
            AttributeCategory::Address => write!(f, "address"),
            AttributeCategory::Tax => write!(f, "tax"),
            AttributeCategory::Employment => write!(f, "employment"),
            AttributeCategory::Product => write!(f, "product"),
            AttributeCategory::Entity => write!(f, "entity"),
            AttributeCategory::UBO => write!(f, "ubo"),
            AttributeCategory::ISDA => write!(f, "isda"),
        }
    }
}

/// Data types for attribute values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// UTF-8 string
    String,
    /// Integer number
    Integer,
    /// Floating point number
    Number,
    /// Boolean value
    Boolean,
    /// ISO 8601 date
    Date,
    /// ISO 8601 datetime with timezone
    DateTime,
    /// Email address
    Email,
    /// Phone number
    Phone,
    /// Postal address
    Address,
    /// Currency amount
    Currency,
    /// Percentage value
    Percentage,
    /// Tax identifier
    TaxId,
    /// Arbitrary JSON structure
    Json,
}

impl Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::String => write!(f, "string"),
            DataType::Integer => write!(f, "integer"),
            DataType::Number => write!(f, "number"),
            DataType::Boolean => write!(f, "boolean"),
            DataType::Date => write!(f, "date"),
            DataType::DateTime => write!(f, "datetime"),
            DataType::Email => write!(f, "email"),
            DataType::Phone => write!(f, "phone"),
            DataType::Address => write!(f, "address"),
            DataType::Currency => write!(f, "currency"),
            DataType::Percentage => write!(f, "percentage"),
            DataType::TaxId => write!(f, "tax_id"),
            DataType::Json => write!(f, "json"),
        }
    }
}

/// Validation rules for an attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    /// Is this attribute required?
    pub required: bool,
    /// Minimum numeric value
    pub min_value: Option<f64>,
    /// Maximum numeric value
    pub max_value: Option<f64>,
    /// Minimum string length
    pub min_length: Option<usize>,
    /// Maximum string length
    pub max_length: Option<usize>,
    /// Regex pattern for validation
    pub pattern: Option<String>,
    /// Allowed enumeration values
    pub allowed_values: Option<Vec<String>>,
    /// Custom validation function name
    pub custom_validator: Option<String>,
}

impl ValidationRules {
    /// Create a new validation rules builder
    pub fn new() -> Self {
        Self {
            required: false,
            min_value: None,
            max_value: None,
            min_length: None,
            max_length: None,
            pattern: None,
            allowed_values: None,
            custom_validator: None,
        }
    }

    /// Mark attribute as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set minimum value
    pub fn min_value(mut self, min: f64) -> Self {
        self.min_value = Some(min);
        self
    }

    /// Set maximum value
    pub fn max_value(mut self, max: f64) -> Self {
        self.max_value = Some(max);
        self
    }

    /// Set minimum length
    pub fn min_length(mut self, min: usize) -> Self {
        self.min_length = Some(min);
        self
    }

    /// Set maximum length
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    /// Set regex pattern
    pub fn pattern(mut self, pattern: &str) -> Self {
        self.pattern = Some(pattern.to_string());
        self
    }

    /// Set allowed values
    pub fn allowed_values(mut self, values: Vec<String>) -> Self {
        self.allowed_values = Some(values);
        self
    }
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub attribute_id: String,
    pub message: String,
    pub error_type: ValidationErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationErrorType {
    Required,
    MinValue,
    MaxValue,
    MinLength,
    MaxLength,
    Pattern,
    AllowedValues,
    Custom,
}

impl Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.attribute_id, self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Complete metadata for an attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeMetadata {
    pub id: String,
    pub uuid: Uuid,
    pub display_name: String,
    pub category: AttributeCategory,
    pub data_type: DataType,
    pub validation: ValidationRules,
}

/// Type-safe reference to an attribute in DSL
///
/// This allows us to reference attributes in a type-safe way while
/// maintaining compatibility with the DSL parser.
#[derive(Debug, Clone)]
pub struct TypedAttributeRef<T: AttributeType> {
    _phantom: PhantomData<T>,
}

impl<T: AttributeType> TypedAttributeRef<T> {
    /// Create a new typed attribute reference
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Get the attribute ID
    pub fn id(&self) -> &'static str {
        T::ID
    }

    /// Get the display name
    pub fn display_name(&self) -> &'static str {
        T::DISPLAY_NAME
    }

    /// Get the category
    pub fn category(&self) -> AttributeCategory {
        T::CATEGORY
    }

    /// Validate a value
    pub fn validate(&self, value: &T::Value) -> Result<(), ValidationError> {
        T::validate(value)
    }

    /// Get metadata
    pub fn metadata(&self) -> AttributeMetadata {
        T::metadata()
    }
}

impl<T: AttributeType> Default for TypedAttributeRef<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: AttributeType> Display for TypedAttributeRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", T::ID)
    }
}

/// Helper for creating validation errors
pub fn validation_error(
    attribute_id: &str,
    error_type: ValidationErrorType,
    message: &str,
) -> ValidationError {
    ValidationError {
        attribute_id: attribute_id.to_string(),
        message: message.to_string(),
        error_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Example attribute for testing
    struct TestFirstName;

    impl AttributeType for TestFirstName {
        type Value = String;
        const ID: &'static str = "attr.identity.first_name";
        const UUID_STR: &'static str = "3020d46f-472c-5437-9647-1b0682c35935";
        const DISPLAY_NAME: &'static str = "First Name";
        const CATEGORY: AttributeCategory = AttributeCategory::Identity;
        const DATA_TYPE: DataType = DataType::String;

        fn validation_rules() -> ValidationRules {
            ValidationRules::new()
                .required()
                .min_length(1)
                .max_length(100)
                .pattern(r"^[A-Za-z\s\-']+$")
        }

        fn validate(value: &Self::Value) -> Result<(), ValidationError> {
            let rules = Self::validation_rules();

            if rules.required && value.is_empty() {
                return Err(validation_error(
                    Self::ID,
                    ValidationErrorType::Required,
                    "First name is required",
                ));
            }

            if let Some(min) = rules.min_length {
                if value.len() < min {
                    return Err(validation_error(
                        Self::ID,
                        ValidationErrorType::MinLength,
                        &format!("First name must be at least {} characters", min),
                    ));
                }
            }

            if let Some(max) = rules.max_length {
                if value.len() > max {
                    return Err(validation_error(
                        Self::ID,
                        ValidationErrorType::MaxLength,
                        &format!("First name must not exceed {} characters", max),
                    ));
                }
            }

            Ok(())
        }
    }

    #[test]
    fn test_attribute_metadata() {
        let metadata = TestFirstName::metadata();
        assert_eq!(metadata.id, "attr.identity.first_name");
        assert_eq!(metadata.display_name, "First Name");
        assert_eq!(metadata.category, AttributeCategory::Identity);
        assert_eq!(metadata.data_type, DataType::String);
    }

    #[test]
    fn test_validation_success() {
        let result = TestFirstName::validate(&"John".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_required() {
        let result = TestFirstName::validate(&"".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_max_length() {
        let long_name = "A".repeat(101);
        let result = TestFirstName::validate(&long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_typed_attribute_ref() {
        let attr_ref = TypedAttributeRef::<TestFirstName>::new();
        assert_eq!(attr_ref.id(), "attr.identity.first_name");
        assert_eq!(attr_ref.display_name(), "First Name");
        assert_eq!(attr_ref.category(), AttributeCategory::Identity);
    }

    #[test]
    fn test_dsl_token() {
        // Phase 1: to_dsl_token() now returns UUID-based format
        let token = TestFirstName::to_dsl_token();
        assert_eq!(token, "@attr{3020d46f-472c-5437-9647-1b0682c35935}");

        // Semantic token is still available via to_semantic_token()
        let semantic_token = TestFirstName::to_semantic_token();
        assert_eq!(semantic_token, "@attr.identity.first_name");
    }

    #[test]
    fn test_category_display() {
        assert_eq!(AttributeCategory::Identity.to_string(), "identity");
        assert_eq!(AttributeCategory::Financial.to_string(), "financial");
        assert_eq!(AttributeCategory::Compliance.to_string(), "compliance");
    }

    #[test]
    fn test_data_type_display() {
        assert_eq!(DataType::String.to_string(), "string");
        assert_eq!(DataType::Email.to_string(), "email");
        assert_eq!(DataType::Currency.to_string(), "currency");
    }
}
