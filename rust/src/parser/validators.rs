//! Enhanced DSL Validators for Phase 2 Implementation
//!
//! This module implements enhanced validation logic for the KYC orchestration DSL,
//! supporting new semantics including link identity, append-only notes, and evidence linking.

use crate::ast::types::{ErrorSeverity, SourceLocation, ValidationError, ValidationWarning};
use crate::{Form, Key, PropertyMap, Value, VerbForm};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidatorError {
    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },

    #[error("Invalid field type for {field}: expected {expected}, got {actual}")]
    InvalidFieldType {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("Invalid link identity: {message}")]
    InvalidLinkIdentity { message: String },

    #[error("Evidence linking error: {message}")]
    EvidenceLinkingError { message: String },

    #[error("Invalid relationship structure: {message}")]
    InvalidRelationshipStructure { message: String },

    #[error("Note format validation failed: {message}")]
    NoteFormatError { message: String },
}

/// Enhanced validator for DSL forms with new Phase 2 semantics
pub struct DslValidator {
    /// Known entity IDs for reference validation
    entity_registry: HashSet<String>,
    /// Known document IDs for reference validation
    document_registry: HashSet<String>,
    /// Link ID registry for tracking updates
    link_registry: HashMap<String, LinkInfo>,
    /// Case ID registry for tracking notes
    case_registry: HashMap<String, CaseInfo>,
}

#[derive(Debug, Clone)]
struct LinkInfo {
    from_entity: String,
    to_entity: String,
    relationship_type: String,
    first_seen_location: Option<usize>, // Form index where first defined
}

#[derive(Debug, Clone)]
struct CaseInfo {
    case_type: Option<String>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub suggestions: Vec<String>,
}

impl DslValidator {
    /// Create a new enhanced validator
    pub fn new() -> Self {
        Self {
            entity_registry: HashSet::new(),
            document_registry: HashSet::new(),
            link_registry: HashMap::new(),
            case_registry: HashMap::new(),
        }
    }

    /// Validate a complete DSL program with enhanced semantics
    pub fn validate_program(&mut self, program: &[Form]) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        // First pass: register entities, documents, and cases
        self.register_definitions(program, &mut errors);

        // Second pass: validate forms with cross-references
        for (index, form) in program.iter().enumerate() {
            if let Form::Verb(verb_form) = form {
                match self.validate_verb_form(verb_form, index) {
                    Ok(result) => {
                        errors.extend(result.errors);
                        warnings.extend(result.warnings);
                        suggestions.extend(result.suggestions);
                    }
                    Err(validator_error) => {
                        errors.push(ValidationError {
                            code: "VALIDATOR_ERROR".to_string(),
                            message: validator_error.to_string(),
                            severity: ErrorSeverity::Error,
                            location: Some(SourceLocation {
                                line: index + 1,
                                column: 1,
                                file: None,
                                span: None,
                            }),
                            suggestions: vec![],
                        });
                    }
                }
            }
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            suggestions,
        }
    }

    /// Register entity, document, and case definitions for reference validation
    fn register_definitions(&mut self, program: &[Form], _errors: &mut Vec<ValidationError>) {
        for form in program {
            if let Form::Verb(verb_form) = form {
                match verb_form.verb.as_str() {
                    "entity.register" => {
                        if let Ok(entity_id) =
                            self.extract_string_value(&verb_form.pairs, "entity-id")
                        {
                            self.entity_registry.insert(entity_id);
                        }
                    }
                    "document.catalog" => {
                        if let Ok(doc_id) =
                            self.extract_string_value(&verb_form.pairs, "document-id")
                        {
                            self.document_registry.insert(doc_id);
                        }
                    }
                    "case.create" => {
                        if let Ok(case_id) = self.extract_string_value(&verb_form.pairs, "case-id")
                        {
                            let case_type = self
                                .extract_string_value(&verb_form.pairs, "case-type")
                                .ok();
                            self.case_registry.insert(
                                case_id,
                                CaseInfo {
                                    case_type,
                                    notes: Vec::new(),
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Validate individual verb form with enhanced semantics
    fn validate_verb_form(
        &mut self,
        form: &VerbForm,
        form_index: usize,
    ) -> Result<ValidationResult, ValidatorError> {
        let mut result = ValidationResult {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
        };

        match form.verb.as_str() {
            "entity.link" => {
                self.validate_entity_link_with_updates(form, form_index, &mut result)?
            }
            "case.update" => self.validate_case_update_notes(form, &mut result)?,
            "document.use" => self.validate_document_use_evidence(form, &mut result)?,
            "case.create" => self.validate_case_create(form, &mut result)?,
            "entity.register" => self.validate_entity_register(form, &mut result)?,
            "document.catalog" => self.validate_document_catalog(form, &mut result)?,
            _ => {
                // Unknown verb - add warning
                result.warnings.push(ValidationWarning {
                    code: "UNKNOWN_VERB".to_string(),
                    message: format!("Unknown verb: {}", form.verb),
                    location: Some(SourceLocation {
                        line: form_index + 1,
                        column: 1,
                        file: None,
                        span: None,
                    }),
                    auto_fix: None,
                });
            }
        }

        result.is_valid = result.errors.is_empty();
        Ok(result)
    }

    /// Validate entity.link with optional link-id for updates
    fn validate_entity_link_with_updates(
        &mut self,
        form: &VerbForm,
        form_index: usize,
        result: &mut ValidationResult,
    ) -> Result<(), ValidatorError> {
        // Required fields
        let from_entity = self.extract_string_value(&form.pairs, "from-entity")?;
        let to_entity = self.extract_string_value(&form.pairs, "to-entity")?;
        let relationship_type = self.extract_string_value(&form.pairs, "relationship-type")?;

        // Validate entity references exist
        if !self.entity_registry.contains(&from_entity) {
            result.warnings.push(ValidationWarning {
                code: "ENTITY_NOT_FOUND".to_string(),
                message: format!("from-entity '{}' not found in registry", from_entity),
                location: Some(SourceLocation {
                    line: form_index + 1,
                    column: 1,
                    file: None,
                    span: None,
                }),
                auto_fix: None,
            });
        }

        if !self.entity_registry.contains(&to_entity) {
            result.warnings.push(ValidationWarning {
                code: "ENTITY_NOT_FOUND".to_string(),
                message: format!("to-entity '{}' not found in registry", to_entity),
                location: Some(SourceLocation {
                    line: form_index + 1,
                    column: 1,
                    file: None,
                    span: None,
                }),
                auto_fix: None,
            });
        }

        // Handle optional link-id for updates
        if let Ok(link_id) = self.extract_string_value(&form.pairs, "link-id") {
            // This is an update to an existing link
            if let Some(existing_link) = self.link_registry.get(&link_id) {
                // Validate consistency with original link
                if existing_link.from_entity != from_entity
                    || existing_link.to_entity != to_entity
                    || existing_link.relationship_type != relationship_type
                {
                    result.warnings.push(ValidationWarning {
                        code: "LINK_UPDATE_INCONSISTENCY".to_string(),
                        message: format!(
                            "Link update '{}' changes core attributes from original definition",
                            link_id
                        ),
                        location: Some(SourceLocation {
                            line: form_index + 1,
                            column: 1,
                            file: None,
                            span: None,
                        }),
                        auto_fix: None,
                    });
                }
            } else {
                // First time seeing this link-id, register it
                self.link_registry.insert(
                    link_id.clone(),
                    LinkInfo {
                        from_entity: from_entity.clone(),
                        to_entity: to_entity.clone(),
                        relationship_type: relationship_type.clone(),
                        first_seen_location: Some(form_index),
                    },
                );
            }
        } else {
            // No link-id provided - generate natural key for tracking
            let natural_key = format!("{}->{}-{}", from_entity, to_entity, relationship_type);
            self.link_registry
                .entry(natural_key)
                .or_insert_with(|| LinkInfo {
                    from_entity: from_entity.clone(),
                    to_entity: to_entity.clone(),
                    relationship_type: relationship_type.clone(),
                    first_seen_location: Some(form_index),
                });
        }

        // Validate relationship-props structure if present
        if let Ok(Value::Map(props)) = self.extract_value(&form.pairs, "relationship-props") {
            self.validate_relationship_props(props, &relationship_type, form_index, result)?;
        }

        Ok(())
    }

    /// Validate relationship-props map structure
    fn validate_relationship_props(
        &self,
        props: &PropertyMap,
        relationship_type: &str,
        form_index: usize,
        result: &mut ValidationResult,
    ) -> Result<(), ValidatorError> {
        match relationship_type {
            "OWNERSHIP" => {
                // Ownership relationships should have ownership-percentage
                if !props.contains_key(&Key::new("ownership-percentage")) {
                    result.warnings.push(ValidationWarning {
                        code: "MISSING_OWNERSHIP_PERCENTAGE".to_string(),
                        message: "OWNERSHIP relationship missing ownership-percentage".to_string(),
                        location: Some(SourceLocation {
                            line: form_index + 1,
                            column: 1,
                            file: None,
                            span: None,
                        }),
                        auto_fix: Some(
                            "Add :ownership-percentage to relationship-props".to_string(),
                        ),
                    });
                } else {
                    // Validate percentage is reasonable
                    if let Ok(pct) = self.extract_numeric_value(props, "ownership-percentage") {
                        if !(0.0..=100.0).contains(&pct) {
                            result.errors.push(ValidationError {
                                code: "INVALID_OWNERSHIP_PERCENTAGE".to_string(),
                                message: format!(
                                    "Ownership percentage {} is out of range (0-100)",
                                    pct
                                ),
                                severity: ErrorSeverity::Error,
                                location: Some(SourceLocation {
                                    line: form_index + 1,
                                    column: 1,
                                    file: None,
                                    span: None,
                                }),
                                suggestions: vec![
                                    "Use a percentage between 0.0 and 100.0".to_string()
                                ],
                            });
                        }
                    }
                }
            }
            "GENERAL_PARTNER" | "CONTROL" => {
                // Control relationships should have verification-status
                if !props.contains_key(&Key::new("verification-status")) {
                    result.suggestions.push(format!(
                        "Consider adding verification-status to {} relationship",
                        relationship_type
                    ));
                }
            }
            _ => {
                // Unknown relationship type
                result.warnings.push(ValidationWarning {
                    code: "UNKNOWN_RELATIONSHIP_TYPE".to_string(),
                    message: format!("Unknown relationship type: {}", relationship_type),
                    location: Some(SourceLocation {
                        line: form_index + 1,
                        column: 1,
                        file: None,
                        span: None,
                    }),
                    auto_fix: None,
                });
            }
        }

        Ok(())
    }

    /// Validate case.update with append-only notes behavior
    fn validate_case_update_notes(
        &mut self,
        form: &VerbForm,
        result: &mut ValidationResult,
    ) -> Result<(), ValidatorError> {
        let case_id = self.extract_string_value(&form.pairs, "case-id")?;

        // Validate case exists
        if !self.case_registry.contains_key(&case_id) {
            result.errors.push(ValidationError {
                code: "CASE_NOT_FOUND".to_string(),
                message: format!("Case '{}' not found", case_id),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Ensure case is created before updating".to_string()],
            });
            return Ok(());
        }

        // Validate notes field
        if let Ok(notes) = self.extract_string_value(&form.pairs, "notes") {
            // Append to case notes (append-only behavior)
            if let Some(case_info) = self.case_registry.get_mut(&case_id) {
                case_info.notes.push(notes.clone());
            }

            // Validate note format if it contains note-id
            if notes.contains(':') {
                let parts: Vec<&str> = notes.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let note_id = parts[0].trim();
                    if note_id.is_empty() {
                        result.warnings.push(ValidationWarning {
                            code: "EMPTY_NOTE_ID".to_string(),
                            message: "Note ID is empty in note format".to_string(),
                            location: None,
                            auto_fix: Some("Use format 'note-001: Note content'".to_string()),
                        });
                    }
                }
            }
        } else {
            result.errors.push(ValidationError {
                code: "MISSING_NOTES_FIELD".to_string(),
                message: "case.update must have notes field".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Add :notes field with note content".to_string()],
            });
        }

        Ok(())
    }

    /// Validate document.use with evidence linking
    fn validate_document_use_evidence(
        &self,
        form: &VerbForm,
        result: &mut ValidationResult,
    ) -> Result<(), ValidatorError> {
        let document_id = self.extract_string_value(&form.pairs, "document-id")?;

        // Validate document exists
        if !self.document_registry.contains(&document_id) {
            result.warnings.push(ValidationWarning {
                code: "DOCUMENT_NOT_FOUND".to_string(),
                message: format!("Document '{}' not found in registry", document_id),
                location: None,
                auto_fix: None,
            });
        }

        // Validate usage-type
        let usage_type = self
            .extract_string_value(&form.pairs, "usage-type")
            .unwrap_or_else(|_| "GENERAL".to_string());

        // For EVIDENCE usage, validate evidence.of-link
        if usage_type == "EVIDENCE" {
            if let Ok(link_ref) = self.extract_string_value(&form.pairs, "evidence.of-link") {
                // Validate that the referenced link exists
                if !self.link_registry.contains_key(&link_ref) {
                    result.warnings.push(ValidationWarning {
                        code: "EVIDENCE_LINK_NOT_FOUND".to_string(),
                        message: format!("Evidence links to unknown link-id '{}'", link_ref),
                        location: None,
                        auto_fix: None,
                    });
                }
            } else {
                result.warnings.push(ValidationWarning {
                    code: "MISSING_EVIDENCE_LINK".to_string(),
                    message: "EVIDENCE usage-type should specify evidence.of-link".to_string(),
                    location: None,
                    auto_fix: Some("Add :evidence.of-link field".to_string()),
                });
            }
        }

        // Validate used-by-process
        if let Ok(process) = self.extract_string_value(&form.pairs, "used-by-process") {
            let valid_processes = [
                "UBO_ANALYSIS",
                "KYC_VERIFICATION",
                "COMPLIANCE_CHECK",
                "GENERAL",
            ];
            if !valid_processes.contains(&process.as_str()) {
                result.suggestions.push(format!(
                    "Consider using a standard process name: {}",
                    valid_processes.join(", ")
                ));
            }
        }

        Ok(())
    }

    /// Validate case.create form
    fn validate_case_create(
        &self,
        form: &VerbForm,
        result: &mut ValidationResult,
    ) -> Result<(), ValidatorError> {
        let _case_id = self.extract_string_value(&form.pairs, "case-id")?;

        // Validate case-type if present
        if let Ok(case_type) = self.extract_string_value(&form.pairs, "case-type") {
            let valid_types = ["KYC_CASE", "UBO_CASE", "COMPLIANCE_CASE", "GENERAL_CASE"];
            if !valid_types.contains(&case_type.as_str()) {
                result.suggestions.push(format!(
                    "Consider using a standard case type: {}",
                    valid_types.join(", ")
                ));
            }
        }

        // Validate other optional fields
        if form.pairs.contains_key(&Key::new("assigned-to")) {
            // Could validate against user registry in future
            result
                .suggestions
                .push("Consider validating assigned-to against user directory".to_string());
        }

        Ok(())
    }

    /// Validate entity.register form
    fn validate_entity_register(
        &self,
        form: &VerbForm,
        result: &mut ValidationResult,
    ) -> Result<(), ValidatorError> {
        let _entity_id = self.extract_string_value(&form.pairs, "entity-id")?;
        let _entity_type = self.extract_string_value(&form.pairs, "entity-type")?;

        // Validate props map if present
        if let Ok(Value::Map(props)) = self.extract_value(&form.pairs, "props") {
            // Check for legal-name which is commonly required
            if !props.contains_key(&Key::new("legal-name")) {
                result
                    .suggestions
                    .push("Consider adding legal-name to entity props".to_string());
            }
        }

        Ok(())
    }

    /// Validate document.catalog form
    fn validate_document_catalog(
        &self,
        form: &VerbForm,
        result: &mut ValidationResult,
    ) -> Result<(), ValidatorError> {
        let _document_id = self.extract_string_value(&form.pairs, "document-id")?;
        let _document_type = self.extract_string_value(&form.pairs, "document-type")?;

        // Validate file-hash format if present
        if let Ok(file_hash) = self.extract_string_value(&form.pairs, "file-hash") {
            if !file_hash.starts_with("sha256:") && !file_hash.starts_with("md5:") {
                result.warnings.push(ValidationWarning {
                    code: "INVALID_HASH_FORMAT".to_string(),
                    message: "file-hash should include hash algorithm prefix (e.g., 'sha256:')"
                        .to_string(),
                    location: None,
                    auto_fix: Some("Use format 'sha256:abcd1234...'".to_string()),
                });
            }
        }

        Ok(())
    }

    /// Helper to extract string value from property map
    fn extract_string_value(
        &self,
        pairs: &PropertyMap,
        key: &str,
    ) -> Result<String, ValidatorError> {
        let key_obj = Key::new(key);
        match pairs.get(&key_obj) {
            Some(Value::String(s)) => Ok(s.clone()),
            Some(Value::Identifier(s)) => Ok(s.clone()),
            Some(Value::Literal(crate::Literal::String(s))) => Ok(s.clone()),
            Some(_) => Err(ValidatorError::InvalidFieldType {
                field: key.to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            }),
            None => Err(ValidatorError::MissingRequiredField {
                field: key.to_string(),
            }),
        }
    }

    /// Helper to extract any value from property map
    fn extract_value<'a>(
        &self,
        pairs: &'a PropertyMap,
        key: &str,
    ) -> Result<&'a Value, ValidatorError> {
        let key_obj = Key::new(key);
        pairs
            .get(&key_obj)
            .ok_or(ValidatorError::MissingRequiredField {
                field: key.to_string(),
            })
    }

    /// Helper to extract numeric values from property map
    fn extract_numeric_value(&self, pairs: &PropertyMap, key: &str) -> Result<f64, ValidatorError> {
        let key_obj = Key::new(key);
        match pairs.get(&key_obj) {
            Some(Value::Double(n)) => Ok(*n),
            Some(Value::Integer(i)) => Ok(*i as f64),
            Some(Value::Literal(crate::Literal::Number(n))) => Ok(*n),
            Some(_) => Err(ValidatorError::InvalidFieldType {
                field: key.to_string(),
                expected: "number".to_string(),
                actual: "other".to_string(),
            }),
            None => Err(ValidatorError::MissingRequiredField {
                field: key.to_string(),
            }),
        }
    }
}

impl Default for DslValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Form, VerbForm};
    use std::collections::HashMap;

    fn create_test_entity_register(entity_id: &str, entity_type: &str) -> Form {
        let mut pairs = HashMap::new();
        pairs.insert(Key::new("entity-id"), Value::String(entity_id.to_string()));
        pairs.insert(
            Key::new("entity-type"),
            Value::String(entity_type.to_string()),
        );

        Form::Verb(VerbForm {
            verb: "entity.register".to_string(),
            pairs,
        })
    }

    fn create_test_entity_link(
        from: &str,
        to: &str,
        rel_type: &str,
        link_id: Option<&str>,
    ) -> Form {
        let mut pairs = HashMap::new();
        pairs.insert(Key::new("from-entity"), Value::String(from.to_string()));
        pairs.insert(Key::new("to-entity"), Value::String(to.to_string()));
        pairs.insert(
            Key::new("relationship-type"),
            Value::String(rel_type.to_string()),
        );

        if let Some(id) = link_id {
            pairs.insert(Key::new("link-id"), Value::String(id.to_string()));
        }

        // Add relationship-props for ownership
        if rel_type == "OWNERSHIP" {
            let mut props = HashMap::new();
            props.insert(Key::new("ownership-percentage"), Value::Double(60.0));
            props.insert(
                Key::new("verification-status"),
                Value::String("ALLEGED".to_string()),
            );
            pairs.insert(Key::new("relationship-props"), Value::Map(props));
        }

        Form::Verb(VerbForm {
            verb: "entity.link".to_string(),
            pairs,
        })
    }

    #[test]
    fn test_entity_link_validation() {
        let mut validator = DslValidator::new();

        let program = vec![
            create_test_entity_register("entity-1", "COMPANY"),
            create_test_entity_register("entity-2", "PERSON"),
            create_test_entity_link("entity-1", "entity-2", "OWNERSHIP", Some("link-001")),
        ];

        let result = validator.validate_program(&program);
        assert!(result.is_valid);
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn test_link_update_validation() {
        let mut validator = DslValidator::new();

        let program = vec![
            create_test_entity_register("entity-1", "COMPANY"),
            create_test_entity_register("entity-2", "PERSON"),
            create_test_entity_link("entity-1", "entity-2", "OWNERSHIP", Some("link-001")),
            create_test_entity_link("entity-1", "entity-2", "OWNERSHIP", Some("link-001")), // Update
        ];

        let result = validator.validate_program(&program);
        assert!(result.is_valid);
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn test_case_update_validation() {
        let mut validator = DslValidator::new();

        let mut case_pairs = HashMap::new();
        case_pairs.insert(Key::new("case-id"), Value::String("case-001".to_string()));
        case_pairs.insert(Key::new("case-type"), Value::String("KYC_CASE".to_string()));

        let mut update_pairs = HashMap::new();
        update_pairs.insert(Key::new("case-id"), Value::String("case-001".to_string()));
        update_pairs.insert(
            Key::new("notes"),
            Value::String("note-001: Test finding".to_string()),
        );

        let program = vec![
            Form::Verb(VerbForm {
                verb: "case.create".to_string(),
                pairs: case_pairs,
            }),
            Form::Verb(VerbForm {
                verb: "case.update".to_string(),
                pairs: update_pairs,
            }),
        ];

        let result = validator.validate_program(&program);
        assert!(result.is_valid);
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn test_document_use_evidence_validation() {
        let mut validator = DslValidator::new();

        let mut doc_pairs = HashMap::new();
        doc_pairs.insert(
            Key::new("document-id"),
            Value::String("doc-001".to_string()),
        );
        doc_pairs.insert(
            Key::new("document-type"),
            Value::String("CONTRACT".to_string()),
        );

        let mut use_pairs = HashMap::new();
        use_pairs.insert(
            Key::new("document-id"),
            Value::String("doc-001".to_string()),
        );
        use_pairs.insert(
            Key::new("usage-type"),
            Value::String("EVIDENCE".to_string()),
        );
        use_pairs.insert(
            Key::new("evidence.of-link"),
            Value::String("link-001".to_string()),
        );
        use_pairs.insert(
            Key::new("used-by-process"),
            Value::String("UBO_ANALYSIS".to_string()),
        );

        let program = vec![
            Form::Verb(VerbForm {
                verb: "document.catalog".to_string(),
                pairs: doc_pairs,
            }),
            create_test_entity_register("entity-1", "COMPANY"),
            create_test_entity_register("entity-2", "PERSON"),
            create_test_entity_link("entity-1", "entity-2", "OWNERSHIP", Some("link-001")),
            Form::Verb(VerbForm {
                verb: "document.use".to_string(),
                pairs: use_pairs,
            }),
        ];

        let result = validator.validate_program(&program);
        assert!(result.is_valid);
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn test_invalid_ownership_percentage() {
        let mut validator = DslValidator::new();

        let mut pairs = HashMap::new();
        pairs.insert(
            Key::new("from-entity"),
            Value::String("entity-1".to_string()),
        );
        pairs.insert(Key::new("to-entity"), Value::String("entity-2".to_string()));
        pairs.insert(
            Key::new("relationship-type"),
            Value::String("OWNERSHIP".to_string()),
        );

        let mut props = HashMap::new();
        props.insert(Key::new("ownership-percentage"), Value::Double(150.0)); // Invalid!
        pairs.insert(Key::new("relationship-props"), Value::Map(props));

        let program = vec![
            create_test_entity_register("entity-1", "COMPANY"),
            create_test_entity_register("entity-2", "PERSON"),
            Form::Verb(VerbForm {
                verb: "entity.link".to_string(),
                pairs,
            }),
        ];

        let result = validator.validate_program(&program);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());

        let error = &result.errors[0];
        assert_eq!(error.code, "INVALID_OWNERSHIP_PERCENTAGE");
    }

    #[test]
    fn test_complete_parse_normalize_validate_pipeline() {
        // Test the complete pipeline with legacy DSL that gets normalized and validated
        let legacy_dsl = r#"
        (kyc.start_case :case_type "KYC_CASE" :business_reference "KYC-2025-001")
        (entity.register :entity-id "entity-1" :entity-type "COMPANY")
        (entity.register :entity-id "entity-2" :entity-type "PERSON")
        (ubo.link_ownership :from_entity "entity-1" :to_entity "entity-2" :percent 60.0 :status "alleged")
        (document.catalog :document-id "doc-001" :document-type "CONTRACT")
        (ubo.add_evidence :document_id "doc-001" :target_link_id "entity-1->entity-2-OWNERSHIP")
        (kyc.add_finding :case_id "case-001" :finding_id "note-001" :text "Sample finding")
        "#;

        // Debug: Test parsing first
        use crate::parser::parse_program;
        let parse_result = parse_program(legacy_dsl);
        assert!(
            parse_result.is_ok(),
            "DSL should parse correctly: {:?}",
            parse_result.err()
        );

        // Debug: Test normalization
        use crate::parser::parse_and_normalize;
        let normalize_result = parse_and_normalize(legacy_dsl);
        assert!(
            normalize_result.is_ok(),
            "Normalization should work: {:?}",
            normalize_result.err()
        );

        // Use the complete pipeline from parser mod
        use crate::parser::parse_normalize_and_validate;

        let result = parse_normalize_and_validate(legacy_dsl);
        assert!(
            result.is_ok(),
            "Pipeline should complete successfully: {:?}",
            result.err()
        );

        let (program, validation_result) = result.unwrap();

        // Verify DSL was normalized
        let verb_forms: Vec<_> = program
            .iter()
            .filter_map(|f| match f {
                crate::Form::Verb(vf) => Some(vf),
                _ => None,
            })
            .collect();

        // Check that legacy verbs were converted to canonical
        assert_eq!(verb_forms[0].verb, "case.create"); // kyc.start_case -> case.create
        assert_eq!(verb_forms[3].verb, "entity.link"); // ubo.link_ownership -> entity.link
        assert_eq!(verb_forms[5].verb, "document.use"); // ubo.add_evidence -> document.use
        assert_eq!(verb_forms[6].verb, "case.update"); // kyc.add_finding -> case.update

        // Validation should pass with some warnings
        assert!(!validation_result.errors.is_empty() || validation_result.warnings.len() > 0);

        // Should have warnings about case ID mismatch but no critical errors
        if !validation_result.is_valid {
            // Check that errors are about expected issues (case ID references)
            let error_codes: Vec<_> = validation_result.errors.iter().map(|e| &e.code).collect();
            assert!(error_codes.contains(&&"CASE_NOT_FOUND".to_string()));
        }
    }

    #[test]
    fn test_canonical_dsl_validation_perfect_score() {
        // Test that properly structured canonical DSL gets perfect validation
        let canonical_dsl = r#"
        (case.create :case-id "case-001" :case-type "KYC_CASE")
        (entity.register :entity-id "entity-1" :entity-type "COMPANY")
        (entity.register :entity-id "entity-2" :entity-type "PERSON")
        (entity.link :link-id "link-001" :from-entity "entity-1" :to-entity "entity-2"
                     :relationship-type "OWNERSHIP"
                     :relationship-props {:ownership-percentage 75.0 :verification-status "VERIFIED"})
        (document.catalog :document-id "doc-001" :document-type "CONTRACT" :file-hash "sha256:abc123")
        (document.use :document-id "doc-001" :usage-type "EVIDENCE" :evidence.of-link "link-001"
                      :used-by-process "UBO_ANALYSIS")
        (case.update :case-id "case-001" :notes "note-001: All documentation verified")
        "#;

        use crate::parser::parse_normalize_and_validate;

        let result = parse_normalize_and_validate(canonical_dsl);
        assert!(result.is_ok());

        let (program, validation_result) = result.unwrap();

        // Should have no normalization changes (already canonical)
        let verb_forms: Vec<_> = program
            .iter()
            .filter_map(|f| match f {
                crate::Form::Verb(vf) => Some(vf),
                _ => None,
            })
            .collect();

        // Verify canonical verbs remain unchanged
        assert_eq!(verb_forms[0].verb, "case.create");
        assert_eq!(verb_forms[3].verb, "entity.link");
        assert_eq!(verb_forms[5].verb, "document.use");
        assert_eq!(verb_forms[6].verb, "case.update");

        // Validation should be perfect or near-perfect
        assert!(
            validation_result.is_valid || validation_result.errors.is_empty(),
            "Validation failed: errors={:?}, warnings={:?}",
            validation_result.errors,
            validation_result.warnings
        );

        // Any warnings should be minor suggestions
        for warning in &validation_result.warnings {
            assert!(!warning.code.contains("ERROR"));
        }
    }

    #[test]
    fn test_validation_error_detection() {
        // Test that validation properly catches errors
        let problematic_dsl = r#"
        (entity.link :from-entity "nonexistent-1" :to-entity "nonexistent-2"
                     :relationship-type "OWNERSHIP"
                     :relationship-props {:ownership-percentage 150.0})
        (case.update :case-id "nonexistent-case" :notes "test note")
        (document.use :document-id "nonexistent-doc" :usage-type "EVIDENCE"
                      :evidence.of-link "nonexistent-link")
        "#;

        use crate::parser::parse_normalize_and_validate;

        let result = parse_normalize_and_validate(problematic_dsl);
        assert!(result.is_ok()); // Should parse successfully

        let (_program, validation_result) = result.unwrap();

        // Should have validation errors
        assert!(!validation_result.is_valid);
        assert!(!validation_result.errors.is_empty());

        // Check for expected error types
        let error_codes: Vec<_> = validation_result.errors.iter().map(|e| &e.code).collect();
        println!("Actual error codes: {:?}", error_codes);
        println!("All errors: {:?}", validation_result.errors);
        assert!(error_codes.contains(&&"INVALID_OWNERSHIP_PERCENTAGE".to_string()));
        assert!(error_codes.contains(&&"CASE_NOT_FOUND".to_string()));

        // Should also have warnings about missing entities
        assert!(!validation_result.warnings.is_empty());
        let warning_codes: Vec<_> = validation_result.warnings.iter().map(|w| &w.code).collect();
        assert!(warning_codes.contains(&&"ENTITY_NOT_FOUND".to_string()));
    }

    #[test]
    fn test_link_identity_tracking() {
        // Test that link identity and updates are properly tracked
        let mut validator = DslValidator::new();

        let program = vec![
            create_test_entity_register("entity-1", "COMPANY"),
            create_test_entity_register("entity-2", "PERSON"),
            create_test_entity_link("entity-1", "entity-2", "OWNERSHIP", Some("link-001")),
            create_test_entity_link("entity-1", "entity-2", "OWNERSHIP", Some("link-001")), // Update
        ];

        let result = validator.validate_program(&program);
        assert!(result.is_valid);

        // Should have the link registered
        assert!(validator.link_registry.contains_key("link-001"));

        let link_info = &validator.link_registry["link-001"];
        assert_eq!(link_info.from_entity, "entity-1");
        assert_eq!(link_info.to_entity, "entity-2");
        assert_eq!(link_info.relationship_type, "OWNERSHIP");
    }

    #[test]
    fn test_append_only_notes() {
        // Test that case notes are properly accumulated
        let mut validator = DslValidator::new();

        let mut case_pairs = HashMap::new();
        case_pairs.insert(Key::new("case-id"), Value::String("case-001".to_string()));
        case_pairs.insert(Key::new("case-type"), Value::String("KYC_CASE".to_string()));

        let mut update1_pairs = HashMap::new();
        update1_pairs.insert(Key::new("case-id"), Value::String("case-001".to_string()));
        update1_pairs.insert(Key::new("notes"), Value::String("First note".to_string()));

        let mut update2_pairs = HashMap::new();
        update2_pairs.insert(Key::new("case-id"), Value::String("case-001".to_string()));
        update2_pairs.insert(Key::new("notes"), Value::String("Second note".to_string()));

        let program = vec![
            Form::Verb(VerbForm {
                verb: "case.create".to_string(),
                pairs: case_pairs,
            }),
            Form::Verb(VerbForm {
                verb: "case.update".to_string(),
                pairs: update1_pairs,
            }),
            Form::Verb(VerbForm {
                verb: "case.update".to_string(),
                pairs: update2_pairs,
            }),
        ];

        let result = validator.validate_program(&program);
        assert!(result.is_valid);

        // Check that notes were accumulated
        let case_info = &validator.case_registry["case-001"];
        assert_eq!(case_info.notes.len(), 2);
        assert_eq!(case_info.notes[0], "First note");
        assert_eq!(case_info.notes[1], "Second note");
    }
}
