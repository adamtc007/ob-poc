//! `cargo x dag-test` — runs the cross-workspace DAG test harness.
//! `cargo x dag-coverage` — reports DAG-taxonomy coverage gaps.
//! `cargo x dag-fixture` — scaffolds a new fixture YAML.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use xshell::{cmd, Shell};

const FIXTURES_DIR: &str = "rust/crates/dsl-runtime/tests/fixtures/cross_workspace_dag";
const DAG_TAXONOMIES_DIR: &str = "rust/config/sem_os_seeds/dag_taxonomies";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DagTestMode {
    Both,
    MockOnly,
    LiveOnly,
}

/// Run the cross-workspace DAG harness.
///
/// `--reset` clears artifacts under target/ that may shadow fresh runs.
/// `--filter <name>` runs only test functions matching the substring.
pub fn run(sh: &Shell, mode: DagTestMode, reset: bool, filter: Option<String>) -> Result<()> {
    if reset {
        println!("[dag-test] Resetting test artifacts…");
        let workspace_target = repo_root().join("rust/target");
        if workspace_target.exists() {
            // Remove the harness-failure dump dir if it exists. We don't
            // touch the cargo build cache; sqlx::test ephemeral DBs are
            // dropped automatically when each test exits.
            let dumps = workspace_target.join("harness_failures");
            if dumps.exists() {
                std::fs::remove_dir_all(&dumps).ok();
            }
        }
    }

    let _push = sh.push_dir(repo_root().join("rust"));

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///postgres".into());
    sh.set_var("DATABASE_URL", &database_url);

    if mode != DagTestMode::LiveOnly {
        println!("\n=== Mock-mode scenarios ===");
        let mut args = vec![
            "test",
            "-p",
            "dsl-runtime",
            "--features",
            "harness",
            "--test",
            "cross_workspace_dag_scenarios",
        ];
        if let Some(f) = &filter {
            args.push(f);
        }
        cmd!(sh, "cargo {args...}").run()?;
    }

    if mode != DagTestMode::MockOnly {
        println!("\n=== Live-mode scenarios (DATABASE_URL={}) ===", database_url);
        let mut args = vec![
            "test",
            "-p",
            "dsl-runtime",
            "--features",
            "harness",
            "--test",
            "cross_workspace_dag_live_scenarios",
        ];
        if let Some(f) = &filter {
            args.push(f);
        }
        cmd!(sh, "cargo {args...}").run()?;
    }

    println!("\n[dag-test] OK");
    Ok(())
}

/// Coverage report — enumerate every cross_workspace_constraint, derived
/// state, and cascade rule across all DAG taxonomies, cross-reference
/// against fixtures, report gaps.
pub fn coverage(workspace_filter: Option<String>, json: bool) -> Result<()> {
    use dsl_core::config::DagRegistry;

    let dag_path = repo_root().join(DAG_TAXONOMIES_DIR);
    let registry = DagRegistry::from_dir(&dag_path)
        .with_context(|| format!("loading DAG taxonomies from {}", dag_path.display()))?;

    let exercised = scan_fixtures_for_exercised_ids()?;

    let mut report = CoverageReport::default();

    for (workspace, dag) in registry.iter() {
        if let Some(filter) = &workspace_filter {
            if workspace != filter {
                continue;
            }
        }

        let mut ws_report = WorkspaceCoverage {
            workspace: workspace.clone(),
            ..Default::default()
        };

        // Cross-workspace constraints
        for c in &dag.cross_workspace_constraints {
            let exercised = exercised.constraints.contains(&c.id);
            ws_report.constraints.push((c.id.clone(), exercised));
        }

        // Derived states
        for d in &dag.derived_cross_workspace_state {
            let exercised = exercised.derived.contains(&d.id);
            ws_report.derived.push((d.id.clone(), exercised));
        }

        // Cascade rules (per slot.state_dependency.cascade_rules)
        for slot in &dag.slots {
            if let Some(dep) = &slot.state_dependency {
                for rule in &dep.cascade_rules {
                    let key = format!("{}.{}/parent={}", workspace, slot.id, rule.parent_state);
                    let exercised = exercised.cascades.contains(&key);
                    ws_report.cascades.push((key, exercised));
                }
            }
        }

        report.workspaces.push(ws_report);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        report.print_human_readable();
    }

    Ok(())
}

/// Scaffold a new fixture YAML.
pub fn scaffold_fixture(name: &str, mode: &str) -> Result<()> {
    let path = repo_root().join(FIXTURES_DIR).join(format!("{}.yaml", name));
    if path.exists() {
        anyhow::bail!("fixture {} already exists", path.display());
    }

    let content = format!(
        r#"name: "TODO: human-readable scenario name"
suite_id: "{name}"
description: |
  TODO: what this scenario verifies.

mode: {mode}

entity_aliases:
  alias-1: "00000000-0000-0000-0000-000000000001"

initial_state:
  - workspace: TODO
    slot: TODO
    entity: "alias-1"
    state: "TODO_STATE"
    # attrs: ...   (live mode only — bridge columns for predicate joins)

# (live mode only) Mock-only field; ignored by live runner.
predicates: {{}}

# children: {{}}    (cascade tests only)

steps:
  - name: "TODO: step description"
    check_transition:
      workspace: TODO
      slot: TODO
      entity: "alias-1"
      from: "FROM_STATE"
      to: "TO_STATE"
    expect:
      violations: []
"#,
        name = name,
        mode = mode,
    );

    std::fs::write(&path, content)?;
    println!(
        "[dag-fixture] Created {}\n\nNext steps:\n  1. Edit the file with your scenario.\n  2. Append `scenario_test!({}, \"tests/fixtures/cross_workspace_dag/{}.yaml\");`\n     to the appropriate test runner under rust/crates/dsl-runtime/tests/.\n  3. Run with `cargo x dag-test --filter {}`.",
        path.display(), name, name, name,
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────
// Coverage scanning
// ─────────────────────────────────────────────────────────────────

#[derive(Default)]
struct ExercisedIds {
    constraints: HashSet<String>,
    derived: HashSet<String>,
    cascades: HashSet<String>,
}

fn scan_fixtures_for_exercised_ids() -> Result<ExercisedIds> {
    let mut out = ExercisedIds::default();
    let dir = repo_root().join(FIXTURES_DIR);
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        scan_text(&text, &mut out);
    }
    Ok(out)
}

fn scan_text(text: &str, out: &mut ExercisedIds) {
    // Heuristic regex-free extraction:
    //   constraint_id: "foo"        → out.constraints.insert("foo")
    //   derived_id: "foo"            → out.derived.insert("foo")
    //   plan_cascade with a parent_new_state line → out.cascades
    //
    // Lines may be list items (`- constraint_id: "foo"`); strip the
    // leading `- ` before testing prefixes.
    let mut pending_cascade: Option<(String, String)> = None;
    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches("- ").trim();
        if let Some(rest) = line.strip_prefix("constraint_id:") {
            if let Some(id) = extract_quoted(rest) {
                out.constraints.insert(id);
            }
        } else if let Some(rest) = line.strip_prefix("derived_id:") {
            if let Some(id) = extract_quoted(rest) {
                out.derived.insert(id);
            }
        } else if let Some(rest) = line.strip_prefix("parent_workspace:") {
            if let Some(ws) = extract_quoted(rest) {
                pending_cascade = Some((ws, String::new()));
            }
        } else if let Some(rest) = line.strip_prefix("parent_slot:") {
            if let Some(slot) = extract_quoted(rest) {
                if let Some((_, ref mut s)) = pending_cascade {
                    *s = slot;
                }
            }
        } else if let Some(rest) = line.strip_prefix("parent_new_state:") {
            if let Some(state) = extract_quoted(rest) {
                if let Some((ws, slot)) = pending_cascade.take() {
                    out.cascades.insert(format!("{}.{}/parent={}", ws, slot, state));
                }
            }
        }
    }
}

fn extract_quoted(s: &str) -> Option<String> {
    let s = s.trim();
    let s = s.trim_start_matches(|c| c == ':' || c == ' ');
    let s = s.trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────
// Report types
// ─────────────────────────────────────────────────────────────────

#[derive(Default, serde::Serialize)]
struct CoverageReport {
    workspaces: Vec<WorkspaceCoverage>,
}

#[derive(Default, serde::Serialize)]
struct WorkspaceCoverage {
    workspace: String,
    constraints: Vec<(String, bool)>,
    derived: Vec<(String, bool)>,
    cascades: Vec<(String, bool)>,
}

impl CoverageReport {
    fn print_human_readable(&self) {
        println!("\nDAG Coverage Report\n===================\n");
        let (mut total_c, mut hit_c) = (0, 0);
        let (mut total_d, mut hit_d) = (0, 0);
        let (mut total_x, mut hit_x) = (0, 0);

        for ws in &self.workspaces {
            if ws.constraints.is_empty() && ws.derived.is_empty() && ws.cascades.is_empty() {
                continue;
            }
            println!(
                "── {} ─────────────────────────────────────",
                ws.workspace
            );

            for (id, ex) in &ws.constraints {
                println!(
                    "  [{}] constraint  {}",
                    if *ex { "✓" } else { " " },
                    id
                );
            }
            for (id, ex) in &ws.derived {
                println!(
                    "  [{}] derived     {}",
                    if *ex { "✓" } else { " " },
                    id
                );
            }
            for (id, ex) in &ws.cascades {
                println!(
                    "  [{}] cascade     {}",
                    if *ex { "✓" } else { " " },
                    id
                );
            }

            let ws_c_total = ws.constraints.len();
            let ws_c_hit = ws.constraints.iter().filter(|(_, e)| *e).count();
            let ws_d_total = ws.derived.len();
            let ws_d_hit = ws.derived.iter().filter(|(_, e)| *e).count();
            let ws_x_total = ws.cascades.len();
            let ws_x_hit = ws.cascades.iter().filter(|(_, e)| *e).count();

            if ws_c_total > 0 {
                println!(
                    "    Constraints: {}/{} ({}%)",
                    ws_c_hit,
                    ws_c_total,
                    pct(ws_c_hit, ws_c_total)
                );
            }
            if ws_d_total > 0 {
                println!(
                    "    Derived:     {}/{} ({}%)",
                    ws_d_hit,
                    ws_d_total,
                    pct(ws_d_hit, ws_d_total)
                );
            }
            if ws_x_total > 0 {
                println!(
                    "    Cascades:    {}/{} ({}%)",
                    ws_x_hit,
                    ws_x_total,
                    pct(ws_x_hit, ws_x_total)
                );
            }
            println!();

            total_c += ws_c_total;
            hit_c += ws_c_hit;
            total_d += ws_d_total;
            hit_d += ws_d_hit;
            total_x += ws_x_total;
            hit_x += ws_x_hit;
        }

        println!("OVERALL");
        println!("  Constraints: {}/{} ({}%)", hit_c, total_c, pct(hit_c, total_c));
        println!("  Derived:     {}/{} ({}%)", hit_d, total_d, pct(hit_d, total_d));
        println!("  Cascades:    {}/{} ({}%)", hit_x, total_x, pct(hit_x, total_x));
    }
}

fn pct(hit: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        (hit * 100) / total
    }
}

fn repo_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set when invoked via cargo");
    Path::new(&manifest_dir)
        .parent()
        .and_then(Path::parent)
        .expect("xtask is at rust/xtask, so two levels up is repo root")
        .to_path_buf()
}
