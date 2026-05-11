use ob_poc::acp_dag_semantic::{resolve_acp_dag_semantic_prompt, AcpDagSemanticStatus};
use ob_poc::acp_runtime_context::{
    build_acp_runtime_context_projection, AcpRuntimeContextProjection, AcpRuntimeContextSource,
    ACP_RUNTIME_CONTEXT_FRESHNESS_POLICY_V1, ACP_RUNTIME_CONTEXT_REDACTION_POLICY_V1,
    ACP_RUNTIME_CONTEXT_SCHEMA_VERSION,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

const SLICE_2_FIXTURES_JSONL: &str =
    include_str!("../../todo/acp-pack-context-parity-gate-a/slice-2-fixtures-v1.jsonl");

#[derive(Debug, Deserialize)]
struct Slice2Fixture {
    id: String,
    group: String,
    pack_id: String,
    utterance: String,
    runtime_source_fixture: String,
    expected_pack: String,
    expected_outcome: String,
    expected_runtime_fields: Vec<String>,
    forbidden_runtime_fields: Vec<String>,
    expected_trace_fields: Vec<String>,
    expected_mutation_posture: String,
}

fn fixtures() -> Vec<Slice2Fixture> {
    SLICE_2_FIXTURES_JSONL
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(serde_json::from_str(trimmed).unwrap_or_else(|error| {
                    panic!(
                        "slice-2-fixtures-v1.jsonl line {} should parse: {}",
                        line_index + 1,
                        error
                    )
                }))
            }
        })
        .collect()
}

#[test]
fn slice2_fixture_set_is_frozen_at_expected_group_counts() {
    let fixtures = fixtures();
    let mut group_counts = BTreeMap::<String, usize>::new();
    for fixture in &fixtures {
        *group_counts.entry(fixture.group.clone()).or_default() += 1;
        assert_eq!(
            fixture.expected_mutation_posture, "no-mutation",
            "{} must not be an execution fixture",
            fixture.id
        );
    }

    assert_eq!(fixtures.len(), 31);
    assert_eq!(group_counts.get("S2-ONB"), Some(&8));
    assert_eq!(group_counts.get("S2-CBU"), Some(&5));
    assert_eq!(group_counts.get("S2-SRDEF"), Some(&5));
    assert_eq!(group_counts.get("S2-STALE"), Some(&4));
    assert_eq!(group_counts.get("S2-REDACT"), Some(&4));
    assert_eq!(group_counts.get("S2-GHOST"), Some(&5));
}

#[test]
fn slice2_runtime_projection_satisfies_non_ghost_fixtures() {
    for fixture in fixtures()
        .into_iter()
        .filter(|fixture| fixture.group != "S2-GHOST")
    {
        let projection = build_acp_runtime_context_projection(source_for_fixture(&fixture));

        assert_eq!(
            projection.schema_version, ACP_RUNTIME_CONTEXT_SCHEMA_VERSION,
            "{} schema version",
            fixture.id
        );
        assert_eq!(
            projection.pack_id, fixture.expected_pack,
            "{} pack",
            fixture.id
        );
        assert_eq!(
            projection.redaction_policy, ACP_RUNTIME_CONTEXT_REDACTION_POLICY_V1,
            "{} redaction policy",
            fixture.id
        );
        assert_eq!(
            projection.freshness_policy, ACP_RUNTIME_CONTEXT_FRESHNESS_POLICY_V1,
            "{} freshness policy",
            fixture.id
        );
        assert!(
            !projection.runtime_hash.is_empty(),
            "{} runtime hash",
            fixture.id
        );
        assert!(
            !projection.projection_hash.is_empty(),
            "{} projection hash",
            fixture.id
        );

        for expected_field in &fixture.expected_runtime_fields {
            assert!(
                projection.runtime_fields.contains_key(expected_field),
                "{} missing expected runtime field `{}` in {:?}",
                fixture.id,
                expected_field,
                projection.runtime_fields.keys().collect::<Vec<_>>()
            );
        }
        for forbidden_field in &fixture.forbidden_runtime_fields {
            assert_forbidden_field_absent(&fixture.id, &projection, forbidden_field);
        }
        for trace_field in &fixture.expected_trace_fields {
            assert_trace_field_present(&fixture.id, &projection, trace_field);
        }

        match fixture.expected_outcome.as_str() {
            "refusal" if fixture.group == "S2-STALE" => {
                assert!(
                    projection
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.code.starts_with("runtime_context_stale_")),
                    "{} stale fixture should carry stale diagnostic",
                    fixture.id
                );
                assert!(
                    !projection.verified,
                    "{} stale fixture should fail closed",
                    fixture.id
                );
            }
            "pending-question" if fixture.runtime_source_fixture == "rt_missing_source" => {
                assert!(
                    projection
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.code == "runtime_context_missing_source"),
                    "{} missing-source fixture should carry missing-source diagnostic",
                    fixture.id
                );
                assert!(
                    !projection.verified,
                    "{} missing-source fixture should fail closed",
                    fixture.id
                );
            }
            _ => {}
        }
    }
}

#[test]
fn slice2_ghost_fixtures_still_refuse_before_runtime_projection() {
    for fixture in fixtures()
        .into_iter()
        .filter(|fixture| fixture.group == "S2-GHOST")
    {
        let resolution = resolve_acp_dag_semantic_prompt(&fixture.utterance)
            .unwrap_or_else(|error| panic!("{} resolver should not error: {}", fixture.id, error))
            .unwrap_or_else(|| panic!("{} should produce structured refusal", fixture.id));

        assert_eq!(fixture.pack_id, "none", "{} fixture pack", fixture.id);
        assert_eq!(
            fixture.expected_pack, "none",
            "{} expected pack",
            fixture.id
        );
        assert_eq!(
            resolution.status,
            AcpDagSemanticStatus::Refused,
            "{}",
            fixture.id
        );
        assert!(resolution.draft_dsl.is_none(), "{} emitted DSL", fixture.id);
        assert!(
            !resolution.mutation_allowed,
            "{} allowed mutation",
            fixture.id
        );
    }
}

fn source_for_fixture(fixture: &Slice2Fixture) -> AcpRuntimeContextSource {
    let mut fields = BTreeMap::new();
    for expected_field in &fixture.expected_runtime_fields {
        match expected_field.as_str() {
            "redacted_count" | "blocked_field_codes" => {}
            field => {
                fields.insert(field.to_string(), value_for_field(field));
            }
        }
    }
    for forbidden_field in &fixture.forbidden_runtime_fields {
        fields.insert(
            forbidden_field.to_string(),
            value_for_forbidden_field(forbidden_field),
        );
    }

    let stale = fixture.runtime_source_fixture == "rt_stale_snapshot";
    let missing_source_codes = if fixture.runtime_source_fixture == "rt_missing_source" {
        vec!["runtime_source_unavailable".to_string()]
    } else {
        Vec::new()
    };
    let force_count_only = fixture.runtime_source_fixture == "rt_budget_breach";
    if force_count_only {
        fields.insert("active_srdef_count".to_string(), serde_json::json!(32));
        fields.insert("raw_extra_1".to_string(), serde_json::json!("blocked"));
        fields.insert("raw_extra_2".to_string(), serde_json::json!("blocked"));
    }

    AcpRuntimeContextSource {
        pack_id: fixture.expected_pack.clone(),
        session_id: Some(format!("session-{}", fixture.id)),
        snapshot_id: format!("snapshot-{}", fixture.id),
        snapshot_created_at: "2026-05-10T20:00:00Z".to_string(),
        source_refs: vec![format!("fixture:{}", fixture.runtime_source_fixture)],
        static_envelope_hash: format!("static-envelope-hash-{}", fixture.expected_pack),
        fields,
        stale,
        missing_source_codes,
        force_count_only,
        field_budget: Some(12),
    }
}

fn value_for_field(field: &str) -> Value {
    match field {
        field if field.ends_with("_count") => serde_json::json!(2),
        "count_only_projection" => serde_json::json!(true),
        "source_version_refs" => serde_json::json!(["source:v1", "source:v2"]),
        field if field.ends_with("_ids") => serde_json::json!(["id-1", "id-2"]),
        field if field.ends_with("_codes") => serde_json::json!(["code-1"]),
        "workbook_step_statuses" => serde_json::json!([
            {"step_id": "step-1", "status": "ready"},
            {"step_id": "step-2", "status": "blocked"}
        ]),
        "snapshot_id" => serde_json::json!("snapshot-source"),
        "run_sheet_cursor" => serde_json::json!(1),
        _ => serde_json::json!("allowed"),
    }
}

fn value_for_forbidden_field(field: &str) -> Value {
    serde_json::json!(format!("forbidden-value-for-{field}"))
}

fn assert_forbidden_field_absent(
    fixture_id: &str,
    projection: &AcpRuntimeContextProjection,
    forbidden_field: &str,
) {
    assert!(
        !projection.runtime_fields.contains_key(forbidden_field),
        "{} leaked forbidden field `{}`",
        fixture_id,
        forbidden_field
    );
    let serialized = serde_json::to_string(projection).expect("projection should serialize");
    assert!(
        !serialized.contains(&format!("forbidden-value-for-{forbidden_field}")),
        "{} leaked forbidden field value for `{}`",
        fixture_id,
        forbidden_field
    );
}

fn assert_trace_field_present(
    fixture_id: &str,
    projection: &AcpRuntimeContextProjection,
    trace_field: &str,
) {
    let present = match trace_field {
        "runtime_schema_version" => !projection.schema_version.is_empty(),
        "runtime_pack_id" => !projection.pack_id.is_empty(),
        "runtime_snapshot_id" => !projection.snapshot_id.is_empty(),
        "runtime_hash" => !projection.runtime_hash.is_empty(),
        "runtime_verified" => true,
        "runtime_redaction_policy" => !projection.redaction_policy.is_empty(),
        "runtime_freshness_policy" => !projection.freshness_policy.is_empty(),
        "static_envelope_hash" => !projection.static_envelope_hash.is_empty(),
        "projection_hash" => !projection.projection_hash.is_empty(),
        other => projection.runtime_fields.contains_key(other),
    };
    assert!(
        present,
        "{} missing trace field `{}`",
        fixture_id, trace_field
    );
}
