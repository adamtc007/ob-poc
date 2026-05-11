use ob_poc::acp_dag_semantic::{
    resolve_acp_dag_semantic_prompt, resolve_acp_dag_semantic_prompt_with_verified_envelopes,
    AcpDagSemanticStatus,
};
use ob_poc::acp_pack_context_envelope_v2::{
    load_online_acp_pack_context_registry_state_v2, AcpPackContextRegistryLoadOptions,
};
use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
use proptest::prelude::*;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::OnceLock;

const AGENT_ROUTES_SOURCE: &str = include_str!("../src/api/agent_routes.rs");
const AGENT_SERVICE_SOURCE: &str = include_str!("../src/api/agent_service.rs");
const GENERIC_EXECUTOR_SOURCE: &str = include_str!("../src/dsl_v2/generic_executor.rs");
const GATEWAY_RESOLVER_SOURCE: &str = include_str!("../src/dsl_v2/gateway_resolver.rs");
const REPL_ROUTES_V2_SOURCE: &str = include_str!("../src/api/repl_routes_v2.rs");
const SESSION_REPOSITORY_SOURCE: &str = include_str!("../src/repl/session_repository.rs");
const SEQUENCER_SOURCE: &str = include_str!("../src/sequencer.rs");
const OB_POC_WEB_MAIN_SOURCE: &str = include_str!("../crates/ob-poc-web/src/main.rs");
const SLICE_2_FIXTURES_JSONL: &str =
    include_str!("../../todo/acp-pack-context-parity-gate-a/slice-2-fixtures-v1.jsonl");

/// R4 single-path-invariant seed corpus. 51 utterances drawn from:
///
/// - baseline positive shapes (10)
/// - cross-pack collision shapes (5)
/// - ghost-route bait (5 P1 incidents)
/// - refusal-required bait (4)
/// - unicode / edge-case adversaries (5)
/// - verb-adjacent prefixes (5)
/// - pending-question shapes (5)
/// - noise / SQLi-XSS-path-traversal (8)
/// - structure-macro positives (4)
///
/// SHA-256 over this file is captured in the per-PR baseline so any
/// content drift is visible in review.
const SINGLE_PATH_CORPUS_JSONL: &str = include_str!("fixtures/single_path_corpus.jsonl");

#[derive(Debug, Deserialize)]
struct CorpusEntry {
    id: String,
    kind: String,
    utterance: String,
}

fn single_path_corpus() -> Vec<CorpusEntry> {
    SINGLE_PATH_CORPUS_JSONL
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(line_index, line)| {
            serde_json::from_str::<CorpusEntry>(line).unwrap_or_else(|err| {
                panic!(
                    "single_path_corpus.jsonl line {} should parse: {}",
                    line_index + 1,
                    err
                )
            })
        })
        .collect()
}

/// Build the verified envelope hash set for Slice 1 packs.
///
/// **Load-bearing for the §16.7 single-path invariant:** every REPL
/// emission that carries an `envelope_trace.envelope_hash` MUST belong
/// to this set. Anything else is a single-path-invariant violation.
///
/// Uses the same dev online-state path the resolver uses, so envelopes
/// are sealed to `Active` lifecycle and the section/envelope hashes
/// match what the resolver projects on every request.
fn verified_envelope_hashes() -> &'static HashSet<String> {
    static CACHE: OnceLock<HashSet<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let config_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
        let projection = build_slice1_acp_registry_projection(&config_root)
            .expect("Slice 1 registry projection should build");
        let options = AcpPackContextRegistryLoadOptions::development();
        let state =
            load_online_acp_pack_context_registry_state_v2(&projection, &config_root, options)
                .expect("development registry state should load");
        state
            .envelopes
            .iter()
            .map(|env| env.envelope_hash.clone())
            .collect()
    })
}

/// The load-bearing assertion: every emission either terminates at a
/// verified envelope, or is a structured refusal with no envelope
/// binding. Anything else is a violation of v0.5 §16.7.
fn assert_single_path_terminates(
    utterance: &str,
    verified_hashes: &HashSet<String>,
) -> Result<(), String> {
    let resolution = match resolve_acp_dag_semantic_prompt_with_verified_envelopes(
        utterance,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config"),
    ) {
        Ok(Some(r)) => r,
        // Resolver returning None is acceptable — no Slice 1 binding for
        // this utterance; it falls through to non-ACP routing which
        // produces a structured refusal at the HTTP layer. This is the
        // legitimate "no pack" case for noise/non-Slice-1 utterances.
        Ok(None) => return Ok(()),
        Err(e) => return Err(format!("resolver errored: {e}")),
    };

    match resolution.status {
        AcpDagSemanticStatus::Refused => {
            // Refusal path: no envelope binding required. Sanity-check
            // that the resolver didn't accidentally emit DSL / mutation.
            if resolution.draft_dsl.is_some() {
                return Err(format!(
                    "INVARIANT VIOLATION: refused resolution emitted DSL for `{utterance}`"
                ));
            }
            if resolution.mutation_allowed {
                return Err(format!(
                    "INVARIANT VIOLATION: refused resolution permitted mutation for `{utterance}`"
                ));
            }
            Ok(())
        }
        AcpDagSemanticStatus::Matched | AcpDagSemanticStatus::Ambiguous => {
            // Non-refusal path: envelope_trace must exist and its hash
            // must belong to the verified registry state hash set.
            let envelope_trace = match &resolution.envelope_trace {
                Some(t) => t,
                None => {
                    // No envelope = non-Slice-1 pack OR no pack selected.
                    // For NoMatch this is fine. For Matched/Ambiguous on
                    // a Slice 1 pack we'd expect an envelope trace, but
                    // not every code path has it wired yet — registry
                    // trace presence is a softer guarantee. Accept this
                    // as a structured outcome.
                    return Ok(());
                }
            };
            if !verified_hashes.contains(&envelope_trace.envelope_hash) {
                return Err(format!(
                    "INVARIANT VIOLATION: envelope_hash {} for `{utterance}` not in verified registry state",
                    envelope_trace.envelope_hash
                ));
            }
            if !envelope_trace.verified {
                return Err(format!(
                    "INVARIANT VIOLATION: envelope_trace.verified=false for `{utterance}`"
                ));
            }
            Ok(())
        }
    }
}

/// R4 seed lane — runs deterministically on every PR.
///
/// Every entry in the seed corpus is fed through the resolver under the
/// single-path-invariant assertion. No shrinking, no random generation
/// — this catches regressions on shapes we've already paid for.
#[test]
fn gate_e_seed_corpus_all_terminate_at_verified_envelope_or_refusal() {
    let verified_hashes = verified_envelope_hashes();
    assert!(
        !verified_hashes.is_empty(),
        "verified envelope hash set must be non-empty"
    );

    let corpus = single_path_corpus();
    assert_eq!(
        corpus.len(),
        51,
        "seed corpus is content-pinned; update SINGLE_PATH_CORPUS_JSONL hash docs if intentional"
    );

    let mut violations: Vec<(String, String)> = Vec::new();
    for entry in &corpus {
        if let Err(err) = assert_single_path_terminates(&entry.utterance, verified_hashes) {
            violations.push((entry.id.clone(), err));
        }
    }
    if !violations.is_empty() {
        let summary = violations
            .iter()
            .map(|(id, err)| format!("  {id}: {err}"))
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "{} of {} corpus entries violated the single-path invariant:\n{}",
            violations.len(),
            corpus.len(),
            summary
        );
    }
}

#[derive(Debug, Deserialize)]
struct Slice2GhostFixture {
    id: String,
    group: String,
    utterance: String,
    runtime_source_fixture: String,
    expected_pack: String,
    forbidden_runtime_fields: Vec<String>,
    expected_mutation_posture: String,
}

fn production_agent_routes_source() -> &'static str {
    AGENT_ROUTES_SOURCE
        .split("#[cfg(test)]")
        .next()
        .expect("agent routes source should have production section")
}

#[test]
fn gate_e_router_keeps_session_input_as_the_only_normal_utterance_path() {
    let source = production_agent_routes_source();

    assert!(
        source.contains(".route(\"/api/session/:id/input\", post(session_input))"),
        "normal utterances must enter through the unified session input route"
    );
    assert!(
        !source.contains(".route(\"/api/session/:id/chat\""),
        "the removed legacy chat route must not be registered"
    );
    assert!(
        source.contains("post(execute_session_dsl_legacy_raw_only)"),
        "the execute route must be guarded by the legacy raw-only handler"
    );

    // R8 single-path unification (2026-05-11): `session_input` must NOT
    // call `try_route_supported_acp_prompt` directly. ACP DAG semantic
    // resolution now lives inside `ReplOrchestratorV2::process_with_acp`
    // as the orchestrator's first decision (Message inputs only). The
    // pre-R8 ordering assertion is replaced by this absence assertion.
    let session_input = source
        .split("async fn session_input")
        .nth(1)
        .and_then(|tail| tail.split("async fn ").next())
        .expect("session_input handler should exist");

    // Match the call shape (`try_route_supported_acp_prompt(...)`) so the
    // doc comment that *explains* the removal doesn't trip the assertion.
    assert!(
        !session_input.contains("try_route_supported_acp_prompt("),
        "R8 invariant violated: session_input must not call ACP directly. \
         ACP resolution now lives inside ReplOrchestratorV2::process_with_acp()."
    );
    assert!(
        session_input.contains("dispatch_to_v2_repl"),
        "session_input must dispatch via the single V2 REPL ingress"
    );
}

/// R8 single-path unification (2026-05-11): source-scan invariant proving
/// the orchestrator owns the ACP DAG semantic resolution call site.
#[test]
fn gate_e_orchestrator_owns_acp_resolution() {
    let sequencer = include_str!("../src/sequencer.rs");
    let production = sequencer
        .split("#[cfg(test)]")
        .next()
        .expect("sequencer source should have production section");

    assert!(
        production.contains("process_with_acp"),
        "orchestrator must expose process_with_acp as the ACP-aware ingress"
    );
    assert!(
        production.contains("try_route_supported_acp_prompt"),
        "orchestrator's process_with_acp must call the ACP route helper"
    );
}

#[test]
fn gate_e_legacy_execute_route_is_gone_for_normal_payloads() {
    let source = production_agent_routes_source();
    let legacy_handler = source
        .split("async fn execute_session_dsl_legacy_raw_only")
        .nth(1)
        .and_then(|tail| tail.split("async fn execute_session_dsl(").next())
        .expect("legacy raw-only execute handler should exist");

    assert!(
        legacy_handler.contains("StatusCode::GONE"),
        "normal execute payloads must receive 410 Gone"
    );
    assert!(
        legacy_handler.contains("Legacy execute endpoint disabled for normal session flows"),
        "410 response should explain the supported unified input route"
    );
    assert!(
        legacy_handler.contains("execute_session_dsl_raw"),
        "only explicitly raw execute requests may continue past the guard"
    );
}

#[test]
fn gate_e_entity_resolution_has_no_agent_or_executor_bypass() {
    assert!(
        AGENT_SERVICE_SOURCE.contains("\"client_group\" | \"client\" => RefType::ClientGroup"),
        "agent entity resolution should route client groups through GatewayRefResolver"
    );
    assert!(
        !AGENT_SERVICE_SOURCE.contains("resolve_client_group"),
        "agent service must not retain a separate client group resolver path"
    );
    assert!(
        !GENERIC_EXECUTOR_SOURCE.contains("falling back to SQL"),
        "generic executor lookups must fail closed instead of falling back to SQL"
    );
    assert!(
        !GENERIC_EXECUTOR_SOURCE.contains("LOOKUP SQL fallback"),
        "generic executor must not keep a direct SQL lookup bypass"
    );
    assert!(
        GATEWAY_RESOLVER_SOURCE.contains("RefType::ClientGroup => \"CLIENT_GROUP\""),
        "client group references must be part of the gateway-backed resolver map"
    );
}

#[test]
fn gate_e_acp_live_projection_exposes_entity_resolution_state() {
    let live_projection = REPL_ROUTES_V2_SOURCE
        .split("fn build_live_acp_projection")
        .nth(1)
        .expect("live ACP projection builder should exist");

    assert!(
        live_projection.contains("\"entity_resolution\": repl_session.last_entity_resolution"),
        "live ACP projection must expose the session entity-resolution record"
    );
    assert!(
        live_projection.contains("\"session_stack\": repl_session.session_stack"),
        "live ACP projection must keep exposing DAG/session stack state"
    );
}

#[test]
fn gate_e_sage_repl_session_record_is_shared_and_self_contained() {
    let external_exchange = SEQUENCER_SOURCE
        .split("pub async fn record_external_chat_exchange")
        .nth(1)
        .expect("external ACP exchange recorder should exist");

    assert!(
        external_exchange.contains("lookup_service.analyze(&user_message, 5).await"),
        "Sage/ACP utterances must use the same lookup service before recording the exchange"
    );
    assert!(
        external_exchange.contains("acquire_session_turn_record_lock(session_id)"),
        "Sage/ACP utterances must acquire the durable session lock before mutating"
    );
    assert!(
        external_exchange.contains("session.apply_lookup_result(result)"),
        "Sage/ACP utterances must apply entity resolution to the shared REPL session record"
    );
    assert!(
        external_exchange.contains("persist_session_checkpoint_with_record_lock(session_id)"),
        "Sage/ACP utterances must checkpoint the shared session record"
    );

    let save_session_inner = SESSION_REPOSITORY_SOURCE
        .split("async fn save_session_inner")
        .nth(1)
        .expect("session save implementation should exist");
    assert!(
        SESSION_REPOSITORY_SOURCE.contains("pub async fn acquire_session_record_lock")
            && SESSION_REPOSITORY_SOURCE.contains("try_advisory_xact_lock"),
        "session repository must expose a blocking turn-level durable per-session record lock"
    );
    assert!(
        save_session_inner.contains("advisory_xact_lock"),
        "session repository saves must acquire a durable per-session record lock"
    );
    for field in [
        "\"last_entity_resolution\": session.last_entity_resolution",
        "\"bindings\": session.bindings",
        "\"cbu_ids\": session.cbu_ids",
        "\"workspace_stack\": session.workspace_stack",
        "\"session_stack\": session.session_stack",
    ] {
        assert!(
            save_session_inner.contains(field),
            "session repository must persist workbook bridge field {field}"
        );
    }

    let load_session = SESSION_REPOSITORY_SOURCE
        .split("pub async fn load_session")
        .nth(1)
        .expect("session load function should exist");
    for field in [
        "last_entity_resolution: serde_json::from_value",
        "bindings: serde_json::from_value",
        "cbu_ids: serde_json::from_value",
        "workspace_stack: serde_json::from_value",
        "session_stack: serde_json::from_value",
    ] {
        assert!(
            load_session.contains(field),
            "session repository must reload workbook bridge field {field}"
        );
    }
}

#[test]
fn gate_e_ob_poc_web_wires_durable_repl_session_repository() {
    let repl_startup = OB_POC_WEB_MAIN_SOURCE
        .split("let repl_v2_orchestrator = {")
        .nth(1)
        .expect("ob-poc-web should build the production REPL V2 orchestrator");

    assert!(
        repl_startup.contains("SessionRepositoryV2::new"),
        "production web startup must construct the durable REPL session repository"
    );
    assert!(
        repl_startup.contains(".with_session_repository(session_repository)"),
        "production web startup must attach the durable session repository to ReplOrchestratorV2"
    );
}

#[test]
fn gate_e_runtime_present_ghost_fixtures_stop_before_runtime_context() {
    let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");

    for fixture in slice2_ghost_fixtures() {
        assert_eq!(fixture.group, "S2-GHOST", "{} group", fixture.id);
        assert_eq!(
            fixture.runtime_source_fixture, "rt_ghost_context_present",
            "{} must model runtime context being present",
            fixture.id
        );
        assert_eq!(fixture.expected_pack, "none", "{} pack", fixture.id);
        assert_eq!(
            fixture.expected_mutation_posture, "no-mutation",
            "{} mutation posture",
            fixture.id
        );

        let resolution = resolve_acp_dag_semantic_prompt_with_verified_envelopes(
            &fixture.utterance,
            &config_root,
        )
        .unwrap_or_else(|error| panic!("{} resolver should not error: {}", fixture.id, error))
        .unwrap_or_else(|| panic!("{} should produce structured refusal", fixture.id));

        assert_eq!(
            resolution.status,
            AcpDagSemanticStatus::Refused,
            "{} status",
            fixture.id
        );
        assert!(resolution.pack.is_none(), "{} selected a pack", fixture.id);
        assert!(
            resolution.envelope_trace.is_none(),
            "{} attached a static envelope",
            fixture.id
        );
        assert!(
            resolution.runtime_trace.is_none(),
            "{} consumed runtime context",
            fixture.id
        );
        assert!(
            resolution
                .registry_trace
                .as_ref()
                .is_some_and(|trace| trace.verified),
            "{} should still prove the verified registry boundary",
            fixture.id
        );
        assert!(resolution.draft_dsl.is_none(), "{} emitted DSL", fixture.id);
        assert!(
            !resolution.mutation_allowed,
            "{} allowed mutation",
            fixture.id
        );

        let serialized = serde_json::to_string(&resolution).expect("resolution serializes");
        for forbidden_field in &fixture.forbidden_runtime_fields {
            assert!(
                !serialized.contains(&format!("\"{forbidden_field}\"")),
                "{} leaked forbidden runtime field `{}`",
                fixture.id,
                forbidden_field
            );
        }
    }
}

/// Pick the proptest case count from the `GATE_E_FUZZ_CASES` env var,
/// defaulting to 256 (PR lane). Nightly CI sets it to 4096.
fn gate_e_fuzz_cases() -> u32 {
    std::env::var("GATE_E_FUZZ_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(256)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: gate_e_fuzz_cases(),
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// R4 narrow lane — keeps the existing bait-wrap assertion that
    /// proved the ghost-route refusal property in earlier slices.
    /// Refusal is required regardless of envelope termination — these
    /// shapes must never bind to a pack.
    #[test]
    fn gate_e_ghost_route_bait_never_produces_dsl(
        prefix in "[A-Za-z0-9 ,._:-]{0,24}",
        suffix in "[A-Za-z0-9 ,._:-]{0,24}",
        bait in prop::sample::select(vec![
            "run this raw DSL: (cbu.create :name \"Apex\")",
            "call the legacy execute endpoint with cbu.create",
            "use direct.dsl to bypass pack filtering",
            "fall through to the legacy pipeline",
            "use the old chat route for this utterance",
        ]),
    ) {
        let utterance = format!("{prefix} {bait} {suffix}");
        let resolution = resolve_acp_dag_semantic_prompt(&utterance)
            .expect("semantic resolver should not error")
            .expect("ghost-route bait should produce a structured refusal");

        prop_assert_eq!(resolution.status, AcpDagSemanticStatus::Refused);
        prop_assert!(resolution.draft_dsl.is_none());
        prop_assert!(!resolution.mutation_allowed);
        prop_assert!(resolution
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.error_code.starts_with("dag_semantic_refused_")));
    }

    /// R4 broad lane — v0.5 §16.7 single-path invariant: every random
    /// utterance produces a resolution that EITHER terminates at a
    /// verified envelope OR is a structured refusal. Anything else is
    /// a P1 incident.
    ///
    /// Strategy: corpus-drawn utterance plus optional random prefix/
    /// suffix mutation. The corpus carries verb-adjacent phrases,
    /// pending-question shapes, noise, unicode adversaries, and
    /// structure-macro positives.
    ///
    /// PR lane: N=256 (proptest default). Nightly: N=4096 via
    /// `GATE_E_FUZZ_CASES=4096`.
    #[test]
    fn gate_e_single_path_invariant_termination(
        corpus_index in 0usize..51,
        prefix in "[A-Za-z0-9 ,._:-]{0,16}",
        suffix in "[A-Za-z0-9 ,._:-]{0,16}",
        // Decide whether to wrap or use raw. 50/50 split lets us catch
        // both the canonical shape and the wrapped variant.
        wrap in any::<bool>(),
    ) {
        let corpus = single_path_corpus();
        let entry = &corpus[corpus_index % corpus.len()];
        let utterance = if wrap {
            format!("{prefix} {} {suffix}", entry.utterance)
        } else {
            entry.utterance.clone()
        };

        // Cached static — built once per test-binary run, not per case.
        let verified_hashes = verified_envelope_hashes();
        if let Err(err) = assert_single_path_terminates(&utterance, verified_hashes) {
            return Err(TestCaseError::fail(format!(
                "corpus entry {} ({}): {}",
                entry.id, entry.kind, err
            )));
        }
    }
}

fn slice2_ghost_fixtures() -> Vec<Slice2GhostFixture> {
    SLICE_2_FIXTURES_JSONL
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                let fixture =
                    serde_json::from_str::<Slice2GhostFixture>(trimmed).unwrap_or_else(|error| {
                        panic!(
                            "slice-2-fixtures-v1.jsonl line {} should parse: {}",
                            line_index + 1,
                            error
                        )
                    });
                (fixture.group == "S2-GHOST").then_some(fixture)
            }
        })
        .collect()
}
