//! Phase 4.7 — canonical schema authority audit.
//!
//! Walks the workspace (`rust/crates/**/*.rs` + `rust/src/**/*.rs`)
//! and reports every `pub struct <Name>` / `pub enum <Name>` whose
//! `<Name>` matches a curated list of schema-authority type names
//! that V&S §O7 / ADN §7.3 lock to `sem_os_core` as the single
//! source. Each canonical name lives only in `sem_os_core`; any
//! parallel definition in another crate is reported as drift.
//!
//! ## Why a curated list, not a graph walker
//!
//! Catching *every* DAG / verb / entity / transition type by graph
//! analysis is out of scope for the spike. The curated list keeps
//! the audit cheap (regex over file contents), reproducible (the
//! authority list lives in this source file), and easy to ratchet
//! (add a name when a new schema authority lands; persist any new
//! parallel definitions in the allowlist file with a reason).
//!
//! ## Ratcheting policy
//!
//! The known-drift allowlist (`tools/schema-authority-drift-
//! allowlist.txt`) is a "this is where we are today" snapshot. New
//! parallel definitions fail the audit; existing ones stay listed
//! with a `# reason:` comment so reviewers see why the mirror
//! exists. Removing an entry (because the mirror was merged back
//! into `sem_os_core`) also fails the audit until refreshed via
//! `--bless`, which forces a deliberate ack of the schema-authority
//! posture change.
//!
//! ## CLI shape
//!
//! `cargo run -p xtask -- audit` — fail on drift / removal not in
//! allowlist.
//! `cargo run -p xtask -- audit --bless` — refresh the allowlist
//! after deliberate review.

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// File where the audit persists the known-drift snapshot.
const ALLOWLIST_PATH: &str = "tools/schema-authority-drift-allowlist.txt";

/// Canonical crate. Any definition of a [`CANONICAL_NAMES`] member
/// inside this crate is the authoritative one and never reported.
const CANONICAL_CRATE_PATH_FRAGMENT: &str = "/sem_os_core/";

/// Schema-authority type names V&S §O7 / ADN §7.3 lock to
/// `sem_os_core`. Each line names the canonical owner module for
/// reviewer context; the audit only matches on the type name.
const CANONICAL_NAMES: &[&str] = &[
    // State-graph (DAG) primitives — sem_os_core/state_graph_def.rs.
    "StateGraphDefBody",
    "GraphNode",
    "GraphEdge",
    "GraphGate",
    "NodeType",
    "EdgeType",
    "SignalCondition",
    // FSM / transition primitives — sem_os_core/state_machine_def.rs.
    "StateMachineDefBody",
    "TransitionDef",
    "ReducerDef",
    "ConditionDef",
    "OverlaySourceDef",
    "RuleDef",
    "ConsistencyCheckDef",
    // Verb contract — sem_os_core/verb_contract.rs.
    "VerbContractBody",
    "VerbCrudMapping",
    "VerbArgDef",
    "VerbReturnSpec",
    "VerbPrecondition",
    "VerbProducesSpec",
    "VerbOutput",
    // Entity-type lifecycle — sem_os_core/entity_type_def.rs.
    "EntityTypeDefBody",
    "LifecycleStateDef",
    "LifecycleTransition",
    // Relationship type — sem_os_core/relationship_type_def.rs.
    "RelationshipTypeDefBody",
    // Attribute / typed-attribute schema — sem_os_core/attribute_def.rs.
    "AttributeDefBody",
    // Constellation surface — sem_os_core/constellation_*.
    "ConstellationMapDefBody",
    // Document + evidence + observation + obligation + profile —
    // sem_os_core/*_def.rs.
    "DocumentTypeDefBody",
    "EvidenceStrategyDefBody",
    "ObservationDefBody",
    "ProofObligationDefBody",
    "RequirementProfileDefBody",
    // Service-resource discovery & provisioning — sem_os_core/
    // service_resource_def.rs.
    "ServiceResourceDefBody",
    // Taxonomy / universe / view surface — sem_os_core/*_def.rs.
    "TaxonomyDefBody",
    "UniverseDefBody",
    "ViewDefBody",
    // Policy rule — sem_os_core/policy_rule.rs.
    "PolicyRuleBody",
];

/// Directories the audit walks. Workspace-relative; the audit runs
/// from the `rust/` working directory (xtask default).
const SCAN_ROOTS: &[&str] = &["crates", "src"];

/// Run the schema-authority audit.
///
/// Returns `Ok(())` on clean / blessed runs; `Err` if drift was
/// detected and `bless` is `false`.
pub(crate) fn run(bless: bool) -> Result<()> {
    let observed = collect_drift(Path::new("."))?;
    if bless {
        write_allowlist(Path::new(ALLOWLIST_PATH), &observed)?;
        println!(
            "Schema-authority drift allowlist refreshed with {} entries at {ALLOWLIST_PATH}",
            observed.len()
        );
        return Ok(());
    }

    let expected = read_allowlist(Path::new(ALLOWLIST_PATH))?;
    let added: Vec<&String> = observed.difference(&expected).collect();
    let removed: Vec<&String> = expected.difference(&observed).collect();

    if added.is_empty() && removed.is_empty() {
        println!(
            "Schema-authority audit clean: {} canonical names tracked, {} known mirrors held \
             at status quo",
            CANONICAL_NAMES.len(),
            observed.len()
        );
        return Ok(());
    }

    if !added.is_empty() {
        eprintln!(
            "New schema-authority parallel definitions detected (not in {ALLOWLIST_PATH}):"
        );
        for entry in &added {
            eprintln!("  + {entry}");
        }
    }
    if !removed.is_empty() {
        eprintln!(
            "Allowlist entries no longer observed (mirror merged back into sem_os_core or moved):"
        );
        for entry in &removed {
            eprintln!("  - {entry}");
        }
    }
    bail!(
        "schema-authority audit failed; run `cargo run -p xtask -- audit --bless` only after \
         reviewing whether each change preserves or weakens sem_os_core as the single schema \
         authority"
    )
}

fn collect_drift(root: &Path) -> Result<BTreeSet<String>> {
    let matcher = build_matcher();
    let mut entries = BTreeSet::new();
    for scan_root in SCAN_ROOTS {
        let path = root.join(scan_root);
        if !path.exists() {
            continue;
        }
        walk(&path, &matcher, &mut entries)?;
    }
    Ok(entries)
}

/// Recursively scan `.rs` files under `dir`, appending drift
/// entries. Skips the canonical crate so its own definitions are
/// never reported.
fn walk(dir: &Path, matcher: &Regex, entries: &mut BTreeSet<String>) -> Result<()> {
    let read = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for child in read {
        let child = child?;
        let path = child.path();
        let kind = child.file_type()?;
        if kind.is_dir() {
            // Skip the canonical crate, target dirs, and node_modules
            // (defensive — none should exist under SCAN_ROOTS).
            let lossy = path.to_string_lossy();
            if lossy.contains(CANONICAL_CRATE_PATH_FRAGMENT)
                || lossy.contains("/target/")
                || lossy.contains("/node_modules/")
            {
                continue;
            }
            walk(&path, matcher, entries)?;
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        scan_file(&path, matcher, entries)?;
    }
    Ok(())
}

fn scan_file(path: &Path, matcher: &Regex, entries: &mut BTreeSet<String>) -> Result<()> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("reading source file {}", path.display()))?;
    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub ") {
            continue;
        }
        if let Some(captures) = matcher.captures(trimmed) {
            let name = captures
                .get(1)
                .expect("name capture is always present when match")
                .as_str()
                .to_string();
            let relative = path
                .strip_prefix(".")
                .unwrap_or(path)
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            entries.insert(format!("{relative}:{}:{name}", line_idx + 1));
        }
    }
    Ok(())
}

/// Builds a regex of the form
/// `^pub (struct|enum) (Name1|Name2|...)\b` matching the canonical
/// names with a word boundary so e.g. `GraphNodeInput` doesn't fire
/// when scanning for `GraphNode`.
fn build_matcher() -> Regex {
    let alternation = CANONICAL_NAMES.join("|");
    Regex::new(&format!(
        r"^pub\s+(?:struct|enum)\s+({alternation})\b"
    ))
    .expect("regex over hand-curated names should compile")
}

fn read_allowlist(path: &Path) -> Result<BTreeSet<String>> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("reading drift allowlist {}", path.display()))?;
    Ok(source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect())
}

fn write_allowlist(path: &Path, items: &BTreeSet<String>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "creating drift allowlist parent directory {}",
                parent.display()
            )
        })?;
    }
    let mut buf = String::new();
    buf.push_str("# Schema-authority drift allowlist (Phase 4.7).\n");
    buf.push_str(
        "# Each line is `<path>:<line>:<TypeName>` for a parallel\n",
    );
    buf.push_str(
        "# definition of a name V&S §O7 / ADN §7.3 locks to sem_os_core.\n",
    );
    buf.push_str(
        "# Regenerate with `cargo run -p xtask -- audit --bless` after\n",
    );
    buf.push_str("# reviewing whether the mirror is still justified.\n");
    buf.push_str("# Run `cargo run -p xtask -- audit` to fail on new mirrors.\n");
    buf.push('\n');
    for entry in items {
        buf.push_str(entry);
        buf.push('\n');
    }
    fs::write(path, buf).with_context(|| format!("writing drift allowlist {}", path.display()))?;
    Ok(())
}

/// Test helper: the canonical name set, exposed so unit tests can
/// assert authority-list shape without re-spelling it.
#[cfg(test)]
fn canonical_names() -> Vec<&'static str> {
    CANONICAL_NAMES.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn matcher_matches_pub_struct_with_canonical_name() {
        let matcher = build_matcher();
        let caps = matcher.captures("pub struct GraphNode {").unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "GraphNode");
    }

    #[test]
    fn matcher_matches_pub_enum_with_canonical_name() {
        let matcher = build_matcher();
        let caps = matcher.captures("pub enum NodeType {").unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "NodeType");
    }

    #[test]
    fn matcher_rejects_longer_name_with_canonical_prefix() {
        // `GraphNodeInput` shares the `GraphNode` prefix but isn't a
        // schema-authority name — must not match.
        let matcher = build_matcher();
        assert!(matcher.captures("pub struct GraphNodeInput {").is_none());
    }

    #[test]
    fn matcher_rejects_non_pub_definition() {
        let matcher = build_matcher();
        assert!(matcher.captures("struct GraphNode {").is_none());
        assert!(matcher.captures("pub(crate) struct GraphNode {").is_none());
    }

    #[test]
    fn collect_drift_skips_canonical_crate() {
        // The real workspace already has the canonical defs in
        // `crates/sem_os_core/src/*.rs`. They must NOT appear in the
        // observed drift set when scanning from the repo root.
        let observed = collect_drift(Path::new(".")).unwrap();
        let canonical_hits: Vec<_> = observed
            .iter()
            .filter(|e| e.contains("sem_os_core/"))
            .collect();
        assert!(
            canonical_hits.is_empty(),
            "canonical defs should be skipped, got: {canonical_hits:?}"
        );
    }

    #[test]
    fn canonical_names_are_unique() {
        let names = canonical_names();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "canonical_names list contains duplicates"
        );
    }

    #[test]
    fn round_trip_through_temp_allowlist() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("allowlist.txt");
        let mut items = BTreeSet::new();
        items.insert("crates/some/src/foo.rs:42:GraphNode".to_string());
        items.insert("crates/other/src/bar.rs:7:NodeType".to_string());
        write_allowlist(&path, &items).unwrap();
        let round_tripped = read_allowlist(&path).unwrap();
        assert_eq!(round_tripped, items);
    }

    #[test]
    fn drift_collected_from_synthetic_tree() {
        // Build a temp tree with one fake crate containing a parallel
        // definition; assert the scanner reports it. Skips the
        // canonical-crate filter because the synthetic tree has no
        // sem_os_core path fragment.
        let tmp = TempDir::new().unwrap();
        let crate_dir = tmp.path().join("crates/sample/src");
        fs::create_dir_all(&crate_dir).unwrap();
        fs::write(
            crate_dir.join("lib.rs"),
            "// fake source\npub struct GraphNode {}\n",
        )
        .unwrap();
        let observed = collect_drift(tmp.path()).unwrap();
        assert!(
            observed.iter().any(|e| e.ends_with(":2:GraphNode")),
            "expected GraphNode to be reported, got: {observed:?}"
        );
    }
}
