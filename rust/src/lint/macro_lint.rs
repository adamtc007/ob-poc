//! Macro Schema Lint Implementation
//!
//! This module implements lint rules for validating macro YAML schemas.
//! Rules are organized into two passes:
//!
//! **Pass 1 (Schema-only):** Validates structure without external dependencies
//! **Pass 2 (Cross-registry):** Validates references to primitive verbs and other macros
//!
//! ## Rule Codes
//!
//! | Range | Category |
//! |-------|----------|
//! | MACRO000-009 | Parse errors |
//! | MACRO010-019 | Structure/kind |
//! | MACRO011-019 | UI fields |
//! | MACRO020-029 | Routing |
//! | MACRO030-039 | Target |
//! | MACRO040-049 | Args |
//! | MACRO050-059 | Prereqs |
//! | MACRO060-069 | Expansion |
//! | MACRO070-079 | Cross-registry |
//! | MACRO080-089 | UX warnings |

use std::collections::HashSet;

use regex::Regex;
use serde_yaml::Value;

use super::diagnostic::Diagnostic;

// =============================================================================
// PUBLIC API
// =============================================================================

/// Registry of primitive verbs for cross-registry validation (Pass 2)
pub trait PrimitiveRegistry {
    /// Check if a verb exists in the primitive registry
    fn has_verb(&self, verb_fqn: &str) -> bool;
}

/// Empty registry for when no cross-registry validation is needed
#[allow(dead_code)]
pub struct EmptyRegistry;

impl PrimitiveRegistry for EmptyRegistry {
    fn has_verb(&self, _verb_fqn: &str) -> bool {
        true // Accept all verbs when no registry is provided
    }
}

/// Lint a macro YAML file and return diagnostics
///
/// # Arguments
/// * `yaml_text` - The YAML source text
/// * `primitive_registry` - Optional registry for cross-registry validation
///
/// # Returns
/// Vector of diagnostics (errors, warnings, info)
pub fn lint_macro_file(
    yaml_text: &str,
    primitive_registry: Option<&dyn PrimitiveRegistry>,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // MACRO000: Parse YAML
    let doc: Value = match serde_yaml::from_str(yaml_text) {
        Ok(v) => v,
        Err(e) => {
            diags.push(Diagnostic::error(
                "MACRO000",
                "$",
                format!("YAML parse error: {}", e),
            ));
            return diags;
        }
    };

    // MACRO001: Top-level must be mapping
    let top = match doc.as_mapping() {
        Some(m) => m,
        None => {
            diags.push(Diagnostic::error(
                "MACRO001",
                "$",
                "Schema must be a mapping of <verb-fqn> → spec",
            ));
            return diags;
        }
    };

    // Collect verb names for unlock validation
    let verb_names: HashSet<String> = top
        .iter()
        .filter_map(|(k, _)| k.as_str().map(String::from))
        .collect();

    // Lint each verb spec
    for (k, v) in top.iter() {
        // MACRO002: Keys must be strings
        let verb = match k.as_str() {
            Some(s) => s,
            None => {
                diags.push(Diagnostic::error(
                    "MACRO002",
                    "$",
                    "Verb FQN must be a string",
                ));
                continue;
            }
        };

        // MACRO003: Spec must be a mapping
        let spec = match v.as_mapping() {
            Some(m) => m,
            None => {
                diags.push(Diagnostic::error(
                    "MACRO003",
                    verb,
                    "Verb spec must be a mapping",
                ));
                continue;
            }
        };

        // Run Pass 1 rules
        lint_verb_spec(&mut diags, verb, spec, &verb_names);

        // Run Pass 2 rules if registry provided
        if let Some(registry) = primitive_registry {
            lint_cross_registry(&mut diags, verb, spec, registry);
        }
    }

    diags
}

// =============================================================================
// PASS 1: SCHEMA-ONLY RULES
// =============================================================================

fn lint_verb_spec(
    diags: &mut Vec<Diagnostic>,
    verb: &str,
    spec: &serde_yaml::Mapping,
    verb_names: &HashSet<String>,
) {
    // MACRO010: kind required
    let kind = get_str(spec, "kind").unwrap_or("");
    if kind != "macro" && kind != "primitive" {
        diags.push(
            Diagnostic::error("MACRO010", verb, "kind must be 'macro' or 'primitive'")
                .with_hint("Add: kind: macro"),
        );
    }

    // Only apply macro-specific rules if kind is macro
    if kind == "macro" {
        lint_macro_spec(diags, verb, spec, verb_names);
    }

    // MACRO043: Global scan for 'kinds' outside internal block
    walk_for_kinds(diags, &Value::Mapping(spec.clone()), verb);
}

fn lint_macro_spec(
    diags: &mut Vec<Diagnostic>,
    verb: &str,
    spec: &serde_yaml::Mapping,
    verb_names: &HashSet<String>,
) {
    // MACRO011: UI fields required
    check_ui_fields(diags, verb, spec);

    // MACRO012: Forbidden tokens in UI text
    check_forbidden_ui_tokens(diags, verb, spec);

    // MACRO020: routing.mode_tags required
    check_routing(diags, verb, spec);

    // MACRO030-032: target validation
    check_target(diags, verb, spec);

    // MACRO040-045: args validation
    let (arg_names, enum_args) = check_args(diags, verb, spec);

    // MACRO050-051: prereqs validation
    check_prereqs(diags, verb, spec);

    // MACRO060-063: expands_to validation
    check_expansion(diags, verb, spec, &arg_names, &enum_args);

    // MACRO070: unlocks references (local check)
    check_unlocks(diags, verb, spec, verb_names);

    // MACRO080: UX friction warnings
    check_ux_warnings(diags, verb, spec);
}

// =============================================================================
// UI RULES (MACRO011-012)
// =============================================================================

fn check_ui_fields(diags: &mut Vec<Diagnostic>, verb: &str, spec: &serde_yaml::Mapping) {
    let ui = get_map(spec, "ui");

    if ui.is_none() {
        diags.push(
            Diagnostic::error("MACRO011", verb, "ui section is required for macros")
                .with_hint("Add: ui: { label: ..., description: ..., target_label: ... }"),
        );
        return;
    }

    let ui = ui.unwrap();
    let path = format!("{}.ui", verb);

    // Check required fields
    for field in ["label", "description", "target_label"] {
        let value = get_str(ui, field);
        if value.is_none() || value.unwrap().trim().is_empty() {
            diags.push(
                Diagnostic::error(
                    "MACRO011",
                    format!("{}.{}", path, field),
                    format!("ui.{} is required and must be non-empty", field),
                )
                .with_hint(format!("Add: {}: \"...\"", field)),
            );
        }
    }
}

/// Tokens that must never appear in UI text (implementation jargon)
const FORBIDDEN_TOKENS: &[&str] = &[
    "cbu",
    "entity_ref",
    "trading-profile",
    "cbu_id",
    "entity_id",
    "cbu-id",
    "uboprong",
    "resolver",
    "kyc-case",
    ":kind",
];

fn check_forbidden_ui_tokens(diags: &mut Vec<Diagnostic>, verb: &str, spec: &serde_yaml::Mapping) {
    if let Some(ui) = get_map(spec, "ui") {
        for field in ["label", "description", "target_label"] {
            if let Some(value) = get_str(ui, field) {
                if let Some(forbidden) = contains_forbidden_token(value) {
                    diags.push(
                        Diagnostic::error(
                            "MACRO012",
                            format!("{}.ui.{}", verb, field),
                            format!("UI text contains forbidden token: '{}'", forbidden),
                        )
                        .with_hint(
                            "Replace with operator vocabulary (e.g., 'Structure' instead of 'CBU')",
                        ),
                    );
                }
            }
        }
    }
}

fn contains_forbidden_token(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    FORBIDDEN_TOKENS
        .iter()
        .find(|&&t| lower.contains(t))
        .copied()
}

// =============================================================================
// ROUTING RULES (MACRO020)
// =============================================================================

fn check_routing(diags: &mut Vec<Diagnostic>, verb: &str, spec: &serde_yaml::Mapping) {
    let routing = get_map(spec, "routing");

    if routing.is_none() {
        diags.push(
            Diagnostic::error("MACRO020", verb, "routing section is required")
                .with_hint("Add: routing: { mode_tags: [...] }"),
        );
        return;
    }

    let routing = routing.unwrap();

    // Check mode_tags
    let mode_tags = get_seq(routing, "mode_tags");
    if mode_tags.is_none() || mode_tags.unwrap().is_empty() {
        diags.push(
            Diagnostic::error(
                "MACRO020",
                format!("{}.routing.mode_tags", verb),
                "mode_tags is required and must be a non-empty list",
            )
            .with_hint("Add: mode_tags: [onboarding, kyc]"),
        );
    }
}

// =============================================================================
// TARGET RULES (MACRO030-032)
// =============================================================================

/// Valid operator types for target.operates_on
const VALID_OPERATOR_TYPES: &[&str] = &[
    "client_ref",
    "structure_ref",
    "party_ref",
    "case_ref",
    "mandate_ref",
    "document_ref",
];

/// Valid structure types for allowed_structure_types
const VALID_STRUCTURE_TYPES: &[&str] = &["pe", "sicav", "hedge", "etf", "pension", "trust", "fof"];

fn check_target(diags: &mut Vec<Diagnostic>, verb: &str, spec: &serde_yaml::Mapping) {
    let target = get_map(spec, "target");

    if target.is_none() {
        diags.push(
            Diagnostic::error("MACRO030", verb, "target section is required")
                .with_hint("Add: target: { operates_on: ..., produces: ... }"),
        );
        return;
    }

    let target = target.unwrap();
    let path = format!("{}.target", verb);

    // MACRO030: operates_on required
    if let Some(operates_on) = get_str(target, "operates_on") {
        if !VALID_OPERATOR_TYPES.contains(&operates_on) {
            diags.push(Diagnostic::error(
                "MACRO030",
                format!("{}.operates_on", path),
                format!(
                    "Invalid operator type '{}'. Valid types: {:?}",
                    operates_on, VALID_OPERATOR_TYPES
                ),
            ));
        }
    } else {
        diags.push(
            Diagnostic::error(
                "MACRO030",
                format!("{}.operates_on", path),
                "operates_on is required",
            )
            .with_hint("Add: operates_on: structure_ref"),
        );
    }

    // MACRO031: produces must be valid if present
    if let Some(produces) = get_str(target, "produces") {
        if produces != "null" && !VALID_OPERATOR_TYPES.contains(&produces) {
            diags.push(Diagnostic::error(
                "MACRO031",
                format!("{}.produces", path),
                format!(
                    "Invalid operator type '{}'. Valid types: {:?} or 'null'",
                    produces, VALID_OPERATOR_TYPES
                ),
            ));
        }
    }

    // MACRO032: allowed_structure_types values must be valid
    if let Some(allowed) = get_seq(target, "allowed_structure_types") {
        for (i, item) in allowed.iter().enumerate() {
            if let Some(s) = item.as_str() {
                if !VALID_STRUCTURE_TYPES.contains(&s) {
                    diags.push(Diagnostic::error(
                        "MACRO032",
                        format!("{}.allowed_structure_types[{}]", path, i),
                        format!(
                            "Invalid structure type '{}'. Valid types: {:?}",
                            s, VALID_STRUCTURE_TYPES
                        ),
                    ));
                }
            }
        }
    }
}

// =============================================================================
// ARGS RULES (MACRO040-045)
// =============================================================================

fn check_args(
    diags: &mut Vec<Diagnostic>,
    verb: &str,
    spec: &serde_yaml::Mapping,
) -> (HashSet<String>, HashSet<String>) {
    let mut arg_names = HashSet::new();
    let mut enum_args = HashSet::new();

    let args = get_map(spec, "args");
    if args.is_none() {
        diags.push(
            Diagnostic::error("MACRO040", verb, "args section is required")
                .with_hint("Add: args: { style: keyworded, required: {...}, optional: {...} }"),
        );
        return (arg_names, enum_args);
    }

    let args = args.unwrap();
    let path = format!("{}.args", verb);

    // MACRO040: style must be keyworded
    let style = get_str(args, "style");
    if style.is_none() || style.unwrap() != "keyworded" {
        diags.push(
            Diagnostic::error(
                "MACRO040",
                format!("{}.style", path),
                "args.style must be 'keyworded'",
            )
            .with_hint("Add: style: keyworded"),
        );
    }

    // Check required and optional args
    for section in ["required", "optional"] {
        if let Some(section_map) = get_map(args, section) {
            for (k, v) in section_map {
                let arg_name = k.as_str().unwrap_or("?");
                arg_names.insert(arg_name.to_string());

                if let Some(arg_spec) = v.as_mapping() {
                    let arg_path = format!("{}.{}.{}", path, section, arg_name);

                    // MACRO041/045: type and ui_label required
                    if get_str(arg_spec, "type").is_none() {
                        diags.push(Diagnostic::error(
                            if section == "required" {
                                "MACRO041"
                            } else {
                                "MACRO045"
                            },
                            format!("{}.type", arg_path),
                            "type is required",
                        ));
                    }
                    if get_str(arg_spec, "ui_label").is_none() {
                        diags.push(Diagnostic::error(
                            if section == "required" {
                                "MACRO041"
                            } else {
                                "MACRO045"
                            },
                            format!("{}.ui_label", arg_path),
                            "ui_label is required",
                        ));
                    }

                    // MACRO042: No entity_ref type
                    if let Some(arg_type) = get_str(arg_spec, "type") {
                        if arg_type == "entity_ref" {
                            diags.push(
                                Diagnostic::error(
                                    "MACRO042",
                                    format!("{}.type", arg_path),
                                    "entity_ref type is not allowed. Use operator types instead.",
                                )
                                .with_hint(
                                    "Use: structure_ref, party_ref, case_ref, mandate_ref, etc.",
                                ),
                            );
                        }

                        // Track enum args for expansion validation
                        if arg_type == "enum" {
                            // Check if enum has internal mapping
                            if let Some(values) = get_seq(arg_spec, "values") {
                                let has_internal = values.iter().all(|v| {
                                    v.as_mapping()
                                        .map(|m| m.contains_key("internal"))
                                        .unwrap_or(false)
                                });
                                if has_internal {
                                    enum_args.insert(arg_name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (arg_names, enum_args)
}

/// Walk the YAML tree to find 'kinds' keys outside 'internal' blocks
fn walk_for_kinds(diags: &mut Vec<Diagnostic>, value: &Value, current_path: &str) {
    if let Some(map) = value.as_mapping() {
        for (k, v) in map {
            if let Some(key) = k.as_str() {
                let new_path = if current_path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", current_path, key)
                };

                if key == "kinds" && !current_path.ends_with(".internal") {
                    diags.push(
                        Diagnostic::error(
                            "MACRO043",
                            &new_path,
                            "'kinds' may only appear under an 'internal' block",
                        )
                        .with_hint("Move to: internal: { kinds: [...] }"),
                    );
                }

                walk_for_kinds(diags, v, &new_path);
            }
        }
    } else if let Some(seq) = value.as_sequence() {
        for (i, item) in seq.iter().enumerate() {
            walk_for_kinds(diags, item, &format!("{}[{}]", current_path, i));
        }
    }
}

// =============================================================================
// PREREQS RULES (MACRO050-051)
// =============================================================================

fn check_prereqs(diags: &mut Vec<Diagnostic>, verb: &str, spec: &serde_yaml::Mapping) {
    let prereqs = spec.get(Value::String("prereqs".to_string()));

    // MACRO050: prereqs must exist
    if prereqs.is_none() {
        diags.push(
            Diagnostic::error("MACRO050", verb, "prereqs is required (can be empty [])")
                .with_hint("Add: prereqs: []"),
        );
        return;
    }

    let prereqs = prereqs.unwrap();
    let path = format!("{}.prereqs", verb);

    // Must be a sequence
    if !prereqs.is_sequence() {
        diags.push(Diagnostic::error(
            "MACRO050",
            &path,
            "prereqs must be a list",
        ));
        return;
    }

    // MACRO051: Each item must be valid prereq structure
    if let Some(seq) = prereqs.as_sequence() {
        for (i, item) in seq.iter().enumerate() {
            let item_path = format!("{}[{}]", path, i);
            if let Some(map) = item.as_mapping() {
                let has_requires = map.contains_key("requires");
                let has_any_of = map.contains_key("any_of");
                let has_state = map.contains_key("state");

                if !has_requires && !has_any_of && !has_state {
                    diags.push(
                        Diagnostic::error(
                            "MACRO051",
                            &item_path,
                            "Prereq must have one of: requires, any_of, or state",
                        )
                        .with_hint("Example: { requires: [structure.exists] }"),
                    );
                }
            } else {
                diags.push(Diagnostic::error(
                    "MACRO051",
                    &item_path,
                    "Prereq item must be a mapping",
                ));
            }
        }
    }
}

// =============================================================================
// EXPANSION RULES (MACRO060-063)
// =============================================================================

fn check_expansion(
    diags: &mut Vec<Diagnostic>,
    verb: &str,
    spec: &serde_yaml::Mapping,
    arg_names: &HashSet<String>,
    enum_args: &HashSet<String>,
) {
    let expands_to = get_seq(spec, "expands_to");

    // MACRO060: expands_to required for macros
    if expands_to.is_none() {
        diags.push(
            Diagnostic::error("MACRO060", verb, "expands_to is required for macros")
                .with_hint("Add: expands_to: [{ verb: ..., args: {...} }]"),
        );
        return;
    }

    let expands_to = expands_to.unwrap();
    let path = format!("{}.expands_to", verb);

    if expands_to.is_empty() {
        diags.push(Diagnostic::error(
            "MACRO060",
            &path,
            "expands_to must have at least one step",
        ));
        return;
    }

    for (i, step) in expands_to.iter().enumerate() {
        let step_path = format!("{}[{}]", path, i);

        if let Some(step_map) = step.as_mapping() {
            // MACRO061: Each step must have verb and args
            if !step_map.contains_key("verb") {
                diags.push(Diagnostic::error(
                    "MACRO061",
                    format!("{}.verb", step_path),
                    "Expansion step must have 'verb' field",
                ));
            } else if let Some(verb_value) = step_map.get(Value::String("verb".into())) {
                // MACRO062: No raw s-expr strings
                if let Some(verb_str) = verb_value.as_str() {
                    if verb_str.starts_with('(') && verb_str.ends_with(')') {
                        diags.push(
                            Diagnostic::error(
                                "MACRO062",
                                format!("{}.verb", step_path),
                                "Raw s-expression strings are not allowed",
                            )
                            .with_hint("Use structured format: verb: cbu.create"),
                        );
                    }
                }
            }

            // Validate args if present
            if let Some(args) = step_map.get(Value::String("args".into())) {
                validate_expansion_args(
                    diags,
                    args,
                    arg_names,
                    enum_args,
                    &format!("{}.args", step_path),
                );
            }
        } else {
            diags.push(Diagnostic::error(
                "MACRO061",
                &step_path,
                "Expansion step must be a mapping",
            ));
        }
    }
}

/// Validate variable references in expansion args
fn validate_expansion_args(
    diags: &mut Vec<Diagnostic>,
    args: &Value,
    arg_names: &HashSet<String>,
    enum_args: &HashSet<String>,
    path: &str,
) {
    let var_regex = Regex::new(r"\$\{([^}]+)\}").unwrap();

    fn walk_value(
        diags: &mut Vec<Diagnostic>,
        value: &Value,
        arg_names: &HashSet<String>,
        enum_args: &HashSet<String>,
        path: &str,
        var_regex: &Regex,
    ) {
        match value {
            Value::String(s) => {
                for cap in var_regex.captures_iter(s) {
                    let var_content = &cap[1];
                    if let Err(e) = validate_variable(var_content, arg_names, enum_args) {
                        diags.push(Diagnostic::error(
                            "MACRO063",
                            path,
                            format!("Invalid variable ${{{}}}: {}", var_content, e),
                        ));
                    }

                    // MACRO044: Check enum args use .internal
                    check_enum_internal_usage(diags, s, var_content, enum_args, path);
                }
            }
            Value::Mapping(m) => {
                for (k, v) in m {
                    let key = k.as_str().unwrap_or("?");
                    walk_value(
                        diags,
                        v,
                        arg_names,
                        enum_args,
                        &format!("{}.{}", path, key),
                        var_regex,
                    );
                }
            }
            Value::Sequence(seq) => {
                for (i, item) in seq.iter().enumerate() {
                    walk_value(
                        diags,
                        item,
                        arg_names,
                        enum_args,
                        &format!("{}[{}]", path, i),
                        var_regex,
                    );
                }
            }
            _ => {}
        }
    }

    walk_value(diags, args, arg_names, enum_args, path, &var_regex);
}

/// Validate a single variable reference
fn validate_variable(
    var: &str,
    arg_names: &HashSet<String>,
    enum_args: &HashSet<String>,
) -> Result<(), String> {
    let parts: Vec<&str> = var.split('.').collect();

    match parts.first() {
        Some(&"arg") => {
            let name = parts.get(1).ok_or("Missing arg name after 'arg.'")?;
            if !arg_names.contains(*name) {
                return Err(format!("Unknown arg: {}", name));
            }
            // Check .internal suffix validity
            if parts.get(2) == Some(&"internal") && !enum_args.contains(*name) {
                return Err(format!(
                    "Arg '{}' is not an enum, cannot use .internal",
                    name
                ));
            }
            if parts.len() > 3 || (parts.len() == 3 && parts[2] != "internal") {
                return Err("Invalid arg path: only .internal suffix allowed".into());
            }
            Ok(())
        }
        Some(&"scope") => {
            if parts.len() < 2 {
                return Err("scope.* requires a field name".into());
            }
            Ok(())
        }
        Some(&"session") => {
            if parts.len() < 2 {
                return Err("session.* requires a path".into());
            }
            Ok(())
        }
        Some(other) => Err(format!(
            "Unknown variable root '{}'. Allowed: arg, scope, session",
            other
        )),
        None => Err("Empty variable".into()),
    }
}

/// Check that enum args use ${arg.X.internal} in expansions
fn check_enum_internal_usage(
    diags: &mut Vec<Diagnostic>,
    _full_value: &str,
    var_content: &str,
    enum_args: &HashSet<String>,
    path: &str,
) {
    // Parse the variable to check if it's an enum arg without .internal
    if var_content.starts_with("arg.") {
        let parts: Vec<&str> = var_content.split('.').collect();
        if parts.len() == 2 {
            // ${arg.X} without .internal
            let arg_name = parts[1];
            if enum_args.contains(arg_name) {
                // This is an enum arg used without .internal
                diags.push(
                    Diagnostic::error(
                        "MACRO044",
                        path,
                        format!(
                            "Enum arg '{}' must use ${{arg.{}.internal}} in expansion, not ${{arg.{}}}",
                            arg_name, arg_name, arg_name
                        ),
                    )
                    .with_hint(format!(
                        "Change to: ${{arg.{}.internal}}",
                        arg_name
                    )),
                );
            }
        }
    }
}

// =============================================================================
// UNLOCKS RULES (MACRO070)
// =============================================================================

fn check_unlocks(
    diags: &mut Vec<Diagnostic>,
    verb: &str,
    spec: &serde_yaml::Mapping,
    verb_names: &HashSet<String>,
) {
    if let Some(unlocks) = get_seq(spec, "unlocks") {
        let path = format!("{}.unlocks", verb);

        for (i, item) in unlocks.iter().enumerate() {
            if let Some(unlock_verb) = item.as_str() {
                // Check if the referenced verb exists in this file
                // Cross-registry check is done in Pass 2
                if !verb_names.contains(unlock_verb) {
                    // This is a warning in Pass 1, error in Pass 2 if registry provided
                    diags.push(Diagnostic::warn(
                        "MACRO070",
                        format!("{}[{}]", path, i),
                        format!(
                            "Unlocks reference '{}' not found in this file (may be in another file)",
                            unlock_verb
                        ),
                    ));
                }
            }
        }
    }
}

// =============================================================================
// UX WARNINGS (MACRO080)
// =============================================================================

fn check_ux_warnings(diags: &mut Vec<Diagnostic>, verb: &str, spec: &serde_yaml::Mapping) {
    // Check args for missing autofill_from and picker
    if let Some(args) = get_map(spec, "args") {
        for section in ["required", "optional"] {
            if let Some(section_map) = get_map(args, section) {
                for (k, v) in section_map {
                    let arg_name = k.as_str().unwrap_or("?");
                    if let Some(arg_spec) = v.as_mapping() {
                        let arg_path = format!("{}.args.{}.{}", verb, section, arg_name);

                        if let Some(arg_type) = get_str(arg_spec, "type") {
                            // MACRO080a: Missing autofill_from for *_ref types
                            if arg_type.ends_with("_ref") && !arg_spec.contains_key("autofill_from")
                            {
                                diags.push(
                                    Diagnostic::warn(
                                        "MACRO080a",
                                        &arg_path,
                                        format!("Missing autofill_from for {} type", arg_type),
                                    )
                                    .with_hint("Add: autofill_from: [session.current_structure]"),
                                );
                            }

                            // MACRO080b: Missing picker for *_ref types
                            if arg_type.ends_with("_ref") && !arg_spec.contains_key("picker") {
                                diags.push(
                                    Diagnostic::warn(
                                        "MACRO080b",
                                        &arg_path,
                                        format!("Missing picker for {} type", arg_type),
                                    )
                                    .with_hint("Add: picker: structure_in_scope"),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // MACRO080c: Check description length
    if let Some(ui) = get_map(spec, "ui") {
        if let Some(desc) = get_str(ui, "description") {
            if desc.len() < 12 {
                diags.push(
                    Diagnostic::warn(
                        "MACRO080c",
                        format!("{}.ui.description", verb),
                        "Description is too short (< 12 chars)",
                    )
                    .with_hint("Write a more descriptive explanation for operators"),
                );
            }
        }
    }
}

// =============================================================================
// PASS 2: CROSS-REGISTRY RULES
// =============================================================================

fn lint_cross_registry(
    diags: &mut Vec<Diagnostic>,
    verb: &str,
    spec: &serde_yaml::Mapping,
    registry: &dyn PrimitiveRegistry,
) {
    // MACRO071: expands_to verbs must exist
    if let Some(expands_to) = get_seq(spec, "expands_to") {
        for (i, step) in expands_to.iter().enumerate() {
            if let Some(step_map) = step.as_mapping() {
                if let Some(verb_value) = step_map.get(Value::String("verb".into())) {
                    if let Some(target_verb) = verb_value.as_str() {
                        if !registry.has_verb(target_verb) {
                            diags.push(Diagnostic::error(
                                "MACRO071",
                                format!("{}.expands_to[{}].verb", verb, i),
                                format!("Primitive verb '{}' not found in registry", target_verb),
                            ));
                        }
                    }
                }
            }
        }
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn get_str<'a>(map: &'a serde_yaml::Mapping, key: &str) -> Option<&'a str> {
    map.get(Value::String(key.into()))?.as_str()
}

fn get_map<'a>(map: &'a serde_yaml::Mapping, key: &str) -> Option<&'a serde_yaml::Mapping> {
    map.get(Value::String(key.into()))?.as_mapping()
}

fn get_seq<'a>(map: &'a serde_yaml::Mapping, key: &str) -> Option<&'a serde_yaml::Sequence> {
    map.get(Value::String(key.into()))?.as_sequence()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_macro() {
        let yaml = r#"
structure.setup:
  kind: macro
  ui:
    label: "Set up Structure"
    description: "Create a structure in the current client scope"
    target_label: "Structure"
  routing:
    mode_tags: [onboarding, kyc]
    operator_domain: structure
  target:
    operates_on: client_ref
    produces: structure_ref
  args:
    style: keyworded
    required:
      structure_type:
        type: enum
        ui_label: "Type"
        values:
          - key: pe
            label: "Private Equity"
            internal: private-equity
        default_key: pe
      name:
        type: str
        ui_label: "Structure name"
    optional: {}
  prereqs: []
  expands_to:
    - verb: cbu.create
      args:
        kind: "${arg.structure_type.internal}"
        name: "${arg.name}"
  unlocks: []
"#;

        let diags = lint_macro_file(yaml, None);
        let errors: Vec<_> = diags.iter().filter(|d| d.is_error()).collect();
        assert!(errors.is_empty(), "Unexpected errors: {:?}", errors);
    }

    #[test]
    fn test_forbidden_token_in_ui() {
        let yaml = r#"
bad.macro:
  kind: macro
  ui:
    label: "Create CBU"
    description: "Creates a CBU for the client"
    target_label: "CBU"
  routing:
    mode_tags: [test]
  target:
    operates_on: client_ref
  args:
    style: keyworded
    required: {}
    optional: {}
  prereqs: []
  expands_to:
    - verb: cbu.create
      args: {}
"#;

        let diags = lint_macro_file(yaml, None);
        let macro012: Vec<_> = diags.iter().filter(|d| d.code == "MACRO012").collect();
        assert!(
            !macro012.is_empty(),
            "Should have MACRO012 errors for 'cbu'"
        );
    }

    #[test]
    fn test_entity_ref_not_allowed() {
        let yaml = r#"
bad.macro:
  kind: macro
  ui:
    label: "Test"
    description: "Test description here"
    target_label: "Test"
  routing:
    mode_tags: [test]
  target:
    operates_on: client_ref
  args:
    style: keyworded
    required:
      entity:
        type: entity_ref
        ui_label: "Entity"
    optional: {}
  prereqs: []
  expands_to:
    - verb: test.verb
      args: {}
"#;

        let diags = lint_macro_file(yaml, None);
        let macro042: Vec<_> = diags.iter().filter(|d| d.code == "MACRO042").collect();
        assert!(
            !macro042.is_empty(),
            "Should have MACRO042 error for entity_ref"
        );
    }

    #[test]
    fn test_enum_without_internal() {
        let yaml = r#"
bad.macro:
  kind: macro
  ui:
    label: "Test"
    description: "Test description here"
    target_label: "Test"
  routing:
    mode_tags: [test]
  target:
    operates_on: client_ref
  args:
    style: keyworded
    required:
      role:
        type: enum
        ui_label: "Role"
        values:
          - key: gp
            label: "General Partner"
            internal: general-partner
    optional: {}
  prereqs: []
  expands_to:
    - verb: test.verb
      args:
        role: "${arg.role}"
"#;

        let diags = lint_macro_file(yaml, None);
        let macro044: Vec<_> = diags.iter().filter(|d| d.code == "MACRO044").collect();
        assert!(
            !macro044.is_empty(),
            "Should have MACRO044 error for enum without .internal"
        );
    }

    #[test]
    fn test_kinds_outside_internal() {
        let yaml = r#"
bad.macro:
  kind: macro
  ui:
    label: "Test"
    description: "Test description here"
    target_label: "Test"
  routing:
    mode_tags: [test]
  target:
    operates_on: client_ref
  args:
    style: keyworded
    required:
      party:
        type: party_ref
        ui_label: "Party"
        kinds: [person, company]
    optional: {}
  prereqs: []
  expands_to:
    - verb: test.verb
      args: {}
"#;

        let diags = lint_macro_file(yaml, None);
        let macro043: Vec<_> = diags.iter().filter(|d| d.code == "MACRO043").collect();
        assert!(
            !macro043.is_empty(),
            "Should have MACRO043 error for kinds outside internal"
        );
    }

    #[test]
    fn test_missing_required_fields() {
        let yaml = r#"
incomplete.macro:
  kind: macro
"#;

        let diags = lint_macro_file(yaml, None);
        let errors: Vec<_> = diags.iter().filter(|d| d.is_error()).collect();

        // Should have errors for missing ui, routing, target, args, prereqs, expands_to
        assert!(
            errors.iter().any(|d| d.code == "MACRO011"),
            "Missing MACRO011"
        );
        assert!(
            errors.iter().any(|d| d.code == "MACRO020"),
            "Missing MACRO020"
        );
        assert!(
            errors.iter().any(|d| d.code == "MACRO030"),
            "Missing MACRO030"
        );
        assert!(
            errors.iter().any(|d| d.code == "MACRO040"),
            "Missing MACRO040"
        );
        assert!(
            errors.iter().any(|d| d.code == "MACRO050"),
            "Missing MACRO050"
        );
        assert!(
            errors.iter().any(|d| d.code == "MACRO060"),
            "Missing MACRO060"
        );
    }

    #[test]
    fn test_invalid_variable_syntax() {
        let yaml = r#"
bad.macro:
  kind: macro
  ui:
    label: "Test"
    description: "Test description here"
    target_label: "Test"
  routing:
    mode_tags: [test]
  target:
    operates_on: client_ref
  args:
    style: keyworded
    required:
      name:
        type: str
        ui_label: "Name"
    optional: {}
  prereqs: []
  expands_to:
    - verb: test.verb
      args:
        value: "${unknown.path}"
        bad: "${arg.nonexistent}"
"#;

        let diags = lint_macro_file(yaml, None);
        let macro063: Vec<_> = diags.iter().filter(|d| d.code == "MACRO063").collect();
        assert!(
            macro063.len() >= 2,
            "Should have MACRO063 errors for invalid variables"
        );
    }

    #[test]
    fn test_ux_warnings() {
        let yaml = r#"
missing.autofill:
  kind: macro
  ui:
    label: "Test"
    description: "Short"
    target_label: "Test"
  routing:
    mode_tags: [test]
  target:
    operates_on: client_ref
  args:
    style: keyworded
    required:
      structure:
        type: structure_ref
        ui_label: "Structure"
    optional: {}
  prereqs: []
  expands_to:
    - verb: test.verb
      args: {}
"#;

        let diags = lint_macro_file(yaml, None);
        let warnings: Vec<_> = diags.iter().filter(|d| d.is_warning()).collect();

        // Should have warnings for missing autofill_from, picker, and short description
        assert!(
            warnings.iter().any(|d| d.code == "MACRO080a"),
            "Missing MACRO080a warning"
        );
        assert!(
            warnings.iter().any(|d| d.code == "MACRO080b"),
            "Missing MACRO080b warning"
        );
        assert!(
            warnings.iter().any(|d| d.code == "MACRO080c"),
            "Missing MACRO080c warning"
        );
    }

    #[test]
    fn test_contains_forbidden_token() {
        assert!(contains_forbidden_token("Create CBU").is_some());
        assert!(contains_forbidden_token("create cbu").is_some());
        assert!(contains_forbidden_token("entity_ref lookup").is_some());
        assert!(contains_forbidden_token("Create Structure").is_none());
        assert!(contains_forbidden_token("Open Case").is_none());
    }
}
