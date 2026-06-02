//! `cargo x reconcile` — minimal client for catalogue validation +
//! declaration status (Pilot P.7 — 2026-04-23).
//!
//! Three subcommands per pilot plan §P.7:
//!   --validate : run the P.1.c catalogue validator over the full
//!                config/verbs/*.yaml tree; exit non-zero on errors.
//!   --batch    : scaffold for bulk per-verb operations at estate scale
//!                (pilot validates the shape; full batch ops are
//!                Tranche-2 work).
//!   --status   : print declaration coverage — X of N verbs declared,
//!                per-workspace breakdown, escalation-rule count.
//!
//! Shares the same validator code path as the ob-poc-web startup gate
//! (P.1.g) and the `cargo x verbs lint` command (P.1.h). This xtask
//! subcommand is the operator-facing CLI wrapper.

use anyhow::{Context, Result};
use clap::Subcommand;
use dsl_core::{
    collect_declared_fqns, entity_kinds_from_taxonomy_yaml, flatten_pack_entries,
    load_dags_from_dir, load_packs_from_dir, validate_dags_with_context, validate_pack_fqns,
    validate_verbs_config, ConfigLoader, DagValidationContext, LoadedPack, ValidationContext,
    VerbsConfig,
};
use dsl_core::{EntryVia, SlotStateMachine};
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub(crate) enum ReconcileAction {
    /// Run catalogue validator (structural + well-formedness + warnings)
    Validate {
        /// Fail on policy-sanity warnings too (default: warnings are
        /// informational only)
        #[arg(long)]
        strict_warnings: bool,
    },
    /// Print declaration coverage + tier distribution + escalation
    /// inventory
    Status,
    /// Scaffold for bulk per-verb operations (Tranche-2 scope).
    /// Currently enumerates scope but performs no mutations.
    Batch {
        /// Operation name — scaffolded but not executed in pilot
        op: String,
    },
    /// Report DAG/SemOS hygiene drift without failing the validation gate.
    HygieneReport,
}

pub(crate) async fn run(action: ReconcileAction) -> Result<()> {
    match action {
        ReconcileAction::Validate { strict_warnings } => validate(strict_warnings).await,
        ReconcileAction::Status => status().await,
        ReconcileAction::Batch { op } => batch(op).await,
        ReconcileAction::HygieneReport => hygiene_report().await,
    }
}

fn load_catalogue() -> Result<VerbsConfig> {
    let loader = ConfigLoader::from_env();
    loader
        .load_verbs()
        .context("catalogue load failed (pre-DB; pure YAML)")
}

fn collect_macro_fqns() -> HashSet<String> {
    // Macros live at config/verb_schemas/macros/*.yaml with their FQN as
    // the top-level key (kind: macro).
    let mut out = HashSet::new();
    let macros_dir = PathBuf::from("config/verb_schemas/macros");
    if !macros_dir.exists() {
        return out;
    }
    let Ok(entries) = std::fs::read_dir(&macros_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&raw) else {
            continue;
        };
        if let Some(mapping) = value.as_mapping() {
            for (key, body) in mapping {
                if let (Some(fqn), Some(body_map)) = (key.as_str(), body.as_mapping()) {
                    // Only include entries explicitly marked `kind: macro`.
                    let is_macro = body_map
                        .get(serde_yaml::Value::String("kind".to_string()))
                        .and_then(|v| v.as_str())
                        == Some("macro");
                    if is_macro {
                        out.insert(fqn.to_string());
                    }
                }
            }
        }
    }
    out
}

async fn validate(strict_warnings: bool) -> Result<()> {
    println!("===========================================");
    println!("  cargo x reconcile --validate");
    println!("===========================================\n");

    let cfg = load_catalogue()?;
    let total: usize = cfg.domains.values().map(|d| d.verbs.len()).sum();
    println!(
        "Loaded {} domains, {} verbs from config/verbs/",
        cfg.domains.len(),
        total
    );

    // V1.3 — load DAGs FIRST so we can populate known_slots for the
    // verb validator's transition_args slot-resolution check (v1.3
    // amendment, 2026-04-26).
    let dags_dir = PathBuf::from("config/sem_os_seeds/dag_taxonomies");
    let dags_loaded = if dags_dir.exists() {
        let loaded = load_dags_from_dir(&dags_dir)
            .context("loading config/sem_os_seeds/dag_taxonomies/ for v1.3 checks")?;
        println!(
            "Loaded {} DAG taxonomies from config/sem_os_seeds/dag_taxonomies/",
            loaded.len()
        );
        Some(loaded)
    } else {
        None
    };
    let dag_report = if let Some(loaded) = &dags_loaded {
        validate_dags_with_context(loaded, &load_dag_validation_context()?)
    } else {
        dsl_core::DagValidationReport::default()
    };

    // Populate known_slots from loaded DAGs for v1.3 transition_args
    // slot-resolution check.
    let mut known_slots: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    if let Some(loaded) = &dags_loaded {
        for (_path, loaded_dag) in loaded.iter() {
            let dag = &loaded_dag.dag;
            for slot in &dag.slots {
                known_slots.insert((dag.workspace.clone(), slot.id.clone()));
            }
        }
    }

    let ctx = ValidationContext {
        require_declaration: false,
        require_effect_class: true, // T08: all verbs now have effect_class; fail closed
        known_slots: known_slots.clone(),
        ..ValidationContext::default()
    };
    let report = validate_verbs_config(&cfg, &ctx);

    // V1.2-5 — pack-hygiene cross-check.
    let packs_dir = PathBuf::from("config/packs");
    let (pack_errors, pack_workspace_errors) = if packs_dir.exists() {
        let declared = collect_declared_fqns(&cfg);
        let macros = collect_macro_fqns();
        let packs =
            load_packs_from_dir(&packs_dir).context("loading config/packs/ for V1.2-5 check")?;
        println!(
            "Loaded {} packs from config/packs/ ({} declared verbs + {} macros for cross-check)",
            packs.len(),
            declared.len(),
            macros.len()
        );
        let fqn_errors = validate_pack_fqns(&declared, &macros, flatten_pack_entries(&packs));
        let workspace_errors = validate_pack_workspaces_against_dags(&packs, &known_slots);
        (fqn_errors, workspace_errors)
    } else {
        (Vec::new(), Vec::new())
    };

    let struct_n = report.structural.len();
    let wf_n = report.well_formedness.len()
        + pack_errors.len()
        + pack_workspace_errors.len()
        + dag_report.errors.len();
    let warn_n = report.warnings.len() + dag_report.warnings.len();
    let hygiene_failures = hygiene_failure_summary()?;

    println!();
    println!("Structural errors:           {struct_n}");
    println!(
        "Well-formedness errors:      {wf_n}  ({} three-axis + {} pack-hygiene + {} pack-workspace + {} cross-DAG)",
        report.well_formedness.len(),
        pack_errors.len(),
        pack_workspace_errors.len(),
        dag_report.errors.len()
    );
    println!(
        "Policy-sanity warnings:      {warn_n}  ({} three-axis + {} DAG lint)",
        report.warnings.len(),
        dag_report.warnings.len()
    );
    println!("Hygiene drift errors:        {}", hygiene_failures.len());

    if struct_n > 0 {
        println!("\nStructural errors:");
        for e in &report.structural {
            println!("  ✗ {e}");
        }
    }
    if !report.well_formedness.is_empty() {
        println!("\nWell-formedness errors (three-axis):");
        for e in &report.well_formedness {
            println!("  ✗ {e}");
        }
    }
    if !pack_errors.is_empty() {
        println!("\nWell-formedness errors (pack hygiene — V1.2-5):");
        for e in &pack_errors {
            println!("  ✗ {e}");
        }
    }
    if !pack_workspace_errors.is_empty() {
        println!("\nWell-formedness errors (pack workspace alignment):");
        for e in &pack_workspace_errors {
            println!("  ✗ {e}");
        }
    }
    if !dag_report.errors.is_empty() {
        println!("\nWell-formedness errors (cross-DAG — v1.3):");
        for e in &dag_report.errors {
            println!("  ✗ {e}");
        }
    }
    if warn_n > 0 {
        println!("\nPolicy-sanity warnings:");
        for w in &report.warnings {
            println!("  ~ {w}");
        }
        for w in &dag_report.warnings {
            println!("  ~ {w}");
        }
    }
    if !hygiene_failures.is_empty() {
        println!("\nHygiene drift errors:");
        for failure in &hygiene_failures {
            println!("  ✗ {failure}");
        }
    }

    let failed =
        struct_n > 0 || wf_n > 0 || !hygiene_failures.is_empty() || (strict_warnings && warn_n > 0);
    if failed {
        println!("\n===========================================");
        println!("  FAILED");
        println!("===========================================");
        std::process::exit(1);
    }
    println!("\n===========================================");
    println!("  OK");
    println!("===========================================");
    Ok(())
}

fn load_dag_validation_context() -> Result<DagValidationContext> {
    let taxonomy_path = PathBuf::from("config/ontology/entity_taxonomy.yaml");
    let taxonomy = std::fs::read_to_string(&taxonomy_path)
        .with_context(|| format!("reading entity taxonomy from {taxonomy_path:?}"))?;
    let known_entity_kinds = entity_kinds_from_taxonomy_yaml(&taxonomy)
        .with_context(|| format!("parsing entity taxonomy from {taxonomy_path:?}"))?;
    Ok(DagValidationContext { known_entity_kinds })
}

#[derive(Debug, Clone)]
struct SimpleStatusEntry {
    line: usize,
    fqn: String,
    table: String,
    pk_col: String,
    state_col: String,
    target_state: String,
}

#[derive(Debug, Default)]
struct TableSchema {
    columns: HashSet<String>,
    checks: std::collections::HashMap<String, HashSet<String>>,
}

async fn hygiene_report() -> Result<()> {
    println!("===========================================");
    println!("  cargo x reconcile hygiene-report");
    println!("===========================================\n");

    let cfg = load_catalogue()?;
    let declared = collect_declared_fqns(&cfg);
    let macros = collect_macro_fqns();
    let declared_or_macro: HashSet<_> = declared.union(&macros).cloned().collect();
    let mut registry = sem_os_postgres::ops::build_registry();
    ob_poc::domain_ops::extend_registry(&mut registry);
    let registered: HashSet<String> = registry.manifest().into_iter().collect();

    let simple_status = load_simple_status_entries()?;
    let schema = load_schema_tables()?;
    let dag_refs = collect_dag_verb_refs()?;
    let writer_index = collect_writer_index(&cfg, &simple_status);

    println!("Inputs:");
    println!("  declared YAML verbs:      {}", declared.len());
    println!("  declared macro YAML:      {}", macros.len());
    println!("  registered SemOs ops:     {}", registered.len());
    println!("  SimpleStatus configs:     {}", simple_status.len());
    println!("  DAG verb references:      {}", dag_refs.len());
    println!("  schema tables loaded:     {}", schema.len());

    println!("\nSimpleStatus drift findings:");
    let mut simple_findings = Vec::new();
    for entry in &simple_status {
        match schema.get(&entry.table) {
            None => simple_findings.push(format!(
                "{} at simple_status_op.rs:{}: table '{}' not found",
                entry.fqn, entry.line, entry.table
            )),
            Some(table) => {
                if !table.columns.contains(&entry.pk_col) {
                    simple_findings.push(format!(
                        "{} at simple_status_op.rs:{}: pk_col '{}' not found on '{}'",
                        entry.fqn, entry.line, entry.pk_col, entry.table
                    ));
                }
                if !table.columns.contains(&entry.state_col) {
                    simple_findings.push(format!(
                        "{} at simple_status_op.rs:{}: state_col '{}' not found on '{}'",
                        entry.fqn, entry.line, entry.state_col, entry.table
                    ));
                    continue;
                }
                if let Some(allowed) = table.checks.get(&entry.state_col) {
                    if !allowed.contains(&entry.target_state) {
                        simple_findings.push(format!(
                            "{} at simple_status_op.rs:{}: target_state '{}' not allowed by '{}.{}' CHECK {:?}",
                            entry.fqn,
                            entry.line,
                            entry.target_state,
                            entry.table,
                            entry.state_col,
                            sorted_set(allowed)
                        ));
                    }
                }
            }
        }
    }
    if simple_findings.is_empty() {
        println!("  none");
    } else {
        for finding in &simple_findings {
            println!("  - {finding}");
        }
    }

    println!("\nDAG references without YAML declaration:");
    let missing_yaml: Vec<_> = dag_refs
        .iter()
        .filter(|fqn| !declared_or_macro.contains(*fqn))
        .cloned()
        .collect();
    print_limited(&missing_yaml, 50);

    println!("\nDAG references without registered SemOs op:");
    let missing_runtime: Vec<_> = dag_refs
        .iter()
        .filter(|fqn| {
            declared.contains(*fqn)
                && !registered.contains(*fqn)
                && verb_behavior(&cfg, fqn)
                    .is_some_and(|behavior| matches!(behavior, dsl_core::VerbBehavior::Plugin))
        })
        .cloned()
        .collect();
    print_limited(&missing_runtime, 50);

    println!("\nPlugin YAML declarations without registered SemOs op:");
    let mut plugin_missing = Vec::new();
    for (domain, dblock) in &cfg.domains {
        for (verb, vblock) in &dblock.verbs {
            if matches!(vblock.behavior, dsl_core::VerbBehavior::Plugin) {
                let fqn = format!("{domain}.{verb}");
                if !registered.contains(&fqn) {
                    plugin_missing.push(fqn);
                }
            }
        }
    }
    plugin_missing.sort();
    print_limited(&plugin_missing, 100);

    println!("\nDeal substate closure:");
    let deal_required_writes = [
        ("deals", "bac_status", "in_review"),
        ("deals", "bac_status", "approved"),
        ("deals", "bac_status", "rejected"),
        ("deals", "kyc_clearance_status", "pending"),
        ("deals", "kyc_clearance_status", "in_review"),
        ("deals", "kyc_clearance_status", "approved"),
        ("deals", "kyc_clearance_status", "rejected"),
    ];
    let mut deal_closure_missing = Vec::new();
    for (table, column, value) in deal_required_writes {
        let key = (table.to_string(), column.to_string(), value.to_string());
        match writer_index.get(&key) {
            Some(writers) if !writers.is_empty() => {
                println!("  OK {table}.{column} = {value}: {}", writers.join(", "));
            }
            Some(_) | None => {
                let finding = format!("{table}.{column} = {value}");
                println!("  MISSING {finding}");
                deal_closure_missing.push(finding);
            }
        }
    }

    println!("\nSummary:");
    println!("  SimpleStatus drift findings: {}", simple_findings.len());
    println!("  DAG refs missing YAML:        {}", missing_yaml.len());
    println!("  DAG refs missing runtime:     {}", missing_runtime.len());
    println!("  plugin YAML missing runtime:  {}", plugin_missing.len());
    println!(
        "  deal substate closure gaps:   {}",
        deal_closure_missing.len()
    );
    println!("\nReport-only: this command exits zero by design.");
    Ok(())
}

fn collect_writer_index(
    cfg: &VerbsConfig,
    simple_status: &[SimpleStatusEntry],
) -> std::collections::HashMap<(String, String, String), Vec<String>> {
    let mut index = std::collections::HashMap::<(String, String, String), Vec<String>>::new();

    for entry in simple_status {
        index
            .entry((
                entry.table.clone(),
                entry.state_col.clone(),
                entry.target_state.clone(),
            ))
            .or_default()
            .push(entry.fqn.clone());
    }

    for (domain, dblock) in &cfg.domains {
        for (verb, vblock) in &dblock.verbs {
            let fqn = format!("{domain}.{verb}");
            for write in &vblock.writes {
                let Some(value) = &write.value else {
                    continue;
                };
                index
                    .entry((write.table.clone(), write.column.clone(), value.clone()))
                    .or_default()
                    .push(fqn.clone());
            }
        }
    }

    for writers in index.values_mut() {
        writers.sort();
        writers.dedup();
    }

    index
}

fn hygiene_failure_summary() -> Result<Vec<String>> {
    let cfg = load_catalogue()?;
    let declared = collect_declared_fqns(&cfg);
    let mut registry = sem_os_postgres::ops::build_registry();
    ob_poc::domain_ops::extend_registry(&mut registry);
    let registered: HashSet<String> = registry.manifest().into_iter().collect();

    let simple_status = load_simple_status_entries()?;
    let schema = load_schema_tables()?;
    let dag_refs = collect_dag_verb_refs()?;
    let writer_index = collect_writer_index(&cfg, &simple_status);
    let mut failures = Vec::new();

    failures.extend(simple_status_drift_failures(&simple_status, &schema));

    for fqn in dag_refs.iter().filter(|fqn| !declared.contains(*fqn)) {
        failures.push(format!("{fqn}: DAG reference has no YAML declaration"));
    }

    for fqn in dag_refs.iter().filter(|fqn| {
        declared.contains(*fqn)
            && !registered.contains(*fqn)
            && verb_behavior(&cfg, fqn)
                .is_some_and(|behavior| matches!(behavior, dsl_core::VerbBehavior::Plugin))
    }) {
        failures.push(format!(
            "{fqn}: DAG references plugin verb without registered SemOs op"
        ));
    }

    for (domain, dblock) in &cfg.domains {
        for (verb, vblock) in &dblock.verbs {
            if matches!(vblock.behavior, dsl_core::VerbBehavior::Plugin) {
                let fqn = format!("{domain}.{verb}");
                if !registered.contains(&fqn) {
                    failures.push(format!(
                        "{fqn}: plugin YAML declaration has no registered SemOs op"
                    ));
                }
            }
        }
    }

    for (table, column, value) in [
        ("deals", "bac_status", "in_review"),
        ("deals", "bac_status", "approved"),
        ("deals", "bac_status", "rejected"),
        ("deals", "kyc_clearance_status", "pending"),
        ("deals", "kyc_clearance_status", "in_review"),
        ("deals", "kyc_clearance_status", "approved"),
        ("deals", "kyc_clearance_status", "rejected"),
    ] {
        let key = (table.to_string(), column.to_string(), value.to_string());
        if writer_index
            .get(&key)
            .is_none_or(|writers| writers.is_empty())
        {
            failures.push(format!("{table}.{column} = {value}: no declared writer"));
        }
    }

    failures.extend(entry_via_consistency_failures()?);
    failures.extend(cascade_source_scan_failures()?);

    failures.sort();
    Ok(failures)
}

fn simple_status_drift_failures(
    simple_status: &[SimpleStatusEntry],
    schema: &std::collections::HashMap<String, TableSchema>,
) -> Vec<String> {
    let mut failures = Vec::new();
    for entry in simple_status {
        match schema.get(&entry.table) {
            None => failures.push(format!(
                "{}: SimpleStatus table '{}' not found",
                entry.fqn, entry.table
            )),
            Some(table) => {
                if !table.columns.contains(&entry.pk_col) {
                    failures.push(format!(
                        "{}: SimpleStatus pk_col '{}.{}' not found",
                        entry.fqn, entry.table, entry.pk_col
                    ));
                }
                if !table.columns.contains(&entry.state_col) {
                    failures.push(format!(
                        "{}: SimpleStatus state_col '{}.{}' not found",
                        entry.fqn, entry.table, entry.state_col
                    ));
                    continue;
                }
                if let Some(allowed) = table.checks.get(&entry.state_col) {
                    if !allowed.contains(&entry.target_state) {
                        failures.push(format!(
                            "{}: SimpleStatus target_state '{}' not allowed by '{}.{}' CHECK",
                            entry.fqn, entry.target_state, entry.table, entry.state_col
                        ));
                    }
                }
            }
        }
    }
    failures
}

fn entry_via_consistency_failures() -> Result<Vec<String>> {
    let dags = load_dags_from_dir(&PathBuf::from("config/sem_os_seeds/dag_taxonomies"))
        .context("loading DAG taxonomies for entry_via hygiene")?;
    let cfg = load_catalogue()?;
    let declared = collect_declared_fqns(&cfg);
    let mut failures = Vec::new();

    for (_path, loaded) in dags {
        let dag = loaded.dag;
        for slot in &dag.slots {
            let Some(SlotStateMachine::Structured(machine)) = &slot.state_machine else {
                continue;
            };
            let mut incoming = std::collections::HashSet::new();
            for transition in &machine.transitions {
                incoming.insert(transition.to.clone());
            }
            for state in &machine.states {
                let location = format!("{}.{}.{}", dag.workspace, slot.id, state.id);
                if !state.entry && !incoming.contains(&state.id) && state.entry_via.is_none() {
                    failures.push(format!(
                        "{location}: state has no incoming transition and no entry_via"
                    ));
                }
                match &state.entry_via {
                    Some(EntryVia::Verb) if !incoming.contains(&state.id) => {
                        failures.push(format!(
                            "{location}: entry_via verb requires an incoming transition"
                        ));
                    }
                    Some(EntryVia::Cascade { parent }) => {
                        if parent.trim().is_empty() {
                            failures.push(format!("{location}: entry_via cascade parent is empty"));
                        } else if !declared.contains(parent) {
                            failures.push(format!(
                                "{location}: entry_via cascade parent '{parent}' is not a declared verb"
                            ));
                        }
                    }
                    Some(EntryVia::Trigger { name }) if name.trim().is_empty() => {
                        failures.push(format!("{location}: entry_via trigger name is empty"));
                    }
                    Some(EntryVia::Scheduler { name }) if name.trim().is_empty() => {
                        failures.push(format!("{location}: entry_via scheduler name is empty"));
                    }
                    Some(EntryVia::Signal { source }) if source.trim().is_empty() => {
                        failures.push(format!("{location}: entry_via signal source is empty"));
                    }
                    Some(EntryVia::Verb)
                    | Some(EntryVia::Trigger { .. })
                    | Some(EntryVia::Scheduler { .. })
                    | Some(EntryVia::Signal { .. }) => {}
                    None => {}
                }
            }
        }
    }

    Ok(failures)
}

fn cascade_source_scan_failures() -> Result<Vec<String>> {
    let mut failures = Vec::new();
    let cbu_src = std::fs::read_to_string("crates/sem_os_postgres/src/ops/cbu.rs")
        .context("reading cbu cascade parent source")?;
    let role_src = std::fs::read_to_string("crates/sem_os_postgres/src/ops/cbu_role.rs")
        .context("reading cbu role cascade parent source")?;

    for finding in scan_forbidden_mutations(
        source_section(&cbu_src, "// cbu.create", "// cbu.link-structure"),
        "cbu.create",
        &[
            "cbu_entity_roles",
            "client_group_entity",
            "entity_relationships",
        ],
    ) {
        failures.push(finding);
    }
    for finding in scan_forbidden_mutations(
        source_section(&cbu_src, "// cbu.add-product", "// cbu.inspect"),
        "cbu.add-product",
        &["service_delivery_map", "cbu_resource_instances"],
    ) {
        failures.push(finding);
    }
    for finding in scan_forbidden_mutations(
        source_section(&cbu_src, "// cbu.decide", "// cbu.delete-cascade"),
        "cbu.decide",
        &["cases"],
    ) {
        failures.push(finding);
    }
    for finding in scan_forbidden_mutations(
        source_section(&cbu_src, "// cbu.delete-cascade", ""),
        "cbu.delete-cascade",
        &[
            "client_group_entity",
            "cbu_group_members",
            "cbu_structure_links",
            "entities",
            "cbu_entity_roles",
        ],
    ) {
        failures.push(finding);
    }
    for finding in scan_forbidden_mutations(
        &role_src,
        "cbu.assign-ownership/control/trust-role",
        &["entity_relationships"],
    ) {
        failures.push(finding);
    }

    Ok(failures)
}

fn source_section<'a>(source: &'a str, start_marker: &str, end_marker: &str) -> &'a str {
    let Some(start) = source.find(start_marker) else {
        return "";
    };
    let rest = &source[start..];
    if end_marker.is_empty() {
        return rest;
    }
    rest.find(end_marker)
        .map(|end| &rest[..end])
        .unwrap_or(rest)
}

fn scan_forbidden_mutations(section: &str, parent_fqn: &str, tables: &[&str]) -> Vec<String> {
    let collapsed = section.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = collapsed.to_ascii_uppercase();
    let mut failures = Vec::new();
    for table in tables {
        let quoted = format!("\"OB-POC\".{table}").to_ascii_uppercase();
        let unquoted = table.to_ascii_uppercase();
        for op in ["INSERT INTO", "UPDATE", "DELETE FROM"] {
            if upper.contains(&format!("{op} {quoted}"))
                || upper.contains(&format!("{op} {unquoted}"))
            {
                failures.push(format!(
                    "{parent_fqn}: direct off-carrier {op} on {table}; use registry child dispatch"
                ));
            }
        }
    }
    failures
}

fn verb_behavior<'a>(cfg: &'a VerbsConfig, fqn: &str) -> Option<&'a dsl_core::VerbBehavior> {
    let (domain, verb) = fqn.rsplit_once('.')?;
    cfg.domains
        .get(domain)
        .and_then(|domain_config| domain_config.verbs.get(verb))
        .map(|verb_config| &verb_config.behavior)
}

fn load_simple_status_entries() -> Result<Vec<SimpleStatusEntry>> {
    let path = PathBuf::from("src/domain_ops/simple_status_op.rs");
    let raw = std::fs::read_to_string(&path).with_context(|| format!("reading {path:?}"))?;
    let field_re =
        regex::Regex::new(r#"(?m)^\s*(fqn|table|pk_col|state_col|target_state):\s*"([^"]+)""#)?;
    let mut entries = Vec::new();
    let mut in_block = false;
    let mut start_line = 0usize;
    let mut block = String::new();

    for (idx, line) in raw.lines().enumerate() {
        if line.contains("SimpleStatusConfig {") {
            in_block = true;
            start_line = idx + 1;
            block.clear();
        }
        if in_block {
            block.push_str(line);
            block.push('\n');
            if line.trim() == "}," {
                let mut fields = std::collections::HashMap::new();
                for cap in field_re.captures_iter(&block) {
                    fields.insert(cap[1].to_string(), cap[2].to_string());
                }
                if let (Some(fqn), Some(table), Some(pk_col), Some(state_col), Some(target_state)) = (
                    fields.get("fqn"),
                    fields.get("table"),
                    fields.get("pk_col"),
                    fields.get("state_col"),
                    fields.get("target_state"),
                ) {
                    entries.push(SimpleStatusEntry {
                        line: start_line,
                        fqn: fqn.clone(),
                        table: table.clone(),
                        pk_col: pk_col.clone(),
                        state_col: state_col.clone(),
                        target_state: target_state.clone(),
                    });
                }
                in_block = false;
            }
        }
    }

    Ok(entries)
}

fn load_schema_tables() -> Result<std::collections::HashMap<String, TableSchema>> {
    let mut paths = vec![PathBuf::from("../migrations/master-schema.sql")];
    let rust_migrations = PathBuf::from("migrations");
    if rust_migrations.exists() {
        for entry in std::fs::read_dir(&rust_migrations)
            .with_context(|| format!("reading migration dir {rust_migrations:?}"))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("sql")
                && path.file_name().and_then(|name| name.to_str()) != Some("master-schema.sql")
            {
                paths.push(path);
            }
        }
    }

    let create_re =
        regex::Regex::new(r#"^CREATE TABLE (?:IF NOT EXISTS )?"ob-poc"\.([A-Za-z0-9_]+) \("#)?;
    let column_re = regex::Regex::new(r#"^\s+"?([A-Za-z_][A-Za-z0-9_]*)"?\s+[^,]+,?\s*$"#)?;
    let quoted_value_re = regex::Regex::new(r#"'([^']+)'"#)?;

    let mut tables = std::collections::HashMap::<String, TableSchema>::new();
    for path in paths {
        let raw = std::fs::read_to_string(&path).with_context(|| format!("reading {path:?}"))?;
        parse_schema_sql(&raw, &create_re, &column_re, &quoted_value_re, &mut tables);
    }

    Ok(tables)
}

fn parse_schema_sql(
    raw: &str,
    create_re: &regex::Regex,
    column_re: &regex::Regex,
    quoted_value_re: &regex::Regex,
    tables: &mut std::collections::HashMap<String, TableSchema>,
) {
    let mut current: Option<(String, TableSchema)> = None;

    for line in raw.lines() {
        if let Some(cap) = create_re.captures(line) {
            current = Some((cap[1].to_string(), TableSchema::default()));
            continue;
        }

        if let Some((name, table)) = current.as_mut() {
            let trimmed = line.trim();
            if trimmed == ");" {
                tables.insert(name.clone(), std::mem::take(table));
                current = None;
                continue;
            }
            if trimmed.starts_with("CONSTRAINT") {
                if let Some(column) = table
                    .columns
                    .iter()
                    .find(|column| line.contains(&format!("({column})::text")))
                    .cloned()
                {
                    let values: HashSet<String> = quoted_value_re
                        .captures_iter(line)
                        .map(|cap| cap[1].to_string())
                        .collect();
                    if !values.is_empty() {
                        table.checks.insert(column, values);
                    }
                }
                continue;
            }
            if let Some(cap) = column_re.captures(line) {
                let column = cap[1].to_string();
                if column != "CONSTRAINT" {
                    table.columns.insert(column);
                }
            }
        }
    }
}

fn collect_dag_verb_refs() -> Result<HashSet<String>> {
    let mut refs = HashSet::new();
    let path = PathBuf::from("config/sem_os_seeds/dag_taxonomies");
    let verb_re = regex::Regex::new(r#"\b[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*)+\b"#)?;

    for entry in std::fs::read_dir(&path).with_context(|| format!("reading {path:?}"))? {
        let entry = entry?;
        let file_path = entry.path();
        if file_path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }
        let raw = std::fs::read_to_string(&file_path)
            .with_context(|| format!("reading DAG taxonomy {file_path:?}"))?;
        for line in raw.lines() {
            if !line.contains("via:") {
                continue;
            }
            for cap in verb_re.captures_iter(line) {
                refs.insert(cap[0].to_string());
            }
        }
    }

    Ok(refs)
}

fn sorted_set(values: &HashSet<String>) -> Vec<String> {
    let mut out: Vec<_> = values.iter().cloned().collect();
    out.sort();
    out
}

fn validate_pack_workspaces_against_dags(
    packs: &BTreeMap<String, LoadedPack>,
    known_slots: &HashSet<(String, String)>,
) -> Vec<String> {
    let dag_workspaces: HashSet<&str> = known_slots
        .iter()
        .map(|(workspace, _slot)| workspace.as_str())
        .collect();
    let aliases = [
        ("on_boarding", "onboarding_request"),
        ("sem_os_maintenance", "semos_maintenance"),
    ];
    let mut failures = Vec::new();

    for pack in packs.values() {
        for workspace in &pack.workspaces {
            if dag_workspaces.contains(workspace.as_str()) {
                continue;
            }
            if let Some((_, canonical)) = aliases
                .iter()
                .find(|(legacy, _canonical)| workspace == legacy)
            {
                failures.push(format!(
                    "{}: pack '{}' uses legacy workspace '{}'; use DAG workspace '{}'",
                    pack.source_path.display(),
                    pack.name,
                    workspace,
                    canonical
                ));
            } else {
                failures.push(format!(
                    "{}: pack '{}' references workspace '{}' with no DAG taxonomy",
                    pack.source_path.display(),
                    pack.name,
                    workspace
                ));
            }
        }
    }

    failures
}

fn print_limited(values: &[String], limit: usize) {
    if values.is_empty() {
        println!("  none");
        return;
    }
    for value in values.iter().take(limit) {
        println!("  - {value}");
    }
    if values.len() > limit {
        println!("  ... {} more", values.len() - limit);
    }
}

async fn status() -> Result<()> {
    println!("===========================================");
    println!("  cargo x reconcile --status");
    println!("===========================================\n");

    let cfg = load_catalogue()?;
    let mut total = 0usize;
    let mut declared = 0usize;
    let mut escalation_rules = 0usize;

    // Per-tier baseline tallies
    let mut tier_tally = std::collections::BTreeMap::<&'static str, usize>::new();
    for t in [
        "benign",
        "reviewable",
        "requires_confirmation",
        "requires_explicit_authorisation",
    ] {
        tier_tally.insert(t, 0);
    }

    // Per-domain breakdown
    let mut domain_counts: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new(); // domain -> (total, declared)

    for (dname, dblock) in &cfg.domains {
        let (t, d) = domain_counts.entry(dname.clone()).or_insert((0, 0));
        for vblock in dblock.verbs.values() {
            total += 1;
            *t += 1;
            if let Some(ref ta) = vblock.three_axis {
                declared += 1;
                *d += 1;
                escalation_rules += ta.consequence.escalation.len();
                let tier_name = match ta.consequence.baseline {
                    dsl_core::ConsequenceTier::Benign => "benign",
                    dsl_core::ConsequenceTier::Reviewable => "reviewable",
                    dsl_core::ConsequenceTier::RequiresConfirmation => "requires_confirmation",
                    dsl_core::ConsequenceTier::RequiresExplicitAuthorisation => {
                        "requires_explicit_authorisation"
                    }
                };
                *tier_tally.entry(tier_name).or_insert(0) += 1;
            }
        }
    }

    let pct = if total > 0 {
        (declared as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    println!("Declared: {declared} / {total}  ({pct:.1}%)",);
    println!("Escalation rules (total across all verbs): {escalation_rules}");
    println!();
    println!("By baseline tier (declared verbs only):");
    for (tier, n) in &tier_tally {
        println!("  {:36} {n}", tier);
    }
    println!();
    println!("By domain (showing only domains with verbs):");
    for (d, (t, dc)) in domain_counts.iter().filter(|(_, (t, _))| *t > 0) {
        let dpct = if *t > 0 {
            (*dc as f64 / *t as f64) * 100.0
        } else {
            0.0
        };
        println!("  {:30} {:>4} / {:<4}  ({:>5.1}%)", d, dc, t, dpct);
    }
    Ok(())
}

async fn batch(op: String) -> Result<()> {
    println!("===========================================");
    println!("  cargo x reconcile --batch {op}");
    println!("===========================================\n");

    let cfg = load_catalogue()?;
    let total: usize = cfg.domains.values().map(|d| d.verbs.len()).sum();

    println!(
        "Pilot P.7 scaffold — batch operation '{op}' would target {total} verbs \
         across {} domains.",
        cfg.domains.len()
    );
    println!();
    println!(
        "NOTE: Batch mutations are Tranche-2 scope. Pilot P.7 validates the CLI \
         shape only — no mutations performed."
    );
    Ok(())
}

#[cfg(test)]
mod hygiene_tests {
    use super::*;

    fn entry(
        fqn: &str,
        table: &str,
        pk_col: &str,
        state_col: &str,
        target_state: &str,
    ) -> SimpleStatusEntry {
        SimpleStatusEntry {
            line: 1,
            fqn: fqn.to_string(),
            table: table.to_string(),
            pk_col: pk_col.to_string(),
            state_col: state_col.to_string(),
            target_state: target_state.to_string(),
        }
    }

    fn table(columns: &[&str], check_col: &str, values: &[&str]) -> TableSchema {
        TableSchema {
            columns: columns.iter().map(|value| (*value).to_string()).collect(),
            checks: std::collections::HashMap::from([(
                check_col.to_string(),
                values.iter().map(|value| (*value).to_string()).collect(),
            )]),
        }
    }

    #[test]
    fn simple_status_drift_detects_missing_table() {
        let failures = simple_status_drift_failures(
            &[entry("x.y", "missing_table", "id", "status", "ACTIVE")],
            &std::collections::HashMap::new(),
        );

        assert!(failures[0].contains("table 'missing_table' not found"));
    }

    #[test]
    fn simple_status_drift_detects_missing_pk_column() {
        let schema = std::collections::HashMap::from([(
            "things".to_string(),
            table(&["id", "status"], "status", &["ACTIVE"]),
        )]);
        let failures = simple_status_drift_failures(
            &[entry("x.y", "things", "thing_id", "status", "ACTIVE")],
            &schema,
        );

        assert!(failures[0].contains("pk_col 'things.thing_id' not found"));
    }

    #[test]
    fn simple_status_drift_detects_missing_state_column() {
        let schema = std::collections::HashMap::from([(
            "things".to_string(),
            table(&["id", "status"], "status", &["ACTIVE"]),
        )]);
        let failures = simple_status_drift_failures(
            &[entry("x.y", "things", "id", "lifecycle_status", "ACTIVE")],
            &schema,
        );

        assert!(failures[0].contains("state_col 'things.lifecycle_status' not found"));
    }

    #[test]
    fn simple_status_drift_detects_invalid_target_value() {
        let schema = std::collections::HashMap::from([(
            "things".to_string(),
            table(&["id", "status"], "status", &["ACTIVE"]),
        )]);
        let failures = simple_status_drift_failures(
            &[entry("x.y", "things", "id", "status", "BROKEN")],
            &schema,
        );

        assert!(failures[0].contains("target_state 'BROKEN' not allowed"));
    }

    #[test]
    fn cascade_scan_detects_direct_off_carrier_mutation() {
        let section = r##"
            sqlx::query(r#"UPDATE "ob-poc".entity_relationships SET updated_at = now()"#);
        "##;
        let failures = scan_forbidden_mutations(section, "parent.verb", &["entity_relationships"]);

        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("parent.verb"));
    }

    #[test]
    fn cascade_scan_allows_child_dispatch_without_direct_mutation() {
        let section = r#"
            dispatch_child_verb(self.fqn(), "entity-relationship.upsert", &args, ctx, scope).await?;
        "#;
        let failures = scan_forbidden_mutations(section, "parent.verb", &["entity_relationships"]);

        assert!(failures.is_empty());
    }
}
