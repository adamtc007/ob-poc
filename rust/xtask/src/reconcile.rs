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
use dsl_core::config::{
    collect_declared_fqns, flatten_pack_entries, load_dags_from_dir, load_packs_from_dir,
    validate_dags, validate_pack_fqns, validate_verbs_config, ConfigLoader, ValidationContext,
    VerbsConfig,
};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum ReconcileAction {
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
}

pub async fn run(action: ReconcileAction) -> Result<()> {
    match action {
        ReconcileAction::Validate { strict_warnings } => validate(strict_warnings).await,
        ReconcileAction::Status => status().await,
        ReconcileAction::Batch { op } => batch(op).await,
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

    let ctx = ValidationContext {
        require_declaration: false,
        ..ValidationContext::default()
    };
    let report = validate_verbs_config(&cfg, &ctx);

    // V1.2-5 — pack-hygiene cross-check.
    let packs_dir = PathBuf::from("config/packs");
    let pack_errors = if packs_dir.exists() {
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
        validate_pack_fqns(&declared, &macros, flatten_pack_entries(&packs))
    } else {
        Vec::new()
    };

    // V1.3 — DAG taxonomy cross-DAG validation (R-2b).
    let dags_dir = PathBuf::from("config/sem_os_seeds/dag_taxonomies");
    let dag_report = if dags_dir.exists() {
        let loaded = load_dags_from_dir(&dags_dir)
            .context("loading config/sem_os_seeds/dag_taxonomies/ for v1.3 checks")?;
        println!(
            "Loaded {} DAG taxonomies from config/sem_os_seeds/dag_taxonomies/",
            loaded.len()
        );
        validate_dags(&loaded)
    } else {
        dsl_core::config::DagValidationReport::default()
    };

    let struct_n = report.structural.len();
    let wf_n = report.well_formedness.len() + pack_errors.len() + dag_report.errors.len();
    let warn_n = report.warnings.len() + dag_report.warnings.len();

    println!();
    println!("Structural errors:           {struct_n}");
    println!(
        "Well-formedness errors:      {wf_n}  ({} three-axis + {} pack-hygiene + {} cross-DAG)",
        report.well_formedness.len(),
        pack_errors.len(),
        dag_report.errors.len()
    );
    println!(
        "Policy-sanity warnings:      {warn_n}  ({} three-axis + {} DAG lint)",
        report.warnings.len(),
        dag_report.warnings.len()
    );

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

    let failed = struct_n > 0 || wf_n > 0 || (strict_warnings && warn_n > 0);
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
                    dsl_core::config::ConsequenceTier::Benign => "benign",
                    dsl_core::config::ConsequenceTier::Reviewable => "reviewable",
                    dsl_core::config::ConsequenceTier::RequiresConfirmation => {
                        "requires_confirmation"
                    }
                    dsl_core::config::ConsequenceTier::RequiresExplicitAuthorisation => {
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
