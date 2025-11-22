//! Attribute validation logic

use super::*;

pub fn validate_attribute_value(
    attr: &AttributeDefinition,
    value: &serde_json::Value,
) -> Result<(), String> {
    // Type checking
    match attr.data_type {
        DataType::Numeric | DataType::Percentage => {
            if !value.is_f64() && !value.is_i64() {
                return Err(format!("Expected numeric value for {}", attr.attr_id));
            }
        }
        DataType::String | DataType::Email | DataType::Phone => {
            if !value.is_string() {
                return Err(format!("Expected string value for {}", attr.attr_id));
            }
        }
        _ => {}
    }

    // Constraint checking
    if let Some(constraints) = &attr.constraints {
        if constraints.required && value.is_null() {
            return Err(format!("Required attribute {} is null", attr.attr_id));
        }

        if let Some(min) = constraints.min {
            if let Some(num) = value.as_f64() {
                if num < min {
                    return Err(format!("Value {} below minimum {}", num, min));
                }
            }
        }

        if let Some(max) = constraints.max {
            if let Some(num) = value.as_f64() {
                if num > max {
                    return Err(format!("Value {} above maximum {}", num, max));
                }
            }
        }
    }

    Ok(())
}
