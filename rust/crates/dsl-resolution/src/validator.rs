//! REPL-facing validate function — full pipeline: parse → bag → resolve → assemble.

use dsl_ast::AtomBag;
use dsl_atoms::{AtomKindClass, DeclarativeKind};
use dsl_diagnostics::{Diagnostic, DiagnosticBag, DiagnosticSeverity};

use crate::pack_registry::PackRegistry;
use crate::resolve::{extract_string_list, get_slot_str, resolve};

// ---------------------------------------------------------------------------
// Public response types
// ---------------------------------------------------------------------------

/// A single diagnostic serialised for the REPL response.
#[derive(Debug, serde::Serialize)]
pub struct DiagnosticSummary {
    pub severity: String,
    pub code: Option<String>,
    pub message: String,
}

impl From<&Diagnostic> for DiagnosticSummary {
    fn from(d: &Diagnostic) -> Self {
        let severity = match d.severity {
            DiagnosticSeverity::Error => "Error",
            DiagnosticSeverity::Warning => "Warning",
            DiagnosticSeverity::Note => "Note",
        }
        .to_string();
        Self {
            severity,
            code: d.code.clone(),
            message: d.message.clone(),
        }
    }
}

/// A single pack instantiation recorded in a `(provenance ...)` atom.
#[derive(Debug, serde::Serialize)]
pub struct ProvenanceInstantiation {
    pub pack_id: String,
    pub version: String,
    pub covered_atoms: Vec<String>,
    pub session: Option<String>,
}

/// Aggregated provenance summary for the response.
#[derive(Debug, serde::Serialize)]
pub struct ProvenanceSummary {
    pub instantiations: Vec<ProvenanceInstantiation>,
    /// Structural atom names not covered by any provenance atom.
    pub uncovered_atoms: Vec<String>,
}

/// Full validation result returned to the REPL.
#[derive(Debug, serde::Serialize)]
pub struct ValidateResponse {
    pub diagnostics: Vec<DiagnosticSummary>,
    pub provenance_summary: ProvenanceSummary,
    pub has_errors: bool,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Full pipeline: parse → build atom bag → resolve → assemble (bpmn).
///
/// Returns a `ValidateResponse` suitable for returning to the REPL caller.
/// Assembly (dsl-bpmn-frontend) is skipped when there are parse or
/// resolution errors to avoid cascading noise.
pub fn validate_bpmn(
    source: &str,
    _process_name: &str,
    registry: &mut PackRegistry,
) -> ValidateResponse {
    let mut diag = DiagnosticBag::new();

    // Parse
    let (source_file, parse_diag) = dsl_parser::parse(source);
    for d in parse_diag.diagnostics {
        diag.push(d);
    }

    // Build atom bag
    let bag = AtomBag::from_source_file(source_file, &mut diag);

    // Resolve (validates packs, provenance, governance; indexes packs)
    resolve(&bag, registry, &mut diag);

    // Extract provenance summary
    let provenance_summary = extract_provenance_summary(&bag, registry);

    // Assemble BPMN graph only when there are no errors
    if !diag.has_errors() {
        let _graph = dsl_bpmn_frontend::assemble(&bag, &mut diag);
    }

    let has_errors = diag.has_errors();
    ValidateResponse {
        diagnostics: diag
            .diagnostics
            .iter()
            .map(DiagnosticSummary::from)
            .collect(),
        provenance_summary,
        has_errors,
    }
}

// ---------------------------------------------------------------------------
// Provenance summary extraction
// ---------------------------------------------------------------------------

fn extract_provenance_summary(bag: &AtomBag, _registry: &PackRegistry) -> ProvenanceSummary {
    let mut instantiations = Vec::new();
    let mut covered: std::collections::HashSet<String> = std::collections::HashSet::new();

    for atom in bag.declarative_atoms() {
        if atom.kind_class != AtomKindClass::Declarative(DeclarativeKind::Provenance) {
            continue;
        }
        let covers = extract_string_list(&atom.raw, "covers");
        let source_id = get_slot_str(&atom.raw, "source-id").unwrap_or_default();
        let version = get_slot_str(&atom.raw, "version").unwrap_or_default();
        let session = get_slot_str(&atom.raw, "session");
        for c in &covers {
            covered.insert(c.clone());
        }
        instantiations.push(ProvenanceInstantiation {
            pack_id: source_id,
            version,
            covered_atoms: covers,
            session,
        });
    }

    let all_structural: Vec<String> = bag
        .structural_atoms()
        .filter_map(|a| a.name.clone())
        .collect();
    let uncovered = all_structural
        .into_iter()
        .filter(|n| !covered.contains(n))
        .collect();

    ProvenanceSummary {
        instantiations,
        uncovered_atoms: uncovered,
    }
}
