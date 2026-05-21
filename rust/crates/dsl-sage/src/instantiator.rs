//! Tranche 3 — Sage instantiation pipeline.
//!
//! Given a confirmed pack name, version, and parameter map, this module:
//!
//! 1. Looks up the pack from the registry.
//! 2. Expands the pack template (parameter substitution + `for-each` unrolling).
//! 3. Emits a `(provenance ...)` atom recording authoring metadata.
//! 4. Optionally validates the structural DSL through the compiler.
//!
//! # Entry points
//!
//! - [`instantiate`] — main function; returns [`InstantiationResult`].
//! - [`validate_instantiation`] — run the structural DSL through the compile pipeline.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use dsl_resolution::{DecisionPack, PackRegistry};
use serde::{Deserialize, Serialize};

use crate::types::SageContext;

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// The outcome of a successful pack instantiation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstantiationResult {
    /// The complete DSL source: structural atoms followed by the provenance atom.
    pub dsl_source: String,
    /// Only the structural atoms (without the provenance atom).
    ///
    /// Use this for compilation / testing; the provenance atom is opaque to the
    /// BPMN assembler.
    pub structural_dsl: String,
    /// Names of the expanded structural atoms (nodes, gateways, parallel-joins).
    ///
    /// These are listed in the provenance atom's `:covers` slot.
    pub atom_names: Vec<String>,
    /// The pack that was instantiated.
    pub pack_name: String,
    /// The version of the pack that was instantiated.
    pub pack_version: String,
    /// The confirmed parameter values used during expansion.
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Summary returned by [`validate_instantiation`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationSummary {
    /// `true` when the compile pipeline produced at least one error diagnostic.
    pub has_errors: bool,
    /// Total number of nodes + gateways in the assembled graph.
    pub node_count: usize,
    /// Total number of directed edges in the assembled graph.
    pub edge_count: usize,
    /// Human-readable diagnostic messages (errors and warnings).
    pub diagnostics: Vec<String>,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Instantiate a decision pack.
///
/// # Arguments
///
/// - `pack_name` — canonical pack name (e.g., `"conjunctive-gate"`).
/// - `pack_version` — pack version string (e.g., `"1.0.0"`).
/// - `parameters` — confirmed parameter values keyed by parameter name.
/// - `pre_node` — optional `$pre-node` insertion point; defaults to
///   `"process-start"` when `None`.
/// - `context` — Sage context (session metadata used in the provenance atom).
/// - `registry` — the pack registry to look up the pack from.
///
/// # Errors
///
/// Returns `Err` when the pack is not found in the registry or when template
/// expansion fails.
pub fn instantiate(
    pack_name: &str,
    pack_version: &str,
    parameters: &HashMap<String, serde_json::Value>,
    pre_node: Option<&str>,
    _context: &SageContext,
    registry: &PackRegistry,
) -> Result<InstantiationResult> {
    let pack = registry
        .lookup(pack_name, pack_version)
        .ok_or_else(|| anyhow!("pack not found: {}@{}", pack_name, pack_version))?;

    // Step 1: expand the template into structural DSL + atom names
    let (structural_dsl, atom_names) = expand_template(pack, parameters, pre_node)?;

    // Step 2: emit the provenance atom
    let provenance_dsl = emit_provenance(pack_name, pack_version, parameters, &atom_names);

    // Step 3: combine into the full DSL source
    let dsl_source = format!("{}\n{}", structural_dsl.trim_end(), provenance_dsl);

    Ok(InstantiationResult {
        dsl_source,
        structural_dsl,
        atom_names,
        pack_name: pack_name.to_string(),
        pack_version: pack_version.to_string(),
        parameters: parameters.clone(),
    })
}

// ---------------------------------------------------------------------------
// Compile validation
// ---------------------------------------------------------------------------

/// Run the structural DSL through the v0.1 compile pipeline.
///
/// Parses, assembles, and assembles the graph via `dsl_bpmn_frontend::assemble`.
/// Returns a [`ValidationSummary`] rather than an `Err` so that partial results
/// (e.g. node counts) are accessible even when there are errors.
///
/// # Note on condition expressions
///
/// Flow condition strings such as `"all-conditions-met"` are opaque string
/// literals to the assembler and compile cleanly.  Complex s-expression
/// conditions that the parser does not yet support are stored as literal
/// strings in the pack templates, so they pass through safely.
pub fn validate_instantiation(structural_dsl: &str) -> Result<ValidationSummary> {
    let (source_file, parse_diag) = dsl_parser::parse(structural_dsl);

    let mut diag = dsl_diagnostics::DiagnosticBag::new();
    for d in parse_diag.diagnostics {
        diag.push(d);
    }

    let bag = dsl_ast::AtomBag::from_source_file(source_file, &mut diag);

    // Assemble into the railway graph (BPMN frontend)
    let graph = dsl_bpmn_frontend::assemble(&bag, &mut diag);

    let has_errors = diag.has_errors();
    let node_count = graph.nodes.len() + graph.gateways.len();
    let edge_count = graph.edges.len();
    let diagnostics = diag
        .diagnostics
        .iter()
        .map(|d| format!("{:?}: {}", d.severity, d.message))
        .collect();

    Ok(ValidationSummary {
        has_errors,
        node_count,
        edge_count,
        diagnostics,
    })
}

// ---------------------------------------------------------------------------
// Template expansion dispatcher
// ---------------------------------------------------------------------------

/// Expand the pack template into structural DSL source text and a list of
/// atom names.
///
/// The dispatch is pack-name-based.  Each private `expand_*` function mirrors
/// the expansion logic in `bpmn-test-harness/src/lib.rs` but returns only the
/// structural atoms (no provenance — that is emitted separately by
/// [`emit_provenance`]).
pub fn expand_template(
    pack: &DecisionPack,
    parameters: &HashMap<String, serde_json::Value>,
    pre_node: Option<&str>,
) -> Result<(String, Vec<String>)> {
    let _pre = pre_node.unwrap_or("process-start");

    let dsl = match pack.name.as_str() {
        "conjunctive-gate" => expand_conjunctive_gate(parameters),
        "disjunctive-gate" => expand_disjunctive_gate(parameters),
        "sanction-hit-escalation" => expand_sanction_hit_escalation(parameters),
        "periodic-refresh-trigger" => expand_periodic_refresh_trigger(parameters),
        "manual-override-checkpoint" => expand_manual_override_checkpoint(parameters),
        "threshold-band-routing" => expand_threshold_band_routing(parameters),
        "multi-jurisdiction-overlay" => expand_multi_jurisdiction_overlay(parameters),
        "linked-switch-chain" => expand_linked_switch_chain(parameters),
        "cascading-decision" => expand_cascading_decision(parameters),
        "decision-table-classification" => expand_decision_table_classification(parameters),
        "parallel-evaluation-with-veto" => expand_parallel_evaluation_with_veto(parameters),
        "required-evidence-checklist" => expand_required_evidence_checklist(parameters),
        other => return Err(anyhow!("unknown pack: {}", other)),
    };

    let atom_names = extract_atom_names(&dsl);
    Ok((dsl, atom_names))
}

// ---------------------------------------------------------------------------
// Provenance emission
// ---------------------------------------------------------------------------

fn emit_provenance(
    pack_name: &str,
    pack_version: &str,
    parameters: &HashMap<String, serde_json::Value>,
    atom_names: &[String],
) -> String {
    let session_id = {
        let id = uuid::Uuid::new_v4().to_string().replace('-', "");
        format!("sess-{}", &id[..12])
    };
    let now = chrono::Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let covers = atom_names.join(" ");
    let params_str = serde_json::to_string(parameters).unwrap_or_else(|_| "{}".to_string());

    format!(
        r#"(provenance {pack_name}-prov
  :covers [{covers}]
  :source pack
  :source-id {pack_name}
  :version "{pack_version}"
  :session "{session_id}"
  :authored-at "{now}"
  :confirmed-at "{now}"
  :params {params_str})"#,
        pack_name = pack_name,
        covers = covers,
        pack_version = pack_version,
        session_id = session_id,
        now = now,
        params_str = params_str,
    )
}

// ---------------------------------------------------------------------------
// Atom name extraction
// ---------------------------------------------------------------------------

/// Scan expanded DSL for top-level atom declarations and return their names.
///
/// Recognised prefixes: `(node`, `(gateway`, `(parallel-join`.
/// Flow atoms are not structural atom declarations; they are edges.
fn extract_atom_names(dsl: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in dsl.lines() {
        let trimmed = line.trim();
        for prefix in &["(node ", "(gateway ", "(parallel-join "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                if let Some(name) = rest.split_whitespace().next() {
                    let clean: String = name
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                        .collect();
                    if !clean.is_empty()
                        && !clean.starts_with('$')
                        && !clean.starts_with(',')
                    {
                        names.push(clean);
                    }
                }
            }
        }
    }
    names.dedup();
    names
}

// ---------------------------------------------------------------------------
// for-each expansion helper (ported from bpmn-test-harness)
// ---------------------------------------------------------------------------

/// Expand a `for-each` template body over a list of JSON objects.
///
/// For each element in `elements`, one copy of `template` is emitted with
/// every `,var_name.field` replaced by the field value from that element.
///
/// The last element automatically gets `:default true` appended to each
/// `(flow` line that does not already declare `:default`.
fn expand_for_each(template: &str, var_name: &str, elements: &[serde_json::Value]) -> String {
    let mut result = String::new();
    let len = elements.len();
    for (i, element) in elements.iter().enumerate() {
        let is_last = i == len - 1;
        let mut copy = template.to_string();
        if let Some(obj) = element.as_object() {
            for (field, value) in obj {
                let accessor = format!(",{}.{}", var_name, field);
                let replacement = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    v => v.to_string(),
                };
                copy = copy.replace(&accessor, &replacement);
            }
        }
        if is_last {
            copy = copy
                .lines()
                .map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with("(flow") && !trimmed.contains(":default") {
                        let close = line.rfind(')').unwrap_or(line.len());
                        format!("{} :default true)", &line[..close])
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
        result.push_str(&copy);
        result.push('\n');
    }
    result
}

// ---------------------------------------------------------------------------
// Per-pack expansion functions (structural atoms only — no provenance)
// ---------------------------------------------------------------------------

fn str_param<'a>(params: &'a HashMap<String, serde_json::Value>, key: &str, default: &'a str) -> &'a str {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
}

fn expand_conjunctive_gate(params: &HashMap<String, serde_json::Value>) -> String {
    let gate_name = str_param(params, "gate-name", "activation-gate");
    let enhanced_path = str_param(params, "enhanced-path", "enhanced-end");
    let standard_path = str_param(params, "standard-path", "standard-end");
    format!(
        r#"
(node process-start         :kind start-event)
(node pre-activation-check  :kind service-task)
(gateway {gate_name}        :kind exclusive)
(node {enhanced_path}       :kind end-event)
(node {standard_path}       :kind end-event)

(flow process-start          -> pre-activation-check)
(flow pre-activation-check   -> {gate_name})
(flow {gate_name}            -> {enhanced_path} :condition "all-conditions-met" :default false)
(flow {gate_name}            -> {standard_path} :default true)
"#,
        gate_name = gate_name,
        enhanced_path = enhanced_path,
        standard_path = standard_path,
    )
}

fn expand_disjunctive_gate(params: &HashMap<String, serde_json::Value>) -> String {
    let gate_name = str_param(params, "gate-name", "disjunctive-gate");
    let escalation_path = str_param(params, "escalation-path", "escalation-end");
    let standard_path = str_param(params, "standard-path", "standard-end");
    format!(
        r#"
(node dg-start :kind start-event)
(gateway {gate_name} :kind exclusive)
(node {escalation_path} :kind end-event)
(node {standard_path} :kind end-event)
(flow dg-start -> {gate_name})
(flow {gate_name} -> {escalation_path} :condition "any-condition-met")
(flow {gate_name} -> {standard_path} :default true)
"#,
        gate_name = gate_name,
        escalation_path = escalation_path,
        standard_path = standard_path,
    )
}

fn expand_sanction_hit_escalation(params: &HashMap<String, serde_json::Value>) -> String {
    let check_name = str_param(params, "sanctions-check-name", "sanctions-check");
    let gate_name = str_param(params, "sanctions-gate-name", "sanctions-gate");
    let escalation_path = str_param(params, "escalation-path", "escalation-end");
    let clear_path = str_param(params, "clear-path", "clear-end");
    format!(
        r#"
(node sh-start :kind start-event)
(node {check_name} :kind service-task)
(gateway {gate_name} :kind exclusive)
(node {escalation_path} :kind end-event)
(node {clear_path} :kind end-event)
(flow sh-start -> {check_name})
(flow {check_name} -> {gate_name})
(flow {gate_name} -> {escalation_path} :condition "hit")
(flow {gate_name} -> {clear_path} :default true)
"#,
        check_name = check_name,
        gate_name = gate_name,
        escalation_path = escalation_path,
        clear_path = clear_path,
    )
}

fn expand_periodic_refresh_trigger(params: &HashMap<String, serde_json::Value>) -> String {
    let gate_name = str_param(params, "age-gate-name", "age-gate");
    let refresh_path = str_param(params, "refresh-path", "refresh-end");
    let current_path = str_param(params, "current-path", "current-end");
    format!(
        r#"
(node prt-start :kind start-event)
(gateway {gate_name} :kind exclusive)
(node {refresh_path} :kind end-event)
(node {current_path} :kind end-event)
(flow prt-start -> {gate_name})
(flow {gate_name} -> {refresh_path} :condition "record-stale")
(flow {gate_name} -> {current_path} :default true)
"#,
        gate_name = gate_name,
        refresh_path = refresh_path,
        current_path = current_path,
    )
}

fn expand_manual_override_checkpoint(params: &HashMap<String, serde_json::Value>) -> String {
    let auto_eval = str_param(params, "auto-eval-name", "auto-eval");
    let review_task = str_param(params, "review-task-name", "review-task");
    let gate_name = str_param(params, "override-gate-name", "override-gate");
    let confirmed_path = str_param(params, "confirmed-path", "confirmed-end");
    let override_path = str_param(params, "override-path", "override-end");
    format!(
        r#"
(node moc-start :kind start-event)
(node {auto_eval} :kind service-task)
(node {review_task} :kind user-task)
(gateway {gate_name} :kind exclusive)
(node {confirmed_path} :kind end-event)
(node {override_path} :kind end-event)
(flow moc-start -> {auto_eval})
(flow {auto_eval} -> {review_task})
(flow {review_task} -> {gate_name})
(flow {gate_name} -> {override_path} :condition "human-override")
(flow {gate_name} -> {confirmed_path} :default true)
"#,
        auto_eval = auto_eval,
        review_task = review_task,
        gate_name = gate_name,
        confirmed_path = confirmed_path,
        override_path = override_path,
    )
}

fn expand_threshold_band_routing(params: &HashMap<String, serde_json::Value>) -> String {
    let gate_name = str_param(params, "band-gate-name", "band-gate");

    if let Some(serde_json::Value::Array(bands)) = params.get("bands") {
        let band_template = format!("(flow {gate_name} -> ,band.path)\n", gate_name = gate_name);
        let flows = expand_for_each(&band_template, "band", bands);
        let paths: Vec<String> = bands
            .iter()
            .filter_map(|b| b.get("path").and_then(|v| v.as_str()).map(String::from))
            .collect();
        let node_decls: String = paths
            .iter()
            .map(|p| format!("(node {} :kind end-event)\n", p))
            .collect();
        format!(
            r#"
(node tbr-start :kind start-event)
(gateway {gate_name} :kind exclusive)
{node_decls}
(flow tbr-start -> {gate_name})
{flows}
"#,
            gate_name = gate_name,
            node_decls = node_decls,
            flows = flows,
        )
    } else {
        let path_low = str_param(params, "path-low", "band-low-end");
        let path_mid = str_param(params, "path-mid", "band-mid-end");
        let path_high = str_param(params, "path-high", "band-high-end");
        format!(
            r#"
(node tbr-start :kind start-event)
(gateway {gate_name} :kind exclusive)
(node {path_low} :kind end-event)
(node {path_mid} :kind end-event)
(node {path_high} :kind end-event)
(flow tbr-start -> {gate_name})
(flow {gate_name} -> {path_low} :condition "band-low")
(flow {gate_name} -> {path_mid} :condition "band-mid")
(flow {gate_name} -> {path_high} :default true)
"#,
            gate_name = gate_name,
            path_low = path_low,
            path_mid = path_mid,
            path_high = path_high,
        )
    }
}

fn expand_multi_jurisdiction_overlay(params: &HashMap<String, serde_json::Value>) -> String {
    let gate_name = str_param(params, "jur-gate-name", "jur-gate");
    let default_path = str_param(params, "default-path", "jur-default");

    if let Some(serde_json::Value::Array(jur_paths)) = params.get("jurisdiction-paths") {
        let jp_template = format!(
            "(flow {gate_name} -> ,jp.path :condition \"juris-,jp.code\")\n",
            gate_name = gate_name
        );
        let flows = expand_for_each(&jp_template, "jp", jur_paths);
        let paths: Vec<String> = jur_paths
            .iter()
            .filter_map(|jp| jp.get("path").and_then(|v| v.as_str()).map(String::from))
            .collect();
        let node_decls: String = paths
            .iter()
            .map(|p| format!("(node {} :kind end-event)\n", p))
            .collect();
        format!(
            r#"
(node mjo-start :kind start-event)
(gateway {gate_name} :kind exclusive)
{node_decls}(node {default_path} :kind end-event)
(flow mjo-start -> {gate_name})
{flows}(flow {gate_name} -> {default_path} :default true)
"#,
            gate_name = gate_name,
            node_decls = node_decls,
            default_path = default_path,
            flows = flows,
        )
    } else {
        let path_a = str_param(params, "path-a", "jur-path-a");
        let path_b = str_param(params, "path-b", "jur-path-b");
        format!(
            r#"
(node mjo-start :kind start-event)
(gateway {gate_name} :kind exclusive)
(node {path_a} :kind end-event)
(node {path_b} :kind end-event)
(node {default_path} :kind end-event)
(flow mjo-start -> {gate_name})
(flow {gate_name} -> {path_a} :condition "jurisdiction-a")
(flow {gate_name} -> {path_b} :condition "jurisdiction-b")
(flow {gate_name} -> {default_path} :default true)
"#,
            gate_name = gate_name,
            path_a = path_a,
            path_b = path_b,
            default_path = default_path,
        )
    }
}

fn expand_linked_switch_chain(params: &HashMap<String, serde_json::Value>) -> String {
    // Variable-arity: gateways: [{name, exit-path}], final-path
    if let Some(serde_json::Value::Array(gateways)) = params.get("gateway-names") {
        let final_path = str_param(params, "final-path", "lsc-final");
        let mut node_decls = String::new();
        let mut flow_lines = String::new();

        let gw_names: Vec<(&str, &str)> = gateways
            .iter()
            .filter_map(|g| {
                let name = g.get("name").and_then(|v| v.as_str())?;
                let exit = g.get("exit-path").and_then(|v| v.as_str())?;
                Some((name, exit))
            })
            .collect();

        // Gateway + exit-node declarations
        for (name, exit) in &gw_names {
            node_decls.push_str(&format!("(gateway {} :kind exclusive)\n", name));
            node_decls.push_str(&format!("(node {} :kind end-event)\n", exit));
        }
        node_decls.push_str(&format!("(node {} :kind end-event)\n", final_path));

        // Flows: start → first gateway
        flow_lines.push_str("(flow lsc-start -> ");
        if let Some((first, _)) = gw_names.first() {
            flow_lines.push_str(first);
        }
        flow_lines.push_str(")\n");

        // Each gateway: early-exit flow + default flow to next (or final)
        for (i, (name, exit)) in gw_names.iter().enumerate() {
            flow_lines.push_str(&format!(
                "(flow {} -> {} :condition \"check-{}-failed\")\n",
                name,
                exit,
                i + 1
            ));
            let next = if i + 1 < gw_names.len() {
                gw_names[i + 1].0
            } else {
                final_path
            };
            flow_lines.push_str(&format!("(flow {} -> {} :default true)\n", name, next));
        }

        format!(
            r#"
(node lsc-start :kind start-event)
{node_decls}
{flow_lines}"#,
            node_decls = node_decls,
            flow_lines = flow_lines,
        )
    } else {
        // Legacy fixed-arity
        let gate1 = str_param(params, "gate-1-name", "lsc-gate-1");
        let gate2 = str_param(params, "gate-2-name", "lsc-gate-2");
        let exit1 = str_param(params, "exit-path-1", "lsc-exit-1");
        let exit2 = str_param(params, "exit-path-2", "lsc-exit-2");
        let final_path = str_param(params, "final-path", "lsc-final");
        format!(
            r#"
(node lsc-start :kind start-event)
(gateway {gate1} :kind exclusive)
(gateway {gate2} :kind exclusive)
(node {exit1} :kind end-event)
(node {exit2} :kind end-event)
(node {final_path} :kind end-event)
(flow lsc-start -> {gate1})
(flow {gate1} -> {exit1} :condition "check-1-failed")
(flow {gate1} -> {gate2} :default true)
(flow {gate2} -> {exit2} :condition "check-2-failed")
(flow {gate2} -> {final_path} :default true)
"#,
            gate1 = gate1,
            gate2 = gate2,
            exit1 = exit1,
            exit2 = exit2,
            final_path = final_path,
        )
    }
}

fn expand_cascading_decision(params: &HashMap<String, serde_json::Value>) -> String {
    let eval_name = str_param(params, "primary-eval-name", "cd-eval");
    let gate_name = str_param(params, "primary-gate-name", "cd-gate");

    if let Some(serde_json::Value::Array(paths)) = params.get("paths") {
        let p_template = format!("(flow {gate_name} -> ,p.path)\n", gate_name = gate_name);
        let flows = expand_for_each(&p_template, "p", paths);
        let path_nodes: Vec<String> = paths
            .iter()
            .filter_map(|p| p.get("path").and_then(|v| v.as_str()).map(String::from))
            .collect();
        let node_decls: String = path_nodes
            .iter()
            .map(|p| format!("(node {} :kind end-event)\n", p))
            .collect();
        format!(
            r#"
(node cd-start :kind start-event)
(node {eval_name} :kind service-task)
(gateway {gate_name} :kind exclusive)
{node_decls}(flow cd-start -> {eval_name})
(flow {eval_name} -> {gate_name})
{flows}"#,
            eval_name = eval_name,
            gate_name = gate_name,
            node_decls = node_decls,
            flows = flows,
        )
    } else {
        let path_a = str_param(params, "path-a", "cd-path-a");
        let path_b = str_param(params, "path-b", "cd-path-b");
        format!(
            r#"
(node cd-start :kind start-event)
(node {eval_name} :kind service-task)
(gateway {gate_name} :kind exclusive)
(node {path_a} :kind end-event)
(node {path_b} :kind end-event)
(flow cd-start -> {eval_name})
(flow {eval_name} -> {gate_name})
(flow {gate_name} -> {path_a} :condition "class-a")
(flow {gate_name} -> {path_b} :default true)
"#,
            eval_name = eval_name,
            gate_name = gate_name,
            path_a = path_a,
            path_b = path_b,
        )
    }
}

fn expand_decision_table_classification(params: &HashMap<String, serde_json::Value>) -> String {
    let classify_name = str_param(params, "classify-name", "dtc-classify");
    let gate_name = str_param(params, "route-gate-name", "dtc-gate");

    if let Some(serde_json::Value::Array(paths)) = params.get("paths") {
        let p_template = format!("(flow {gate_name} -> ,p.path)\n", gate_name = gate_name);
        let flows = expand_for_each(&p_template, "p", paths);
        let path_nodes: Vec<String> = paths
            .iter()
            .filter_map(|p| p.get("path").and_then(|v| v.as_str()).map(String::from))
            .collect();
        let node_decls: String = path_nodes
            .iter()
            .map(|p| format!("(node {} :kind end-event)\n", p))
            .collect();
        format!(
            r#"
(node dtc-start :kind start-event)
(node {classify_name} :kind service-task)
(gateway {gate_name} :kind exclusive)
{node_decls}(flow dtc-start -> {classify_name})
(flow {classify_name} -> {gate_name})
{flows}"#,
            classify_name = classify_name,
            gate_name = gate_name,
            node_decls = node_decls,
            flows = flows,
        )
    } else {
        let path_a = str_param(params, "path-a", "dtc-path-a");
        let default_path = str_param(params, "default-path", "dtc-default");
        format!(
            r#"
(node dtc-start :kind start-event)
(node {classify_name} :kind service-task)
(gateway {gate_name} :kind exclusive)
(node {path_a} :kind end-event)
(node {default_path} :kind end-event)
(flow dtc-start -> {classify_name})
(flow {classify_name} -> {gate_name})
(flow {gate_name} -> {path_a} :condition "class-a-match")
(flow {gate_name} -> {default_path} :default true)
"#,
            classify_name = classify_name,
            gate_name = gate_name,
            path_a = path_a,
            default_path = default_path,
        )
    }
}

fn expand_parallel_evaluation_with_veto(params: &HashMap<String, serde_json::Value>) -> String {
    let fork_name = str_param(params, "fork-name", "parallel-fork");
    let join_name = str_param(params, "join-name", "parallel-join");
    let gate_name = str_param(params, "post-join-gate", "veto-gate");
    let vetoed_path = str_param(params, "vetoed-path", "vetoed-end");
    let approved_path = str_param(params, "approved-path", "approved-end");

    if let Some(serde_json::Value::Array(tasks)) = params.get("eval-tasks") {
        let fork_template = format!("(flow {fork_name} -> ,task.name)\n", fork_name = fork_name);
        let join_template = format!("(flow ,task.name -> {join_name})\n", join_name = join_name);
        let fork_flows = expand_for_each(&fork_template, "task", tasks);
        let join_flows = expand_for_each(&join_template, "task", tasks);
        let task_nodes: Vec<String> = tasks
            .iter()
            .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
            .collect();
        let node_decls: String = task_nodes
            .iter()
            .map(|n| format!("(node {} :kind service-task)\n", n))
            .collect();
        format!(
            r#"
(node veto-start :kind start-event)
(gateway {fork_name} :kind parallel)
{node_decls}(parallel-join {join_name} :expects [{fork_name}] :merge [])
(gateway {gate_name} :kind exclusive)
(node {vetoed_path} :kind end-event)
(node {approved_path} :kind end-event)
(flow veto-start -> {fork_name})
{fork_flows}{join_flows}(flow {join_name} -> {gate_name})
(flow {gate_name} -> {vetoed_path} :condition "vetoed")
(flow {gate_name} -> {approved_path} :default true)
"#,
            fork_name = fork_name,
            join_name = join_name,
            gate_name = gate_name,
            node_decls = node_decls,
            fork_flows = fork_flows,
            join_flows = join_flows,
            vetoed_path = vetoed_path,
            approved_path = approved_path,
        )
    } else {
        // Legacy fixed-arity
        format!(
            r#"
(node veto-start :kind start-event)
(gateway {fork_name} :kind parallel)
(node pev-eval-task-1 :kind service-task)
(node pev-eval-task-2 :kind service-task)
(parallel-join {join_name} :expects [{fork_name}] :merge [])
(gateway {gate_name} :kind exclusive)
(node {vetoed_path} :kind end-event)
(node {approved_path} :kind end-event)
(flow veto-start -> {fork_name})
(flow {fork_name} -> pev-eval-task-1)
(flow {fork_name} -> pev-eval-task-2)
(flow pev-eval-task-1 -> {join_name})
(flow pev-eval-task-2 -> {join_name})
(flow {join_name} -> {gate_name})
(flow {gate_name} -> {vetoed_path} :condition "vetoed")
(flow {gate_name} -> {approved_path} :default true)
"#,
            fork_name = fork_name,
            join_name = join_name,
            gate_name = gate_name,
            vetoed_path = vetoed_path,
            approved_path = approved_path,
        )
    }
}

fn expand_required_evidence_checklist(params: &HashMap<String, serde_json::Value>) -> String {
    let gate_name = str_param(params, "checklist-gate-name", "rec-gate");
    let approval_path = str_param(params, "approval-path", "rec-approved");
    let rejection_path = str_param(params, "rejection-path", "rec-rejected");

    // Variable-arity: tasks: [{name}]
    if let Some(serde_json::Value::Array(tasks)) = params.get("tasks") {
        let task_names: Vec<String> = tasks
            .iter()
            .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
            .collect();

        if task_names.is_empty() {
            // Degenerate case: no tasks, go straight to gate
            return format!(
                r#"
(node rec-start :kind start-event)
(gateway {gate_name} :kind exclusive)
(node {approval_path} :kind end-event)
(node {rejection_path} :kind end-event)
(flow rec-start -> {gate_name})
(flow {gate_name} -> {approval_path} :condition "all-evidence-verified")
(flow {gate_name} -> {rejection_path} :default true)
"#,
                gate_name = gate_name,
                approval_path = approval_path,
                rejection_path = rejection_path,
            );
        }

        let node_decls: String = task_names
            .iter()
            .map(|n| format!("(node {} :kind user-task)\n", n))
            .collect();

        let mut flow_lines = String::new();
        flow_lines.push_str(&format!("(flow rec-start -> {})\n", task_names[0]));
        for i in 1..task_names.len() {
            flow_lines.push_str(&format!(
                "(flow {} -> {})\n",
                task_names[i - 1],
                task_names[i]
            ));
        }
        flow_lines.push_str(&format!(
            "(flow {} -> {})\n",
            task_names.last().unwrap(),
            gate_name
        ));

        format!(
            r#"
(node rec-start :kind start-event)
{node_decls}(gateway {gate_name} :kind exclusive)
(node {approval_path} :kind end-event)
(node {rejection_path} :kind end-event)
{flow_lines}(flow {gate_name} -> {approval_path} :condition "all-evidence-verified")
(flow {gate_name} -> {rejection_path} :default true)
"#,
            node_decls = node_decls,
            gate_name = gate_name,
            approval_path = approval_path,
            rejection_path = rejection_path,
            flow_lines = flow_lines,
        )
    } else {
        // Legacy fixed-arity: task-1 / task-2 / task-3
        let task1 = str_param(params, "task-1", "rec-task-1");
        let task2 = str_param(params, "task-2", "rec-task-2");
        let task3 = str_param(params, "task-3", "rec-task-3");
        format!(
            r#"
(node rec-start :kind start-event)
(node {task1} :kind user-task)
(node {task2} :kind user-task)
(node {task3} :kind user-task)
(gateway {gate_name} :kind exclusive)
(node {approval_path} :kind end-event)
(node {rejection_path} :kind end-event)
(flow rec-start -> {task1})
(flow {task1} -> {task2})
(flow {task2} -> {task3})
(flow {task3} -> {gate_name})
(flow {gate_name} -> {approval_path} :condition "all-evidence-verified")
(flow {gate_name} -> {rejection_path} :default true)
"#,
            task1 = task1,
            task2 = task2,
            task3 = task3,
            gate_name = gate_name,
            approval_path = approval_path,
            rejection_path = rejection_path,
        )
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_atom_names_recognises_node_and_gateway() {
        let dsl = r#"
(node process-start :kind start-event)
(gateway kyc-gate :kind exclusive)
(parallel-join pj-merge :expects [fork] :merge [])
(flow kyc-gate -> approved-end :default true)
"#;
        let names = extract_atom_names(dsl);
        assert!(names.contains(&"process-start".to_string()));
        assert!(names.contains(&"kyc-gate".to_string()));
        assert!(names.contains(&"pj-merge".to_string()));
        // flow is not a structural atom declaration
        assert!(!names.contains(&"approved-end".to_string()));
    }

    #[test]
    fn conjunctive_gate_expansion_contains_gate_name() {
        let params: HashMap<String, serde_json::Value> = [
            ("gate-name".to_string(), serde_json::json!("my-gate")),
            ("enhanced-path".to_string(), serde_json::json!("enhanced")),
            ("standard-path".to_string(), serde_json::json!("standard")),
        ]
        .into_iter()
        .collect();
        let dsl = expand_conjunctive_gate(&params);
        assert!(dsl.contains("my-gate"), "gate name should appear in DSL");
        assert!(dsl.contains("enhanced"));
        assert!(dsl.contains("standard"));
    }

    #[test]
    fn threshold_band_routing_variable_arity() {
        let params: HashMap<String, serde_json::Value> = [
            ("band-gate-name".to_string(), serde_json::json!("risk-gate")),
            (
                "bands".to_string(),
                serde_json::json!([
                    {"upper": 10, "path": "low-end"},
                    {"upper": 25, "path": "mid-end"},
                    {"path": "high-end"}
                ]),
            ),
        ]
        .into_iter()
        .collect();
        let dsl = expand_threshold_band_routing(&params);
        assert!(dsl.contains("risk-gate"));
        assert!(dsl.contains("low-end"));
        assert!(dsl.contains("mid-end"));
        assert!(dsl.contains("high-end"));
        // Last band should have :default true
        assert!(dsl.contains(":default true"));
    }

    #[test]
    fn required_evidence_checklist_variable_arity() {
        let params: HashMap<String, serde_json::Value> = [
            (
                "tasks".to_string(),
                serde_json::json!([{"name": "id-check"}, {"name": "address-check"}]),
            ),
            ("checklist-gate-name".to_string(), serde_json::json!("ev-gate")),
            ("approval-path".to_string(), serde_json::json!("approved")),
            ("rejection-path".to_string(), serde_json::json!("rejected")),
        ]
        .into_iter()
        .collect();
        let dsl = expand_required_evidence_checklist(&params);
        assert!(dsl.contains("id-check"));
        assert!(dsl.contains("address-check"));
        assert!(dsl.contains("ev-gate"));
    }
}
