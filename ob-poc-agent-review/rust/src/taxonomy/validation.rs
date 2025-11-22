//! Comprehensive validation for taxonomy operations
//!
//! Validates service options, dependencies, and business rules.

use crate::models::taxonomy::{ServiceOptionChoice, ServiceOptionDefinition};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.is_valid = false;
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

pub struct OptionValidator {
    definitions: HashMap<Uuid, ServiceOptionDefinition>,
    choices: HashMap<Uuid, Vec<ServiceOptionChoice>>,
}

impl OptionValidator {
    pub fn new(
        definitions: Vec<ServiceOptionDefinition>,
        choices_map: HashMap<Uuid, Vec<ServiceOptionChoice>>,
    ) -> Self {
        let definitions = definitions
            .into_iter()
            .map(|def| (def.option_def_id, def))
            .collect();

        Self {
            definitions,
            choices: choices_map,
        }
    }

    /// Validate service option selections
    pub fn validate_options(&self, options: &serde_json::Value) -> ValidationResult {
        let mut result = ValidationResult::valid();

        let obj = match options.as_object() {
            Some(obj) => obj,
            None => {
                result.add_error("Options must be a JSON object".to_string());
                return result;
            }
        };

        // Check each provided option
        for (key, value) in obj {
            // Find the definition for this option
            let def = self.definitions.values().find(|d| d.option_key == *key);

            match def {
                None => {
                    result.add_error(format!("Unknown option key: {}", key));
                }
                Some(def) => {
                    // Validate based on option type
                    match def.option_type.as_str() {
                        "single_select" => {
                            self.validate_single_select(def, value, &mut result);
                        }
                        "multi_select" => {
                            self.validate_multi_select(def, value, &mut result);
                        }
                        "numeric" => {
                            self.validate_numeric(def, value, &mut result);
                        }
                        "boolean" => {
                            self.validate_boolean(value, &mut result);
                        }
                        "text" => {
                            self.validate_text(def, value, &mut result);
                        }
                        _ => {
                            result.add_error(format!("Unknown option type: {}", def.option_type));
                        }
                    }
                }
            }
        }

        // Check for missing required options
        for def in self.definitions.values() {
            if def.is_required.unwrap_or(false) && !obj.contains_key(&def.option_key) {
                result.add_error(format!("Required option missing: {}", def.option_key));
            }
        }

        // Validate dependencies
        self.validate_dependencies(obj, &mut result);

        result
    }

    fn validate_single_select(
        &self,
        def: &ServiceOptionDefinition,
        value: &serde_json::Value,
        result: &mut ValidationResult,
    ) {
        let selected = match value.as_str() {
            Some(s) => s,
            None => {
                result.add_error(format!(
                    "Option '{}' must be a string for single_select",
                    def.option_key
                ));
                return;
            }
        };

        // Check if the value is in allowed choices
        if let Some(choices) = self.choices.get(&def.option_def_id) {
            if !choices
                .iter()
                .any(|c| c.choice_value == selected && c.is_active.unwrap_or(true))
            {
                result.add_error(format!(
                    "Invalid choice '{}' for option '{}'",
                    selected, def.option_key
                ));
            }
        }
    }

    fn validate_multi_select(
        &self,
        def: &ServiceOptionDefinition,
        value: &serde_json::Value,
        result: &mut ValidationResult,
    ) {
        let selected = match value.as_array() {
            Some(arr) => arr,
            None => {
                result.add_error(format!(
                    "Option '{}' must be an array for multi_select",
                    def.option_key
                ));
                return;
            }
        };

        // Check each selected value
        if let Some(choices) = self.choices.get(&def.option_def_id) {
            for sel_val in selected {
                if let Some(sel_str) = sel_val.as_str() {
                    if !choices
                        .iter()
                        .any(|c| c.choice_value == sel_str && c.is_active.unwrap_or(true))
                    {
                        result.add_error(format!(
                            "Invalid choice '{}' in multi_select option '{}'",
                            sel_str, def.option_key
                        ));
                    }
                } else {
                    result.add_error(format!(
                        "Multi-select values must be strings in option '{}'",
                        def.option_key
                    ));
                }
            }
        }
    }

    fn validate_numeric(
        &self,
        def: &ServiceOptionDefinition,
        value: &serde_json::Value,
        result: &mut ValidationResult,
    ) {
        if !value.is_number() {
            result.add_error(format!(
                "Option '{}' must be a number for numeric type",
                def.option_key
            ));
            return;
        }

        // Could add min/max validation if defined in validation_rules
        if let Some(rules) = &def.validation_rules {
            if let Some(min) = rules.get("min").and_then(|v| v.as_f64()) {
                if let Some(num) = value.as_f64() {
                    if num < min {
                        result.add_error(format!(
                            "Option '{}' value {} is below minimum {}",
                            def.option_key, num, min
                        ));
                    }
                }
            }
            if let Some(max) = rules.get("max").and_then(|v| v.as_f64()) {
                if let Some(num) = value.as_f64() {
                    if num > max {
                        result.add_error(format!(
                            "Option '{}' value {} exceeds maximum {}",
                            def.option_key, num, max
                        ));
                    }
                }
            }
        }
    }

    fn validate_boolean(&self, value: &serde_json::Value, result: &mut ValidationResult) {
        if !value.is_boolean() {
            result.add_error("Boolean option must be true or false".to_string());
        }
    }

    fn validate_text(
        &self,
        def: &ServiceOptionDefinition,
        value: &serde_json::Value,
        result: &mut ValidationResult,
    ) {
        let text = match value.as_str() {
            Some(s) => s,
            None => {
                result.add_error(format!(
                    "Option '{}' must be a string for text type",
                    def.option_key
                ));
                return;
            }
        };

        // Validate text length if validation_rules exist
        if let Some(rules) = &def.validation_rules {
            if let Some(max_len) = rules.get("max_length").and_then(|v| v.as_u64()) {
                if text.len() > max_len as usize {
                    result.add_error(format!(
                        "Option '{}' exceeds maximum length of {}",
                        def.option_key, max_len
                    ));
                }
            }
            if let Some(min_len) = rules.get("min_length").and_then(|v| v.as_u64()) {
                if text.len() < min_len as usize {
                    result.add_error(format!(
                        "Option '{}' is below minimum length of {}",
                        def.option_key, min_len
                    ));
                }
            }
        }
    }

    fn validate_dependencies(
        &self,
        options: &serde_json::Map<String, serde_json::Value>,
        result: &mut ValidationResult,
    ) {
        // Check conditional dependencies defined in validation_rules
        for def in self.definitions.values() {
            if let Some(rules) = &def.validation_rules {
                if let Some(depends_on) = rules.get("depends_on").and_then(|v| v.as_str()) {
                    if options.contains_key(&def.option_key) {
                        // This option is selected, check if dependency is met
                        if !options.contains_key(depends_on) {
                            result.add_error(format!(
                                "Option '{}' requires '{}' to be set",
                                def.option_key, depends_on
                            ));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::valid();
        assert!(result.is_valid);

        result.add_error("Test error".to_string());
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_missing_required_option() {
        let def = ServiceOptionDefinition {
            option_def_id: Uuid::new_v4(),
            service_id: Uuid::new_v4(),
            option_key: "required_field".to_string(),
            option_label: Some("Required Field".to_string()),
            option_type: "text".to_string(),
            validation_rules: None,
            is_required: Some(true),
            display_order: Some(1),
            help_text: None,
        };

        let validator = OptionValidator::new(vec![def], HashMap::new());
        let options = serde_json::json!({});

        let result = validator.validate_options(&options);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("required_field")));
    }
}
