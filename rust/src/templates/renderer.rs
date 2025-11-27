//! Template Renderer - Convert filled slots to DSL

use super::slot_types::{FormTemplate, SlotType};
use serde_json::Value;
use std::collections::HashMap;

pub struct TemplateRenderer;

impl TemplateRenderer {
    /// Render a template with filled slot values to DSL
    pub fn render(
        template: &FormTemplate,
        values: &HashMap<String, Value>,
    ) -> Result<String, RenderError> {
        // Validate required slots
        for slot in template.required_slots() {
            if !values.contains_key(&slot.name) {
                return Err(RenderError::MissingRequired(slot.name.clone()));
            }
        }

        // Build DSL s-expression
        let mut parts = vec![format!("({}", template.verb)];

        for slot in &template.slots {
            if let Some(value) = values.get(&slot.name) {
                let dsl_param = slot.dsl_param_name();
                let dsl_value = Self::value_to_dsl(value, &slot.slot_type)?;
                parts.push(format!(":{} {}", dsl_param, dsl_value));
            }
        }

        parts.push(")".to_string());

        Ok(parts.join(" "))
    }

    /// Convert a JSON value to DSL string based on slot type
    fn value_to_dsl(value: &Value, slot_type: &SlotType) -> Result<String, RenderError> {
        match (value, slot_type) {
            // String types
            (Value::String(s), SlotType::Text { .. })
            | (Value::String(s), SlotType::Date)
            | (Value::String(s), SlotType::Country)
            | (Value::String(s), SlotType::Currency)
            | (Value::String(s), SlotType::Enum { .. }) => {
                Ok(format!("\"{}\"", s.replace('\"', "\\\"")))
            }

            // UUID (entity references)
            (Value::String(s), SlotType::EntityRef { .. })
            | (Value::String(s), SlotType::Uuid { .. }) => Ok(format!("\"{}\"", s)),

            // Numbers
            (Value::Number(n), SlotType::Integer { .. }) => Ok(n.to_string()),
            (Value::Number(n), SlotType::Decimal { .. })
            | (Value::Number(n), SlotType::Percentage)
            | (Value::Number(n), SlotType::Money { .. }) => Ok(n.to_string()),

            // Boolean
            (Value::Bool(b), SlotType::Boolean) => {
                Ok(if *b { "true" } else { "false" }.to_string())
            }

            // Type mismatches - try to coerce
            (Value::String(s), SlotType::Integer { .. }) => s
                .parse::<i64>()
                .map(|n| n.to_string())
                .map_err(|_| RenderError::TypeMismatch {
                    slot: String::new(),
                    expected: "integer".into(),
                    got: "string".into(),
                }),

            (Value::String(s), SlotType::Percentage)
            | (Value::String(s), SlotType::Decimal { .. }) => s
                .parse::<f64>()
                .map(|n| n.to_string())
                .map_err(|_| RenderError::TypeMismatch {
                    slot: String::new(),
                    expected: "number".into(),
                    got: "string".into(),
                }),

            _ => Err(RenderError::TypeMismatch {
                slot: String::new(),
                expected: format!("{:?}", slot_type),
                got: format!("{:?}", value),
            }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Missing required slot: {0}")]
    MissingRequired(String),

    #[error("Type mismatch for slot '{slot}': expected {expected}, got {got}")]
    TypeMismatch {
        slot: String,
        expected: String,
        got: String,
    },

    #[error("Invalid value: {0}")]
    InvalidValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::TemplateRegistry;

    #[test]
    fn test_render_create_cbu() {
        let registry = TemplateRegistry::new();
        let template = registry.get("cbu.create").unwrap();

        let mut values = HashMap::new();
        values.insert("cbu_name".into(), Value::String("Apex Capital".into()));
        values.insert("client_type".into(), Value::String("COMPANY".into()));
        values.insert("jurisdiction".into(), Value::String("GB".into()));

        let dsl = TemplateRenderer::render(template, &values).unwrap();

        assert!(dsl.starts_with("(cbu.ensure"));
        assert!(dsl.contains(":cbu-name \"Apex Capital\""));
        assert!(dsl.contains(":client-type \"COMPANY\""));
        assert!(dsl.contains(":jurisdiction \"GB\""));
    }
}
