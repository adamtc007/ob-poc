//! `bpmn-test-harness` — Scenario runner and test helpers for the bpmn-runtime.
//!
//! Provides a builder-style API for authoring runtime integration tests without
//! touching any Postgres infrastructure. All state is held in
//! [`bpmn_runtime::InMemoryJourneyStore`].
//!
//! # Example
//!
//! ```rust,ignore
//! use bpmn_test_harness::{Scenario, compile_dsl};
//! use bpmn_runtime::InstanceStatus;
//!
//! #[tokio::test]
//! async fn linear_sequence_completes() {
//!     let result = Scenario::new(MY_DSL)
//!         .run_to_quiescence(serde_json::json!({}))
//!         .await;
//!     assert_eq!(result.status().await, InstanceStatus::Completed);
//! }
//! ```

// Bring dsl-resolution into scope so instantiate_pack can call validate_bpmn.
pub use dsl_resolution;

use bpmn_runtime::{
    InMemoryJourneyStore, InstanceId, InstanceStatus, JourneyStore, RuntimeEngine, ScriptedAdaptor,
    SwitchAdaptor, VerbRegistry,
};
pub use bpmn_runtime::{VerbEffect, VerbError, VerbHandler, VerbOutput};
use dsl_lowering::JourneySpec;
use std::{collections::HashMap, sync::Arc};

// ---------------------------------------------------------------------------
// compile_dsl helper
// ---------------------------------------------------------------------------

/// Parse + assemble + lower a DSL source string into a [`JourneySpec`].
///
/// Panics if the source has syntax or assembly errors (appropriate for tests).
pub fn compile_dsl(source: &str) -> JourneySpec {
    let (source_file, parse_diag) = dsl_parser::parse(source);
    let mut diag = dsl_diagnostics::DiagnosticBag::new();
    for d in &parse_diag.diagnostics {
        diag.push(d.clone());
    }
    let bag = dsl_ast::AtomBag::from_source_file(source_file, &mut diag);
    assert!(
        !diag.has_errors(),
        "DSL compile errors: {:?}",
        diag.errors().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
    let graph = dsl_bpmn_frontend::assemble(&bag, &mut diag);
    assert!(
        !diag.has_errors(),
        "Assembly errors: {:?}",
        diag.errors().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
    dsl_lowering::lower(&graph, "test-process")
}

// ---------------------------------------------------------------------------
// Scenario builder
// ---------------------------------------------------------------------------

/// Builder for a single runtime scenario.
///
/// Configure gateway replies and verb handlers before calling
/// [`Scenario::run_to_quiescence`].
pub struct Scenario {
    spec: Arc<JourneySpec>,
    gateway_replies: HashMap<String, Vec<String>>,
    verb_handlers: Vec<Box<dyn VerbHandler>>,
}

impl Scenario {
    /// Construct a scenario from a DSL source string.
    ///
    /// Panics on parse / assembly errors — that is intentional for tests.
    pub fn new(source: &str) -> Self {
        Self {
            spec: Arc::new(compile_dsl(source)),
            gateway_replies: HashMap::new(),
            verb_handlers: Vec::new(),
        }
    }

    /// Register a verb handler. When the runtime reaches a node with this
    /// verb_ref it calls the handler rather than parking.
    pub fn with_verb_handler(mut self, handler: Box<dyn VerbHandler>) -> Self {
        self.verb_handlers.push(handler);
        self
    }

    /// Programme a gateway reply: when the runtime reaches `gateway` it will
    /// activate `targets` without calling any external adaptor.
    pub fn with_gateway_reply(mut self, gateway: &str, targets: Vec<&str>) -> Self {
        self.gateway_replies.insert(
            gateway.to_string(),
            targets.into_iter().map(String::from).collect(),
        );
        self
    }

    /// Start the instance and run until quiescence.
    ///
    /// Returns a [`RunResult`] with query helpers for assertions.
    pub async fn run_to_quiescence(self, initial_data: serde_json::Value) -> RunResult {
        let store: Arc<InMemoryJourneyStore> = Arc::new(InMemoryJourneyStore::new());
        let mut adaptor = ScriptedAdaptor::new();
        for (gw, targets) in &self.gateway_replies {
            adaptor.set_reply(gw, targets.clone());
        }
        let adaptor: Arc<dyn SwitchAdaptor> = Arc::new(adaptor);
        let mut registry = VerbRegistry::new();
        for handler in self.verb_handlers {
            registry.register(handler);
        }
        let verb_registry = Arc::new(registry);

        let engine = RuntimeEngine::new(
            Arc::clone(&store) as Arc<dyn JourneyStore>,
            Arc::clone(&self.spec),
            verb_registry,
            adaptor,
        );

        let instance_id = engine
            .start_instance(initial_data)
            .await
            .expect("start_instance failed");

        RunResult {
            engine,
            store,
            instance_id,
        }
    }
}

// ---------------------------------------------------------------------------
// RunResult
// ---------------------------------------------------------------------------

/// The outcome of running a scenario to quiescence.
pub struct RunResult {
    pub engine: RuntimeEngine,
    pub store: Arc<InMemoryJourneyStore>,
    pub instance_id: InstanceId,
}

impl RunResult {
    /// Current instance status.
    pub async fn status(&self) -> InstanceStatus {
        self.engine
            .get_instance_status(self.instance_id)
            .await
            .expect("get_instance_status failed")
            .expect("instance not found")
    }

    /// All active tokens for this instance.
    pub async fn tokens(&self) -> Vec<bpmn_runtime::ActiveToken> {
        self.engine
            .get_tokens(self.instance_id)
            .await
            .expect("get_tokens failed")
    }

    /// Assert that there is exactly one token at `node_name` and return it.
    pub async fn assert_token_at(&self, node_name: &str) -> bpmn_runtime::ActiveToken {
        let tokens = self.tokens().await;
        tokens
            .into_iter()
            .find(|t| t.current_node == node_name)
            .unwrap_or_else(|| {
                // Get all token positions for a helpful panic message.
                let rt = tokio::runtime::Handle::current();
                let all = rt
                    .block_on(self.engine.get_tokens(self.instance_id))
                    .unwrap_or_default();
                panic!(
                    "expected a token at '{}', found tokens at: {:?}",
                    node_name,
                    all.iter().map(|t| &t.current_node).collect::<Vec<_>>()
                )
            })
    }

    /// Deliver an external verb-completion result and run to quiescence.
    pub async fn complete_task(
        self,
        node_name: &str,
        token_id: uuid::Uuid,
        output: serde_json::Value,
    ) -> Self {
        self.engine
            .complete_task(self.instance_id, node_name, token_id, output)
            .await
            .expect("complete_task failed");
        self
    }

    /// Read a value from the instance data store.
    pub async fn read_data(&self, key: &str) -> Option<serde_json::Value> {
        self.store
            .read_instance_data(self.instance_id, key)
            .await
            .expect("read_instance_data failed")
    }
}

// ---------------------------------------------------------------------------
// for-each expansion helper
// ---------------------------------------------------------------------------

/// Expand a `for-each` template body over a list of JSON objects.
///
/// For each element in `elements`, one copy of `template` is emitted with:
/// - Every `,var_name.field` replaced by the field value from the current
///   element.
///
/// The last element automatically gets `:default true` appended to each
/// flow atom in its copy (recognised by `(flow` prefix) when the copy does
/// not already contain `:default`.
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
        // Last element: append :default true to every flow line that does
        // not already declare :default.
        if is_last {
            copy = copy
                .lines()
                .map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with("(flow") && !trimmed.contains(":default") {
                        // Insert :default true before the closing ')'
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
// Sage stub: pack instantiation
// ---------------------------------------------------------------------------

/// Sage stub: given a pack name and parameter map, expand the pack template
/// into a DSL fragment.
///
/// This is **not** production Sage — it is a minimal template expander for
/// testing only.  Supports all 12 seed packs, including the 5 packs that use
/// `for-each` for variable arity.
///
/// The returned string is a complete, self-contained DSL fragment that can be
/// compiled and run with [`Scenario::new`].
pub fn instantiate_pack(
    pack_name: &str,
    params: &serde_json::Map<String, serde_json::Value>,
) -> String {
    match pack_name {
        "conjunctive-gate" => {
            let gate_name = params
                .get("gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("activation-gate");
            let enhanced_path = params
                .get("enhanced-path")
                .and_then(|v| v.as_str())
                .unwrap_or("enhanced-end");
            let standard_path = params
                .get("standard-path")
                .and_then(|v| v.as_str())
                .unwrap_or("standard-end");

            // Build a standalone, runnable DSL fragment:
            //   start → pre-activation-check → gate → enhanced-end | standard-end
            // The gate condition is a simplified string literal (the current parser
            // does not handle nested s-expressions with dot notation in flow conditions).
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

(provenance {gate_name}-prov
  :covers [{gate_name}]
  :source pack
  :source-id conjunctive-gate
  :version "1.0.0"
  :session "sess-test-001"
  :authored-at "2026-05-21T12:00:00Z"
  :confirmed-at "2026-05-21T12:00:28Z")
"#,
                gate_name = gate_name,
                enhanced_path = enhanced_path,
                standard_path = standard_path,
            )
        }
        "disjunctive-gate" => {
            let gate_name = params
                .get("gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("disjunctive-gate");
            let escalation_path = params
                .get("escalation-path")
                .and_then(|v| v.as_str())
                .unwrap_or("escalation-end");
            let standard_path = params
                .get("standard-path")
                .and_then(|v| v.as_str())
                .unwrap_or("standard-end");
            format!(
                r#"
(node dg-start :kind start-event)
(gateway {gate_name} :kind exclusive)
(node {escalation_path} :kind end-event)
(node {standard_path} :kind end-event)
(flow dg-start -> {gate_name})
(flow {gate_name} -> {escalation_path} :condition "any-condition-met")
(flow {gate_name} -> {standard_path} :default true)
(provenance dg-prov :covers [{gate_name}]
  :source pack :source-id disjunctive-gate :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                gate_name = gate_name,
                escalation_path = escalation_path,
                standard_path = standard_path
            )
        }

        "sanction-hit-escalation" => {
            let check_name = params
                .get("sanctions-check-name")
                .and_then(|v| v.as_str())
                .unwrap_or("sanctions-check");
            let gate_name = params
                .get("sanctions-gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("sanctions-gate");
            let escalation_path = params
                .get("escalation-path")
                .and_then(|v| v.as_str())
                .unwrap_or("escalation-end");
            let clear_path = params
                .get("clear-path")
                .and_then(|v| v.as_str())
                .unwrap_or("clear-end");
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
(provenance sh-prov :covers [{check_name} {gate_name}]
  :source pack :source-id sanction-hit-escalation :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                check_name = check_name,
                gate_name = gate_name,
                escalation_path = escalation_path,
                clear_path = clear_path
            )
        }

        "periodic-refresh-trigger" => {
            let gate_name = params
                .get("age-gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("age-gate");
            let refresh_path = params
                .get("refresh-path")
                .and_then(|v| v.as_str())
                .unwrap_or("refresh-end");
            let current_path = params
                .get("current-path")
                .and_then(|v| v.as_str())
                .unwrap_or("current-end");
            format!(
                r#"
(node prt-start :kind start-event)
(gateway {gate_name} :kind exclusive)
(node {refresh_path} :kind end-event)
(node {current_path} :kind end-event)
(flow prt-start -> {gate_name})
(flow {gate_name} -> {refresh_path} :condition "record-stale")
(flow {gate_name} -> {current_path} :default true)
(provenance prt-prov :covers [{gate_name}]
  :source pack :source-id periodic-refresh-trigger :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                gate_name = gate_name,
                refresh_path = refresh_path,
                current_path = current_path
            )
        }

        "manual-override-checkpoint" => {
            let auto_eval = params
                .get("auto-eval-name")
                .and_then(|v| v.as_str())
                .unwrap_or("auto-eval");
            let review_task = params
                .get("review-task-name")
                .and_then(|v| v.as_str())
                .unwrap_or("review-task");
            let gate_name = params
                .get("override-gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("override-gate");
            let confirmed_path = params
                .get("confirmed-path")
                .and_then(|v| v.as_str())
                .unwrap_or("confirmed-end");
            let override_path = params
                .get("override-path")
                .and_then(|v| v.as_str())
                .unwrap_or("override-end");
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
(provenance moc-prov :covers [{auto_eval} {review_task} {gate_name}]
  :source pack :source-id manual-override-checkpoint :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                auto_eval = auto_eval,
                review_task = review_task,
                gate_name = gate_name,
                confirmed_path = confirmed_path,
                override_path = override_path
            )
        }

        "parallel-evaluation-with-veto" => {
            let fork_name = params
                .get("fork-name")
                .and_then(|v| v.as_str())
                .unwrap_or("parallel-fork");
            let join_name = params
                .get("join-name")
                .and_then(|v| v.as_str())
                .unwrap_or("parallel-join");
            let gate_name = params
                .get("post-join-gate")
                .and_then(|v| v.as_str())
                .unwrap_or("veto-gate");
            let vetoed_path = params
                .get("vetoed-path")
                .and_then(|v| v.as_str())
                .unwrap_or("vetoed-end");
            let approved_path = params
                .get("approved-path")
                .and_then(|v| v.as_str())
                .unwrap_or("approved-end");

            let tasks_val = params.get("eval-tasks");
            if let Some(serde_json::Value::Array(tasks)) = tasks_val {
                // Variable-arity: for-each expansion over eval-tasks
                let fork_template =
                    format!("(flow {fork_name} -> ,task.name)\n", fork_name = fork_name);
                let join_template =
                    format!("(flow ,task.name -> {join_name})\n", join_name = join_name);
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
(provenance pev-prov :covers [{fork_name} {join_name} {gate_name}]
  :source pack :source-id parallel-evaluation-with-veto :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
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
                // Legacy fixed-arity: eval-task-1 / eval-task-2
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
(provenance pev-prov :covers [{fork_name} {join_name} {gate_name}]
  :source pack :source-id parallel-evaluation-with-veto :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    fork_name = fork_name,
                    join_name = join_name,
                    gate_name = gate_name,
                    vetoed_path = vetoed_path,
                    approved_path = approved_path
                )
            }
        }

        "threshold-band-routing" => {
            let gate_name = params
                .get("band-gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("band-gate");

            // Support both old fixed-arity API (path-low/mid/high) and new
            // variable-arity API (bands: [{upper, path}]).
            let bands_val = params.get("bands");
            if let Some(serde_json::Value::Array(bands)) = bands_val {
                // Variable-arity: use for-each expansion
                let band_template =
                    format!("(flow {gate_name} -> ,band.path)\n", gate_name = gate_name);
                let flows = expand_for_each(&band_template, "band", bands);
                // Collect unique path names for node declarations
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
(provenance tbr-prov :covers [{gate_name}]
  :source pack :source-id threshold-band-routing :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    gate_name = gate_name,
                    node_decls = node_decls,
                    flows = flows,
                )
            } else {
                // Legacy fixed-arity: path-low / path-mid / path-high
                let path_low = params
                    .get("path-low")
                    .and_then(|v| v.as_str())
                    .unwrap_or("band-low-end");
                let path_mid = params
                    .get("path-mid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("band-mid-end");
                let path_high = params
                    .get("path-high")
                    .and_then(|v| v.as_str())
                    .unwrap_or("band-high-end");
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
(provenance tbr-prov :covers [{gate_name}]
  :source pack :source-id threshold-band-routing :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    gate_name = gate_name,
                    path_low = path_low,
                    path_mid = path_mid,
                    path_high = path_high
                )
            }
        }

        "multi-jurisdiction-overlay" => {
            let gate_name = params
                .get("jur-gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("jur-gate");
            let default_path = params
                .get("default-path")
                .and_then(|v| v.as_str())
                .unwrap_or("jur-default");

            let jur_paths_val = params.get("jurisdiction-paths");
            if let Some(serde_json::Value::Array(jur_paths)) = jur_paths_val {
                // Variable-arity: for-each expansion over jurisdiction-paths
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
(provenance mjo-prov :covers [{gate_name}]
  :source pack :source-id multi-jurisdiction-overlay :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    gate_name = gate_name,
                    node_decls = node_decls,
                    default_path = default_path,
                    flows = flows,
                )
            } else {
                // Legacy fixed-arity: path-a / path-b / default-path
                let path_a = params
                    .get("path-a")
                    .and_then(|v| v.as_str())
                    .unwrap_or("jur-path-a");
                let path_b = params
                    .get("path-b")
                    .and_then(|v| v.as_str())
                    .unwrap_or("jur-path-b");
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
(provenance mjo-prov :covers [{gate_name}]
  :source pack :source-id multi-jurisdiction-overlay :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    gate_name = gate_name,
                    path_a = path_a,
                    path_b = path_b,
                    default_path = default_path
                )
            }
        }

        "linked-switch-chain" => {
            let gate1 = params
                .get("gate-1-name")
                .and_then(|v| v.as_str())
                .unwrap_or("lsc-gate-1");
            let gate2 = params
                .get("gate-2-name")
                .and_then(|v| v.as_str())
                .unwrap_or("lsc-gate-2");
            let exit1 = params
                .get("exit-path-1")
                .and_then(|v| v.as_str())
                .unwrap_or("lsc-exit-1");
            let exit2 = params
                .get("exit-path-2")
                .and_then(|v| v.as_str())
                .unwrap_or("lsc-exit-2");
            let final_path = params
                .get("final-path")
                .and_then(|v| v.as_str())
                .unwrap_or("lsc-final");
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
(provenance lsc-prov :covers [{gate1} {gate2}]
  :source pack :source-id linked-switch-chain :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                gate1 = gate1,
                gate2 = gate2,
                exit1 = exit1,
                exit2 = exit2,
                final_path = final_path
            )
        }

        "cascading-decision" => {
            let eval_name = params
                .get("primary-eval-name")
                .and_then(|v| v.as_str())
                .unwrap_or("cd-eval");
            let gate_name = params
                .get("primary-gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("cd-gate");

            let paths_val = params.get("paths");
            if let Some(serde_json::Value::Array(paths)) = paths_val {
                // Variable-arity: for-each expansion over paths
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
{flows}(provenance cd-prov :covers [{eval_name} {gate_name}]
  :source pack :source-id cascading-decision :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    eval_name = eval_name,
                    gate_name = gate_name,
                    node_decls = node_decls,
                    flows = flows,
                )
            } else {
                // Legacy fixed-arity: path-a / path-b
                let path_a = params
                    .get("path-a")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cd-path-a");
                let path_b = params
                    .get("path-b")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cd-path-b");
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
(provenance cd-prov :covers [{eval_name} {gate_name}]
  :source pack :source-id cascading-decision :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    eval_name = eval_name,
                    gate_name = gate_name,
                    path_a = path_a,
                    path_b = path_b
                )
            }
        }

        "decision-table-classification" => {
            let classify_name = params
                .get("classify-name")
                .and_then(|v| v.as_str())
                .unwrap_or("dtc-classify");
            let gate_name = params
                .get("route-gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("dtc-gate");

            let paths_val = params.get("paths");
            if let Some(serde_json::Value::Array(paths)) = paths_val {
                // Variable-arity: for-each expansion over paths
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
{flows}(provenance dtc-prov :covers [{classify_name} {gate_name}]
  :source pack :source-id decision-table-classification :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    classify_name = classify_name,
                    gate_name = gate_name,
                    node_decls = node_decls,
                    flows = flows,
                )
            } else {
                // Legacy fixed-arity: path-a / default-path
                let path_a = params
                    .get("path-a")
                    .and_then(|v| v.as_str())
                    .unwrap_or("dtc-path-a");
                let default_path = params
                    .get("default-path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("dtc-default");
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
(provenance dtc-prov :covers [{classify_name} {gate_name}]
  :source pack :source-id decision-table-classification :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                    classify_name = classify_name,
                    gate_name = gate_name,
                    path_a = path_a,
                    default_path = default_path
                )
            }
        }

        "required-evidence-checklist" => {
            let task1 = params
                .get("task-1")
                .and_then(|v| v.as_str())
                .unwrap_or("rec-task-1");
            let task2 = params
                .get("task-2")
                .and_then(|v| v.as_str())
                .unwrap_or("rec-task-2");
            let task3 = params
                .get("task-3")
                .and_then(|v| v.as_str())
                .unwrap_or("rec-task-3");
            let gate_name = params
                .get("checklist-gate-name")
                .and_then(|v| v.as_str())
                .unwrap_or("rec-gate");
            let approval_path = params
                .get("approval-path")
                .and_then(|v| v.as_str())
                .unwrap_or("rec-approved");
            let rejection_path = params
                .get("rejection-path")
                .and_then(|v| v.as_str())
                .unwrap_or("rec-rejected");
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
(provenance rec-prov :covers [{task1} {task2} {task3} {gate_name}]
  :source pack :source-id required-evidence-checklist :version "1.0.0"
  :session "sess-test" :authored-at "2026-05-21T12:00:00Z")
"#,
                task1 = task1,
                task2 = task2,
                task3 = task3,
                gate_name = gate_name,
                approval_path = approval_path,
                rejection_path = rejection_path
            )
        }

        _ => String::new(),
    }
}
