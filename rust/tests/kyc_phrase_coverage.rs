//! T7.2 §2 point 1 — "kit-pin the phrase surface": closure tooth pinning
//! invocation-phrase coverage for the 21-verb dsl.kyc kit (`phase1_lexicon()`),
//! mirroring the K-G7 lesson (`EOP-DD-KYCUBO-KIT-T0.3`) applied to phrases
//! instead of verbs: every kit verb must carry `>= MIN_PHRASES` invocation
//! phrases, and no `invocation_phrases` block may name a verb outside the kit
//! (a phrase leaking onto a retired/foreign verb is exactly the K-G7 defect
//! class — fold-blind, silently wrong, just for phrases instead of writes).
//!
//! Deliberately reads the raw YAML as `serde_yaml::Value` rather than the
//! strict `VerbConfig` deserializer — this tooth only needs fqn + phrase
//! count, so it stays decoupled from unrelated `VerbConfig` schema changes.

use std::collections::{BTreeMap, BTreeSet};

use ob_poc_kyc_substrate::phase1_lexicon;

const MIN_PHRASES: usize = 3;

const YAML_SOURCES: &[&str] = &[
    include_str!("../config/verbs/kyc/dsl-kyc.yaml"),
    include_str!("../config/verbs/kyc/dsl-kyc-obligation.yaml"),
];

/// Walks `domains.<domain>.verbs.<verb>.invocation_phrases` out of the raw
/// YAML, keyed by `"{domain}.{verb}"` — the same fqn shape `phase1_lexicon()`
/// uses.
fn phrase_counts(yaml: &str) -> BTreeMap<String, usize> {
    let doc: serde_yaml::Value = serde_yaml::from_str(yaml).expect("valid verb YAML");
    let mut out = BTreeMap::new();
    let Some(domains) = doc.get("domains").and_then(|d| d.as_mapping()) else {
        return out;
    };
    for (domain_key, domain_val) in domains {
        let domain = domain_key.as_str().unwrap_or_default();
        let Some(verbs) = domain_val.get("verbs").and_then(|v| v.as_mapping()) else {
            continue;
        };
        for (verb_key, verb_val) in verbs {
            let verb = verb_key.as_str().unwrap_or_default();
            let fqn = format!("{domain}.{verb}");
            let count = verb_val
                .get("invocation_phrases")
                .and_then(|p| p.as_sequence())
                .map(|s| s.len())
                .unwrap_or(0);
            out.insert(fqn, count);
        }
    }
    out
}

#[test]
fn phrase_coverage_tooth() {
    let kit_fqns: BTreeSet<String> = phase1_lexicon().entries.keys().cloned().collect();

    let mut yaml_phrases: BTreeMap<String, usize> = BTreeMap::new();
    for src in YAML_SOURCES {
        yaml_phrases.extend(phrase_counts(src));
    }

    // Every kit verb has >= MIN_PHRASES phrases declared.
    let under_covered: Vec<(String, usize)> = kit_fqns
        .iter()
        .map(|fqn| (fqn.clone(), yaml_phrases.get(fqn).copied().unwrap_or(0)))
        .filter(|(_, n)| *n < MIN_PHRASES)
        .collect();
    assert!(
        under_covered.is_empty(),
        "kit verbs with fewer than {MIN_PHRASES} invocation_phrases (T7.2 phrase pin): \
         {under_covered:#?}"
    );

    // No phrase block maps to a verb outside the kit (K-G7 lesson: a phrase
    // pointing at a retired/foreign verb is fold-blind, silently wrong).
    let foreign: Vec<&String> = yaml_phrases
        .keys()
        .filter(|fqn| !kit_fqns.contains(*fqn))
        .collect();
    assert!(
        foreign.is_empty(),
        "invocation_phrases blocks naming verbs outside phase1_lexicon()'s 21-verb kit \
         (retired/foreign verb — re-home or delete the phrase block): {foreign:#?}"
    );
}
