//! Attribute Definition Macros
//!
//! This module provides macros for easily defining typed attributes with full
//! validation rules and metadata. These macros eliminate boilerplate and ensure
//! consistent implementation of the AttributeType trait.
//!
//! ## Usage
//!
//! ```rust
//! use ob_poc::define_string_attribute;
//!
//! define_string_attribute!(
//!     FirstName,
//!     id = "attr.identity.first_name",
//!     display_name = "First Name",
//!     category = Identity,
//!     required = true,
//!     min_length = 1,
//!     max_length = 100,
//!     pattern = r"^[A-Za-z\s\-']+$"
//! );
//! ```

/// Define a string-typed attribute with validation rules
///
/// # Example
/// ```ignore
/// define_string_attribute!(
///     LegalEntityName,
///     id = "attr.identity.legal_name",
///     display_name = "Legal Entity Name",
///     category = Identity,
///     required = true,
///     min_length = 1,
///     max_length = 255
/// );
/// ```
#[macro_export]
macro_rules! define_string_attribute {
    (
        $name:ident,
        id = $id:expr,
        uuid = $uuid:expr,
        display_name = $display_name:expr,
        category = $category:ident
        $(, required = $required:expr)?
        $(, min_length = $min_length:expr)?
        $(, max_length = $max_length:expr)?
        $(, pattern = $pattern:expr)?
        $(, allowed_values = [$($allowed:expr),* $(,)?])?
    ) => {
        pub struct $name;

        impl $crate::domains::attributes::types::AttributeType for $name {
            type Value = String;
            const ID: &'static str = $id;
            const UUID_STR: &'static str = $uuid;
            const DISPLAY_NAME: &'static str = $display_name;
            const CATEGORY: $crate::domains::attributes::types::AttributeCategory =
                $crate::domains::attributes::types::AttributeCategory::$category;
            const DATA_TYPE: $crate::domains::attributes::types::DataType =
                $crate::domains::attributes::types::DataType::String;

            fn validation_rules() -> $crate::domains::attributes::types::ValidationRules {
                let mut rules = $crate::domains::attributes::types::ValidationRules::new();

                $(rules.required = $required;)?
                $(rules.min_length = Some($min_length);)?
                $(rules.max_length = Some($max_length);)?
                $(rules.pattern = Some($pattern.to_string());)?
                $(rules.allowed_values = Some(vec![$($allowed.to_string()),*]);)?

                rules
            }

            fn validate(value: &Self::Value) -> Result<(), $crate::domains::attributes::types::ValidationError> {
                use $crate::domains::attributes::types::{validation_error, ValidationErrorType};

                let rules = Self::validation_rules();

                // Required check
                if rules.required && value.is_empty() {
                    return Err(validation_error(
                        Self::ID,
                        ValidationErrorType::Required,
                        &format!("{} is required", Self::DISPLAY_NAME),
                    ));
                }

                // Skip other validations if empty and not required
                if !rules.required && value.is_empty() {
                    return Ok(());
                }

                // Min length check
                if let Some(min) = rules.min_length {
                    if value.len() < min {
                        return Err(validation_error(
                            Self::ID,
                            ValidationErrorType::MinLength,
                            &format!("{} must be at least {} characters", Self::DISPLAY_NAME, min),
                        ));
                    }
                }

                // Max length check
                if let Some(max) = rules.max_length {
                    if value.len() > max {
                        return Err(validation_error(
                            Self::ID,
                            ValidationErrorType::MaxLength,
                            &format!("{} must not exceed {} characters", Self::DISPLAY_NAME, max),
                        ));
                    }
                }

                // Pattern check
                if let Some(pattern) = &rules.pattern {
                    let re = regex::Regex::new(pattern)
                        .map_err(|_| validation_error(
                            Self::ID,
                            ValidationErrorType::Pattern,
                            "Invalid regex pattern",
                        ))?;

                    if !re.is_match(value) {
                        return Err(validation_error(
                            Self::ID,
                            ValidationErrorType::Pattern,
                            &format!("{} format is invalid", Self::DISPLAY_NAME),
                        ));
                    }
                }

                // Allowed values check
                if let Some(allowed) = &rules.allowed_values {
                    if !allowed.contains(value) {
                        return Err(validation_error(
                            Self::ID,
                            ValidationErrorType::AllowedValues,
                            &format!("{} must be one of: {}", Self::DISPLAY_NAME, allowed.join(", ")),
                        ));
                    }
                }

                Ok(())
            }
        }
    };
}

/// Define a numeric attribute with validation rules
#[macro_export]
macro_rules! define_number_attribute {
    (
        $name:ident,
        id = $id:expr,
        uuid = $uuid:expr,
        display_name = $display_name:expr,
        category = $category:ident
        $(, required = $required:expr)?
        $(, min_value = $min_value:expr)?
        $(, max_value = $max_value:expr)?
    ) => {
        pub struct $name;

        impl $crate::domains::attributes::types::AttributeType for $name {
            type Value = f64;
            const ID: &'static str = $id;
            const UUID_STR: &'static str = $uuid;
            const DISPLAY_NAME: &'static str = $display_name;
            const CATEGORY: $crate::domains::attributes::types::AttributeCategory =
                $crate::domains::attributes::types::AttributeCategory::$category;
            const DATA_TYPE: $crate::domains::attributes::types::DataType =
                $crate::domains::attributes::types::DataType::Number;

            fn validation_rules() -> $crate::domains::attributes::types::ValidationRules {
                let mut rules = $crate::domains::attributes::types::ValidationRules::new();

                $(rules.required = $required;)?
                $(rules.min_value = Some($min_value);)?
                $(rules.max_value = Some($max_value);)?

                rules
            }

            fn validate(value: &Self::Value) -> Result<(), $crate::domains::attributes::types::ValidationError> {
                use $crate::domains::attributes::types::{validation_error, ValidationErrorType};

                let rules = Self::validation_rules();

                if value.is_nan() {
                    return Err(validation_error(
                        Self::ID,
                        ValidationErrorType::Custom,
                        &format!("{} must be a valid number", Self::DISPLAY_NAME),
                    ));
                }

                // Min value check
                if let Some(min) = rules.min_value {
                    if *value < min {
                        return Err(validation_error(
                            Self::ID,
                            ValidationErrorType::MinValue,
                            &format!("{} must be at least {}", Self::DISPLAY_NAME, min),
                        ));
                    }
                }

                // Max value check
                if let Some(max) = rules.max_value {
                    if *value > max {
                        return Err(validation_error(
                            Self::ID,
                            ValidationErrorType::MaxValue,
                            &format!("{} must not exceed {}", Self::DISPLAY_NAME, max),
                        ));
                    }
                }

                Ok(())
            }
        }
    };
}

/// Define an integer attribute with validation rules
#[macro_export]
macro_rules! define_integer_attribute {
    (
        $name:ident,
        id = $id:expr,
        uuid = $uuid:expr,
        display_name = $display_name:expr,
        category = $category:ident
        $(, required = $required:expr)?
        $(, min_value = $min_value:expr)?
        $(, max_value = $max_value:expr)?
    ) => {
        pub struct $name;

        impl $crate::domains::attributes::types::AttributeType for $name {
            type Value = i64;
            const ID: &'static str = $id;
            const UUID_STR: &'static str = $uuid;
            const DISPLAY_NAME: &'static str = $display_name;
            const CATEGORY: $crate::domains::attributes::types::AttributeCategory =
                $crate::domains::attributes::types::AttributeCategory::$category;
            const DATA_TYPE: $crate::domains::attributes::types::DataType =
                $crate::domains::attributes::types::DataType::Integer;

            fn validation_rules() -> $crate::domains::attributes::types::ValidationRules {
                let mut rules = $crate::domains::attributes::types::ValidationRules::new();

                $(rules.required = $required;)?
                $(rules.min_value = Some($min_value as f64);)?
                $(rules.max_value = Some($max_value as f64);)?

                rules
            }

            fn validate(value: &Self::Value) -> Result<(), $crate::domains::attributes::types::ValidationError> {
                use $crate::domains::attributes::types::{validation_error, ValidationErrorType};

                let rules = Self::validation_rules();

                // Min value check
                if let Some(min) = rules.min_value {
                    if (*value as f64) < min {
                        return Err(validation_error(
                            Self::ID,
                            ValidationErrorType::MinValue,
                            &format!("{} must be at least {}", Self::DISPLAY_NAME, min as i64),
                        ));
                    }
                }

                // Max value check
                if let Some(max) = rules.max_value {
                    if (*value as f64) > max {
                        return Err(validation_error(
                            Self::ID,
                            ValidationErrorType::MaxValue,
                            &format!("{} must not exceed {}", Self::DISPLAY_NAME, max as i64),
                        ));
                    }
                }

                Ok(())
            }
        }
    };
}

/// Define a boolean attribute
#[macro_export]
macro_rules! define_boolean_attribute {
    (
        $name:ident,
        id = $id:expr,
        uuid = $uuid:expr,
        display_name = $display_name:expr,
        category = $category:ident
    ) => {
        pub struct $name;

        impl $crate::domains::attributes::types::AttributeType for $name {
            type Value = bool;
            const ID: &'static str = $id;
            const UUID_STR: &'static str = $uuid;
            const DISPLAY_NAME: &'static str = $display_name;
            const CATEGORY: $crate::domains::attributes::types::AttributeCategory =
                $crate::domains::attributes::types::AttributeCategory::$category;
            const DATA_TYPE: $crate::domains::attributes::types::DataType =
                $crate::domains::attributes::types::DataType::Boolean;

            fn validation_rules() -> $crate::domains::attributes::types::ValidationRules {
                $crate::domains::attributes::types::ValidationRules::new()
            }

            fn validate(
                _value: &Self::Value,
            ) -> Result<(), $crate::domains::attributes::types::ValidationError> {
                Ok(()) // Booleans are always valid
            }
        }
    };
}

/// Define a date attribute
#[macro_export]
macro_rules! define_date_attribute {
    (
        $name:ident,
        id = $id:expr,
        uuid = $uuid:expr,
        display_name = $display_name:expr,
        category = $category:ident
        $(, required = $required:expr)?
    ) => {
        pub struct $name;

        impl $crate::domains::attributes::types::AttributeType for $name {
            type Value = String; // ISO 8601 date string
            const ID: &'static str = $id;
            const UUID_STR: &'static str = $uuid;
            const DISPLAY_NAME: &'static str = $display_name;
            const CATEGORY: $crate::domains::attributes::types::AttributeCategory =
                $crate::domains::attributes::types::AttributeCategory::$category;
            const DATA_TYPE: $crate::domains::attributes::types::DataType =
                $crate::domains::attributes::types::DataType::Date;

            fn validation_rules() -> $crate::domains::attributes::types::ValidationRules {
                let mut rules = $crate::domains::attributes::types::ValidationRules::new();
                $(rules.required = $required;)?
                // ISO 8601 date pattern: YYYY-MM-DD
                rules.pattern = Some(r"^\d{4}-\d{2}-\d{2}$".to_string());
                rules
            }

            fn validate(value: &Self::Value) -> Result<(), $crate::domains::attributes::types::ValidationError> {
                use $crate::domains::attributes::types::{validation_error, ValidationErrorType};

                let rules = Self::validation_rules();

                if rules.required && value.is_empty() {
                    return Err(validation_error(
                        Self::ID,
                        ValidationErrorType::Required,
                        &format!("{} is required", Self::DISPLAY_NAME),
                    ));
                }

                if !value.is_empty() {
                    if let Some(pattern) = &rules.pattern {
                        let re = regex::Regex::new(pattern)
                            .map_err(|_| validation_error(
                                Self::ID,
                                ValidationErrorType::Pattern,
                                "Invalid regex pattern",
                            ))?;

                        if !re.is_match(value) {
                            return Err(validation_error(
                                Self::ID,
                                ValidationErrorType::Pattern,
                                &format!("{} must be in YYYY-MM-DD format", Self::DISPLAY_NAME),
                            ));
                        }
                    }
                }

                Ok(())
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::domains::attributes::types::AttributeType;

    // Test the string attribute macro
    define_string_attribute!(
        TestName,
        id = "attr.test.name",
        uuid = "00000000-0000-0000-0000-000000000001",
        display_name = "Test Name",
        category = Identity,
        required = true,
        min_length = 1,
        max_length = 50
    );

    // Test the number attribute macro
    define_number_attribute!(
        TestAge,
        id = "attr.test.age",
        uuid = "00000000-0000-0000-0000-000000000002",
        display_name = "Test Age",
        category = Identity,
        min_value = 0.0,
        max_value = 150.0
    );

    // Test the boolean attribute macro
    define_boolean_attribute!(
        TestActive,
        id = "attr.test.active",
        uuid = "00000000-0000-0000-0000-000000000003",
        display_name = "Test Active",
        category = Compliance
    );

    #[test]
    fn test_string_attribute_macro() {
        assert_eq!(TestName::ID, "attr.test.name");
        assert_eq!(TestName::DISPLAY_NAME, "Test Name");

        // Valid value
        assert!(TestName::validate(&"John".to_string()).is_ok());

        // Empty (required)
        assert!(TestName::validate(&"".to_string()).is_err());

        // Too long
        assert!(TestName::validate(&"A".repeat(51)).is_err());
    }

    #[test]
    fn test_number_attribute_macro() {
        assert_eq!(TestAge::ID, "attr.test.age");

        // Valid value
        assert!(TestAge::validate(&25.0).is_ok());

        // Too low
        assert!(TestAge::validate(&-1.0).is_err());

        // Too high
        assert!(TestAge::validate(&151.0).is_err());
    }

    #[test]
    fn test_boolean_attribute_macro() {
        assert_eq!(TestActive::ID, "attr.test.active");

        // Both values are valid
        assert!(TestActive::validate(&true).is_ok());
        assert!(TestActive::validate(&false).is_ok());
    }
}
