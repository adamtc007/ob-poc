//! DSL Enrichment Service
//!
//! Converts raw DSL source + session bindings into EnrichedDsl for UI display.
//! This is a single-pass operation optimized for fast round-trips.
//!
//! ## Pipeline
//! ```text
//! DSL Source + Session Bindings
//!     │
//!     ▼ parse (NOM)
//! Raw AST (SymbolRef nodes)
//!     │
//!     ▼ walk + lookup bindings
//! EnrichedDsl { source, segments, binding_summary }
//! ```
//!
//! ## Design Principles
//! - Single parse, single walk
//! - No async I/O (bindings already in memory)
//! - Returns immediately - no EntityGateway calls here
//! - Unresolved refs are flagged for UI to trigger resolution

use ob_poc_types::{BindingSummary, DslDisplaySegment, EnrichedDsl};
use std::collections::HashMap;
use uuid::Uuid;

use crate::dsl_v2::ast::{AstNode, Program, Statement, VerbCall};
use crate::dsl_v2::parse_program;

/// Session binding info (minimal struct for lookups)
#[derive(Debug, Clone)]
pub struct BindingInfo {
    pub id: Uuid,
    pub display_name: String,
    pub entity_type: String,
}

/// Enriches DSL source with session binding information.
///
/// This is a fast, synchronous operation that:
/// 1. Parses DSL to AST
/// 2. Walks AST to find SymbolRef and EntityRef nodes
/// 3. Looks up each reference in the provided bindings map
/// 4. Produces segments for rich UI rendering
pub fn enrich_dsl(
    source: &str,
    bindings: &HashMap<String, BindingInfo>,
    active_cbu: Option<&BindingInfo>,
) -> Result<EnrichedDsl, String> {
    // Parse DSL
    let program = parse_program(source).map_err(|e| format!("Parse error: {}", e))?;

    // Build segments by walking the source with AST guidance
    let segments = build_segments(source, &program, bindings);

    // Build binding summary
    let binding_summary = build_binding_summary(bindings, active_cbu);

    // Check if fully resolved
    let fully_resolved = segments.iter().all(|seg| {
        !matches!(
            seg,
            DslDisplaySegment::UnresolvedRef { .. }
                | DslDisplaySegment::Binding {
                    entity_id: None,
                    ..
                }
        )
    });

    Ok(EnrichedDsl {
        source: source.to_string(),
        segments,
        binding_summary,
        fully_resolved,
    })
}

/// Build display segments from source + AST + bindings
///
/// Strategy: Walk source character by character, using AST spans to identify
/// special regions (symbols, entity refs). This preserves exact formatting.
fn build_segments(
    source: &str,
    program: &Program,
    bindings: &HashMap<String, BindingInfo>,
) -> Vec<DslDisplaySegment> {
    // Collect all symbol/entity ref spans from AST
    let mut special_regions: Vec<SpecialRegion> = Vec::new();
    for stmt in &program.statements {
        collect_special_regions(stmt, &mut special_regions);
    }

    // Sort by start offset
    special_regions.sort_by_key(|r| r.start);

    // Build segments
    let mut segments = Vec::new();
    let mut pos = 0;
    for region in special_regions {
        // Add text before this region
        if region.start > pos {
            let text = &source[pos..region.start];
            if !text.is_empty() {
                // Split into text and whitespace segments
                add_text_segments(&mut segments, text);
            }
        }

        // Add the special region
        match region.kind {
            RegionKind::SymbolRef { ref name } => {
                let binding = bindings.get(name);
                segments.push(DslDisplaySegment::Binding {
                    symbol: name.clone(),
                    display_name: binding.map(|b| b.display_name.clone()),
                    entity_type: binding.map(|b| b.entity_type.clone()),
                    entity_id: binding.map(|b| b.id.to_string()),
                    editable: true,
                    source_offset: region.start,
                });
            }
            RegionKind::EntityRef {
                ref entity_type,
                ref value,
                ref resolved_key,
                ref arg_name,
            } => {
                if let Some(ref key) = resolved_key {
                    // Resolved - show as binding-like
                    segments.push(DslDisplaySegment::Binding {
                        symbol: value.clone(),
                        display_name: Some(value.clone()),
                        entity_type: Some(entity_type.clone()),
                        entity_id: Some(key.clone()),
                        editable: true,
                        source_offset: region.start,
                    });
                } else {
                    // Unresolved - needs resolution
                    segments.push(DslDisplaySegment::UnresolvedRef {
                        search_value: value.clone(),
                        entity_type: entity_type.clone(),
                        arg_name: arg_name.clone(),
                        ref_id: format!("ref_{}_{}", region.start, value),
                        source_offset: region.start,
                    });
                }
            }
            RegionKind::Comment { ref content } => {
                segments.push(DslDisplaySegment::Comment {
                    content: content.clone(),
                });
            }
        }

        pos = region.end;
    }

    // Add remaining text
    if pos < source.len() {
        let text = &source[pos..];
        if !text.is_empty() {
            add_text_segments(&mut segments, text);
        }
    }

    segments
}

/// Add text segments, separating whitespace from content
fn add_text_segments(segments: &mut Vec<DslDisplaySegment>, text: &str) {
    let mut current_start = 0;
    let mut in_whitespace = false;

    for (i, c) in text.char_indices() {
        let is_ws = c.is_whitespace();
        if i == 0 {
            in_whitespace = is_ws;
        } else if is_ws != in_whitespace {
            // Transition - emit previous segment
            let segment_text = &text[current_start..i];
            if in_whitespace {
                segments.push(DslDisplaySegment::Whitespace {
                    content: segment_text.to_string(),
                });
            } else {
                segments.push(DslDisplaySegment::Text {
                    content: segment_text.to_string(),
                });
            }
            current_start = i;
            in_whitespace = is_ws;
        }
    }

    // Emit final segment
    if current_start < text.len() {
        let segment_text = &text[current_start..];
        if in_whitespace {
            segments.push(DslDisplaySegment::Whitespace {
                content: segment_text.to_string(),
            });
        } else {
            segments.push(DslDisplaySegment::Text {
                content: segment_text.to_string(),
            });
        }
    }
}

/// A special region in the source that needs enrichment
#[derive(Debug)]
struct SpecialRegion {
    start: usize,
    end: usize,
    kind: RegionKind,
}

#[derive(Debug)]
enum RegionKind {
    SymbolRef {
        name: String,
    },
    EntityRef {
        entity_type: String,
        value: String,
        resolved_key: Option<String>,
        arg_name: String,
    },
    // Comment variant reserved for future use when AST includes comment spans
    #[allow(dead_code)]
    Comment {
        content: String,
    },
}

/// Collect special regions from a statement
fn collect_special_regions(stmt: &Statement, regions: &mut Vec<SpecialRegion>) {
    match stmt {
        Statement::VerbCall(vc) => {
            collect_from_verb_call(vc, regions);
        }
        Statement::Comment(_content) => {
            // Comments don't have spans in current AST, skip for now
            // Could add if needed
        }
    }
}

/// Collect special regions from a verb call
fn collect_from_verb_call(vc: &VerbCall, regions: &mut Vec<SpecialRegion>) {
    for arg in &vc.arguments {
        collect_from_node(&arg.value, &arg.key, regions);
    }

    // Also check for :as @binding in the verb call
    // Note: :as @binding is handled via SymbolRef nodes in the AST
    // The binding field here is just the name, span info would be needed
    // to locate it precisely in source
}

/// Collect special regions from an AST node
fn collect_from_node(node: &AstNode, arg_name: &str, regions: &mut Vec<SpecialRegion>) {
    match node {
        AstNode::SymbolRef { name, span } => {
            regions.push(SpecialRegion {
                start: span.start,
                end: span.end,
                kind: RegionKind::SymbolRef { name: name.clone() },
            });
        }
        AstNode::EntityRef {
            entity_type,
            value,
            resolved_key,
            span,
            ..
        } => {
            regions.push(SpecialRegion {
                start: span.start,
                end: span.end,
                kind: RegionKind::EntityRef {
                    entity_type: entity_type.clone(),
                    value: value.clone(),
                    resolved_key: resolved_key.clone(),
                    arg_name: arg_name.to_string(),
                },
            });
        }
        AstNode::List { items, .. } => {
            for item in items {
                collect_from_node(item, arg_name, regions);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, value) in entries {
                collect_from_node(value, arg_name, regions);
            }
        }
        AstNode::Nested(vc) => {
            collect_from_verb_call(vc, regions);
        }
        AstNode::Literal(_) => {
            // Literals don't need enrichment
        }
    }
}

/// Build binding summary for display
fn build_binding_summary(
    bindings: &HashMap<String, BindingInfo>,
    active_cbu: Option<&BindingInfo>,
) -> Vec<BindingSummary> {
    let mut summary: Vec<BindingSummary> = bindings
        .iter()
        .map(|(symbol, info)| {
            let is_primary = active_cbu.map(|cbu| cbu.id == info.id).unwrap_or(false);
            BindingSummary {
                symbol: symbol.clone(),
                display_name: info.display_name.clone(),
                entity_type: info.entity_type.clone(),
                entity_id: info.id.to_string(),
                is_primary,
            }
        })
        .collect();

    // Sort: primary first, then by symbol name
    summary.sort_by(|a, b| match (a.is_primary, b.is_primary) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.symbol.cmp(&b.symbol),
    });

    summary
}

/// Convert session context bindings to BindingInfo map
pub fn bindings_from_session_context(
    bindings: &HashMap<String, crate::api::session::BoundEntity>,
) -> HashMap<String, BindingInfo> {
    bindings
        .iter()
        .map(|(name, bound)| {
            (
                name.clone(),
                BindingInfo {
                    id: bound.id,
                    display_name: bound.display_name.clone(),
                    entity_type: bound.entity_type.clone(),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrich_simple_dsl() {
        let source = r#"(cbu.ensure :name "Test Fund" :jurisdiction "LU" :as @fund)"#;
        let bindings = HashMap::new();

        let result = enrich_dsl(source, &bindings, None);
        assert!(result.is_ok());

        let enriched = result.unwrap();
        assert_eq!(enriched.source, source);
        assert!(!enriched.segments.is_empty());
    }

    #[test]
    fn test_enrich_with_bindings() {
        let source = r#"(cbu.add-product :cbu-id @fund :product "Custody")"#;

        let mut bindings = HashMap::new();
        bindings.insert(
            "fund".to_string(),
            BindingInfo {
                id: Uuid::new_v4(),
                display_name: "Apex Capital".to_string(),
                entity_type: "cbu".to_string(),
            },
        );

        let result = enrich_dsl(source, &bindings, None);
        assert!(result.is_ok());

        let enriched = result.unwrap();

        // Should have a Binding segment for @fund
        let binding_seg = enriched
            .segments
            .iter()
            .find(|s| matches!(s, DslDisplaySegment::Binding { symbol, .. } if symbol == "fund"));
        assert!(binding_seg.is_some());

        // Should show display name
        if let Some(DslDisplaySegment::Binding { display_name, .. }) = binding_seg {
            assert_eq!(display_name.as_deref(), Some("Apex Capital"));
        }
    }

    #[test]
    fn test_binding_summary() {
        let mut bindings = HashMap::new();
        let cbu_id = Uuid::new_v4();
        bindings.insert(
            "fund".to_string(),
            BindingInfo {
                id: cbu_id,
                display_name: "Apex Capital".to_string(),
                entity_type: "cbu".to_string(),
            },
        );
        bindings.insert(
            "john".to_string(),
            BindingInfo {
                id: Uuid::new_v4(),
                display_name: "John Smith".to_string(),
                entity_type: "proper_person".to_string(),
            },
        );

        let active_cbu = BindingInfo {
            id: cbu_id,
            display_name: "Apex Capital".to_string(),
            entity_type: "cbu".to_string(),
        };

        let summary = build_binding_summary(&bindings, Some(&active_cbu));

        // Primary should be first
        assert_eq!(summary[0].symbol, "fund");
        assert!(summary[0].is_primary);
    }
}
