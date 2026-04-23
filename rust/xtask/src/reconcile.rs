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
    validate_verbs_config, ConfigLoader, ValidationContext, VerbsConfig,
};

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

async fn validate(strict_warnings: bool) -> Result<()> {
    println!("===========================================");
    println!("  cargo x reconcile --validate");
    println!("===========================================\n");

    let cfg = load_catalogue()?;
    let total: usize = cfg.domains.values().map(|d| d.verbs.len()).sum();
    println!("Loaded {} domains, {} verbs from config/verbs/", cfg.domains.len(), total);

    let ctx = ValidationContext {
        require_declaration: false,
        ..ValidationContext::default()
    };
    let report = validate_verbs_config(&cfg, &ctx);

    let struct_n = report.structural.len();
    let wf_n = report.well_formedness.len();
    let warn_n = report.warnings.len();

    println!();
    println!("Structural errors:      {struct_n}");
    println!("Well-formedness errors: {wf_n}");
    println!("Policy-sanity warnings: {warn_n}");

    if struct_n > 0 {
        println!("\nStructural errors:");
        for e in &report.structural {
            println!("  ✗ {e}");
        }
    }
    if wf_n > 0 {
        println!("\nWell-formedness errors:");
        for e in &report.well_formedness {
            println!("  ✗ {e}");
        }
    }
    if warn_n > 0 {
        println!("\nPolicy-sanity warnings:");
        for w in &report.warnings {
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
        for (_, vblock) in &dblock.verbs {
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

    println!(
        "Declared: {declared} / {total}  ({pct:.1}%)",
    );
    println!("Escalation rules (total across all verbs): {escalation_rules}");
    println!();
    println!("By baseline tier (declared verbs only):");
    for (tier, n) in &tier_tally {
        println!("  {:36} {n}", tier);
    }
    println!();
    println!("By domain (showing only domains with verbs):");
    for (d, (t, dc)) in domain_counts
        .iter()
        .filter(|(_, (t, _))| *t > 0)
    {
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
