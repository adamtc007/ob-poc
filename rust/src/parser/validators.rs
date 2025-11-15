//! Enhanced DSL Validators for Phase 2 Implementation
//!
//! This module implements enhanced validation logic for the KYC orchestration DSL,
//! supporting new semantics including link identity, append-only notes, and evidence linking.

use crate::ast::types::{ErrorSeverity, SourceLocation, ValidationError, ValidationWarning};
use crate::parser_ast::{Form, Key, Literal, PropertyMap, Value, VerbForm};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ValidatorError {
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
            "document.extract" => self.validate_document_extract(form, &mut result)?,
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

    /// Validate document.extract form
    /// Syntax: (document.extract :document-id @doc{uuid} :attributes [@attr{uuid1} @attr{uuid2}])
    fn validate_document_extract(
        &self,
        form: &VerbForm,
        result: &mut ValidationResult,
    ) -> Result<(), ValidatorError> {
        // Validate document-id parameter (required)
        let _document_id = self.extract_value(&form.pairs, "document-id")?;

        // Validate attributes parameter (required, should be a list)
        let attributes_value = self.extract_value(&form.pairs, "attributes")?;

        match attributes_value {
            Value::List(attrs) | Value::Array(attrs) => {
                if attrs.is_empty() {
                    result.warnings.push(ValidationWarning {
                        code: "EMPTY_ATTRIBUTE_LIST".to_string(),
                        message:
                            "document.extract should specify at least one attribute to extract"
                                .to_string(),
                        location: None,
                        auto_fix: None,
                    });
                }

                // Validate each attribute reference
                for (idx, attr) in attrs.iter().enumerate() {
                    match attr {
                        Value::AttrUuid(_)
                        | Value::AttrRef(_)
                        | Value::AttrUuidWithSource(_, _)
                        | Value::AttrRefWithSource(_, _) => {
                            // Valid attribute reference
                        }
                        _ => {
                            result.warnings.push(ValidationWarning {
                                code: "INVALID_ATTRIBUTE_REFERENCE".to_string(),
                                message: format!(
                                    "Attribute at index {} is not a valid attribute reference (should be @attr{{uuid}} or @attr.semantic.id)",
                                    idx
                                ),
                                location: None,
                                auto_fix: None,
                            });
                        }
                    }
                }
            }
            _ => {
                return Err(ValidatorError::InvalidFieldType {
                    field: "attributes".to_string(),
                    expected: "list of attribute references".to_string(),
                    actual: "other".to_string(),
                });
            }
        }

        // Optional: entity-id parameter for storing extracted values
        if self.extract_value(&form.pairs, "entity-id").is_err() {
            result.warnings.push(ValidationWarning {
                code: "MISSING_ENTITY_ID".to_string(),
                message: "document.extract without :entity-id will extract but not store values"
                    .to_string(),
                location: None,
                auto_fix: Some(
                    "Add :entity-id parameter to store extracted attributes".to_string(),
                ),
            });
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
            Some(Value::Literal(Literal::String(s))) => Ok(s.clone()),
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
            Some(Value::Literal(Literal::Number(n))) => Ok(*n),
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
