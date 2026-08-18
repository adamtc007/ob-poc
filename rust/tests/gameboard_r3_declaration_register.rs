//! EOP-PLAN-GAMEBOARD-001 R3 — `declared_states_are_real_states` (RED first).
//!
//! §1 fact 3: the declarations themselves are unvalidated. This test
//! computes, for every verb declaring a non-empty `lifecycle.requires_states`,
//! whether every declared value is a real state in its target slot's real
//! `state_machine` vocabulary (read from the DAG YAML, the single source of
//! truth for what a slot's states actually are — not re-derived, not
//! guessed). It is RED today by design: 15 of 87 verbs fail this check
//! (10 reference a state that doesn't exist on the target slot at all —
//! `WRONG_VOCABULARY`; 5 use the free-text placeholder `"any non-terminal"`,
//! which can never equal a real column value — `FREE_TEXT_ESCAPE`). Full
//! detail, per-verb proposed corrections, and cross-cutting findings (in
//! particular `screening.run`, which appears to target the wrong slot's
//! vocabulary entirely, not just contain a typo) are drafted in
//! `docs/eop/EOP-PLAN-GAMEBOARD-001_R3-Declaration-Register_v0.1.md` — a
//! DRAFT awaiting ratification, not yet encoded.
//!
//! This test's own assertion pins the exact known-bad set so R3's eventual
//! correction is a conscious, verifiable edit (same discipline as the two
//! existing teeth in `domain_pack_config_qualification.rs`), not a silent
//! flip. Target-slot resolution mirrors `resolve_transition_probe`'s own
//! source of truth for 77/87 verbs (`transition_args.target_workspace`/
//! `target_slot`, the field already confirmed correct in this session's R1
//! trace); the remaining 10 (no `transition_args` declared at all) are
//! resolved by the same manual/sibling-verb method the register doc used,
//! hardcoded here since there is no mechanical way to resolve them from the
//! verb's own declaration.

use std::{collections::BTreeMap, fs, path::PathBuf};

fn rust_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn yaml_files_recursive(dir: &PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else { continue };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if matches!(path.extension().and_then(|e| e.to_str()), Some("yaml" | "yml")) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Manual resolution for the 10 verbs with no `transition_args` block —
/// same targets the register doc derived from domain-name/sibling-verb
/// context. Hardcoded because there's no mechanical derivation for these;
/// if any of these 10 verbs gains a real `transition_args` block, this map
/// becomes redundant for that verb (harmless — the mechanical path is
/// tried first).
fn manual_target_overrides() -> BTreeMap<&'static str, (&'static str, &'static str)> {
    [
        ("cbu.add-product", ("cbu", "cbu")),
        ("trade-gateway.activate-gateway", ("instrument_matrix", "trade_gateway")),
        ("trade-gateway.suspend-gateway", ("instrument_matrix", "trade_gateway")),
        ("entity-workstream.update-status", ("kyc", "entity_workstream")),
        ("entity-workstream.complete", ("kyc", "entity_workstream")),
        ("screening.bulk-refresh", ("kyc", "screening")),
        ("governance.submit-for-review", ("semos_maintenance", "changeset")),
        ("governance.record-review", ("semos_maintenance", "changeset")),
        ("service-resource.decommission", ("instrument_matrix", "service_resource")),
        ("trading-profile.create-draft", ("instrument_matrix", "trading_profile")),
    ]
    .into_iter()
    .collect()
}

fn is_free_text_escape(s: &str) -> bool {
    s.trim().eq_ignore_ascii_case("any non-terminal") || s.trim().starts_with('(')
}

#[test]
fn declared_states_are_real_states() {
    // --- Build the real per-(workspace, slot) state vocabulary from the DAGs ---
    let mut vocab: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for path in yaml_files_recursive(&rust_root().join("config/sem_os_seeds/dag_taxonomies")) {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(&path).expect("read dag file"))
                .unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
        let Some(ws) = doc.get("workspace").and_then(|v| v.as_str()) else { continue };
        let Some(slots) = doc.get("slots").and_then(|v| v.as_sequence()) else { continue };
        for slot in slots {
            let Some(slot_id) = slot.get("id").and_then(|v| v.as_str()) else { continue };
            let mut states = Vec::new();
            if let Some(sm) = slot.get("state_machine") {
                if let Some(state_list) = sm.get("states").and_then(|v| v.as_sequence()) {
                    for s in state_list {
                        if let Some(id) = s.get("id").and_then(|v| v.as_str()) {
                            states.push(id.to_string());
                        }
                    }
                }
            }
            if let Some(duals) = slot.get("dual_lifecycle").and_then(|v| v.as_sequence()) {
                for dl in duals {
                    if let Some(state_list) = dl.get("states").and_then(|v| v.as_sequence()) {
                        for s in state_list {
                            if let Some(id) = s.get("id").and_then(|v| v.as_str()) {
                                states.push(id.to_string());
                            }
                        }
                    }
                }
            }
            vocab.insert((ws.to_string(), slot_id.to_string()), states);
        }
    }

    // --- Walk every verb YAML, collect non-empty requires_states ---
    let overrides = manual_target_overrides();
    let mut wrong_vocabulary: Vec<String> = Vec::new();
    let mut free_text_escape: Vec<String> = Vec::new();

    for path in yaml_files_recursive(&rust_root().join("config/verbs")) {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(&path).expect("read verb file"))
                .unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
        let Some(domains) = doc.get("domains").and_then(|v| v.as_mapping()) else { continue };
        for (dom_key, dom_val) in domains {
            let Some(dom_name) = dom_key.as_str() else { continue };
            let Some(verbs) = dom_val.get("verbs").and_then(|v| v.as_mapping()) else { continue };
            for (verb_key, verb_val) in verbs {
                let Some(verb_name) = verb_key.as_str() else { continue };
                let fqn = format!("{dom_name}.{verb_name}");
                let Some(lifecycle) = verb_val.get("lifecycle") else { continue };
                let Some(requires) = lifecycle.get("requires_states").and_then(|v| v.as_sequence())
                else {
                    continue;
                };
                if requires.is_empty() {
                    continue;
                }
                let declared: Vec<String> = requires
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();

                // Resolve target (workspace, slot): transition_args first,
                // manual override second.
                let target = verb_val
                    .get("transition_args")
                    .and_then(|ta| {
                        let ws = ta.get("target_workspace")?.as_str()?.to_string();
                        let slot = ta.get("target_slot")?.as_str()?.to_string();
                        Some((ws, slot))
                    })
                    .or_else(|| {
                        overrides
                            .get(fqn.as_str())
                            .map(|(ws, slot)| (ws.to_string(), slot.to_string()))
                    });

                let Some(target) = target else { continue }; // genuinely unresolvable, skip (documented as UNKNOWN in the register)
                let Some(real_states) = vocab.get(&target) else { continue };

                let mut bad = Vec::new();
                let mut freetext = Vec::new();
                for s in &declared {
                    if real_states.contains(s) {
                        continue;
                    }
                    if is_free_text_escape(s) {
                        freetext.push(s.clone());
                    } else {
                        bad.push(s.clone());
                    }
                }
                if !bad.is_empty() {
                    wrong_vocabulary.push(fqn.clone());
                }
                if !freetext.is_empty() {
                    free_text_escape.push(fqn.clone());
                }
            }
        }
    }
    wrong_vocabulary.sort();
    wrong_vocabulary.dedup();
    free_text_escape.sort();
    free_text_escape.dedup();

    let expected_wrong_vocabulary = vec![
        "entity-workstream.complete",
        "evidence.mark-verified",
        "governance.publish",
        "governance.record-review",
        "kyc-case.approve",
        "kyc-case.reject",
        "screening.run",
        "service-resource.decommission",
        "service-resource.suspend",
        "trading-profile.create-draft",
    ];
    let expected_free_text_escape = vec![
        "entity-workstream.update-status",
        "kyc-case.close",
        "kyc-case.escalate",
        "kyc-case.refer",
        "kyc-case.reject",
        "red-flag.escalate",
    ];

    assert_eq!(
        wrong_vocabulary, expected_wrong_vocabulary,
        "WRONG_VOCABULARY set changed — this is the R3 register's pinned known-bad \
         set (docs/eop/EOP-PLAN-GAMEBOARD-001_R3-Declaration-Register_v0.1.md); \
         update both consciously together, do not just widen this list"
    );
    assert_eq!(
        free_text_escape, expected_free_text_escape,
        "FREE_TEXT_ESCAPE set changed — same register, same discipline"
    );

    // The RED assertion: the plan's real target. Currently false for 15 of
    // 87 verbs (10 WRONG_VOCABULARY ∪ 6 FREE_TEXT_ESCAPE, kyc-case.reject
    // in both) — left failing on purpose until R3 is ratified and encoded.
    assert!(
        wrong_vocabulary.is_empty() && free_text_escape.is_empty(),
        "R3 not yet ratified/encoded: {} verbs declare requires_states values \
         that are not real states in their target slot's vocabulary \
         (wrong_vocabulary={wrong_vocabulary:?}, free_text_escape={free_text_escape:?}). \
         See the R3 register doc for per-verb proposed corrections — this \
         assertion should flip only after Adam ratifies and Sonnet encodes \
         those corrections, not before."
        , wrong_vocabulary.len() + free_text_escape.len()
    );
}
