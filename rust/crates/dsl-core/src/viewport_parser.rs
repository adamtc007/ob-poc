//! Viewport verb parser - converts VerbCall AST to ViewportVerb types
//!
//! This module handles the semantic parsing of viewport domain verbs.
//! The main parser produces VerbCall nodes; this module interprets them
//! as viewport-specific operations.
//!
//! ## DSL Syntax
//!
//! Viewport verbs follow standard DSL syntax:
//! ```text
//! (viewport.focus :target "cbu:Acme Corp")
//! (viewport.enhance :level +)
//! (viewport.navigate :direction left)
//! (viewport.view :type ownership)
//! (viewport.fit :zone core)
//! (viewport.export :format png)
//! (viewport.ascend)
//! (viewport.descend :target "entity:John Smith")
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! use dsl_core::{parse_program, viewport_parser::parse_viewport_verb};
//!
//! let program = parse_program("(viewport.focus :target \"cbu:Acme\")")?;
//! if let Statement::VerbCall(vc) = &program.statements[0] {
//!     if let Some(viewport_verb) = parse_viewport_verb(vc)? {
//!         // viewport_verb is ViewportVerb::Focus { ... }
//!     }
//! }
//! ```

use crate::ast::{
    AstNode, ConfidenceZone, EnhanceArg, ExportFormat, FocusTarget, Literal, NavDirection,
    NavTarget, Span, VerbCall, ViewType, ViewportVerb,
};

/// Error type for viewport verb parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportParseError {
    pub message: String,
    pub span: Option<Span>,
}

impl ViewportParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl std::fmt::Display for ViewportParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ViewportParseError {}

/// Result type for viewport parsing operations
pub type ViewportParseResult<T> = Result<T, ViewportParseError>;

/// Check if a VerbCall is a viewport domain verb
pub fn is_viewport_verb(verb_call: &VerbCall) -> bool {
    verb_call.domain.eq_ignore_ascii_case("viewport")
}

/// Parse a VerbCall into a ViewportVerb if it's in the viewport domain
///
/// Returns `Ok(None)` if the verb is not in the viewport domain.
/// Returns `Ok(Some(ViewportVerb))` if parsing succeeds.
/// Returns `Err(ViewportParseError)` if the verb is viewport but malformed.
pub fn parse_viewport_verb(verb_call: &VerbCall) -> ViewportParseResult<Option<ViewportVerb>> {
    if !is_viewport_verb(verb_call) {
        return Ok(None);
    }

    let verb = parse_viewport_verb_inner(verb_call)?;
    Ok(Some(verb))
}

/// Parse a viewport verb call into a ViewportVerb (assumes domain is "viewport")
fn parse_viewport_verb_inner(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    let verb_lower = verb_call.verb.to_lowercase();

    match verb_lower.as_str() {
        "focus" => parse_focus_verb(verb_call),
        "enhance" => parse_enhance_verb(verb_call),
        "navigate" => parse_navigate_verb(verb_call),
        "ascend" => parse_ascend_verb(verb_call),
        "descend" => parse_descend_verb(verb_call),
        "view" => parse_view_verb(verb_call),
        "fit" => parse_fit_verb(verb_call),
        "export" => parse_export_verb(verb_call),
        _ => Err(
            ViewportParseError::new(format!("Unknown viewport verb: '{}'", verb_call.verb))
                .with_span(verb_call.span),
        ),
    }
}

// ============================================================================
// Individual Verb Parsers
// ============================================================================

/// Parse: (viewport.focus :target "cbu:Acme")
fn parse_focus_verb(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    let target = get_required_arg(verb_call, "target")?;
    let target_str = extract_string_value(&target.value, "target")?;
    let focus_target = parse_focus_target(&target_str, verb_call.span)?;

    Ok(ViewportVerb::Focus {
        target: focus_target,
        span: verb_call.span,
    })
}

/// Parse: (viewport.enhance :level +) or (viewport.enhance) for default +
fn parse_enhance_verb(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    let arg = match get_optional_arg(verb_call, "level") {
        Some(level_arg) => {
            let level_str = extract_string_value(&level_arg.value, "level")?;
            parse_enhance_arg(&level_str)?
        }
        None => EnhanceArg::Plus, // Default to +
    };

    Ok(ViewportVerb::Enhance {
        arg,
        span: verb_call.span,
    })
}

/// Parse: (viewport.navigate :direction left) or (viewport.navigate :target "entity:John")
fn parse_navigate_verb(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    // Try direction first
    if let Some(dir_arg) = get_optional_arg(verb_call, "direction") {
        let dir_str = extract_string_value(&dir_arg.value, "direction")?;
        let direction = NavDirection::parse(&dir_str).ok_or_else(|| {
            ViewportParseError::new(format!("Invalid navigation direction: '{}'", dir_str))
                .with_span(dir_arg.span)
        })?;
        return Ok(ViewportVerb::Navigate {
            target: NavTarget::Direction {
                direction,
                span: verb_call.span,
            },
            span: verb_call.span,
        });
    }

    // Try target
    if let Some(target_arg) = get_optional_arg(verb_call, "target") {
        let target_str = extract_string_value(&target_arg.value, "target")?;
        return Ok(ViewportVerb::Navigate {
            target: NavTarget::Entity {
                entity_ref: target_str,
                span: verb_call.span,
            },
            span: verb_call.span,
        });
    }

    Err(
        ViewportParseError::new("viewport.navigate requires either :direction or :target argument")
            .with_span(verb_call.span),
    )
}

/// Parse: (viewport.ascend)
fn parse_ascend_verb(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    // ascend takes no arguments
    Ok(ViewportVerb::Ascend {
        span: verb_call.span,
    })
}

/// Parse: (viewport.descend :target "entity:John")
fn parse_descend_verb(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    let target = get_required_arg(verb_call, "target")?;
    let target_str = extract_string_value(&target.value, "target")?;
    let focus_target = parse_focus_target(&target_str, verb_call.span)?;

    Ok(ViewportVerb::Descend {
        target: focus_target,
        span: verb_call.span,
    })
}

/// Parse: (viewport.view :type ownership)
fn parse_view_verb(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    let type_arg = get_required_arg(verb_call, "type")?;
    let type_str = extract_string_value(&type_arg.value, "type")?;

    let view_type = ViewType::parse(&type_str).ok_or_else(|| {
        ViewportParseError::new(format!("Invalid view type: '{}'", type_str))
            .with_span(type_arg.span)
    })?;

    Ok(ViewportVerb::View {
        view_type,
        span: verb_call.span,
    })
}

/// Parse: (viewport.fit) or (viewport.fit :zone core)
fn parse_fit_verb(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    let zone = match get_optional_arg(verb_call, "zone") {
        Some(zone_arg) => {
            let zone_str = extract_string_value(&zone_arg.value, "zone")?;
            Some(ConfidenceZone::parse(&zone_str).ok_or_else(|| {
                ViewportParseError::new(format!("Invalid confidence zone: '{}'", zone_str))
                    .with_span(zone_arg.span)
            })?)
        }
        None => None,
    };

    Ok(ViewportVerb::Fit {
        zone,
        span: verb_call.span,
    })
}

/// Parse: (viewport.export :format png)
fn parse_export_verb(verb_call: &VerbCall) -> ViewportParseResult<ViewportVerb> {
    let format_arg = get_required_arg(verb_call, "format")?;
    let format_str = extract_string_value(&format_arg.value, "format")?;

    let format = ExportFormat::parse(&format_str).ok_or_else(|| {
        ViewportParseError::new(format!("Invalid export format: '{}'", format_str))
            .with_span(format_arg.span)
    })?;

    Ok(ViewportVerb::Export {
        format,
        span: verb_call.span,
    })
}

// ============================================================================
// Argument Extraction Helpers
// ============================================================================

/// Get a required argument by key, returning an error if not found
fn get_required_arg<'a>(
    verb_call: &'a VerbCall,
    key: &str,
) -> ViewportParseResult<&'a crate::ast::Argument> {
    verb_call
        .arguments
        .iter()
        .find(|arg| arg.key.eq_ignore_ascii_case(key))
        .ok_or_else(|| {
            ViewportParseError::new(format!("Missing required argument ':{}'", key))
                .with_span(verb_call.span)
        })
}

/// Get an optional argument by key
fn get_optional_arg<'a>(verb_call: &'a VerbCall, key: &str) -> Option<&'a crate::ast::Argument> {
    verb_call
        .arguments
        .iter()
        .find(|arg| arg.key.eq_ignore_ascii_case(key))
}

/// Extract a string value from an AstNode
fn extract_string_value(node: &AstNode, arg_name: &str) -> ViewportParseResult<String> {
    match node {
        AstNode::Literal(Literal::String(s)) => Ok(s.clone()),
        AstNode::SymbolRef { name, .. } => Ok(format!("@{}", name)),
        _ => Err(ViewportParseError::new(format!(
            "Expected string value for ':{}'",
            arg_name
        ))),
    }
}

// ============================================================================
// Target and Argument Parsers
// ============================================================================

/// Parse a focus target string like "cbu:Acme" or "entity:John Smith"
fn parse_focus_target(target: &str, span: Span) -> ViewportParseResult<FocusTarget> {
    // Handle symbol references
    if let Some(stripped) = target.strip_prefix('@') {
        return Ok(FocusTarget::Symbol {
            name: stripped.to_string(),
            span,
        });
    }

    // Parse prefixed targets: "type:value"
    if let Some((prefix, value)) = target.split_once(':') {
        let prefix_lower = prefix.to_lowercase();
        match prefix_lower.as_str() {
            "cbu" => Ok(FocusTarget::Cbu {
                cbu_ref: value.to_string(),
                span,
            }),
            "entity" => Ok(FocusTarget::Entity {
                entity_ref: value.to_string(),
                span,
            }),
            "member" => Ok(FocusTarget::Member {
                member_ref: value.to_string(),
                span,
            }),
            "edge" => Ok(FocusTarget::Edge {
                edge_ref: value.to_string(),
                span,
            }),
            "type" => Ok(FocusTarget::InstrumentType {
                instrument_type: value.to_string(),
                span,
            }),
            "config" => Ok(FocusTarget::Config {
                config_node: value.to_string(),
                span,
            }),
            _ => Err(ViewportParseError::new(format!(
                "Unknown focus target type: '{}'. Expected one of: cbu, entity, member, edge, type, config",
                prefix
            )).with_span(span)),
        }
    } else if target.eq_ignore_ascii_case("matrix") {
        Ok(FocusTarget::Matrix { span })
    } else {
        // Bare string - assume it's a CBU name for convenience
        Ok(FocusTarget::Cbu {
            cbu_ref: target.to_string(),
            span,
        })
    }
}

/// Parse an enhance argument string
fn parse_enhance_arg(arg: &str) -> ViewportParseResult<EnhanceArg> {
    let arg_trimmed = arg.trim();

    match arg_trimmed {
        "+" => Ok(EnhanceArg::Plus),
        "-" => Ok(EnhanceArg::Minus),
        "max" => Ok(EnhanceArg::Max),
        "reset" => Ok(EnhanceArg::Reset),
        _ => {
            // Try to parse as a level number
            if let Ok(level) = arg_trimmed.parse::<u8>() {
                Ok(EnhanceArg::Level(level))
            } else {
                Err(ViewportParseError::new(format!(
                    "Invalid enhance argument: '{}'. Expected +, -, max, reset, or a number 0-5",
                    arg
                )))
            }
        }
    }
}

// ============================================================================
// Batch Parsing
// ============================================================================

/// Parse all viewport verbs from a program
pub fn extract_viewport_verbs(
    program: &crate::ast::Program,
) -> ViewportParseResult<Vec<ViewportVerb>> {
    let mut verbs = Vec::new();

    for statement in &program.statements {
        if let crate::ast::Statement::VerbCall(vc) = statement {
            if let Some(viewport_verb) = parse_viewport_verb(vc)? {
                verbs.push(viewport_verb);
            }
        }
    }

    Ok(verbs)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_program;

    fn parse_single_viewport(dsl: &str) -> ViewportParseResult<ViewportVerb> {
        let program = parse_program(dsl).map_err(|e| ViewportParseError::new(e))?;
        let vc = match &program.statements[0] {
            crate::ast::Statement::VerbCall(vc) => vc,
            _ => return Err(ViewportParseError::new("Expected VerbCall")),
        };
        parse_viewport_verb(vc)?.ok_or_else(|| ViewportParseError::new("Not a viewport verb"))
    }

    #[test]
    fn test_parse_focus_cbu() {
        let verb = parse_single_viewport(r#"(viewport.focus :target "cbu:Acme Corp")"#).unwrap();
        match verb {
            ViewportVerb::Focus { target, .. } => match target {
                FocusTarget::Cbu { cbu_ref, .. } => assert_eq!(cbu_ref, "Acme Corp"),
                _ => panic!("Expected FocusTarget::Cbu"),
            },
            _ => panic!("Expected Focus verb"),
        }
    }

    #[test]
    fn test_parse_focus_entity() {
        let verb =
            parse_single_viewport(r#"(viewport.focus :target "entity:John Smith")"#).unwrap();
        match verb {
            ViewportVerb::Focus { target, .. } => match target {
                FocusTarget::Entity { entity_ref, .. } => assert_eq!(entity_ref, "John Smith"),
                _ => panic!("Expected FocusTarget::Entity"),
            },
            _ => panic!("Expected Focus verb"),
        }
    }

    #[test]
    fn test_parse_focus_matrix() {
        let verb = parse_single_viewport(r#"(viewport.focus :target "matrix")"#).unwrap();
        match verb {
            ViewportVerb::Focus { target, .. } => {
                assert!(matches!(target, FocusTarget::Matrix { .. }));
            }
            _ => panic!("Expected Focus verb"),
        }
    }

    #[test]
    fn test_parse_focus_bare_string() {
        // Bare string without prefix assumes CBU
        let verb = parse_single_viewport(r#"(viewport.focus :target "Acme")"#).unwrap();
        match verb {
            ViewportVerb::Focus { target, .. } => match target {
                FocusTarget::Cbu { cbu_ref, .. } => assert_eq!(cbu_ref, "Acme"),
                _ => panic!("Expected FocusTarget::Cbu"),
            },
            _ => panic!("Expected Focus verb"),
        }
    }

    #[test]
    fn test_parse_enhance_plus() {
        let verb = parse_single_viewport(r#"(viewport.enhance :level "+")"#).unwrap();
        match verb {
            ViewportVerb::Enhance { arg, .. } => {
                assert_eq!(arg, EnhanceArg::Plus);
            }
            _ => panic!("Expected Enhance verb"),
        }
    }

    #[test]
    fn test_parse_enhance_minus() {
        let verb = parse_single_viewport(r#"(viewport.enhance :level "-")"#).unwrap();
        match verb {
            ViewportVerb::Enhance { arg, .. } => {
                assert_eq!(arg, EnhanceArg::Minus);
            }
            _ => panic!("Expected Enhance verb"),
        }
    }

    #[test]
    fn test_parse_enhance_level() {
        let verb = parse_single_viewport(r#"(viewport.enhance :level "3")"#).unwrap();
        match verb {
            ViewportVerb::Enhance { arg, .. } => {
                assert_eq!(arg, EnhanceArg::Level(3));
            }
            _ => panic!("Expected Enhance verb"),
        }
    }

    #[test]
    fn test_parse_enhance_max() {
        let verb = parse_single_viewport(r#"(viewport.enhance :level "max")"#).unwrap();
        match verb {
            ViewportVerb::Enhance { arg, .. } => {
                assert_eq!(arg, EnhanceArg::Max);
            }
            _ => panic!("Expected Enhance verb"),
        }
    }

    #[test]
    fn test_parse_enhance_default() {
        // No :level argument defaults to +
        let verb = parse_single_viewport(r#"(viewport.enhance)"#).unwrap();
        match verb {
            ViewportVerb::Enhance { arg, .. } => {
                assert_eq!(arg, EnhanceArg::Plus);
            }
            _ => panic!("Expected Enhance verb"),
        }
    }

    #[test]
    fn test_parse_navigate_direction() {
        let verb = parse_single_viewport(r#"(viewport.navigate :direction "left")"#).unwrap();
        match verb {
            ViewportVerb::Navigate { target, .. } => match target {
                NavTarget::Direction { direction, .. } => {
                    assert_eq!(direction, NavDirection::Left);
                }
                _ => panic!("Expected NavTarget::Direction"),
            },
            _ => panic!("Expected Navigate verb"),
        }
    }

    #[test]
    fn test_parse_navigate_entity() {
        let verb = parse_single_viewport(r#"(viewport.navigate :target "entity:John")"#).unwrap();
        match verb {
            ViewportVerb::Navigate { target, .. } => match target {
                NavTarget::Entity { entity_ref, .. } => {
                    assert_eq!(entity_ref, "entity:John");
                }
                _ => panic!("Expected NavTarget::Entity"),
            },
            _ => panic!("Expected Navigate verb"),
        }
    }

    #[test]
    fn test_parse_ascend() {
        let verb = parse_single_viewport(r#"(viewport.ascend)"#).unwrap();
        assert!(matches!(verb, ViewportVerb::Ascend { .. }));
    }

    #[test]
    fn test_parse_descend() {
        let verb = parse_single_viewport(r#"(viewport.descend :target "entity:Child")"#).unwrap();
        match verb {
            ViewportVerb::Descend { target, .. } => match target {
                FocusTarget::Entity { entity_ref, .. } => assert_eq!(entity_ref, "Child"),
                _ => panic!("Expected FocusTarget::Entity"),
            },
            _ => panic!("Expected Descend verb"),
        }
    }

    #[test]
    fn test_parse_view() {
        let verb = parse_single_viewport(r#"(viewport.view :type "ownership")"#).unwrap();
        match verb {
            ViewportVerb::View { view_type, .. } => {
                assert_eq!(view_type, ViewType::Ownership);
            }
            _ => panic!("Expected View verb"),
        }
    }

    #[test]
    fn test_parse_fit_no_zone() {
        let verb = parse_single_viewport(r#"(viewport.fit)"#).unwrap();
        match verb {
            ViewportVerb::Fit { zone, .. } => {
                assert_eq!(zone, None);
            }
            _ => panic!("Expected Fit verb"),
        }
    }

    #[test]
    fn test_parse_fit_with_zone() {
        let verb = parse_single_viewport(r#"(viewport.fit :zone "core")"#).unwrap();
        match verb {
            ViewportVerb::Fit { zone, .. } => {
                assert_eq!(zone, Some(ConfidenceZone::Core));
            }
            _ => panic!("Expected Fit verb"),
        }
    }

    #[test]
    fn test_parse_export() {
        let verb = parse_single_viewport(r#"(viewport.export :format "png")"#).unwrap();
        match verb {
            ViewportVerb::Export { format, .. } => {
                assert_eq!(format, ExportFormat::Png);
            }
            _ => panic!("Expected Export verb"),
        }
    }

    #[test]
    fn test_parse_export_hardcopy() {
        let verb = parse_single_viewport(r#"(viewport.export :format "hardcopy")"#).unwrap();
        match verb {
            ViewportVerb::Export { format, .. } => {
                assert_eq!(format, ExportFormat::Hardcopy);
            }
            _ => panic!("Expected Export verb"),
        }
    }

    #[test]
    fn test_non_viewport_verb_returns_none() {
        let program = parse_program(r#"(cbu.create :name "Test")"#).unwrap();
        let vc = match &program.statements[0] {
            crate::ast::Statement::VerbCall(vc) => vc,
            _ => panic!("Expected VerbCall"),
        };
        let result = parse_viewport_verb(vc).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_unknown_viewport_verb_errors() {
        let result = parse_single_viewport(r#"(viewport.unknown :arg "value")"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unknown viewport verb"));
    }

    #[test]
    fn test_missing_required_arg() {
        let result = parse_single_viewport(r#"(viewport.focus)"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Missing required argument"));
    }

    #[test]
    fn test_invalid_direction() {
        let result = parse_single_viewport(r#"(viewport.navigate :direction "diagonal")"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Invalid navigation direction"));
    }

    #[test]
    fn test_extract_viewport_verbs_from_program() {
        let program = parse_program(
            r#"
            (cbu.create :name "Test")
            (viewport.focus :target "cbu:Test")
            (viewport.enhance :level "+")
            (entity.create :name "John")
            (viewport.view :type "ownership")
        "#,
        )
        .unwrap();

        let viewport_verbs = extract_viewport_verbs(&program).unwrap();
        assert_eq!(viewport_verbs.len(), 3);

        assert!(matches!(viewport_verbs[0], ViewportVerb::Focus { .. }));
        assert!(matches!(viewport_verbs[1], ViewportVerb::Enhance { .. }));
        assert!(matches!(viewport_verbs[2], ViewportVerb::View { .. }));
    }

    #[test]
    fn test_case_insensitive_verb() {
        let verb = parse_single_viewport(r#"(VIEWPORT.FOCUS :target "cbu:Test")"#).unwrap();
        assert!(matches!(verb, ViewportVerb::Focus { .. }));
    }

    #[test]
    fn test_case_insensitive_view_type() {
        let verb = parse_single_viewport(r#"(viewport.view :type "OWNERSHIP")"#).unwrap();
        match verb {
            ViewportVerb::View { view_type, .. } => {
                assert_eq!(view_type, ViewType::Ownership);
            }
            _ => panic!("Expected View verb"),
        }
    }

    #[test]
    fn test_symbol_ref_in_focus_target() {
        let verb = parse_single_viewport(r#"(viewport.focus :target "@my-cbu")"#).unwrap();
        match verb {
            ViewportVerb::Focus { target, .. } => match target {
                FocusTarget::Symbol { name, .. } => assert_eq!(name, "my-cbu"),
                _ => panic!("Expected FocusTarget::Symbol"),
            },
            _ => panic!("Expected Focus verb"),
        }
    }

    #[test]
    fn test_all_view_types() {
        let view_types = [
            ("structure", ViewType::Structure),
            ("ownership", ViewType::Ownership),
            ("accounts", ViewType::Accounts),
            ("compliance", ViewType::Compliance),
            ("geographic", ViewType::Geographic),
            ("temporal", ViewType::Temporal),
            ("instruments", ViewType::Instruments),
        ];

        for (name, expected) in view_types {
            let dsl = format!(r#"(viewport.view :type "{}")"#, name);
            let verb = parse_single_viewport(&dsl).unwrap();
            match verb {
                ViewportVerb::View { view_type, .. } => {
                    assert_eq!(view_type, expected, "Failed for view type: {}", name);
                }
                _ => panic!("Expected View verb"),
            }
        }
    }

    #[test]
    fn test_all_confidence_zones() {
        let zones = [
            ("core", ConfidenceZone::Core),
            ("shell", ConfidenceZone::Shell),
            ("penumbra", ConfidenceZone::Penumbra),
            ("all", ConfidenceZone::All),
        ];

        for (name, expected) in zones {
            let dsl = format!(r#"(viewport.fit :zone "{}")"#, name);
            let verb = parse_single_viewport(&dsl).unwrap();
            match verb {
                ViewportVerb::Fit { zone, .. } => {
                    assert_eq!(zone, Some(expected), "Failed for zone: {}", name);
                }
                _ => panic!("Expected Fit verb"),
            }
        }
    }

    #[test]
    fn test_all_export_formats() {
        let formats = [
            ("png", ExportFormat::Png),
            ("svg", ExportFormat::Svg),
            ("graphml", ExportFormat::GraphMl),
            ("hardcopy", ExportFormat::Hardcopy),
        ];

        for (name, expected) in formats {
            let dsl = format!(r#"(viewport.export :format "{}")"#, name);
            let verb = parse_single_viewport(&dsl).unwrap();
            match verb {
                ViewportVerb::Export { format, .. } => {
                    assert_eq!(format, expected, "Failed for format: {}", name);
                }
                _ => panic!("Expected Export verb"),
            }
        }
    }

    #[test]
    fn test_all_nav_directions() {
        let directions = [
            ("left", NavDirection::Left),
            ("right", NavDirection::Right),
            ("up", NavDirection::Up),
            ("down", NavDirection::Down),
            ("in", NavDirection::In),
            ("out", NavDirection::Out),
        ];

        for (name, expected) in directions {
            let dsl = format!(r#"(viewport.navigate :direction "{}")"#, name);
            let verb = parse_single_viewport(&dsl).unwrap();
            match verb {
                ViewportVerb::Navigate { target, .. } => match target {
                    NavTarget::Direction { direction, .. } => {
                        assert_eq!(direction, expected, "Failed for direction: {}", name);
                    }
                    _ => panic!("Expected NavTarget::Direction"),
                },
                _ => panic!("Expected Navigate verb"),
            }
        }
    }
}
