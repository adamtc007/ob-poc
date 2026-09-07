//! P0 RED gate (2026-09-07, audit item 3) + the "gate so it cannot drift"
//! this same tranche's P1 asks for: the `kyc-case` journey pack's claimed
//! `kyc_ubo.*` verb set must equal the live Assembly-lexicon verb set
//! (`assembly_lexicon()`'s `kyc_ubo.assert.edge.*` /
//! `kyc_ubo.assert.subject.*` / `kyc_ubo.decide.determination.*` domains —
//! the board-game moves `enumerate_placement_set` actually enumerates).
//! `cargo x kyc-alignment`'s V2/V5 checks already surface this as a printed
//! report a human has to remember to run; this is the same fact as a real
//! `cargo test` failure, checked against BOTH pack files that declare the
//! `kyc-case` journey (`config/packs/kyc-case.yaml`,
//! `config/semantic-packs/kyc-case.yaml` — two independent declaration
//! sites with two DIFFERENT schemas, same requirement).
//!
//! RED today: both packs still list the retired `kyc_ubo.assert.subject
//! .register` (merged into `place`, EOP-VS-UBO-GAME-001 T2 §3.2), and are
//! missing `place`/`remove`/`enquiry` entirely — so a human working the
//! journey pack cannot reach the moves that build a board at all.
//!
//! `kyc_ubo.assert.edge.nominee-piercing` is a DIFFERENT case, not checked
//! here as a plain-verb membership fact: it retired as a standalone verb
//! (TS.6 P2, no lexicon entry, no op) but survives as a ratified MACRO
//! (`config/verb_schemas/macros/ubo.yaml`, EOP-DD-KYCUBO-TS.6 §5) composing
//! `connect`+`disconnect` atomics — a pack legitimately references it via
//! the macro channel, not the Assembly-lexicon channel this gate checks.
//! Excluded explicitly below rather than silently passing as "board
//! vocabulary."

use std::collections::BTreeSet;
use std::path::PathBuf;

use dsl_core::load_packs_from_dir;
use ob_poc_kyc_substrate::assembly_lexicon;

const KYC_UBO_ASSEMBLY_PREFIXES: &[&str] = &[
    "kyc_ubo.assert.edge.",
    "kyc_ubo.assert.subject.",
    "kyc_ubo.decide.determination.",
];

fn live_assembly_fqns() -> BTreeSet<String> {
    assembly_lexicon()
        .entries
        .keys()
        .filter(|fqn| KYC_UBO_ASSEMBLY_PREFIXES.iter().any(|p| fqn.starts_with(p)))
        .cloned()
        .collect()
}

/// The one known, ratified macro that shares the `kyc_ubo.assert.edge.*`
/// FQN namespace with plain lexicon verbs — see module doc. Not "board
/// vocabulary" (no `LexiconEntry`, never `enumerate_placement_set`-visible),
/// legitimately pack-referenced anyway.
const NOMINEE_PIERCING_MACRO: &str = "kyc_ubo.assert.edge.nominee-piercing";

fn assembly_scoped(verbs: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    verbs
        .into_iter()
        .filter(|fqn| fqn != NOMINEE_PIERCING_MACRO)
        .filter(|fqn| KYC_UBO_ASSEMBLY_PREFIXES.iter().any(|p| fqn.starts_with(p)))
        .collect()
}

/// `config/packs/kyc-case.yaml` — the flat `id`/`name`/`allowed_verbs`
/// `dsl_core::PackManifest` shape the V2 REPL journey pack loader reads.
fn config_packs_kyc_case_fqns() -> BTreeSet<String> {
    let loaded = load_packs_from_dir(&PathBuf::from("config/packs"))
        .unwrap_or_else(|e| panic!("loading config/packs for kyc-case pack alignment: {e}"));
    let kyc_case = loaded
        .get("kyc-case")
        .unwrap_or_else(|| panic!("config/packs has no 'kyc-case' pack"));
    assembly_scoped(kyc_case.allowed_verbs.iter().cloned())
}

/// `config/semantic-packs/kyc-case.yaml` is NOT a `dsl_core::PackManifest`
/// — confirmed by running `load_packs_from_dir` against it directly: it
/// silently returns an (incorrectly) near-empty verb set rather than
/// erroring, because the loader's `Pack` struct has no field this schema
/// populates. It is a distinct schema (`schema_version: 1`, `pack: {id:
/// ...}`, `capabilities[].extensions."ob-poc.allowed_verbs"`) — read
/// generically instead of forcing the wrong loader onto it.
fn semantic_packs_kyc_case_fqns() -> BTreeSet<String> {
    let path = "config/semantic-packs/kyc-case.yaml";
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("parsing {path}: {e}"));
    let mut verbs = BTreeSet::new();
    let capabilities = doc.get("capabilities").and_then(|c| c.as_sequence());
    for cap in capabilities.into_iter().flatten() {
        let Some(list) = cap
            .get("extensions")
            .and_then(|e| e.get("ob-poc.allowed_verbs"))
            .and_then(|v| v.as_sequence())
        else {
            continue;
        };
        for item in list {
            if let Some(s) = item.as_str() {
                verbs.insert(s.to_string());
            }
        }
    }
    assembly_scoped(verbs)
}

fn assert_matches_live_vocabulary(source: &str, pack: BTreeSet<String>) {
    let live = live_assembly_fqns();
    let missing: Vec<&String> = live.difference(&pack).collect();
    let stale: Vec<&String> = pack.difference(&live).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "{source}'s kyc-case pack must claim exactly the live Assembly board-move \
         vocabulary — missing (live but not in pack): {missing:?}; stale (in pack but \
         retired/never real): {stale:?}. A human working this pack must be able to reach \
         every board move (place/remove/enquiry/connect/evidence/retract/disconnect/freeze), \
         and never be offered a retired FQN."
    );
}

#[test]
fn the_pack_lists_the_live_vocabulary_config_packs() {
    assert_matches_live_vocabulary("config/packs/kyc-case.yaml", config_packs_kyc_case_fqns());
}

#[test]
fn the_pack_lists_the_live_vocabulary_semantic_packs() {
    assert_matches_live_vocabulary(
        "config/semantic-packs/kyc-case.yaml",
        semantic_packs_kyc_case_fqns(),
    );
}
