//! DSL Normalization Module
//!
//! Implements alias transformation from legacy DSL forms to canonical v3.1 forms.
//! This module performs pre-validation normalization to ensure backward compatibility
//! while migrating the codebase to canonical verb and key naming conventions.

use crate::parser_ast::{Form, Key, Literal, PropertyMap, Value, VerbForm};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum NormalizationError {
    #[error("Failed to transform UBO link: {message}")]
    UboLinkTransformation { message: String },

    #[error("Invalid relationship type for control link: {relationship_type}")]
    InvalidControlRelationship { relationship_type: String },

    #[error("Missing required field in alias transformation: {field}")]
    MissingRequiredField { field: String },

    #[error("Value transformation error: {message}")]
    ValueTransformation { message: String },
}

/// DSL Normalizer for transforming legacy aliases to canonical forms
pub struct DslNormalizer {
    verb_aliases: HashMap<String, String>,
    key_aliases: HashMap<String, String>,
}

impl DslNormalizer {
    /// Create a new normalizer with predefined alias mappings
    pub fn new() -> Self {
        Self {
            verb_aliases: Self::init_verb_aliases(),
            key_aliases: Self::init_key_aliases(),
        }
    }

    /// Initialize verb alias mappings from A3 specification
    fn init_verb_aliases() -> HashMap<String, String> {
        [
            ("kyc.start_case", "case.create"),
            ("kyc.transition_state", "workflow.transition"),
            ("kyc.add_finding", "case.update"),
            ("kyc.approve_case", "case.approve"),
            ("ubo.link_ownership", "entity.link"),
            ("ubo.link_control", "entity.link"),
            ("ubo.add_evidence", "document.use"),
            ("ubo.update_link_status", "entity.link"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    /// Initialize key alias mappings from A3 specification
    fn init_key_aliases() -> HashMap<String, String> {
        [
            ("new_state", "to-state"),
            ("file_hash", "file-hash"),
            ("target_cbu_id", "target"),
            ("subject_entity_id", "target-entity"),
            ("label", "entity-type"),
            ("id", "entity-id"),
            ("approver_id", "approved-by"),
            ("status", "verification-status"),
            ("percent", "ownership-percentage"),
            ("from_entity", "from-entity"),
            ("to_entity", "to-entity"),
            ("link_id", "link-id"),
            ("finding_id", "note-id"),
            ("case_type", "case-type"),
            ("case_id", "case-id"),
            ("business_reference", "business-reference"),
            ("assigned_to", "assigned-to"),
            ("document_id", "document-id"),
            ("document_type", "document-type"),
            ("target_link_id", "evidence.of-link"),
        ]
        .iter()
        .map(|(k, v)| (format!(":{}", k), format!(":{}", v)))
        .collect()
    }

    /// Main normalization entry point - normalizes a complete program
    pub(crate) fn normalize_program(&self, program: &mut [Form]) -> Result<(), NormalizationError> {
        for form in program.iter_mut() {
            if let Form::Verb(verb_form) = form {
                self.normalize_verb_form(verb_form)?;
            }
        }
        Ok(())
    }

    /// Normalize a single verb form
    fn normalize_verb_form(&self, form: &mut VerbForm) -> Result<(), NormalizationError> {
        let original_verb = form.verb.clone();

        // Step 1: Apply verb aliases
        if let Some(canonical_verb) = self.verb_aliases.get(&form.verb) {
            form.verb = canonical_verb.clone();
        }

        // Step 2: Apply key aliases and value transformations
        self.normalize_keys(&mut form.pairs)?;

        // Step 3: Apply special transformations based on original verb
        match original_verb.as_str() {
            "ubo.link_ownership" => self.transform_ownership_link(form)?,
            "ubo.link_control" => self.transform_control_link(form)?,
            "ubo.add_evidence" => self.transform_evidence_link(form)?,
            "kyc.add_finding" => self.transform_finding_to_notes(form)?,
            _ => {}
        }

        Ok(())
    }

    /// Apply key aliases to all keys in the property map
    fn normalize_keys(&self, pairs: &mut PropertyMap) -> Result<(), NormalizationError> {
        let mut new_pairs = HashMap::new();

        for (key, value) in pairs.drain() {
            let key_str = key.as_str();
            let key_with_colon = format!(":{}", key_str);
            let canonical_key = if let Some(alias) = self.key_aliases.get(&key_with_colon) {
                Key::new(alias.strip_prefix(':').unwrap_or(alias))
            } else {
                key
            };

            // Recursively normalize values that might contain maps
            let normalized_value = self.normalize_value(value)?;
            new_pairs.insert(canonical_key, normalized_value);
        }

        *pairs = new_pairs;
        Ok(())
    }

    /// Recursively normalize values (maps may contain keys that need normalization)
    fn normalize_value(&self, value: Value) -> Result<Value, NormalizationError> {
        match value {
            Value::Map(mut map) => {
                self.normalize_keys(&mut map)?;
                Ok(Value::Map(map))
            }
            Value::List(list) => {
                let normalized_list: Result<Vec<_>, _> =
                    list.into_iter().map(|v| self.normalize_value(v)).collect();
                Ok(Value::List(normalized_list?))
            }
            // Other value types pass through unchanged
            other => Ok(other),
        }
    }

    /// Transform ubo.link_ownership to canonical entity.link with ownership relationship
    fn transform_ownership_link(&self, form: &mut VerbForm) -> Result<(), NormalizationError> {
        // Ensure we have the required fields
        let from_entity = self.extract_string_value(&form.pairs, "from-entity")?;
        let to_entity = self.extract_string_value(&form.pairs, "to-entity")?;

        // Get ownership percentage (may be under old 'percent' key or new 'ownership-percentage')
        let ownership_percentage = self
            .extract_numeric_value(&form.pairs, "ownership-percentage")
            .or_else(|_| self.extract_numeric_value(&form.pairs, "percent"))?;

        // Get verification status
        let verification_status = self
            .extract_string_value(&form.pairs, "verification-status")
            .unwrap_or_else(|_| "ALLEGED".to_string());

        // Create relationship-props map
        let mut relationship_props = HashMap::new();
        relationship_props.insert(
            Key::new("ownership-percentage"),
            Value::Double(ownership_percentage),
        );
        relationship_props.insert(
            Key::new("verification-status"),
            Value::String(verification_status),
        );

        // Add description if present
        if let Ok(description) = self.extract_string_value(&form.pairs, "description") {
            relationship_props.insert(Key::new("description"), Value::String(description));
        }

        // Rebuild the form with canonical structure
        let mut new_pairs = HashMap::new();

        // Preserve link-id if present
        if let Ok(link_id) = self.extract_string_value(&form.pairs, "link-id") {
            new_pairs.insert(Key::new("link-id"), Value::String(link_id));
        }

        new_pairs.insert(Key::new("from-entity"), Value::String(from_entity));
        new_pairs.insert(Key::new("to-entity"), Value::String(to_entity));
        new_pairs.insert(
            Key::new("relationship-type"),
            Value::String("OWNERSHIP".to_string()),
        );
        new_pairs.insert(
            Key::new("relationship-props"),
            Value::Map(relationship_props),
        );

        form.pairs = new_pairs;
        Ok(())
    }

    /// Transform ubo.link_control to canonical entity.link with control relationship
    fn transform_control_link(&self, form: &mut VerbForm) -> Result<(), NormalizationError> {
        let from_entity = self.extract_string_value(&form.pairs, "from-entity")?;
        let to_entity = self.extract_string_value(&form.pairs, "to-entity")?;

        // Determine control type - default to GENERAL_PARTNER if not specified
        let control_type = self
            .extract_string_value(&form.pairs, "control-type")
            .or_else(|_| self.extract_string_value(&form.pairs, "control_type"))
            .unwrap_or_else(|_| "GENERAL_PARTNER".to_string());

        let verification_status = self
            .extract_string_value(&form.pairs, "verification-status")
            .unwrap_or_else(|_| "ALLEGED".to_string());

        // Create relationship-props map
        let mut relationship_props = HashMap::new();
        relationship_props.insert(
            Key::new("verification-status"),
            Value::String(verification_status),
        );

        if let Ok(description) = self.extract_string_value(&form.pairs, "description") {
            relationship_props.insert(Key::new("description"), Value::String(description));
        }

        // Rebuild the form
        let mut new_pairs = HashMap::new();

        if let Ok(link_id) = self.extract_string_value(&form.pairs, "link-id") {
            new_pairs.insert(Key::new("link-id"), Value::String(link_id));
        }

        new_pairs.insert(Key::new("from-entity"), Value::String(from_entity));
        new_pairs.insert(Key::new("to-entity"), Value::String(to_entity));
        new_pairs.insert(Key::new("relationship-type"), Value::String(control_type));
        new_pairs.insert(
            Key::new("relationship-props"),
            Value::Map(relationship_props),
        );

        form.pairs = new_pairs;
        Ok(())
    }

    /// Transform ubo.add_evidence to canonical document.use
    fn transform_evidence_link(&self, form: &mut VerbForm) -> Result<(), NormalizationError> {
        let document_id = self.extract_string_value(&form.pairs, "document-id")?;

        // Map target_link_id to evidence.of-link
        let evidence_of_link = self
            .extract_string_value(&form.pairs, "evidence.of-link")
            .or_else(|_| self.extract_string_value(&form.pairs, "target-link-id"))
            .or_else(|_| self.extract_string_value(&form.pairs, "target_link_id"))?;

        let mut new_pairs = HashMap::new();
        new_pairs.insert(Key::new("document-id"), Value::String(document_id));
        new_pairs.insert(
            Key::new("used-by-process"),
            Value::String("UBO_ANALYSIS".to_string()),
        );
        new_pairs.insert(
            Key::new("usage-type"),
            Value::String("EVIDENCE".to_string()),
        );
        new_pairs.insert(
            Key::new("evidence.of-link"),
            Value::String(evidence_of_link),
        );

        // Preserve user-id if present
        if let Ok(user_id) = self.extract_string_value(&form.pairs, "user-id") {
            new_pairs.insert(Key::new("user-id"), Value::String(user_id));
        }

        form.pairs = new_pairs;
        Ok(())
    }

    /// Transform kyc.add_finding to case.update with notes
    fn transform_finding_to_notes(&self, form: &mut VerbForm) -> Result<(), NormalizationError> {
        let case_id = self.extract_string_value(&form.pairs, "case-id")?;
        let text = self
            .extract_string_value(&form.pairs, "text")
            .or_else(|_| self.extract_string_value(&form.pairs, "finding"))?;

        // Create note entry with optional note-id
        let note_entry = if let Ok(note_id) = self.extract_string_value(&form.pairs, "note-id") {
            format!("{}: {}", note_id, text)
        } else {
            text
        };

        let mut new_pairs = HashMap::new();
        new_pairs.insert(Key::new("case-id"), Value::String(case_id));
        new_pairs.insert(Key::new("notes"), Value::String(note_entry));

        form.pairs = new_pairs;
        Ok(())
    }

    /// Helper to extract string values from property map
    fn extract_string_value(
        &self,
        pairs: &PropertyMap,
        key: &str,
    ) -> Result<String, NormalizationError> {
        let key_obj = Key::new(key);
        match pairs.get(&key_obj) {
            Some(Value::String(s)) => Ok(s.clone()),
            Some(Value::Identifier(s)) => Ok(s.clone()),
            Some(Value::Literal(Literal::String(s))) => Ok(s.clone()),
            Some(_) => Err(NormalizationError::ValueTransformation {
                message: format!("Expected string value for key '{}'", key),
            }),
            None => Err(NormalizationError::MissingRequiredField {
                field: key.to_string(),
            }),
        }
    }

    /// Helper to extract numeric values from property map
    fn extract_numeric_value(
        &self,
        pairs: &PropertyMap,
        key: &str,
    ) -> Result<f64, NormalizationError> {
        let key_obj = Key::new(key);
        match pairs.get(&key_obj) {
            Some(Value::Double(n)) => Ok(*n),
            Some(Value::Integer(i)) => Ok(*i as f64),
            Some(Value::Literal(Literal::Number(n))) => Ok(*n),
            Some(_) => Err(NormalizationError::ValueTransformation {
                message: format!("Expected numeric value for key '{}'", key),
            }),
            None => Err(NormalizationError::MissingRequiredField {
                field: key.to_string(),
            }),
        }
    }
}

impl Default for DslNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Literal, Value};

    #[test]
    fn test_verb_alias_mapping() {
        let normalizer = DslNormalizer::new();

        let mut form = VerbForm {
            verb: "kyc.start_case".to_string(),
            pairs: HashMap::new(),
        };

        normalizer.normalize_verb_form(&mut form).unwrap();
        assert_eq!(form.verb, "case.create");
    }

    #[test]
    fn test_key_alias_mapping() {
        let normalizer = DslNormalizer::new();

        let mut pairs = HashMap::new();
        pairs.insert(Key::new("new_state"), Value::String("approved".to_string()));
        pairs.insert(Key::new("file_hash"), Value::String("abc123".to_string()));

        normalizer.normalize_keys(&mut pairs).unwrap();

        assert!(pairs.contains_key(&Key::new("to-state")));
        assert!(pairs.contains_key(&Key::new("file-hash")));
        assert!(!pairs.contains_key(&Key::new("new_state")));
        assert!(!pairs.contains_key(&Key::new("file_hash")));
    }

    #[test]
    fn test_ownership_link_transformation() {
        let normalizer = DslNormalizer::new();

        let mut pairs = HashMap::new();
        pairs.insert(
            Key::new("from-entity"),
            Value::String("person-1".to_string()),
        );
        pairs.insert(
            Key::new("to-entity"),
            Value::String("company-1".to_string()),
        );
        pairs.insert(Key::new("ownership-percentage"), Value::Double(60.0));
        pairs.insert(
            Key::new("verification-status"),
            Value::String("ALLEGED".to_string()),
        );

        let mut form = VerbForm {
            verb: "ubo.link_ownership".to_string(),
            pairs,
        };

        normalizer.normalize_verb_form(&mut form).unwrap();

        assert_eq!(form.verb, "entity.link");
        assert_eq!(
            form.pairs.get(&Key::new("relationship-type")).unwrap(),
            &Value::String("OWNERSHIP".to_string())
        );

        if let Some(Value::Map(props)) = form.pairs.get(&Key::new("relationship-props")) {
            assert_eq!(
                props.get(&Key::new("ownership-percentage")).unwrap(),
                &Value::Double(60.0)
            );
        } else {
            panic!("Expected relationship-props map");
        }
    }

    #[test]
    fn test_evidence_link_transformation() {
        let normalizer = DslNormalizer::new();

        let mut pairs = HashMap::new();
        pairs.insert(
            Key::new("document-id"),
            Value::String("doc-001".to_string()),
        );
        pairs.insert(
            Key::new("target_link_id"),
            Value::String("link-001".to_string()),
        );

        let mut form = VerbForm {
            verb: "ubo.add_evidence".to_string(),
            pairs,
        };

        normalizer.normalize_verb_form(&mut form).unwrap();

        assert_eq!(form.verb, "document.use");
        assert_eq!(
            form.pairs.get(&Key::new("usage-type")).unwrap(),
            &Value::String("EVIDENCE".to_string())
        );
        assert_eq!(
            form.pairs.get(&Key::new("evidence.of-link")).unwrap(),
            &Value::String("link-001".to_string())
        );
    }

    #[test]
    fn test_finding_to_notes_transformation() {
        let normalizer = DslNormalizer::new();

        let mut pairs = HashMap::new();
        pairs.insert(Key::new("case-id"), Value::String("kyc-001".to_string()));
        pairs.insert(
            Key::new("text"),
            Value::String("Sample finding".to_string()),
        );
        pairs.insert(Key::new("note-id"), Value::String("note-001".to_string()));

        let mut form = VerbForm {
            verb: "kyc.add_finding".to_string(),
            pairs,
        };

        normalizer.normalize_verb_form(&mut form).unwrap();

        assert_eq!(form.verb, "case.update");
        assert_eq!(
            form.pairs.get(&Key::new("notes")).unwrap(),
            &Value::String("note-001: Sample finding".to_string())
        );
    }

    #[test]
    fn test_nested_map_normalization() {
        let normalizer = DslNormalizer::new();

        let mut inner_map = HashMap::new();
        inner_map.insert(Key::new("file_hash"), Value::String("abc123".to_string()));

        let mut pairs = HashMap::new();
        pairs.insert(Key::new("props"), Value::Map(inner_map));

        normalizer.normalize_keys(&mut pairs).unwrap();

        if let Some(Value::Map(inner)) = pairs.get(&Key::new("props")) {
            assert!(inner.contains_key(&Key::new("file-hash")));
            assert!(!inner.contains_key(&Key::new("file_hash")));
        } else {
            panic!("Expected nested map");
        }
    }

    #[test]
    fn test_ubo_link_ownership_debug() {
        let normalizer = DslNormalizer::new();

        // Create exact test case that's failing
        let mut pairs = HashMap::new();
        pairs.insert(
            Key::new("from_entity"),
            Value::String("entity-1".to_string()),
        );
        pairs.insert(Key::new("to_entity"), Value::String("entity-2".to_string()));
        pairs.insert(Key::new("percent"), Value::Double(60.0));
        pairs.insert(Key::new("status"), Value::String("alleged".to_string()));

        let mut form = VerbForm {
            verb: "ubo.link_ownership".to_string(),
            pairs,
        };

        println!(
            "Before normalization: {:?}",
            form.pairs.keys().map(|k| k.as_str()).collect::<Vec<_>>()
        );

        let result = normalizer.normalize_verb_form(&mut form);

        println!("After normalization - verb: {}", form.verb);
        println!(
            "After normalization - keys: {:?}",
            form.pairs.keys().map(|k| k.as_str()).collect::<Vec<_>>()
        );

        if let Err(e) = &result {
            println!("Normalization error: {:?}", e);
        }

        assert!(
            result.is_ok(),
            "UBO link normalization should succeed: {:?}",
            result.err()
        );
        assert_eq!(form.verb, "entity.link");
    }

    #[test]
    fn test_canonical_dsl_requires_no_normalization() {
        // Test that canonical DSL passes through unchanged
        let canonical_dsl = r#"
        (case.create :case-type "KYC_CASE" :business-reference "KYC-2025-001")
        (workflow.transition :to-state "collecting-documents")
        (entity.link :from-entity "P1" :to-entity "E1"
                     :relationship-type "OWNERSHIP"
                     :relationship-props {:ownership-percentage 60.0 :verification-status "ALLEGED"})
        "#;

        use crate::parser::parse_and_normalize;

        let result = parse_and_normalize(canonical_dsl);
        assert!(result.is_ok());

        let program = result.unwrap();
        let verb_forms: Vec<_> = program
            .iter()
            .filter_map(|f| match f {
                crate::Form::Verb(vf) => Some(vf),
                _ => None,
            })
            .collect();

        // Verify no changes were made to canonical forms
        assert_eq!(verb_forms[0].verb, "case.create");
        assert_eq!(verb_forms[1].verb, "workflow.transition");
        assert_eq!(verb_forms[2].verb, "entity.link");

        // Verify canonical keys remain unchanged
        assert!(verb_forms[1].pairs.contains_key(&Key::new("to-state")));
        assert!(verb_forms[2]
            .pairs
            .contains_key(&Key::new("relationship-props")));
    }

    #[test]
    fn test_mixed_legacy_and_canonical_dsl() {
        // Test mixing legacy and canonical forms in same document
        let mixed_dsl = r#"
        (kyc.start_case :case_type "KYC_CASE")
        (workflow.transition :to-state "approved")
        (ubo.link_ownership :from_entity "P1" :to_entity "E1" :percent 100.0)
        (case.approve :approved-by "officer-1")
        "#;

        use crate::parser::parse_and_normalize;

        let result = parse_and_normalize(mixed_dsl);
        assert!(result.is_ok());

        let program = result.unwrap();
        let verb_forms: Vec<_> = program
            .iter()
            .filter_map(|f| match f {
                crate::Form::Verb(vf) => Some(vf),
                _ => None,
            })
            .collect();

        // All should be normalized to canonical
        assert_eq!(verb_forms[0].verb, "case.create");
        assert_eq!(verb_forms[1].verb, "workflow.transition"); // Already canonical
        assert_eq!(verb_forms[2].verb, "entity.link");
        assert_eq!(verb_forms[3].verb, "case.approve"); // Already canonical

        // Verify mixed normalization worked correctly
        assert!(verb_forms[1].pairs.contains_key(&Key::new("to-state")));
        assert!(verb_forms[3].pairs.contains_key(&Key::new("approved-by")));
    }
}
