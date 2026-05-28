//! Resolution pass for the unified DSL v0.1.
//!
//! Validates `(decision-pack ...)`, `(provenance ...)`, and
//! `(governance-status ...)` atoms; indexes packs into the registry.

use std::collections::HashSet;

use dsl_ast::{AtomBag, TypedAtom};
use dsl_atoms::{AtomKindClass, DeclarativeKind, StructuralKind};
use dsl_diagnostics::{
    Diagnostic, DiagnosticBag, INVALID_PARAMETER_NAME, UNKNOWN_LOOP_VARIABLE,
    UNKNOWN_PACK_REFERENCE, UNKNOWN_TEMPLATE_PARAMETER, UNRESOLVED_NAME_REF,
};
use dsl_parser::RawValue;

use crate::pack_registry::{DecisionPack, PackParam, PackRegistry};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the resolution pass over `bag`.
///
/// Three things happen:
/// 1. `(decision-pack ...)` atoms are validated and indexed into `registry`.
/// 2. `(provenance ...)` atoms have their `:covers` name-refs checked and their
///    `:source-id` looked up in `registry`.
/// 3. `(governance-status ...)` atoms have their `:atom` name-ref checked.
pub fn resolve(bag: &AtomBag, registry: &mut PackRegistry, diagnostics: &mut DiagnosticBag) {
    // 1. Decision-pack atoms
    for atom in bag.atoms_of_structural_kind(StructuralKind::DecisionPack) {
        resolve_decision_pack(atom, registry, diagnostics);
    }

    // 2. Provenance atoms
    for atom in bag.declarative_atoms() {
        if atom.kind_class == AtomKindClass::Declarative(DeclarativeKind::Provenance) {
            resolve_provenance(atom, bag, registry, diagnostics);
        }
    }

    // 3. Governance-status atoms
    for atom in bag.declarative_atoms() {
        if atom.kind_class == AtomKindClass::Declarative(DeclarativeKind::GovernanceStatus) {
            resolve_governance_status(atom, bag, diagnostics);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers — slot accessors
// ---------------------------------------------------------------------------

/// Return the string value of the first slot named `key` in `raw.slots`,
/// or `None` if absent or not a string/symbol/bool/int.
pub(crate) fn get_slot_str(raw: &dsl_parser::RawAtom, key: &str) -> Option<String> {
    raw.slots
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            RawValue::StringLit(s) => Some(s.clone()),
            RawValue::Symbol(s) => Some(s.clone()),
            RawValue::BoolLit(b) => Some(b.to_string()),
            RawValue::IntLit(i) => Some(i.to_string()),
            RawValue::FloatLit(f) => Some(f.to_string()),
            _ => None,
        })
}

/// Return the items of a list-valued slot named `key` as strings.
/// Returns an empty `Vec` if the slot is absent, not a list, or items are
/// non-string.
pub(crate) fn extract_string_list(raw: &dsl_parser::RawAtom, key: &str) -> Vec<String> {
    let Some((_, val)) = raw.slots.iter().find(|(k, _)| k == key) else {
        return Vec::new();
    };
    match val {
        RawValue::List(items) => items
            .iter()
            .filter_map(|v| match v {
                RawValue::StringLit(s) => Some(s.clone()),
                RawValue::Symbol(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        // A single non-list value is treated as a one-element list.
        RawValue::StringLit(s) => vec![s.clone()],
        RawValue::Symbol(s) => vec![s.clone()],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Decision-pack resolution
// ---------------------------------------------------------------------------

fn resolve_decision_pack(
    atom: &TypedAtom,
    registry: &mut PackRegistry,
    diagnostics: &mut DiagnosticBag,
) {
    let raw = &atom.raw;
    let name = atom.name.clone().unwrap_or_default();
    let version = get_slot_str(raw, "version").unwrap_or_else(|| "0.0.0".to_string());
    let description = get_slot_str(raw, "description").unwrap_or_default();

    let params = extract_params(raw, diagnostics);

    // Validate template body: TemplateSubst/TemplateSplice forms must reference
    // declared parameter names.
    let param_names: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    validate_template(raw, &param_names, diagnostics);

    let pack = DecisionPack {
        name: name.clone(),
        version,
        description,
        domain_scope: extract_string_list(raw, "domain-scope"),
        parameters: params,
        example_utterances: extract_string_list(raw, "example-utterances"),
        structural_signature: None, // map slot parsing deferred
        governance_ref: get_slot_str(raw, "governance-ref"),
        template_raw: "<template>".to_string(),
    };
    let _ = registry.register(pack);
}

/// Extract the `:parameters` slot as a list of `PackParam`.
///
/// The `:parameters` slot is a list of maps of the form
/// `[{:name N :type T :required bool ...} ...]`.
/// Malformed entries produce warnings and are skipped.
fn extract_params(raw: &dsl_parser::RawAtom, diagnostics: &mut DiagnosticBag) -> Vec<PackParam> {
    let Some((_, params_val)) = raw.slots.iter().find(|(k, _)| k == "parameters") else {
        return Vec::new();
    };
    let RawValue::List(items) = params_val else {
        diagnostics.push(Diagnostic::warning(
            "decision-pack :parameters slot is not a list",
        ));
        return Vec::new();
    };

    let mut result = Vec::new();
    for item in items {
        match item {
            RawValue::Map(pairs) => {
                let get = |key: &str| -> Option<String> {
                    pairs
                        .iter()
                        .find(|(k, _)| k == key)
                        .and_then(|(_, v)| match v {
                            RawValue::StringLit(s) => Some(s.clone()),
                            RawValue::Symbol(s) => Some(s.clone()),
                            RawValue::BoolLit(b) => Some(b.to_string()),
                            _ => None,
                        })
                };
                let Some(name) = get("name") else {
                    diagnostics.push(Diagnostic::warning(
                        "decision-pack parameter map missing :name",
                    ));
                    continue;
                };
                let param_type = get("type").unwrap_or_else(|| "string".to_string());
                let required = pairs
                    .iter()
                    .find(|(k, _)| k == "required")
                    .map(|(_, v)| matches!(v, RawValue::BoolLit(true)))
                    .unwrap_or(false);
                let description = get("description");
                let default_value = get("default");
                // Dots in parameter names are reserved for the for-each
                // loop-variable accessor syntax (`,var.field`).
                if name.contains('.') {
                    diagnostics.push(Diagnostic::error_with_code(
                        format!(
                            "Parameter name '{}' contains '.'; dots are reserved \
                                 for for-each accessor syntax (,var.field)",
                            name
                        ),
                        INVALID_PARAMETER_NAME,
                    ));
                }
                result.push(PackParam {
                    name,
                    param_type,
                    required,
                    description,
                    default_value,
                });
            }
            _ => {
                diagnostics.push(Diagnostic::warning(
                    "decision-pack :parameters list item is not a map",
                ));
            }
        }
    }
    result
}

/// Walk every `RawValue` in the `:template` slot and check that all
/// `TemplateSubst` and `TemplateSplice` forms reference a declared parameter.
fn validate_template(
    raw: &dsl_parser::RawAtom,
    param_names: &HashSet<String>,
    diagnostics: &mut DiagnosticBag,
) {
    let Some((_, template_val)) = raw.slots.iter().find(|(k, _)| k == "template") else {
        return;
    };
    walk_value(template_val, param_names, None, diagnostics);
}

/// Walk a `RawValue`, validating all template reference forms.
///
/// `loop_var` is `Some(name)` when we are inside a `for-each` body; it
/// permits `,loop_var.field` accessor forms without requiring them to be in
/// `param_names`.
fn walk_value(
    val: &RawValue,
    param_names: &HashSet<String>,
    loop_var: Option<&str>,
    diagnostics: &mut DiagnosticBag,
) {
    match val {
        RawValue::TemplateSubst(name) => {
            // Allow `,var.field` inside for-each bodies where `var` matches
            // the declared loop variable.  The field part is unchecked at
            // v0.2 (dynamic map access; field names are not declared in the
            // parameter schema).
            if let Some(dot_pos) = name.find('.') {
                let var_part = &name[..dot_pos];
                if let Some(lv) = loop_var {
                    if var_part == lv {
                        return; // valid accessor form
                    }
                }
                // Dot outside a for-each context, or var doesn't match.
                diagnostics.push(Diagnostic::error_with_code(
                    format!(
                        "Template substitution ',{}' uses dot accessor but '{}' is not \
                             the active loop variable",
                        name, var_part
                    ),
                    UNKNOWN_LOOP_VARIABLE,
                ));
            } else if !param_names.contains(name) {
                diagnostics.push(Diagnostic::error_with_code(
                    format!("Unknown template parameter ',{}'", name),
                    UNKNOWN_TEMPLATE_PARAMETER,
                ));
            }
        }
        RawValue::TemplateSplice(name) if !param_names.contains(name) => {
            diagnostics.push(Diagnostic::error_with_code(
                format!("Unknown template splice ',@{}'", name),
                UNKNOWN_TEMPLATE_PARAMETER,
            ));
        }
        RawValue::ForEach {
            var,
            list_param,
            body,
        } => {
            // Validate that :in references a declared list-typed parameter.
            if !param_names.contains(list_param.as_str()) {
                diagnostics.push(Diagnostic::error_with_code(
                    format!(
                        "for-each :in '{}' does not reference a declared parameter",
                        list_param
                    ),
                    UNKNOWN_TEMPLATE_PARAMETER,
                ));
            }
            // Walk body with the loop variable in scope.
            for item in body {
                walk_value(item, param_names, Some(var.as_str()), diagnostics);
            }
        }
        RawValue::List(items) => {
            for item in items {
                walk_value(item, param_names, loop_var, diagnostics);
            }
        }
        RawValue::Map(pairs) => {
            for (_, v) in pairs {
                walk_value(v, param_names, loop_var, diagnostics);
            }
        }
        RawValue::Atom(inner) => {
            for (_, v) in &inner.slots {
                walk_value(v, param_names, loop_var, diagnostics);
            }
        }
        // Leaf values — nothing to check.
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Provenance resolution
// ---------------------------------------------------------------------------

fn resolve_provenance(
    atom: &TypedAtom,
    bag: &AtomBag,
    registry: &PackRegistry,
    diagnostics: &mut DiagnosticBag,
) {
    let raw = &atom.raw;

    // :covers — list of name-refs that should resolve to structural atoms.
    let covers = extract_string_list(raw, "covers");
    for name_ref in &covers {
        if bag.find(name_ref).is_none() {
            diagnostics.push(
                Diagnostic::warning(format!(
                    "provenance :covers ref '{}' could not be resolved (may be in another file)",
                    name_ref
                ))
                .with_code(UNRESOLVED_NAME_REF),
            );
        }
    }

    // :source-id — should name a known pack.
    if let Some(source_id) = get_slot_str(raw, "source-id") {
        if source_id != "hand-authored" && registry.lookup_latest(&source_id).is_none() {
            diagnostics.push(
                Diagnostic::warning(format!(
                    "provenance :source-id '{}' does not match any known pack",
                    source_id
                ))
                .with_code(UNKNOWN_PACK_REFERENCE),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Governance-status resolution
// ---------------------------------------------------------------------------

fn resolve_governance_status(atom: &TypedAtom, bag: &AtomBag, diagnostics: &mut DiagnosticBag) {
    let raw = &atom.raw;
    // :atom — name-ref to the governed structural atom.
    if let Some(atom_ref) = get_slot_str(raw, "atom") {
        if bag.find(&atom_ref).is_none() {
            diagnostics.push(
                Diagnostic::warning(format!(
                    "governance-status :atom ref '{}' could not be resolved",
                    atom_ref
                ))
                .with_code(UNRESOLVED_NAME_REF),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_ast::AtomBag;
    use dsl_diagnostics::DiagnosticBag;

    fn parse_and_resolve(src: &str) -> (PackRegistry, DiagnosticBag) {
        let (sf, parse_diag) = dsl_parser::parse(src);
        let mut diag = DiagnosticBag::new();
        for d in parse_diag.diagnostics {
            diag.push(d);
        }
        let bag = AtomBag::from_source_file(sf, &mut diag);
        let mut registry = PackRegistry::new();
        resolve(&bag, &mut registry, &mut diag);
        (registry, diag)
    }

    #[test]
    fn simple_pack_indexes_into_registry() {
        // Note: TemplateSubst/TemplateSplice in ATOM NAME positions are not
        // supported by the v0.1 parser (names must be Symbols). Templates use
        // TemplateSubst only in slot VALUE positions and flow source/target
        // positions (handled by the flow positional-value parser). The gateway
        // placeholder uses a fixed name; TemplateSubst is referenced in flow
        // source/target slots which DO work.
        let src = r#"
(decision-pack my-gate
  :version "1.0.0"
  :description "A simple gate"
  :domain-scope [cbu]
  :parameters [
    {:name gate-name :type symbol :required true}
    {:name ok-path   :type node-ref :required true}
  ]
  :template [
    (flow $pre-node -> ,gate-name)
    (flow ,ok-path -> ,gate-name)
    (flow ,gate-name -> ,ok-path :default true)
  ])
"#;
        let (registry, diag) = parse_and_resolve(src);
        assert!(
            !diag.has_errors(),
            "unexpected errors: {:?}",
            diag.diagnostics
        );
        assert!(registry.lookup("my-gate", "1.0.0").is_some());
    }

    #[test]
    fn unknown_template_parameter_is_error() {
        // TemplateSubst in flow source/target positions works with the parser;
        // use that to trigger the UNKNOWN_TEMPLATE_PARAMETER validation.
        // ,unknown-param is NOT in the :parameters list → resolution error.
        let src = r#"
(decision-pack bad-pack
  :version "1.0.0"
  :description "Bad"
  :parameters [
    {:name known-param :type symbol :required true}
  ]
  :template [
    (flow ,known-param -> ,unknown-param :default true)
  ])
"#;
        let (_, diag) = parse_and_resolve(src);
        assert!(
            diag.has_errors(),
            "expected error for unknown template param"
        );
        let has_code = diag
            .errors()
            .any(|d| d.code.as_deref() == Some(UNKNOWN_TEMPLATE_PARAMETER));
        assert!(has_code, "expected UNKNOWN_TEMPLATE_PARAMETER code");
    }

    #[test]
    fn provenance_source_id_unknown_is_warning() {
        let src = r#"
(provenance my-prov
  :covers [some-atom]
  :source-id nonexistent-pack
  :version "1.0.0")
"#;
        let (_, diag) = parse_and_resolve(src);
        // Should not be an error, just a warning.
        assert!(!diag.has_errors(), "should be a warning, not an error");
        let has_warn = diag
            .warnings()
            .any(|d| d.code.as_deref() == Some(UNKNOWN_PACK_REFERENCE));
        assert!(has_warn, "expected UNKNOWN_PACK_REFERENCE warning");
    }

    #[test]
    fn governance_status_unresolved_atom_ref_is_warning() {
        let src = r#"
(governance-status my-status
  :atom missing-atom
  :state active)
"#;
        let (_, diag) = parse_and_resolve(src);
        assert!(!diag.has_errors(), "should be a warning");
        let has_warn = diag
            .warnings()
            .any(|d| d.code.as_deref() == Some(UNRESOLVED_NAME_REF));
        assert!(has_warn, "expected UNRESOLVED_NAME_REF warning");
    }
}
