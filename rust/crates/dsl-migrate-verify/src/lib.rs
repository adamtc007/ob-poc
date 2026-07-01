//! Round-trip verifier for `dsl-migrate` output.
//!
//! Takes the DSL string produced by `dsl_migrate::emit()` and confirms it
//! passes the full compiler chain: parse → validate → lower → start.
//!
//! Validates **structure and reachability only**. The `:condition` strings on
//! flow atoms are opaque to the local runtime (ScriptedAdaptor ignores them);
//! condition executability is the external dmn-lite peer's responsibility.
#![deny(unreachable_pub)]

use bpmn_runtime::{
    InMemoryJourneyStore, JourneyStore, RuntimeEngine, ScriptedAdaptor, VerbRegistry,
};
use dsl_diagnostics::DiagnosticBag;
use dsl_lowering::JourneySpec;
use std::sync::Arc;

/// Verification result for one DSL source string.
#[derive(Debug)]
pub struct VerifyResult {
    /// DSL source string parsed without errors.
    pub parsed: bool,
    /// Passed `dsl-resolution::validate_bpmn` structural validation.
    pub validated: bool,
    /// Successfully lowered to a `JourneySpec`.
    pub lowered: bool,
    /// `RuntimeEngine::start_instance` succeeded (token reaches first node).
    pub started: bool,
    /// Diagnostic messages collected at each stage.
    pub diagnostics: Vec<String>,
}

impl VerifyResult {
    /// Returns true only when all four stages passed.
    pub fn is_ok(&self) -> bool {
        self.parsed && self.validated && self.lowered && self.started
    }
}

/// Verify a DSL source string produced by `dsl_migrate::emit()`.
///
/// `process_name` is used as the JourneySpec identifier.
pub async fn verify_dsl_source(source: &str, process_name: &str) -> VerifyResult {
    let mut diagnostics = Vec::new();

    // ── Stage 1: Parse ──────────────────────────────────────────────────────
    let (source_file, parse_diag) = dsl_parser::parse(source);
    let mut diag = DiagnosticBag::new();
    for d in &parse_diag.diagnostics {
        diag.push(d.clone());
    }
    if diag.has_errors() {
        for e in diag.errors() {
            diagnostics.push(format!("parse: {}", e.message));
        }
        return VerifyResult {
            parsed: false,
            validated: false,
            lowered: false,
            started: false,
            diagnostics,
        };
    }

    // ── Stage 2: Assemble + validate ────────────────────────────────────────
    let bag = dsl_ast::AtomBag::from_source_file(source_file, &mut diag);
    let graph = dsl_bpmn_frontend::assemble(&bag, &mut diag);
    if diag.has_errors() {
        for e in diag.errors() {
            diagnostics.push(format!("validate: {}", e.message));
        }
        return VerifyResult {
            parsed: true,
            validated: false,
            lowered: false,
            started: false,
            diagnostics,
        };
    }

    // ── Stage 3: Lower ──────────────────────────────────────────────────────
    let spec: JourneySpec = dsl_lowering::lower(&graph, process_name);

    // ── Stage 4: Start in runtime (structure + reachability only) ───────────
    let store: Arc<InMemoryJourneyStore> = Arc::new(InMemoryJourneyStore::new());
    let adaptor = Arc::new(ScriptedAdaptor::default());
    let verb_registry = Arc::new(VerbRegistry::new());

    let engine = RuntimeEngine::new(
        Arc::clone(&store) as Arc<dyn JourneyStore>,
        Arc::new(spec),
        verb_registry,
        adaptor,
    );

    match engine.start_instance(serde_json::json!({})).await {
        Ok(_) => VerifyResult {
            parsed: true,
            validated: true,
            lowered: true,
            started: true,
            diagnostics,
        },
        Err(e) => {
            diagnostics.push(format!("runtime: {}", e));
            VerifyResult {
                parsed: true,
                validated: true,
                lowered: true,
                started: false,
                diagnostics,
            }
        }
    }
}

/// Compile DSL source to a [`JourneySpec`] without starting an engine instance.
///
/// Returns `Err` if parsing, assembly, or lowering fails. Useful for
/// `ProcessRegistry::load_all` which compiles definitions at startup.
pub fn compile_to_spec(source: &str, process_name: &str) -> anyhow::Result<JourneySpec> {
    let (source_file, parse_diag) = dsl_parser::parse(source);
    let mut diag = DiagnosticBag::new();
    for d in &parse_diag.diagnostics {
        diag.push(d.clone());
    }
    if diag.has_errors() {
        let msgs: Vec<_> = diag.errors().map(|e| e.message.as_str()).collect();
        anyhow::bail!("parse errors: {}", msgs.join("; "));
    }

    let bag = dsl_ast::AtomBag::from_source_file(source_file, &mut diag);
    let graph = dsl_bpmn_frontend::assemble(&bag, &mut diag);
    if diag.has_errors() {
        let msgs: Vec<_> = diag.errors().map(|e| e.message.as_str()).collect();
        anyhow::bail!("assembly errors: {}", msgs.join("; "));
    }

    Ok(dsl_lowering::lower(&graph, process_name))
}

/// Convenience wrapper: run the verifier synchronously in a new Tokio runtime.
///
/// Used by the CLI `--verify` flag which already owns a main thread.
pub fn verify_dsl_source_sync(source: &str, process_name: &str) -> VerifyResult {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(verify_dsl_source(source, process_name))
}
