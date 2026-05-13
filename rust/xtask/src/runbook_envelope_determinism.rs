//! Phase 5.5 — RunbookEnvelope determinism harness.
//!
//! V&S §6.5 / Phase 4.5 (locked decision D2=c) names
//! `RunbookEnvelope::envelope_hash` the single hashable audit
//! artefact for replay-grade reproducibility. This harness builds a
//! fixed set of canonical envelopes, hashes each via the boundary
//! crate's stable serialisation path, and asserts the hashes match
//! a persisted baseline in `tools/runbook-envelope-determinism-
//! baseline.json`.
//!
//! ## Why a separate harness from the ACP envelope check?
//!
//! `acp-envelope-byte-equality-check` (R6 / Phase 4 of the ACP pack
//! context parity stream) protects the **discovery projection**
//! envelopes a Sage ACP editor observes. Those are byte-equality
//! over CLI output. The runbook envelope is a different artefact —
//! a per-prompt JSON `{runbook_id, version, source, state_context}`
//! map that travels with each Sage round-trip and is hashed for
//! audit. Same property (replay-grade byte / hash equality), two
//! distinct artefacts; one harness per artefact keeps drift
//! signals separable.
//!
//! ## What this catches
//!
//! - Accidental `serde_json` whitespace changes (a `pretty` flag
//!   added somewhere upstream).
//! - `BTreeMap` ↔ `HashMap` swaps for `state_context` (hash order
//!   non-determinism).
//! - Any addition / removal / reorder of `RunbookEnvelope` fields
//!   that breaks the canonical serialised form.
//! - Rust toolchain bumps that change `f64`/`i64` JSON encoding.
//!
//! ## What this does NOT catch
//!
//! - Whether the *content* (verb FQNs, state references) is right.
//!   That's the planning-loop integration tests' job.
//! - Cross-platform endianness (the canonical form is UTF-8 JSON;
//!   SHA-256 hashing is platform-independent).
//!
//! ## CLI
//!
//! `cargo run -p xtask -- runbook-envelope-determinism-check`
//! `cargo run -p xtask -- runbook-envelope-determinism-check --bless`

use anyhow::{bail, Context, Result};
use ob_poc_boundary::runbook_envelope::RunbookEnvelope;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

const BASELINE_PATH: &str = "tools/runbook-envelope-determinism-baseline.json";

/// One envelope fixture in the harness corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaselineEntry {
    /// Stable fixture id (also the runbook_id on the envelope).
    fixture_id: String,
    /// Final envelope version after the fixture builder ran.
    version: u32,
    /// SHA-256 hex of the canonical JSON serialisation.
    envelope_hash: String,
    /// Byte length of the canonical JSON serialisation. Carried
    /// alongside the hash so drift reports show both axes (hash
    /// shifts say "*something* changed"; byte deltas say "this much
    /// of the bytes moved").
    byte_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Baseline {
    #[serde(rename = "_comment", default, skip_serializing_if = "Option::is_none")]
    comment: Option<String>,
    captured_at: String,
    captured_under: String,
    entries: Vec<BaselineEntry>,
}

/// Run the determinism check.
pub(crate) fn run(bless: bool) -> Result<()> {
    let observed = compute_observed()?;
    let baseline_path = Path::new(BASELINE_PATH);

    if bless {
        let new_baseline = Baseline {
            comment: Some(
                "Phase 5.5 runbook envelope determinism baseline. The envelope_hash \
                 column is V&S §6.5 / Phase 4.5 (D2=c) replay-grade audit artefact for \
                 the canonical RunbookEnvelope shape. Bump via `cargo run -p xtask -- \
                 runbook-envelope-determinism-check --bless` only after reviewing \
                 whether the upstream serialisation change is intentional."
                    .to_string(),
            ),
            captured_at: today_iso(),
            captured_under: "blessed via xtask --bless".to_string(),
            entries: observed,
        };
        let serialized = serde_json::to_string_pretty(&new_baseline)
            .context("serialising refreshed runbook envelope baseline")?;
        std::fs::write(baseline_path, serialized).with_context(|| {
            format!("writing baseline to {}", baseline_path.display())
        })?;
        println!(
            "Runbook envelope determinism baseline refreshed at {} ({} fixtures)",
            baseline_path.display(),
            new_baseline.entries.len()
        );
        return Ok(());
    }

    let expected = read_baseline(baseline_path)?;
    compare(&expected.entries, &observed)
}

fn compute_observed() -> Result<Vec<BaselineEntry>> {
    Ok(fixtures()
        .into_iter()
        .map(|(fixture_id, envelope)| {
            let hash = envelope.envelope_hash();
            let bytes = serde_json::to_string(&envelope)
                .expect("RunbookEnvelope is always serializable")
                .len();
            BaselineEntry {
                fixture_id: fixture_id.to_string(),
                version: envelope.version,
                envelope_hash: hash,
                byte_length: bytes,
            }
        })
        .collect())
}

/// The canonical fixture corpus. Each entry exercises one axis of
/// the envelope's stability surface. New fixtures land at the end
/// of the list so existing baseline indices stay stable.
fn fixtures() -> Vec<(&'static str, RunbookEnvelope)> {
    let mut out = Vec::new();

    // (1) Minimal envelope — fresh + empty state context.
    out.push((
        "minimal_empty_context",
        RunbookEnvelope::new("rb-minimal", "(cbu.create)"),
    ));

    // (2) Single state-context entry.
    {
        let mut ctx = BTreeMap::new();
        ctx.insert(
            "cbu".to_string(),
            Value::String("entity:cbu:abc-123".to_string()),
        );
        out.push((
            "single_state_entry",
            RunbookEnvelope::with_state_context(
                "rb-single",
                "(cbu.attach-product :cbu @cbu)",
                ctx,
            ),
        ));
    }

    // (3) Multiple state-context entries — proves BTreeMap ordering
    // determinism. Insert in non-alphabetical order deliberately.
    {
        let mut ctx = BTreeMap::new();
        ctx.insert("zulu".to_string(), Value::String("z-val".to_string()));
        ctx.insert("alpha".to_string(), Value::String("a-val".to_string()));
        ctx.insert("mike".to_string(), Value::String("m-val".to_string()));
        out.push((
            "multi_state_ordering",
            RunbookEnvelope::with_state_context(
                "rb-multi",
                "(view.universe)",
                ctx,
            ),
        ));
    }

    // (4) After revise(): version bumps + source replaces.
    {
        let mut env = RunbookEnvelope::new("rb-revise", "(cbu.create)");
        env.revise("(cbu.attach-product :cbu @cbu)");
        out.push(("after_revise", env));
    }

    // (5) After apply_state_change(): version bumps + state mutates.
    {
        let mut env = RunbookEnvelope::new("rb-state-change", "(kyc.start-case)");
        env.apply_state_change(|ctx| {
            ctx.insert("kyc_case".to_string(), json!("entity:kyc-case:xyz-789"));
            ctx.insert("jurisdiction".to_string(), json!("LU"));
        });
        out.push(("after_state_change", env));
    }

    // (6) Mixed-type state context — strings, ints, nested objects,
    // arrays. The canonical form must serialise these in JSON shape
    // matching serde_json's default + BTreeMap key order.
    {
        let mut ctx = BTreeMap::new();
        ctx.insert("text".to_string(), json!("plain"));
        ctx.insert("count".to_string(), json!(42));
        ctx.insert("flag".to_string(), json!(true));
        ctx.insert(
            "nested".to_string(),
            json!({"a": 1, "b": [1, 2, 3]}),
        );
        ctx.insert("array".to_string(), json!(["x", "y", "z"]));
        out.push((
            "mixed_value_types",
            RunbookEnvelope::with_state_context(
                "rb-mixed",
                "(deal.assemble)",
                ctx,
            ),
        ));
    }

    out
}

fn read_baseline(path: &Path) -> Result<Baseline> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("reading runbook envelope baseline {}", path.display()))?;
    serde_json::from_str(&source)
        .with_context(|| format!("parsing baseline at {}", path.display()))
}

fn compare(expected: &[BaselineEntry], observed: &[BaselineEntry]) -> Result<()> {
    let expected_by_id: std::collections::BTreeMap<&str, &BaselineEntry> = expected
        .iter()
        .map(|e| (e.fixture_id.as_str(), e))
        .collect();
    let observed_by_id: std::collections::BTreeMap<&str, &BaselineEntry> = observed
        .iter()
        .map(|o| (o.fixture_id.as_str(), o))
        .collect();

    let mut drift: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut added: Vec<String> = Vec::new();

    for (id, exp) in &expected_by_id {
        match observed_by_id.get(id) {
            None => missing.push((*id).to_string()),
            Some(obs) => {
                if obs.envelope_hash != exp.envelope_hash {
                    drift.push(format!(
                        "{id}: hash {} → {} ({} bytes → {} bytes, version {} → {})",
                        exp.envelope_hash,
                        obs.envelope_hash,
                        exp.byte_length,
                        obs.byte_length,
                        exp.version,
                        obs.version,
                    ));
                }
            }
        }
    }
    for id in observed_by_id.keys() {
        if !expected_by_id.contains_key(id) {
            added.push((*id).to_string());
        }
    }

    if drift.is_empty() && missing.is_empty() && added.is_empty() {
        println!(
            "Runbook envelope determinism check clean: {} fixtures hashed and matched against \
             baseline.",
            observed.len()
        );
        return Ok(());
    }

    if !drift.is_empty() {
        eprintln!("Envelope hash drift detected:");
        for d in &drift {
            eprintln!("  ! {d}");
        }
    }
    if !missing.is_empty() {
        eprintln!("Baseline fixtures no longer produced by the harness:");
        for m in &missing {
            eprintln!("  - {m}");
        }
    }
    if !added.is_empty() {
        eprintln!("Harness fixtures not yet in the baseline:");
        for a in &added {
            eprintln!("  + {a}");
        }
    }
    bail!(
        "runbook envelope determinism check failed; run `cargo run -p xtask -- \
         runbook-envelope-determinism-check --bless` only after reviewing whether each \
         change preserves replay-grade byte / hash equality"
    )
}

fn today_iso() -> String {
    chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_ids_are_unique() {
        let ids: Vec<&str> = fixtures().into_iter().map(|(id, _)| id).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "duplicate fixture id");
    }

    #[test]
    fn observed_entries_are_stable_across_runs() {
        // Running the fixture builder twice must produce identical
        // hashes. This is the in-process determinism check; the
        // xtask gate is the cross-run check.
        let a = compute_observed().unwrap();
        let b = compute_observed().unwrap();
        assert_eq!(a.len(), b.len());
        for (left, right) in a.iter().zip(b.iter()) {
            assert_eq!(left.fixture_id, right.fixture_id);
            assert_eq!(left.envelope_hash, right.envelope_hash);
            assert_eq!(left.byte_length, right.byte_length);
            assert_eq!(left.version, right.version);
        }
    }

    #[test]
    fn fixture_versions_match_their_builders() {
        // The fixtures with explicit lifecycle steps must report the
        // correct version. seeds (1/2/3/6) stay at 1; revise (4) and
        // apply_state_change (5) advance to 2.
        let observed = compute_observed().unwrap();
        let by_id: std::collections::HashMap<&str, &BaselineEntry> = observed
            .iter()
            .map(|e| (e.fixture_id.as_str(), e))
            .collect();
        assert_eq!(by_id["minimal_empty_context"].version, 1);
        assert_eq!(by_id["single_state_entry"].version, 1);
        assert_eq!(by_id["multi_state_ordering"].version, 1);
        assert_eq!(by_id["mixed_value_types"].version, 1);
        assert_eq!(by_id["after_revise"].version, 2);
        assert_eq!(by_id["after_state_change"].version, 2);
    }

    #[test]
    fn hashes_are_sha256_hex_strings() {
        for entry in compute_observed().unwrap() {
            assert_eq!(
                entry.envelope_hash.len(),
                64,
                "SHA-256 hex must be 64 chars, got {} for {}",
                entry.envelope_hash.len(),
                entry.fixture_id
            );
            assert!(
                entry.envelope_hash.chars().all(|c| c.is_ascii_hexdigit()),
                "hash must be hex: {}",
                entry.envelope_hash
            );
        }
    }
}
