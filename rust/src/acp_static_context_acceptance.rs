//! Slice 1 static context acceptance checks for ACP pack context envelopes.
//!
//! This module is the first Gate E harness boundary. It validates that the
//! static projection and active envelope registry cover the frozen Slice 1
//! fixture set before any runtime Sage rerun is treated as an acceptance signal.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::acp_pack_context_envelope_v2::{
    load_online_acp_pack_context_registry_state_v2, verify_acp_pack_context_envelope_v2,
    verify_acp_pack_context_registry_state_v2, AcpPackContextEnvelopeV2,
    AcpPackContextRegistryLoadOptions, AcpPackContextRegistryStateV2, AcpPackLifecycleState,
    ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION,
};
use crate::acp_registry_projection::{
    build_slice1_acp_registry_projection, AcpExecutionPolicyProjection, AcpRegistryPackProjection,
    AcpRegistryProjection, SLICE_1_ACP_PACK_IDS,
};

const REQUIRED_ENVELOPE_SECTIONS: &[&str] = &[
    "diagnostic_taxonomy",
    "macro_tiers",
    "pack_summary",
    "production_contracts",
    "verb_bindings",
    "verb_effects",
    "workbook_plans",
];

/// Result of evaluating the Slice 1 static-context acceptance harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcpStaticContextAcceptanceReport {
    pub passed: bool,
    pub fixture_count: usize,
    pub passed_check_count: usize,
    pub failed_check_count: usize,
    pub checks: Vec<AcpStaticContextAcceptanceCheck>,
}

/// One acceptance assertion emitted by the static-context harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcpStaticContextAcceptanceCheck {
    pub code: String,
    pub status: AcpStaticContextAcceptanceStatus,
    pub message: String,
    pub fixture_id: Option<String>,
    pub pack_id: Option<String>,
}

/// Pass/fail status for one static-context acceptance check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpStaticContextAcceptanceStatus {
    Passed,
    Failed,
}

#[derive(Debug, Deserialize)]
struct BaselineFixture {
    id: String,
    category: String,
    expected_pack: String,
    expected_macro_or_template: String,
    expected_verb: String,
    expected_outcome: String,
}

/// Build and evaluate the Slice 1 static-context acceptance report from paths.
///
/// This function builds the current Slice 1 registry projection, loads the
/// development online envelope registry state, verifies active-envelope
/// immutability, and evaluates the frozen baseline fixtures against static
/// pack, verb, macro, template, and refusal surfaces.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_static_context_acceptance::evaluate_slice1_static_context_acceptance;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
///     .join("../todo/acp-pack-context-parity-gate-a/baseline-fixtures-v1.jsonl");
/// let report = evaluate_slice1_static_context_acceptance(config_root, fixtures).unwrap();
/// assert!(report.passed);
/// ```
pub fn evaluate_slice1_static_context_acceptance(
    config_root: impl AsRef<Path>,
    fixtures_path: impl AsRef<Path>,
) -> Result<AcpStaticContextAcceptanceReport> {
    let fixtures_jsonl = fs::read_to_string(fixtures_path.as_ref()).with_context(|| {
        format!(
            "reading Slice 1 baseline fixtures from {}",
            fixtures_path.as_ref().display()
        )
    })?;
    evaluate_slice1_static_context_acceptance_from_jsonl(config_root, &fixtures_jsonl)
}

/// Build and evaluate the Slice 1 static-context acceptance report from JSONL.
///
/// This variant is useful for tests that mutate fixture rows while still using
/// the live projection and verified development envelope registry.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_static_context_acceptance::evaluate_slice1_static_context_acceptance_from_jsonl;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let fixture = r#"{"id":"F001","category":"onboarding-request","expected_pack":"onboarding-request","expected_macro_or_template":"workflow-plan","expected_verb":"onboarding.compile-data-request","expected_outcome":"workflow-plan"}"#;
/// let report = evaluate_slice1_static_context_acceptance_from_jsonl(config_root, fixture).unwrap();
/// assert!(report.passed);
/// ```
pub fn evaluate_slice1_static_context_acceptance_from_jsonl(
    config_root: impl AsRef<Path>,
    fixtures_jsonl: &str,
) -> Result<AcpStaticContextAcceptanceReport> {
    let config_root = config_root.as_ref();
    let projection = build_slice1_acp_registry_projection(config_root)
        .with_context(|| format!("building Slice 1 projection from {}", config_root.display()))?;
    let registry_state = load_online_acp_pack_context_registry_state_v2(
        &projection,
        config_root,
        AcpPackContextRegistryLoadOptions::development(),
    )
    .map_err(|refusal| {
        anyhow::anyhow!(
            "loading verified development ACP pack context registry failed: {} ({})",
            refusal.code,
            refusal.message
        )
    })?;
    verify_acp_pack_context_registry_state_v2(&registry_state, &projection, config_root).map_err(
        |refusal| {
            anyhow::anyhow!(
                "verifying ACP pack context registry state failed: {} ({})",
                refusal.code,
                refusal.message
            )
        },
    )?;
    evaluate_projection_and_registry_state(&projection, &registry_state, fixtures_jsonl)
}

fn evaluate_projection_and_registry_state(
    projection: &AcpRegistryProjection,
    registry_state: &AcpPackContextRegistryStateV2,
    fixtures_jsonl: &str,
) -> Result<AcpStaticContextAcceptanceReport> {
    let fixtures = parse_fixtures(fixtures_jsonl)?;
    let mut checks = Vec::new();
    check_projection_surface(projection, &mut checks);
    check_registry_state_surface(projection, registry_state, &mut checks);
    check_fixture_surface(projection, &fixtures, &mut checks);

    Ok(report_from_checks(fixtures.len(), checks))
}

fn report_from_checks(
    fixture_count: usize,
    checks: Vec<AcpStaticContextAcceptanceCheck>,
) -> AcpStaticContextAcceptanceReport {
    let passed_check_count = checks
        .iter()
        .filter(|check| check.status == AcpStaticContextAcceptanceStatus::Passed)
        .count();
    let failed_check_count = checks.len() - passed_check_count;
    AcpStaticContextAcceptanceReport {
        passed: failed_check_count == 0,
        fixture_count,
        passed_check_count,
        failed_check_count,
        checks,
    }
}

fn parse_fixtures(fixtures_jsonl: &str) -> Result<Vec<BaselineFixture>> {
    fixtures_jsonl
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(
                    serde_json::from_str::<BaselineFixture>(trimmed).with_context(|| {
                        format!("parsing baseline fixture JSONL line {}", index + 1)
                    }),
                )
            }
        })
        .collect()
}

fn check_projection_surface(
    projection: &AcpRegistryProjection,
    checks: &mut Vec<AcpStaticContextAcceptanceCheck>,
) {
    let projected_ids = projection
        .packs
        .iter()
        .map(|pack| pack.pack_id.as_str())
        .collect::<BTreeSet<_>>();
    let required_ids = SLICE_1_ACP_PACK_IDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    push_check(
        checks,
        "slice1_pack_set_complete",
        projected_ids == required_ids,
        format!(
            "Slice 1 projection exposes {} required packs",
            SLICE_1_ACP_PACK_IDS.len()
        ),
        None,
        None,
    );
    push_check(
        checks,
        "slice1_projection_no_diagnostics",
        projection.diagnostics.is_empty(),
        "Slice 1 projection has no blocking diagnostics",
        None,
        None,
    );

    let taxonomy_codes = projection
        .diagnostic_taxonomy
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();
    for required_code in [
        "acp_ambiguous_pack",
        "acp_forbidden_verb",
        "acp_legacy_route_bait",
        "acp_missing_binding",
        "acp_unsupported_macro_tier",
    ] {
        push_check(
            checks,
            "diagnostic_taxonomy_code_present",
            taxonomy_codes.contains(required_code),
            format!("Diagnostic taxonomy contains {required_code}"),
            None,
            None,
        );
    }
}

fn check_registry_state_surface(
    projection: &AcpRegistryProjection,
    registry_state: &AcpPackContextRegistryStateV2,
    checks: &mut Vec<AcpStaticContextAcceptanceCheck>,
) {
    push_check(
        checks,
        "registry_state_projection_hash_matches",
        registry_state.source_projection_hash == projection.projection_hash,
        "Registry state is pinned to the active projection hash",
        None,
        None,
    );
    push_check(
        checks,
        "registry_state_pack_count_matches",
        registry_state.pack_count == projection.pack_count
            && registry_state.envelopes.len() == projection.pack_count,
        "Registry state contains one active envelope per projected pack",
        None,
        None,
    );

    let pack_index = projection
        .packs
        .iter()
        .map(|pack| (pack.pack_id.as_str(), pack))
        .collect::<BTreeMap<_, _>>();
    for envelope in &registry_state.envelopes {
        check_envelope_surface(&pack_index, projection, envelope, checks);
    }
}

fn check_envelope_surface(
    pack_index: &BTreeMap<&str, &AcpRegistryPackProjection>,
    projection: &AcpRegistryProjection,
    envelope: &AcpPackContextEnvelopeV2,
    checks: &mut Vec<AcpStaticContextAcceptanceCheck>,
) {
    let pack_id = envelope.body.pack_id.as_str();
    push_check(
        checks,
        "envelope_schema_version",
        envelope.schema_version == ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION,
        "Envelope schema is v2",
        None,
        Some(pack_id),
    );
    push_check(
        checks,
        "envelope_signature_verified",
        verify_acp_pack_context_envelope_v2(envelope).is_ok(),
        "Envelope hash and signature verify against the development keyring",
        None,
        Some(pack_id),
    );
    push_check(
        checks,
        "envelope_active_lifecycle",
        envelope.body.lifecycle == AcpPackLifecycleState::Active,
        "Envelope lifecycle is active",
        None,
        Some(pack_id),
    );
    push_check(
        checks,
        "envelope_projection_hash_matches",
        envelope.body.build_inputs.source_projection_hash == projection.projection_hash,
        "Envelope build inputs pin the active projection hash",
        None,
        Some(pack_id),
    );
    push_check(
        checks,
        "envelope_no_budget_omissions",
        envelope.body.budget.omitted.is_empty(),
        "Envelope includes all required static sections without budget omissions",
        None,
        Some(pack_id),
    );
    push_check(
        checks,
        "envelope_content_hash_chain_complete",
        envelope.body.content_hash_chain.len() == envelope.body.section_hashes.len()
            && !envelope.body.content_hash_chain.is_empty(),
        "Envelope carries a complete section content hash chain",
        None,
        Some(pack_id),
    );
    for section in REQUIRED_ENVELOPE_SECTIONS {
        push_check(
            checks,
            "envelope_required_section_hash_present",
            envelope.body.section_hashes.contains_key(*section),
            format!("Envelope section hash exists for {section}"),
            None,
            Some(pack_id),
        );
    }

    if let Some(pack) = pack_index.get(pack_id) {
        push_check(
            checks,
            "envelope_pack_summary_fidelity",
            envelope.body.sections.pack_summary.get("pack_id").and_then(serde_json::Value::as_str)
                == Some(pack.pack_id.as_str())
                && envelope
                    .body
                    .sections
                    .pack_summary
                    .get("manifest_hash")
                    .is_none(),
            "Envelope pack summary is pack-scoped and avoids duplicating unneeded manifest internals",
            None,
            Some(pack_id),
        );
        push_check(
            checks,
            "envelope_verb_binding_fidelity",
            serde_json::to_value(&pack.verb_bindings).ok()
                == Some(envelope.body.sections.verb_bindings.clone()),
            "Envelope verb binding section matches the registry projection",
            None,
            Some(pack_id),
        );
        push_check(
            checks,
            "envelope_verb_effect_fidelity",
            serde_json::to_value(&pack.verb_effects).ok()
                == Some(envelope.body.sections.verb_effects.clone()),
            "Envelope verb effect section matches the registry projection",
            None,
            Some(pack_id),
        );
        push_check(
            checks,
            "envelope_macro_tier_fidelity",
            serde_json::to_value(&pack.macro_tiers).ok()
                == Some(envelope.body.sections.macro_tiers.clone()),
            "Envelope macro tier section matches the registry projection",
            None,
            Some(pack_id),
        );
    } else {
        push_check(
            checks,
            "envelope_pack_registered",
            false,
            "Envelope pack id is registered in the projection",
            None,
            Some(pack_id),
        );
    }
}

fn check_fixture_surface(
    projection: &AcpRegistryProjection,
    fixtures: &[BaselineFixture],
    checks: &mut Vec<AcpStaticContextAcceptanceCheck>,
) {
    push_check(
        checks,
        "fixture_set_loaded",
        !fixtures.is_empty(),
        "Baseline fixture JSONL loaded at least one fixture",
        None,
        None,
    );
    push_check(
        checks,
        "fixture_set_expected_size",
        fixtures.len() == 36,
        "Baseline fixture JSONL contains the frozen 36 Slice 1 fixtures",
        None,
        None,
    );

    let pack_index = projection
        .packs
        .iter()
        .map(|pack| (pack.pack_id.as_str(), pack))
        .collect::<BTreeMap<_, _>>();

    for fixture in fixtures {
        if fixture.expected_pack == "none" {
            check_no_pack_fixture(fixture, checks);
            continue;
        }

        let pack = pack_index.get(fixture.expected_pack.as_str()).copied();
        push_check(
            checks,
            "fixture_expected_pack_projected",
            pack.is_some(),
            "Fixture expected pack exists in the Slice 1 projection",
            Some(&fixture.id),
            Some(&fixture.expected_pack),
        );
        let Some(pack) = pack else {
            continue;
        };

        push_check(
            checks,
            "fixture_expected_verb_or_macro_projected",
            fixture_verb_or_macro_covered(pack, fixture.expected_verb.as_str()),
            "Fixture expected verb or macro is projected by the expected pack",
            Some(&fixture.id),
            Some(&fixture.expected_pack),
        );
        push_check(
            checks,
            "fixture_expected_macro_or_template_projected",
            fixture_macro_or_template_covered(
                pack,
                fixture.expected_macro_or_template.as_str(),
                fixture.expected_outcome.as_str(),
            ),
            "Fixture expected macro, template, or workflow surface is projected",
            Some(&fixture.id),
            Some(&fixture.expected_pack),
        );
        push_check(
            checks,
            "fixture_expected_outcome_policy_projected",
            fixture_outcome_policy_covered(pack, fixture),
            "Fixture expected outcome has a static pack policy or binding surface",
            Some(&fixture.id),
            Some(&fixture.expected_pack),
        );
    }
}

fn check_no_pack_fixture(
    fixture: &BaselineFixture,
    checks: &mut Vec<AcpStaticContextAcceptanceCheck>,
) {
    push_check(
        checks,
        "fixture_no_pack_refusal_shape",
        fixture.category == "ghost-route-bait"
            && fixture.expected_verb == "none"
            && fixture.expected_macro_or_template == "none"
            && fixture.expected_outcome == "refusal",
        "No-pack fixtures are explicit ghost-route refusals with no executable verb surface",
        Some(&fixture.id),
        None,
    );
}

fn fixture_verb_or_macro_covered(pack: &AcpRegistryPackProjection, expected: &str) -> bool {
    expected == "none"
        || pack.allowed_verbs.iter().any(|verb| verb == expected)
        || pack.forbidden_verbs.iter().any(|verb| verb == expected)
        || pack
            .macro_tiers
            .iter()
            .any(|tier| tier.macro_id == expected)
}

fn fixture_macro_or_template_covered(
    pack: &AcpRegistryPackProjection,
    expected: &str,
    expected_outcome: &str,
) -> bool {
    if expected == "none" {
        return true;
    }
    if expected == "workflow-plan" {
        return expected_outcome == "workflow-plan" && !pack.workbook_plans.is_empty();
    }
    pack.workbook_plans
        .iter()
        .any(|plan| plan.template_id == expected || plan.plan_id.ends_with(expected))
        || pack
            .macro_tiers
            .iter()
            .any(|tier| tier.macro_id == expected)
}

fn fixture_outcome_policy_covered(
    pack: &AcpRegistryPackProjection,
    fixture: &BaselineFixture,
) -> bool {
    match fixture.expected_outcome.as_str() {
        "dsl-draft" | "workflow-plan" => true,
        "pending-question" => {
            pack_has_pending_question_surface(pack, fixture.expected_verb.as_str())
        }
        "refusal" => pack_has_refusal_surface(pack, fixture.expected_verb.as_str()),
        other => !other.trim().is_empty(),
    }
}

fn pack_has_pending_question_surface(
    pack: &AcpRegistryPackProjection,
    expected_verb: &str,
) -> bool {
    if !pack.required_questions.is_empty() {
        return true;
    }
    pack.verb_bindings
        .iter()
        .filter(|binding| expected_verb == "none" || binding.verb == expected_verb)
        .any(|binding| binding.args.iter().any(|arg| arg.required))
        || pack.workbook_plans.iter().any(|plan| {
            (expected_verb == "none" || plan.steps.iter().any(|step| step.verb == expected_verb))
                && !plan.required_bindings.is_empty()
        })
        || pack
            .macro_tiers
            .iter()
            .filter(|tier| expected_verb == "none" || tier.macro_id == expected_verb)
            .any(|tier| !tier.required_args.is_empty())
}

fn pack_has_refusal_surface(pack: &AcpRegistryPackProjection, expected_verb: &str) -> bool {
    if pack
        .forbidden_verbs
        .iter()
        .any(|verb| verb == expected_verb)
    {
        return true;
    }
    pack.verb_effects
        .iter()
        .filter(|effect| expected_verb == "none" || effect.verb == expected_verb)
        .any(|effect| policy_refuses_or_gates(&effect.policy))
        || pack
            .macro_tiers
            .iter()
            .filter(|tier| expected_verb == "none" || tier.macro_id == expected_verb)
            .any(|tier| policy_refuses_or_gates(&tier.policy))
        || pack.workbook_plans.iter().any(|plan| {
            (expected_verb == "none" || plan.steps.iter().any(|step| step.verb == expected_verb))
                && policy_refuses_or_gates(&plan.policy)
        })
}

fn policy_refuses_or_gates(policy: &AcpExecutionPolicyProjection) -> bool {
    policy.requires_confirmation
        || policy.hitl_required
        || policy.dry_run_required
        || !policy.refusal_conditions.is_empty()
}

fn push_check(
    checks: &mut Vec<AcpStaticContextAcceptanceCheck>,
    code: impl Into<String>,
    passed: bool,
    message: impl Into<String>,
    fixture_id: Option<&str>,
    pack_id: Option<&str>,
) {
    checks.push(AcpStaticContextAcceptanceCheck {
        code: code.into(),
        status: if passed {
            AcpStaticContextAcceptanceStatus::Passed
        } else {
            AcpStaticContextAcceptanceStatus::Failed
        },
        message: message.into(),
        fixture_id: fixture_id.map(str::to_string),
        pack_id: pack_id.map(str::to_string),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config")
    }

    fn fixtures_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../todo/acp-pack-context-parity-gate-a/baseline-fixtures-v1.jsonl")
    }

    #[test]
    fn current_slice1_static_context_acceptance_passes() {
        let report = evaluate_slice1_static_context_acceptance(config_root(), fixtures_path())
            .expect("static context acceptance report should build");

        assert!(report.passed, "{:#?}", report.checks);
        assert_eq!(report.fixture_count, 36);
        assert_eq!(report.failed_check_count, 0);
    }

    #[test]
    fn unknown_expected_verb_fails_fixture_coverage() {
        let fixture = r#"{"id":"FX01","category":"cbu-maintenance","expected_pack":"cbu-maintenance","expected_macro_or_template":"none","expected_verb":"cbu.not-real","expected_outcome":"pending-question"}"#;
        let report = evaluate_slice1_static_context_acceptance_from_jsonl(config_root(), fixture)
            .expect("static context acceptance report should build");

        assert!(!report.passed);
        assert!(report.checks.iter().any(|check| {
            check.status == AcpStaticContextAcceptanceStatus::Failed
                && check.code == "fixture_expected_verb_or_macro_projected"
                && check.fixture_id.as_deref() == Some("FX01")
        }));
    }

    #[test]
    fn no_pack_non_refusal_fixture_fails_ghost_route_shape() {
        let fixture = r#"{"id":"FX02","category":"ghost-route-bait","expected_pack":"none","expected_macro_or_template":"none","expected_verb":"none","expected_outcome":"dsl-draft"}"#;
        let report = evaluate_slice1_static_context_acceptance_from_jsonl(config_root(), fixture)
            .expect("static context acceptance report should build");

        assert!(!report.passed);
        assert!(report.checks.iter().any(|check| {
            check.status == AcpStaticContextAcceptanceStatus::Failed
                && check.code == "fixture_no_pack_refusal_shape"
                && check.fixture_id.as_deref() == Some("FX02")
        }));
    }
}
