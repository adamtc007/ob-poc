//! Main SVG render functions for bpmn-lite DSL sources.

use std::collections::HashMap;

use dsl_ast::AtomBag;
use dsl_atoms::{AtomKindClass, DeclarativeKind};
use dsl_bpmn_frontend::RailwayGraph;
use dsl_parser::RawValue;

use crate::layout::compute_layout;
use crate::shapes::{
    render_boundary_event, render_edge, render_gateway_shape, render_node_shape,
    render_parallel_join, ARROWHEAD_DEF,
};
use crate::style::EMBEDDED_CSS;

/// Options controlling SVG output.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Force a specific canvas width (auto-sized from layout when `None`).
    pub width: Option<f64>,
    /// Force a specific canvas height (auto-sized from layout when `None`).
    pub height: Option<f64>,
    /// Whether to render node and edge labels.
    pub include_labels: bool,
    /// Whether to render pack provenance badges.
    pub include_provenance_badges: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            include_labels: true,
            include_provenance_badges: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse `source` as bpmn-lite DSL, assemble the railway graph, and render to SVG.
pub fn render_dsl(source: &str, opts: &RenderOptions) -> anyhow::Result<String> {
    // Parse
    let (sf, _parse_diag) = dsl_parser::parse(source);
    let mut diag = dsl_diagnostics::DiagnosticBag::new();
    let bag = AtomBag::from_source_file(sf, &mut diag);

    // Extract provenance coverage for badge annotations
    let provenance_coverage = if opts.include_provenance_badges {
        extract_provenance_coverage(&bag)
    } else {
        HashMap::new()
    };

    // Assemble railway graph
    let graph = dsl_bpmn_frontend::assemble(&bag, &mut diag);

    render_graph(&graph, opts, &provenance_coverage)
}

/// Render a pre-assembled `RailwayGraph` to SVG.
pub fn render_graph(
    graph: &RailwayGraph,
    opts: &RenderOptions,
    provenance_coverage: &HashMap<String, String>,
) -> anyhow::Result<String> {
    let layout = compute_layout(graph);

    // Determine canvas size from layout bounds
    let max_x = layout
        .values()
        .map(|n| n.pos.x + n.width)
        .fold(0.0f64, f64::max);
    let max_y = layout
        .values()
        .map(|n| n.pos.y + n.height)
        .fold(0.0f64, f64::max);

    let width = opts.width.unwrap_or(max_x + 60.0).max(200.0);
    let height = opts.height.unwrap_or(max_y + 60.0).max(100.0);

    let mut svg = String::new();

    // SVG header
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{:.0}" height="{:.0}" viewBox="0 0 {:.0} {:.0}">"#,
        width, height, width, height
    ));
    svg.push('\n');
    svg.push_str("<defs>\n");
    svg.push_str(ARROWHEAD_DEF);
    svg.push('\n');
    svg.push_str("</defs>\n");
    svg.push_str(&format!("<style>{}</style>\n", EMBEDDED_CSS));

    // Draw edges first (so nodes render on top)
    for edge in &graph.edges {
        if let (Some(src), Some(tgt)) = (layout.get(&edge.source), layout.get(&edge.target)) {
            svg.push_str(&render_edge(src, tgt, edge, opts.include_labels));
        }
    }

    // Draw boundary event circles (attached to host nodes)
    for ba in &graph.boundary_attachments {
        if let Some(host_layout) = layout.get(&ba.host_node) {
            svg.push_str(&render_boundary_event(
                host_layout,
                &ba.event_name,
                &ba.event_kind,
                ba.interrupting,
            ));
        }
    }

    // Draw process nodes
    for (id, node) in &graph.nodes {
        if let Some(node_layout) = layout.get(id) {
            let badge = provenance_coverage.get(id);
            let svg_shape = render_node_shape(
                node_layout,
                &node.kind,
                &node.name,
                badge,
                opts.include_labels,
            );
            svg.push_str(&svg_shape);
        }
    }

    // Draw gateways
    for (id, gw) in &graph.gateways {
        if let Some(gw_layout) = layout.get(id) {
            let badge = provenance_coverage.get(id);
            let svg_shape =
                render_gateway_shape(gw_layout, &gw.kind, &gw.name, badge, opts.include_labels);
            svg.push_str(&svg_shape);
        }
    }

    // Draw parallel joins
    for (id, pj) in &graph.parallel_joins {
        if let Some(pj_layout) = layout.get(id) {
            svg.push_str(&render_parallel_join(pj_layout, &pj.name));
        }
    }

    svg.push_str("</svg>");
    Ok(svg)
}

// ---------------------------------------------------------------------------
// Provenance extraction
// ---------------------------------------------------------------------------

/// Build a map of `atom_name → pack_name` from `(provenance ...)` atoms.
pub fn extract_provenance_coverage(bag: &AtomBag) -> HashMap<String, String> {
    let mut coverage: HashMap<String, String> = HashMap::new();

    for atom in bag.declarative_atoms() {
        if !matches!(
            atom.kind_class,
            AtomKindClass::Declarative(DeclarativeKind::Provenance)
        ) {
            continue;
        }

        // Extract :source-id (pack name)
        let pack_name = get_slot_str(&atom.raw.slots, "source-id")
            .unwrap_or_default()
            .to_owned();

        // Extract :covers list of atom names
        let covers = extract_string_list(&atom.raw.slots, "covers");
        for atom_name in covers {
            coverage.insert(atom_name, pack_name.clone());
        }
    }

    coverage
}

// ---------------------------------------------------------------------------
// Slot extraction helpers
// ---------------------------------------------------------------------------

/// Extract a string value from a named slot in a slot list.
fn get_slot_str<'a>(slots: &'a [(String, RawValue)], key: &str) -> Option<&'a str> {
    for (k, v) in slots {
        if k == key {
            return match v {
                RawValue::StringLit(s) => Some(s.as_str()),
                RawValue::Symbol(s) => Some(s.as_str()),
                _ => None,
            };
        }
    }
    None
}

/// Extract a list of string values from a named list slot.
fn extract_string_list(slots: &[(String, RawValue)], key: &str) -> Vec<String> {
    for (k, v) in slots {
        if k == key {
            return match v {
                RawValue::List(items) => items
                    .iter()
                    .filter_map(|item| match item {
                        RawValue::Symbol(s) => Some(s.clone()),
                        RawValue::StringLit(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
                RawValue::Symbol(s) => vec![s.clone()],
                _ => vec![],
            };
        }
    }
    vec![]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source_returns_minimal_svg() {
        let svg = render_dsl("", &RenderOptions::default()).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn provenance_coverage_extracted() {
        let source = r#"
(gateway gw1 :kind exclusive)
(provenance p1 :covers [gw1] :source pack :source-id my-pack :version "1.0" :session "s1" :authored-at "2026-01-01T00:00:00Z")
"#;
        let (sf, _) = dsl_parser::parse(source);
        let mut diag = dsl_diagnostics::DiagnosticBag::new();
        let bag = AtomBag::from_source_file(sf, &mut diag);
        let cov = extract_provenance_coverage(&bag);
        assert_eq!(cov.get("gw1").map(|s| s.as_str()), Some("my-pack"));
    }
}
