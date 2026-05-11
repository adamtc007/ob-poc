use ob_poc::acp_dag_semantic::{
    resolve_acp_dag_semantic_prompt, resolve_acp_dag_semantic_prompt_with_verified_envelopes,
    AcpDagSemanticStatus,
};
use proptest::prelude::*;
use serde::Deserialize;

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

    let session_input = source
        .split("async fn session_input")
        .nth(1)
        .expect("session_input handler should exist");
    let acp_first = session_input
        .find("try_route_supported_acp_prompt")
        .expect("session_input should call the ACP semantic route");
    let repl_fallback = session_input
        .find("try_route_through_repl")
        .expect("session_input should retain the REPL fallback");

    assert!(
        acp_first < repl_fallback,
        "ACP semantic routing must run before generic REPL fallback"
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

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

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
