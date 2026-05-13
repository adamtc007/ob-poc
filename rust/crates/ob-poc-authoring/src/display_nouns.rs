//! Display Noun Middleware
//!
//! Translates internal terminology to operator vocabulary in API responses.
//! This ensures operators see business terms (structure, case, mandate) instead
//! of internal terms (cbu, kyc-case, trading-profile).
//!
//! ## Mappings
//!
//! | Internal | Display |
//! |----------|---------|
//! | cbu | structure |
//! | entity_ref | party |
//! | trading-profile | mandate |
//! | kyc-case | case |
//!
//! ## Usage
//!
//! Apply to response strings or JSON values before sending to client.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Noun translation mappings (internal → display)
static NOUN_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Primary entity type mappings
    m.insert("cbu", "structure");
    m.insert("CBU", "Structure");
    m.insert("Cbu", "Structure");

    m.insert("trading-profile", "mandate");
    m.insert("trading_profile", "mandate");
    m.insert("TradingProfile", "Mandate");

    m.insert("kyc-case", "case");
    m.insert("kyc_case", "case");
    m.insert("KycCase", "Case");

    m.insert("entity_ref", "party");
    m.insert("EntityRef", "Party");

    // Field name mappings
    m.insert("cbu_id", "structure_id");
    m.insert("cbu_name", "structure_name");
    m.insert("trading_profile_id", "mandate_id");
    m.insert("kyc_case_id", "case_id");

    // Verb domain mappings
    m.insert("cbu.create", "structure.create");
    m.insert("cbu.assign-role", "structure.assign-role");
    m.insert("kyc-case.create", "case.create");
    m.insert("kyc-case.open", "case.open");

    m
});

/// Error message translations
static ERROR_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("CBU not found", "Structure not found");
    m.insert("Entity not found", "Party not found");
    m.insert("Trading profile not found", "Mandate not found");
    m.insert("KYC case not found", "Case not found");
    m.insert("cbu_id is required", "structure_id is required");
    m
});

/// Translate a string by replacing internal terms with display terms
pub fn translate_string(input: &str) -> String {
    let mut result = input.to_string();

    // Apply noun mappings
    for (internal, display) in NOUN_MAP.iter() {
        result = result.replace(internal, display);
    }

    // Apply error mappings
    for (internal, display) in ERROR_MAP.iter() {
        result = result.replace(internal, display);
    }

    result
}

/// Translate a JSON value recursively
pub fn translate_json(value: &mut Value) {
    match value {
        Value::String(s) => {
            *s = translate_string(s);
        }
        Value::Array(arr) => {
            for item in arr {
                translate_json(item);
            }
        }
        Value::Object(obj) => {
            // Collect keys to rename
            let keys_to_rename: Vec<_> = obj
                .keys()
                .filter_map(|k| {
                    NOUN_MAP
                        .get(k.as_str())
                        .map(|new| (k.clone(), new.to_string()))
                })
                .collect();

            // Rename keys
            for (old_key, new_key) in keys_to_rename {
                if let Some(v) = obj.remove(&old_key) {
                    obj.insert(new_key, v);
                }
            }

            // Recurse into values
            for v in obj.values_mut() {
                translate_json(v);
            }
        }
        _ => {}
    }
}

/// Display noun configuration for customization
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisplayNounConfig {
    /// Noun translations (internal → display)
    pub nouns: HashMap<String, String>,
    /// Error message translations
    pub errors: HashMap<String, String>,
}

impl Default for DisplayNounConfig {
    fn default() -> Self {
        Self {
            nouns: NOUN_MAP
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            errors: ERROR_MAP
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}

/// Translator with custom configuration
pub struct DisplayNounTranslator {
    nouns: HashMap<String, String>,
    errors: HashMap<String, String>,
}

impl DisplayNounTranslator {
    /// Create with default mappings
    pub fn new() -> Self {
        let config = DisplayNounConfig::default();
        Self {
            nouns: config.nouns,
            errors: config.errors,
        }
    }

    /// Create from config
    pub fn from_config(config: DisplayNounConfig) -> Self {
        Self {
            nouns: config.nouns,
            errors: config.errors,
        }
    }

    /// Translate a string
    pub fn translate_string(&self, input: &str) -> String {
        let mut result = input.to_string();

        for (internal, display) in &self.nouns {
            result = result.replace(internal, display);
        }

        for (internal, display) in &self.errors {
            result = result.replace(internal, display);
        }

        result
    }

    /// Translate a JSON value recursively
    pub fn translate_json(&self, value: &mut Value) {
        match value {
            Value::String(s) => {
                *s = self.translate_string(s);
            }
            Value::Array(arr) => {
                for item in arr {
                    self.translate_json(item);
                }
            }
            Value::Object(obj) => {
                // Collect keys to rename
                let keys_to_rename: Vec<_> = obj
                    .keys()
                    .filter_map(|k| self.nouns.get(k).map(|new| (k.clone(), new.clone())))
                    .collect();

                // Rename keys
                for (old_key, new_key) in keys_to_rename {
                    if let Some(v) = obj.remove(&old_key) {
                        obj.insert(new_key, v);
                    }
                }

                // Recurse into values
                for v in obj.values_mut() {
                    self.translate_json(v);
                }
            }
            _ => {}
        }
    }
}

impl Default for DisplayNounTranslator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_string() {
        assert_eq!(translate_string("cbu_id"), "structure_id");
        assert_eq!(translate_string("CBU not found"), "Structure not found");
        assert_eq!(
            translate_string("Created new cbu with cbu_id=123"),
            "Created new structure with structure_id=123"
        );
    }

    #[test]
    fn test_translate_json() {
        let mut value = serde_json::json!({
            "cbu_id": "123",
            "cbu_name": "Test Fund",
            "nested": {
                "trading_profile_id": "456"
            },
            "message": "CBU not found"
        });

        translate_json(&mut value);

        assert_eq!(value["structure_id"], "123");
        assert_eq!(value["structure_name"], "Test Fund");
        assert_eq!(value["nested"]["mandate_id"], "456");
        assert_eq!(value["message"], "Structure not found");
    }

    #[test]
    fn test_translator_instance() {
        let translator = DisplayNounTranslator::new();

        assert_eq!(
            translator.translate_string("kyc-case.create"),
            "case.create"
        );
    }

    #[test]
    fn test_array_translation() {
        let mut value = serde_json::json!({
            "items": [
                {"cbu_id": "1"},
                {"cbu_id": "2"}
            ]
        });

        translate_json(&mut value);

        assert_eq!(value["items"][0]["structure_id"], "1");
        assert_eq!(value["items"][1]["structure_id"], "2");
    }
}
