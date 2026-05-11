//! Read-only ACP registry projection for Slice 1 context metadata.
//!
//! This module is the first Gate C boundary: it normalizes authored pack
//! metadata into a deterministic, execution-free projection. It deliberately
//! stops short of envelope v2 construction, signing, or runtime gating.

use anyhow::{Context, Result};
use dsl_core::config::loader::ConfigLoader;
use dsl_core::config::types::{
    ArgConfig, ArgType, ConsequenceTier, CrudOperation, ExternalEffect, LookupConfig,
    ResolutionMode, ReturnTypeConfig, StateEffect, VerbBehavior, VerbConfig,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::journey::pack::{
    load_packs_from_dir, AnswerKind, PackManifest, PackQuestion, PackTemplate, RiskPolicy,
    TemplateStep,
};
use crate::repl::types_v2::WorkspaceKind;

/// Schema version for the Slice 1 ACP registry projection.
pub const ACP_REGISTRY_PROJECTION_SCHEMA_VERSION: &str = "acp_registry_projection_v1";

/// Pack IDs included in the initial Gate C Slice 1 projection.
pub const SLICE_1_ACP_PACK_IDS: &[&str] = &[
    "onboarding-request",
    "cbu-maintenance",
    "product-service-taxonomy",
];

/// Deterministic Slice 1 projection over pack-scoped ACP registry metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpRegistryProjection {
    pub schema_version: &'static str,
    pub config_root: String,
    pub projection_hash: String,
    pub pack_count: usize,
    pub allowed_verb_count: usize,
    pub forbidden_verb_count: usize,
    pub verb_binding_count: usize,
    pub verb_effect_count: usize,
    pub macro_tier_count: usize,
    pub workbook_plan_count: usize,
    pub packs: Vec<AcpRegistryPackProjection>,
    pub diagnostic_taxonomy: Vec<AcpDiagnosticTaxonomyProjection>,
    pub diagnostics: Vec<AcpRegistryProjectionDiagnostic>,
}

/// Pack-level projection row used by ACP routing and future envelope builders.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpRegistryPackProjection {
    pub pack_id: String,
    pub pack_name: String,
    pub pack_version: String,
    pub manifest_hash: String,
    pub invocation_phrases: Vec<String>,
    pub workspaces: Vec<String>,
    pub required_context: Vec<String>,
    pub optional_context: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub forbidden_verbs: Vec<String>,
    pub verb_bindings: Vec<AcpVerbBindingProjection>,
    pub verb_effects: Vec<AcpVerbEffectProjection>,
    pub macro_tiers: Vec<AcpMacroTierProjection>,
    pub risk_policy: AcpRegistryRiskPolicyProjection,
    pub required_questions: Vec<AcpRegistryQuestionProjection>,
    pub optional_questions: Vec<AcpRegistryQuestionProjection>,
    pub workbook_plans: Vec<AcpWorkbookPlanProjection>,
}

/// Risk policy normalized from pack manifests.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpRegistryRiskPolicyProjection {
    pub require_confirm_before_execute: bool,
    pub max_steps_without_confirm: u32,
}

/// Question metadata used as the static source for pending-question bindings.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpRegistryQuestionProjection {
    pub field: String,
    pub prompt: String,
    pub answer_kind: String,
    pub options_source: Option<String>,
    pub default: Option<serde_json::Value>,
    pub ask_when: Option<String>,
}

/// Per-verb argument binding metadata for a pack-scoped authored verb.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpVerbBindingProjection {
    pub verb: String,
    pub description: String,
    pub binding_hash: String,
    pub args: Vec<AcpVerbArgBindingProjection>,
}

/// Per-argument binding metadata normalized from verb YAML and pack questions.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpVerbArgBindingProjection {
    pub name: String,
    pub arg_type: String,
    pub required: bool,
    pub maps_to: Option<String>,
    pub default: Option<serde_yaml::Value>,
    pub description: Option<String>,
    pub binding_source: String,
    pub pack_question_field: Option<String>,
    pub pack_question_prompt: Option<String>,
    pub lookup: Option<AcpLookupBindingProjection>,
    pub valid_values: Vec<String>,
}

/// Lookup metadata for deterministic argument binding hints.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpLookupBindingProjection {
    pub table: String,
    pub schema: Option<String>,
    pub entity_type: Option<String>,
    pub search_key: String,
    pub search_columns: Vec<String>,
    pub primary_key: String,
    pub resolution_mode: Option<String>,
    pub scope_key: Option<String>,
    pub role_filter: Option<String>,
}

/// Entity-grain read/write effect metadata for a pack-scoped authored verb.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpVerbEffectProjection {
    pub verb: String,
    pub exposure: String,
    pub behavior: String,
    pub side_effects: Option<String>,
    pub crud_operation: Option<String>,
    pub return_type: Option<String>,
    pub produces_entity_grain: Option<String>,
    pub subject_entity_grains: Vec<String>,
    pub read_entity_grains: Vec<String>,
    pub write_entity_grains: Vec<String>,
    pub source_tables: Vec<String>,
    pub lifecycle_entity_arg: Option<String>,
    pub transition_entity_id_arg: Option<String>,
    pub policy: AcpExecutionPolicyProjection,
    pub effect_hash: String,
}

/// Tier classification for a pack-scoped macro reference.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpMacroTierProjection {
    pub macro_id: String,
    pub exposure: String,
    pub tier: String,
    pub source_path: Option<String>,
    pub kind: Option<String>,
    pub step_count: usize,
    pub expands_to_verbs: Vec<String>,
    pub invokes_macros: Vec<String>,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
    pub reason: String,
    pub policy: AcpExecutionPolicyProjection,
    pub macro_hash: String,
}

/// Execution policy metadata for refusal, HITL, confirmation, and dry-run gaps.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpExecutionPolicyProjection {
    pub policy_grade: String,
    pub requires_confirmation: bool,
    pub hitl_required: bool,
    pub dry_run_required: bool,
    pub dry_run_supported: bool,
    pub refusal_conditions: Vec<String>,
    pub policy_sources: Vec<String>,
}

/// First-class workbook-plan projection lifted from a pack-local template.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpWorkbookPlanProjection {
    pub plan_id: String,
    pub template_id: String,
    pub pack_id: String,
    pub plan_hash: String,
    pub trigger_phrases: Vec<String>,
    pub required_bindings: Vec<String>,
    pub optional_bindings: Vec<String>,
    pub steps: Vec<AcpWorkbookPlanStepProjection>,
    pub risk_policy: AcpRegistryRiskPolicyProjection,
    pub state_effects: Vec<String>,
    pub refusal_conditions: Vec<String>,
    pub policy: AcpExecutionPolicyProjection,
}

/// Ordered workbook-plan step with deterministic argument ordering.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpWorkbookPlanStepProjection {
    pub step_index: usize,
    pub verb: String,
    pub args: BTreeMap<String, serde_json::Value>,
    pub repeat_for: Option<String>,
    pub when: Option<String>,
    pub execution_mode: Option<String>,
}

/// Projection diagnostic for missing or inconsistent Slice 1 metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpRegistryProjectionDiagnostic {
    pub code: String,
    pub source: String,
    pub message: String,
    pub expected: Vec<String>,
    pub actual: Option<String>,
}

/// Stable diagnostic taxonomy entries used by future ACP refusals.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcpDiagnosticTaxonomyProjection {
    pub code: String,
    pub severity: String,
    pub applies_to: String,
    pub message: String,
    pub refusal_condition: String,
}

#[derive(Serialize)]
struct ProjectionHashMaterial<'a> {
    schema_version: &'static str,
    pack_count: usize,
    allowed_verb_count: usize,
    forbidden_verb_count: usize,
    verb_binding_count: usize,
    verb_effect_count: usize,
    macro_tier_count: usize,
    workbook_plan_count: usize,
    packs: &'a [AcpRegistryPackProjection],
    diagnostic_taxonomy: &'a [AcpDiagnosticTaxonomyProjection],
    diagnostics: &'a [AcpRegistryProjectionDiagnostic],
}

#[derive(Serialize)]
struct VerbBindingHashMaterial<'a> {
    verb: &'a str,
    description: &'a str,
    args: &'a [AcpVerbArgBindingProjection],
}

#[derive(Serialize)]
struct VerbEffectHashMaterial<'a> {
    verb: &'a str,
    exposure: &'a str,
    behavior: &'a str,
    side_effects: &'a Option<String>,
    crud_operation: &'a Option<String>,
    return_type: &'a Option<String>,
    produces_entity_grain: &'a Option<String>,
    subject_entity_grains: &'a [String],
    read_entity_grains: &'a [String],
    write_entity_grains: &'a [String],
    source_tables: &'a [String],
    lifecycle_entity_arg: &'a Option<String>,
    transition_entity_id_arg: &'a Option<String>,
    policy: &'a AcpExecutionPolicyProjection,
}

#[derive(Serialize)]
struct MacroTierHashMaterial<'a> {
    macro_id: &'a str,
    exposure: &'a str,
    tier: &'a str,
    source_path: &'a Option<String>,
    kind: &'a Option<String>,
    step_count: usize,
    expands_to_verbs: &'a [String],
    invokes_macros: &'a [String],
    required_args: &'a [String],
    optional_args: &'a [String],
    reason: &'a str,
    policy: &'a AcpExecutionPolicyProjection,
}

#[derive(Debug, Clone)]
struct MacroDefinitionSource {
    source_path: String,
    value: serde_yaml::Value,
}

#[derive(Serialize)]
struct PlanHashMaterial<'a> {
    plan_id: &'a str,
    template_id: &'a str,
    pack_id: &'a str,
    trigger_phrases: &'a [String],
    required_bindings: &'a [String],
    optional_bindings: &'a [String],
    steps: &'a [AcpWorkbookPlanStepProjection],
    risk_policy: &'a AcpRegistryRiskPolicyProjection,
    state_effects: &'a [String],
    refusal_conditions: &'a [String],
    policy: &'a AcpExecutionPolicyProjection,
}

/// Build the deterministic Gate C Slice 1 ACP registry projection.
///
/// The input path must be the repository `config` directory. The projection
/// reads only authored pack manifests and does not execute DSL, access the
/// database, or build ACP envelopes.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(config_root).unwrap();
/// assert_eq!(projection.schema_version, "acp_registry_projection_v1");
/// ```
pub fn build_slice1_acp_registry_projection(
    config_root: impl AsRef<Path>,
) -> Result<AcpRegistryProjection> {
    let config_root = config_root.as_ref();
    let packs_dir = config_root.join("packs");
    let loaded_packs = load_packs_from_dir(&packs_dir)
        .with_context(|| format!("loading pack manifests from {}", packs_dir.display()))?;
    let verbs = ConfigLoader::new(config_root.display().to_string())
        .load_verbs()
        .with_context(|| format!("loading verb configs from {}", config_root.display()))?;
    let verb_index = verbs
        .domains
        .iter()
        .flat_map(|(domain, domain_config)| {
            domain_config
                .verbs
                .iter()
                .map(move |(verb_name, verb)| (format!("{domain}.{verb_name}"), verb.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let macro_registry = load_macro_registry(config_root)
        .with_context(|| format!("loading macro registry from {}", config_root.display()))?;
    let mut loaded_by_id = loaded_packs
        .into_iter()
        .map(|(manifest, hash)| (manifest.id.clone(), (manifest, hash)))
        .collect::<BTreeMap<_, _>>();

    let mut diagnostics = Vec::new();
    let mut packs = Vec::new();
    for expected_pack_id in SLICE_1_ACP_PACK_IDS {
        match loaded_by_id.remove(*expected_pack_id) {
            Some((manifest, manifest_hash)) => {
                packs.push(pack_projection_from_manifest(
                    manifest,
                    manifest_hash,
                    &verb_index,
                    &macro_registry,
                ));
            }
            None => diagnostics.push(AcpRegistryProjectionDiagnostic {
                code: "acp_registry_projection_missing_slice1_pack".to_string(),
                source: "acp_registry_projection".to_string(),
                message: "A required Slice 1 pack manifest is missing".to_string(),
                expected: vec![(*expected_pack_id).to_string()],
                actual: None,
            }),
        }
    }
    packs.sort_by(|left, right| left.pack_id.cmp(&right.pack_id));

    let pack_count = packs.len();
    let allowed_verb_count = packs
        .iter()
        .flat_map(|pack| pack.allowed_verbs.iter())
        .collect::<BTreeSet<_>>()
        .len();
    let forbidden_verb_count = packs
        .iter()
        .flat_map(|pack| pack.forbidden_verbs.iter())
        .collect::<BTreeSet<_>>()
        .len();
    let verb_binding_count = packs
        .iter()
        .map(|pack| pack.verb_bindings.len())
        .sum::<usize>();
    let verb_effect_count = packs
        .iter()
        .map(|pack| pack.verb_effects.len())
        .sum::<usize>();
    let macro_tier_count = packs
        .iter()
        .map(|pack| pack.macro_tiers.len())
        .sum::<usize>();
    let workbook_plan_count = packs
        .iter()
        .map(|pack| pack.workbook_plans.len())
        .sum::<usize>();

    let diagnostic_taxonomy = diagnostic_taxonomy_projection();
    let hash_material = ProjectionHashMaterial {
        schema_version: ACP_REGISTRY_PROJECTION_SCHEMA_VERSION,
        pack_count,
        allowed_verb_count,
        forbidden_verb_count,
        verb_binding_count,
        verb_effect_count,
        macro_tier_count,
        workbook_plan_count,
        packs: &packs,
        diagnostic_taxonomy: &diagnostic_taxonomy,
        diagnostics: &diagnostics,
    };
    let projection_hash = stable_json_hash(&hash_material)?;

    Ok(AcpRegistryProjection {
        schema_version: ACP_REGISTRY_PROJECTION_SCHEMA_VERSION,
        config_root: config_root.display().to_string(),
        projection_hash,
        pack_count,
        allowed_verb_count,
        forbidden_verb_count,
        verb_binding_count,
        verb_effect_count,
        macro_tier_count,
        workbook_plan_count,
        packs,
        diagnostic_taxonomy,
        diagnostics,
    })
}

fn pack_projection_from_manifest(
    manifest: PackManifest,
    manifest_hash: String,
    verb_index: &BTreeMap<String, VerbConfig>,
    macro_registry: &BTreeMap<String, MacroDefinitionSource>,
) -> AcpRegistryPackProjection {
    let required_questions = question_projections(&manifest.required_questions);
    let optional_questions = question_projections(&manifest.optional_questions);
    let question_index = question_index(&manifest.required_questions, &manifest.optional_questions);
    let required_fields = question_fields(&manifest.required_questions);
    let optional_fields = question_fields(&manifest.optional_questions);
    let risk_policy = risk_policy_projection(&manifest.risk_policy);
    let mut verb_bindings = manifest
        .allowed_verbs
        .iter()
        .filter_map(|verb| {
            verb_index
                .get(verb)
                .map(|config| verb_binding_projection(verb, config, &question_index))
        })
        .collect::<Vec<_>>();
    verb_bindings.sort_by(|left, right| left.verb.cmp(&right.verb));
    let mut verb_effects = manifest
        .allowed_verbs
        .iter()
        .filter_map(|verb| {
            verb_index
                .get(verb)
                .map(|config| verb_effect_projection(verb, "allowed", config, &risk_policy))
        })
        .chain(manifest.forbidden_verbs.iter().filter_map(|verb| {
            verb_index
                .get(verb)
                .map(|config| verb_effect_projection(verb, "forbidden", config, &risk_policy))
        }))
        .collect::<Vec<_>>();
    verb_effects.sort_by(|left, right| {
        left.exposure
            .cmp(&right.exposure)
            .then(left.verb.cmp(&right.verb))
    });
    let mut macro_tiers = manifest
        .allowed_verbs
        .iter()
        .filter(|reference| !verb_index.contains_key(*reference))
        .map(|reference| {
            macro_tier_projection(
                reference,
                "allowed",
                macro_registry,
                verb_index,
                &risk_policy,
            )
        })
        .chain(
            manifest
                .forbidden_verbs
                .iter()
                .filter(|reference| !verb_index.contains_key(*reference))
                .map(|reference| {
                    macro_tier_projection(
                        reference,
                        "forbidden",
                        macro_registry,
                        verb_index,
                        &risk_policy,
                    )
                }),
        )
        .collect::<Vec<_>>();
    macro_tiers.sort_by(|left, right| {
        left.exposure
            .cmp(&right.exposure)
            .then(left.macro_id.cmp(&right.macro_id))
    });
    let mut workbook_plans = manifest
        .templates
        .iter()
        .map(|template| {
            workbook_plan_projection(
                &manifest.id,
                template,
                &required_fields,
                &optional_fields,
                &risk_policy,
                &manifest.forbidden_verbs,
                verb_index,
            )
        })
        .collect::<Vec<_>>();
    workbook_plans.sort_by(|left, right| left.plan_id.cmp(&right.plan_id));

    AcpRegistryPackProjection {
        pack_id: manifest.id,
        pack_name: manifest.name,
        pack_version: manifest.version,
        manifest_hash,
        invocation_phrases: sorted_unique(manifest.invocation_phrases),
        workspaces: manifest
            .workspaces
            .iter()
            .map(workspace_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        required_context: sorted_unique(manifest.required_context),
        optional_context: sorted_unique(manifest.optional_context),
        allowed_verbs: sorted_unique(manifest.allowed_verbs),
        forbidden_verbs: sorted_unique(manifest.forbidden_verbs),
        verb_bindings,
        verb_effects,
        macro_tiers,
        risk_policy,
        required_questions,
        optional_questions,
        workbook_plans,
    }
}

fn diagnostic_taxonomy_projection() -> Vec<AcpDiagnosticTaxonomyProjection> {
    [
        (
            "acp_ambiguous_pack",
            "refusal",
            "pack_routing",
            "The utterance matches more than one pack and needs disambiguation.",
            "ambiguous_pack",
        ),
        (
            "acp_unsupported_macro_tier",
            "refusal",
            "macro_tier",
            "The macro is not projection-ready for ACP context use.",
            "unsupported_macro_tier",
        ),
        (
            "acp_forbidden_verb",
            "refusal",
            "verb_policy",
            "The selected pack forbids the requested verb.",
            "forbidden_verb",
        ),
        (
            "acp_missing_binding",
            "pending_input",
            "argument_binding",
            "A required verb or plan binding is missing.",
            "missing_binding",
        ),
        (
            "acp_legacy_route_bait",
            "refusal",
            "route_selection",
            "A legacy route conflicts with the projected ACP pack route.",
            "legacy_route_bait",
        ),
    ]
    .into_iter()
    .map(|(code, severity, applies_to, message, refusal_condition)| {
        AcpDiagnosticTaxonomyProjection {
            code: code.to_string(),
            severity: severity.to_string(),
            applies_to: applies_to.to_string(),
            message: message.to_string(),
            refusal_condition: refusal_condition.to_string(),
        }
    })
    .collect()
}

fn load_macro_registry(config_root: &Path) -> Result<BTreeMap<String, MacroDefinitionSource>> {
    let mut registry = BTreeMap::new();
    for dir in [
        config_root.join("verb_schemas/macros"),
        config_root.join("macros"),
    ] {
        if !dir.exists() {
            continue;
        }
        for path in walk_yaml_files(&dir)? {
            let source = fs::read_to_string(&path)
                .with_context(|| format!("reading macro YAML {}", path.display()))?;
            let value = serde_yaml::from_str::<serde_yaml::Value>(&source)
                .with_context(|| format!("parsing macro YAML {}", path.display()))?;
            let Some(mapping) = value.as_mapping() else {
                continue;
            };
            for (key, definition) in mapping {
                let Some(macro_id) = key.as_str() else {
                    continue;
                };
                if value_get(definition, "kind").and_then(serde_yaml::Value::as_str)
                    != Some("macro")
                {
                    continue;
                }
                registry.insert(
                    macro_id.to_string(),
                    MacroDefinitionSource {
                        source_path: relative_config_path(config_root, &path),
                        value: definition.clone(),
                    },
                );
            }
        }
    }
    Ok(registry)
}

fn macro_tier_projection(
    macro_id: &str,
    exposure: &str,
    macro_registry: &BTreeMap<String, MacroDefinitionSource>,
    verb_index: &BTreeMap<String, VerbConfig>,
    risk_policy: &AcpRegistryRiskPolicyProjection,
) -> AcpMacroTierProjection {
    let Some(definition) = macro_registry.get(macro_id) else {
        return missing_macro_tier_projection(macro_id, exposure, risk_policy);
    };
    let kind = value_get(&definition.value, "kind")
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string);
    let steps = value_get(&definition.value, "expands-to")
        .and_then(serde_yaml::Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let expands_to_verbs = macro_step_strings(&steps, "verb");
    let invokes_macros = macro_step_strings(&steps, "invoke-macro");
    let required_args = macro_arg_names(&definition.value, "required");
    let optional_args = macro_arg_names(&definition.value, "optional");
    let missing_verbs = expands_to_verbs
        .iter()
        .filter(|verb| !verb_index.contains_key(*verb))
        .cloned()
        .collect::<Vec<_>>();
    let (tier, reason) = if kind.as_deref() != Some("macro") {
        ("quarantine", "macro_kind_missing_or_invalid")
    } else if steps.is_empty() {
        ("quarantine", "macro_expansion_missing")
    } else if !missing_verbs.is_empty() {
        ("quarantine", "macro_expands_to_unknown_verbs")
    } else if !invokes_macros.is_empty() {
        ("lift", "registry_macro_uses_nested_invocations")
    } else {
        ("project", "registry_macro_projectable")
    };
    let mut refusal_conditions = Vec::new();
    if tier == "quarantine" {
        refusal_conditions.push(reason.to_string());
    }
    if tier == "lift" {
        refusal_conditions.push("nested_macro_requires_lift_before_execution".to_string());
    }
    let mutating = !invokes_macros.is_empty()
        || expands_to_verbs
            .iter()
            .filter_map(|verb| verb_index.get(verb))
            .any(verb_config_is_mutating);
    let dry_run_supported = expands_to_verbs
        .iter()
        .filter_map(|verb| verb_index.get(verb))
        .any(verb_supports_dry_run);
    let policy = execution_policy_projection(
        exposure,
        mutating,
        risk_policy,
        dry_run_supported,
        refusal_conditions,
        vec!["macro_tier".to_string(), "pack_risk_policy".to_string()],
    );
    macro_tier_projection_from_parts(MacroTierProjectionParts {
        macro_id,
        exposure,
        tier,
        source_path: Some(definition.source_path.clone()),
        kind,
        step_count: steps.len(),
        expands_to_verbs,
        invokes_macros,
        required_args,
        optional_args,
        reason,
        policy,
    })
}

struct MacroTierProjectionParts<'a> {
    macro_id: &'a str,
    exposure: &'a str,
    tier: &'a str,
    source_path: Option<String>,
    kind: Option<String>,
    step_count: usize,
    expands_to_verbs: Vec<String>,
    invokes_macros: Vec<String>,
    required_args: Vec<String>,
    optional_args: Vec<String>,
    reason: &'a str,
    policy: AcpExecutionPolicyProjection,
}

fn macro_tier_projection_from_parts(parts: MacroTierProjectionParts<'_>) -> AcpMacroTierProjection {
    let macro_hash = stable_json_hash(&MacroTierHashMaterial {
        macro_id: parts.macro_id,
        exposure: parts.exposure,
        tier: parts.tier,
        source_path: &parts.source_path,
        kind: &parts.kind,
        step_count: parts.step_count,
        expands_to_verbs: &parts.expands_to_verbs,
        invokes_macros: &parts.invokes_macros,
        required_args: &parts.required_args,
        optional_args: &parts.optional_args,
        reason: parts.reason,
        policy: &parts.policy,
    })
    .expect("macro tier projection hash material should serialize");

    AcpMacroTierProjection {
        macro_id: parts.macro_id.to_string(),
        exposure: parts.exposure.to_string(),
        tier: parts.tier.to_string(),
        source_path: parts.source_path,
        kind: parts.kind,
        step_count: parts.step_count,
        expands_to_verbs: parts.expands_to_verbs,
        invokes_macros: parts.invokes_macros,
        required_args: parts.required_args,
        optional_args: parts.optional_args,
        reason: parts.reason.to_string(),
        policy: parts.policy,
        macro_hash,
    }
}

fn missing_macro_tier_projection(
    macro_id: &str,
    exposure: &str,
    risk_policy: &AcpRegistryRiskPolicyProjection,
) -> AcpMacroTierProjection {
    let policy = execution_policy_projection(
        exposure,
        false,
        risk_policy,
        false,
        vec!["macro_definition_missing".to_string()],
        vec!["macro_tier".to_string(), "pack_risk_policy".to_string()],
    );
    macro_tier_projection_from_parts(MacroTierProjectionParts {
        macro_id,
        exposure,
        tier: "quarantine",
        source_path: None,
        kind: None,
        step_count: 0,
        expands_to_verbs: Vec::new(),
        invokes_macros: Vec::new(),
        required_args: Vec::new(),
        optional_args: Vec::new(),
        reason: "macro_definition_missing",
        policy,
    })
}

fn macro_step_strings(steps: &[serde_yaml::Value], key: &str) -> Vec<String> {
    let mut values = steps
        .iter()
        .filter_map(|step| value_get(step, key))
        .filter_map(serde_yaml::Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn macro_arg_names(definition: &serde_yaml::Value, section: &str) -> Vec<String> {
    let Some(args) = value_get(definition, "args") else {
        return Vec::new();
    };
    let Some(section) = value_get(args, section).and_then(serde_yaml::Value::as_mapping) else {
        return Vec::new();
    };
    let mut names = section
        .keys()
        .filter_map(serde_yaml::Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn value_get<'a>(value: &'a serde_yaml::Value, key: &str) -> Option<&'a serde_yaml::Value> {
    value
        .as_mapping()?
        .iter()
        .find_map(|(candidate, value)| (candidate.as_str() == Some(key)).then_some(value))
}

fn walk_yaml_files(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(current) = dirs.pop() {
        for entry in
            fs::read_dir(&current).with_context(|| format!("reading {}", current.display()))?
        {
            let path = entry
                .with_context(|| format!("reading entry in {}", current.display()))?
                .path();
            if path.is_dir() {
                dirs.push(path);
            } else if matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("yaml" | "yml")
            ) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn relative_config_path(config_root: &Path, path: &Path) -> String {
    path.strip_prefix(config_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn verb_effect_projection(
    verb: &str,
    exposure: &str,
    config: &VerbConfig,
    risk_policy: &AcpRegistryRiskPolicyProjection,
) -> AcpVerbEffectProjection {
    let behavior = behavior_id(config.behavior).to_string();
    let side_effects = config
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.side_effects.clone());
    let crud_operation = config
        .crud
        .as_ref()
        .map(|crud| crud_operation_id(crud.operation).to_string());
    let return_type = config
        .returns
        .as_ref()
        .map(|returns| return_type_id(returns.return_type).to_string());
    let produces_entity_grain = config
        .produces
        .as_ref()
        .map(|produces| normalize_entity_grain(&produces.produced_type));
    let subject_entity_grains = derive_subject_entity_grains(verb, config);
    let read_entity_grains = derive_read_entity_grains(config, &subject_entity_grains);
    let write_entity_grains = derive_write_entity_grains(
        config,
        &subject_entity_grains,
        produces_entity_grain.as_deref(),
    );
    let source_tables = derive_source_tables(config);
    let lifecycle_entity_arg = config
        .lifecycle
        .as_ref()
        .and_then(|lifecycle| lifecycle.entity_arg.clone());
    let transition_entity_id_arg = config
        .transition_args
        .as_ref()
        .map(|transition_args| transition_args.entity_id_arg.clone());
    let policy = execution_policy_projection(
        exposure,
        verb_requires_execution_gate(verb, config, side_effects.as_deref(), &write_entity_grains),
        risk_policy,
        verb_supports_dry_run(config),
        Vec::new(),
        vec!["verb_metadata".to_string(), "pack_risk_policy".to_string()],
    );
    let effect_hash = stable_json_hash(&VerbEffectHashMaterial {
        verb,
        exposure,
        behavior: &behavior,
        side_effects: &side_effects,
        crud_operation: &crud_operation,
        return_type: &return_type,
        produces_entity_grain: &produces_entity_grain,
        subject_entity_grains: &subject_entity_grains,
        read_entity_grains: &read_entity_grains,
        write_entity_grains: &write_entity_grains,
        source_tables: &source_tables,
        lifecycle_entity_arg: &lifecycle_entity_arg,
        transition_entity_id_arg: &transition_entity_id_arg,
        policy: &policy,
    })
    .expect("verb effect projection hash material should serialize");

    AcpVerbEffectProjection {
        verb: verb.to_string(),
        exposure: exposure.to_string(),
        behavior,
        side_effects,
        crud_operation,
        return_type,
        produces_entity_grain,
        subject_entity_grains,
        read_entity_grains,
        write_entity_grains,
        source_tables,
        lifecycle_entity_arg,
        transition_entity_id_arg,
        policy,
        effect_hash,
    }
}

fn derive_subject_entity_grains(verb: &str, config: &VerbConfig) -> Vec<String> {
    let mut grains = BTreeSet::new();
    if let Some(metadata) = &config.metadata {
        for kind in &metadata.subject_kinds {
            grains.insert(normalize_entity_grain(kind));
        }
        if let Some(noun) = &metadata.noun {
            grains.extend(entity_grains_from_hint(noun));
        }
    }
    if let Some(produces) = &config.produces {
        grains.insert(normalize_entity_grain(&produces.produced_type));
    }
    for arg in &config.args {
        if let Some(lookup) = &arg.lookup {
            if let Some(entity_type) = &lookup.entity_type {
                grains.insert(normalize_entity_grain(entity_type));
            }
        }
    }
    grains.extend(entity_grains_from_crud(config));
    if let Some(graph_query) = &config.graph_query {
        if let Some(root_type) = &graph_query.root_type {
            grains.insert(normalize_entity_grain(root_type));
        }
    }
    if grains.is_empty() {
        let domain = verb
            .split_once('.')
            .map(|(domain, _name)| domain)
            .unwrap_or(verb);
        grains.extend(entity_grains_from_hint(domain));
    }
    grains.into_iter().collect()
}

fn derive_read_entity_grains(config: &VerbConfig, subject_entity_grains: &[String]) -> Vec<String> {
    let mut grains = BTreeSet::new();
    for arg in &config.args {
        if let Some(lookup) = &arg.lookup {
            if let Some(entity_type) = &lookup.entity_type {
                grains.insert(normalize_entity_grain(entity_type));
            }
        }
    }
    if config
        .crud
        .as_ref()
        .is_some_and(|crud| is_read_crud_operation(crud.operation))
    {
        grains.extend(entity_grains_from_crud(config));
    }
    if let Some(lifecycle) = &config.lifecycle {
        for table in &lifecycle.reads_tables {
            grains.extend(entity_grains_from_hint(table));
        }
    }
    if let Some(graph_query) = &config.graph_query {
        if let Some(root_type) = &graph_query.root_type {
            grains.insert(normalize_entity_grain(root_type));
        }
    }
    if grains.is_empty()
        && matches!(
            config
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.side_effects.as_deref()),
            Some("facts_only")
        )
    {
        grains.extend(subject_entity_grains.iter().cloned());
    }
    grains.into_iter().collect()
}

fn derive_write_entity_grains(
    config: &VerbConfig,
    subject_entity_grains: &[String],
    produces_entity_grain: Option<&str>,
) -> Vec<String> {
    let mut grains = BTreeSet::new();
    if config
        .crud
        .as_ref()
        .is_some_and(|crud| is_write_crud_operation(crud.operation))
    {
        grains.extend(entity_grains_from_crud(config));
    }
    if let Some(lifecycle) = &config.lifecycle {
        for table in &lifecycle.writes_tables {
            grains.extend(entity_grains_from_hint(table));
        }
    }
    if let Some(produces_entity_grain) = produces_entity_grain {
        grains.insert(produces_entity_grain.to_string());
    }
    if grains.is_empty()
        && matches!(
            config
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.side_effects.as_deref()),
            Some("state_write")
        )
    {
        grains.extend(subject_entity_grains.iter().cloned());
    }
    grains.into_iter().collect()
}

fn derive_source_tables(config: &VerbConfig) -> Vec<String> {
    let mut tables = BTreeSet::new();
    if let Some(crud) = &config.crud {
        for table in [
            crud.table.as_deref(),
            crud.base_table.as_deref(),
            crud.extension_table.as_deref(),
            crud.junction.as_deref(),
            crud.primary_table.as_deref(),
            crud.join_table.as_deref(),
            crud.role_table.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            tables.insert(table_ref(crud.schema.as_deref(), table));
        }
    }
    if let Some(lifecycle) = &config.lifecycle {
        for table in lifecycle
            .reads_tables
            .iter()
            .chain(lifecycle.writes_tables.iter())
        {
            tables.insert(table.clone());
        }
    }
    tables.into_iter().collect()
}

fn entity_grains_from_crud(config: &VerbConfig) -> Vec<String> {
    let Some(crud) = &config.crud else {
        return Vec::new();
    };
    let mut grains = BTreeSet::new();
    for table in [
        crud.table.as_deref(),
        crud.base_table.as_deref(),
        crud.extension_table.as_deref(),
        crud.junction.as_deref(),
        crud.primary_table.as_deref(),
        crud.join_table.as_deref(),
        crud.role_table.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        grains.extend(entity_grains_from_hint(table));
    }
    grains.into_iter().collect()
}

fn entity_grains_from_hint(value: &str) -> Vec<String> {
    let normalized = value
        .rsplit('.')
        .next()
        .unwrap_or(value)
        .replace('-', "_")
        .to_ascii_lowercase();
    let mut grains = BTreeSet::new();
    if normalized.contains("service_option") {
        grains.insert("service-option".to_string());
    }
    if normalized.contains("service_resource") || normalized.contains("resource_instance") {
        grains.insert("service-resource".to_string());
    }
    if normalized.contains("product_service") {
        grains.insert("product-service".to_string());
    }
    if normalized.contains("onboarding") || normalized.contains("data_request") {
        grains.insert("onboarding-request".to_string());
    }
    if normalized.contains("slice") {
        grains.insert("onboarding-slice".to_string());
    }
    for (needle, grain) in [
        ("cbus", "cbu"),
        ("cbu", "cbu"),
        ("deal", "deal"),
        ("contract", "contract"),
        ("product", "product"),
        ("service", "service"),
        ("attribute", "attribute"),
        ("entities", "entity"),
        ("entity", "entity"),
        ("legal_entity", "entity"),
        ("principal", "principal"),
        ("booking", "booking"),
        ("source", "research-source"),
    ] {
        if normalized.contains(needle) {
            grains.insert(grain.to_string());
        }
    }
    if grains.is_empty() {
        grains.insert(normalize_entity_grain(&normalized));
    }
    grains.into_iter().collect()
}

fn normalize_entity_grain(value: &str) -> String {
    value.replace('_', "-").to_ascii_lowercase()
}

fn table_ref(schema: Option<&str>, table: &str) -> String {
    match schema {
        Some(schema) => format!("{schema}.{table}"),
        None => table.to_string(),
    }
}

fn is_read_crud_operation(operation: CrudOperation) -> bool {
    match operation {
        CrudOperation::Select
        | CrudOperation::ListByFk
        | CrudOperation::ListParties
        | CrudOperation::SelectWithJoin => true,
        CrudOperation::Insert
        | CrudOperation::Update
        | CrudOperation::Delete
        | CrudOperation::Upsert
        | CrudOperation::Link
        | CrudOperation::Unlink
        | CrudOperation::RoleLink
        | CrudOperation::RoleUnlink
        | CrudOperation::EntityCreate
        | CrudOperation::EntityUpsert => false,
    }
}

fn is_write_crud_operation(operation: CrudOperation) -> bool {
    match operation {
        CrudOperation::Insert
        | CrudOperation::Update
        | CrudOperation::Delete
        | CrudOperation::Upsert
        | CrudOperation::Link
        | CrudOperation::Unlink
        | CrudOperation::RoleLink
        | CrudOperation::RoleUnlink
        | CrudOperation::EntityCreate
        | CrudOperation::EntityUpsert => true,
        CrudOperation::Select
        | CrudOperation::ListByFk
        | CrudOperation::ListParties
        | CrudOperation::SelectWithJoin => false,
    }
}

fn verb_binding_projection(
    verb: &str,
    config: &VerbConfig,
    question_index: &BTreeMap<String, &PackQuestion>,
) -> AcpVerbBindingProjection {
    let args = config
        .args
        .iter()
        .map(|arg| arg_binding_projection(arg, question_index))
        .collect::<Vec<_>>();
    let binding_hash = stable_json_hash(&VerbBindingHashMaterial {
        verb,
        description: &config.description,
        args: &args,
    })
    .expect("verb binding projection hash material should serialize");

    AcpVerbBindingProjection {
        verb: verb.to_string(),
        description: config.description.clone(),
        binding_hash,
        args,
    }
}

fn arg_binding_projection(
    arg: &ArgConfig,
    question_index: &BTreeMap<String, &PackQuestion>,
) -> AcpVerbArgBindingProjection {
    let question = question_index
        .get(&normalize_binding_key(&arg.name))
        .copied();
    AcpVerbArgBindingProjection {
        name: arg.name.clone(),
        arg_type: arg_type_id(arg.arg_type).to_string(),
        required: arg.required,
        maps_to: arg.maps_to.clone(),
        default: arg.default.clone(),
        description: arg.description.clone(),
        binding_source: binding_source(arg, question).to_string(),
        pack_question_field: question.map(|question| question.field.clone()),
        pack_question_prompt: question.map(|question| question.prompt.clone()),
        lookup: arg.lookup.as_ref().map(lookup_projection),
        valid_values: arg.valid_values.clone().unwrap_or_default(),
    }
}

fn lookup_projection(lookup: &LookupConfig) -> AcpLookupBindingProjection {
    AcpLookupBindingProjection {
        table: lookup.table.clone(),
        schema: lookup.schema.clone(),
        entity_type: lookup.entity_type.clone(),
        search_key: lookup.search_key.primary_column().to_string(),
        search_columns: lookup
            .search_key
            .all_columns()
            .into_iter()
            .map(str::to_string)
            .collect(),
        primary_key: lookup.primary_key.clone(),
        resolution_mode: lookup
            .resolution_mode
            .map(resolution_mode_id)
            .map(str::to_string),
        scope_key: lookup.scope_key.clone(),
        role_filter: lookup.role_filter.clone(),
    }
}

fn workbook_plan_projection(
    pack_id: &str,
    template: &PackTemplate,
    required_fields: &BTreeSet<String>,
    optional_fields: &BTreeSet<String>,
    risk_policy: &AcpRegistryRiskPolicyProjection,
    forbidden_verbs: &[String],
    verb_index: &BTreeMap<String, VerbConfig>,
) -> AcpWorkbookPlanProjection {
    let plan_id = format!("{}.{}", pack_id, template.template_id);
    let steps = template
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| step_projection(index + 1, step))
        .collect::<Vec<_>>();
    let referenced_bindings = referenced_answer_bindings(template);
    let required_bindings = referenced_bindings
        .iter()
        .filter(|binding| required_fields.contains(*binding))
        .cloned()
        .collect::<Vec<_>>();
    let optional_bindings = referenced_bindings
        .iter()
        .filter(|binding| optional_fields.contains(*binding))
        .cloned()
        .collect::<Vec<_>>();
    let trigger_phrases = sorted_unique(vec![
        template.template_id.clone(),
        template.when_to_use.clone(),
    ]);
    let state_effects = steps
        .iter()
        .filter(|step| is_mutating_verb(&step.verb))
        .map(|step| format!("step {} may mutate through {}", step.step_index, step.verb))
        .collect::<Vec<_>>();
    let refusal_conditions = forbidden_verbs
        .iter()
        .map(|verb| format!("pack forbids {}", verb))
        .collect::<Vec<_>>();
    let mutating = steps.iter().any(|step| {
        verb_index
            .get(&step.verb)
            .map(verb_config_is_mutating)
            .unwrap_or_else(|| is_mutating_verb(&step.verb))
    });
    let dry_run_supported = steps.iter().any(|step| {
        verb_index
            .get(&step.verb)
            .is_some_and(verb_supports_dry_run)
    });
    let policy = execution_policy_projection(
        "allowed",
        mutating,
        risk_policy,
        dry_run_supported,
        refusal_conditions.clone(),
        vec!["workbook_plan".to_string(), "pack_risk_policy".to_string()],
    );

    let plan_hash = stable_json_hash(&PlanHashMaterial {
        plan_id: &plan_id,
        template_id: &template.template_id,
        pack_id,
        trigger_phrases: &trigger_phrases,
        required_bindings: &required_bindings,
        optional_bindings: &optional_bindings,
        steps: &steps,
        risk_policy,
        state_effects: &state_effects,
        refusal_conditions: &refusal_conditions,
        policy: &policy,
    })
    .expect("workbook plan projection hash material should serialize");

    AcpWorkbookPlanProjection {
        plan_id,
        template_id: template.template_id.clone(),
        pack_id: pack_id.to_string(),
        plan_hash,
        trigger_phrases,
        required_bindings,
        optional_bindings,
        steps,
        risk_policy: risk_policy.clone(),
        state_effects,
        refusal_conditions,
        policy,
    }
}

fn step_projection(step_index: usize, step: &TemplateStep) -> AcpWorkbookPlanStepProjection {
    AcpWorkbookPlanStepProjection {
        step_index,
        verb: step.verb.clone(),
        args: step
            .args
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        repeat_for: step.repeat_for.clone(),
        when: step.when.clone(),
        execution_mode: step.execution_mode.clone(),
    }
}

fn question_projections(questions: &[PackQuestion]) -> Vec<AcpRegistryQuestionProjection> {
    let mut projections = questions
        .iter()
        .map(|question| AcpRegistryQuestionProjection {
            field: question.field.clone(),
            prompt: question.prompt.clone(),
            answer_kind: answer_kind_id(&question.answer_kind).to_string(),
            options_source: question.options_source.clone(),
            default: question.default.clone(),
            ask_when: question.ask_when.clone(),
        })
        .collect::<Vec<_>>();
    projections.sort_by(|left, right| left.field.cmp(&right.field));
    projections
}

fn question_fields(questions: &[PackQuestion]) -> BTreeSet<String> {
    questions
        .iter()
        .map(|question| question.field.clone())
        .collect()
}

fn question_index<'a>(
    required_questions: &'a [PackQuestion],
    optional_questions: &'a [PackQuestion],
) -> BTreeMap<String, &'a PackQuestion> {
    required_questions
        .iter()
        .chain(optional_questions.iter())
        .map(|question| (normalize_binding_key(&question.field), question))
        .collect()
}

fn binding_source(arg: &ArgConfig, question: Option<&PackQuestion>) -> &'static str {
    if question.is_some() {
        "pack_question"
    } else if arg.lookup.is_some() {
        "verb_lookup"
    } else if arg.default.is_some() {
        "verb_default"
    } else if arg.required {
        "required_user_input"
    } else {
        "optional_user_input"
    }
}

fn risk_policy_projection(policy: &RiskPolicy) -> AcpRegistryRiskPolicyProjection {
    AcpRegistryRiskPolicyProjection {
        require_confirm_before_execute: policy.require_confirm_before_execute,
        max_steps_without_confirm: policy.max_steps_without_confirm,
    }
}

fn execution_policy_projection(
    exposure: &str,
    mutating: bool,
    risk_policy: &AcpRegistryRiskPolicyProjection,
    dry_run_supported: bool,
    mut refusal_conditions: Vec<String>,
    mut policy_sources: Vec<String>,
) -> AcpExecutionPolicyProjection {
    let forbidden = exposure == "forbidden";
    let requires_confirmation =
        forbidden || (mutating && risk_policy.require_confirm_before_execute);
    let hitl_required = forbidden || requires_confirmation;
    let dry_run_required = mutating;

    if forbidden {
        refusal_conditions.push("pack_forbidden".to_string());
    }
    if requires_confirmation {
        refusal_conditions.push("confirmation_required".to_string());
    }
    if mutating {
        refusal_conditions.push("mutating_path".to_string());
    }
    if dry_run_required && !dry_run_supported {
        refusal_conditions.push("dry_run_metadata_missing".to_string());
    }
    refusal_conditions.sort();
    refusal_conditions.dedup();
    if dry_run_supported {
        policy_sources.push("dry_run_arg".to_string());
    }
    policy_sources.sort();
    policy_sources.dedup();

    let policy_grade = if forbidden
        || refusal_conditions.iter().any(|condition| {
            matches!(
                condition.as_str(),
                "macro_definition_missing"
                    | "macro_kind_missing_or_invalid"
                    | "macro_expansion_missing"
                    | "macro_expands_to_unknown_verbs"
                    | "nested_macro_requires_lift_before_execution"
                    | "pack_forbidden"
            )
        }) {
        "refusal"
    } else if dry_run_required && !dry_run_supported {
        "policy_gap"
    } else if requires_confirmation {
        "confirm_required"
    } else {
        "read_only"
    };

    AcpExecutionPolicyProjection {
        policy_grade: policy_grade.to_string(),
        requires_confirmation,
        hitl_required,
        dry_run_required,
        dry_run_supported,
        refusal_conditions,
        policy_sources,
    }
}

fn verb_supports_dry_run(config: &VerbConfig) -> bool {
    config.args.iter().any(|arg| arg.name == "dry-run")
}

fn verb_config_is_mutating(config: &VerbConfig) -> bool {
    side_effect_requires_execution_gate(
        config
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.side_effects.as_deref()),
    ) || three_axis_requires_execution_gate(config)
        || config
            .crud
            .as_ref()
            .is_some_and(|crud| is_write_crud_operation(crud.operation))
        || config.produces.is_some()
}

fn verb_requires_execution_gate(
    verb: &str,
    config: &VerbConfig,
    side_effects: Option<&str>,
    write_entity_grains: &[String],
) -> bool {
    !write_entity_grains.is_empty()
        || side_effect_requires_execution_gate(side_effects)
        || three_axis_requires_execution_gate(config)
        || is_mutating_verb(verb)
}

fn side_effect_requires_execution_gate(side_effects: Option<&str>) -> bool {
    match side_effects {
        Some("facts_only") | Some("none") | None => false,
        Some("outbox_write") | Some("state_write") => true,
        Some(_) => false,
    }
}

fn three_axis_requires_execution_gate(config: &VerbConfig) -> bool {
    let Some(three_axis) = config.three_axis.as_ref() else {
        return false;
    };
    let state_transition = match three_axis.state_effect {
        StateEffect::Transition => true,
        StateEffect::Preserving => false,
    };
    let externally_emitting = three_axis
        .external_effects
        .iter()
        .any(|effect| match effect {
            ExternalEffect::Emitting => true,
            ExternalEffect::Observational | ExternalEffect::Navigating => false,
        });
    state_transition
        || externally_emitting
        || three_axis.consequence.baseline >= ConsequenceTier::RequiresConfirmation
}

fn referenced_answer_bindings(template: &PackTemplate) -> Vec<String> {
    let mut bindings = BTreeSet::new();
    for step in &template.steps {
        for value in step.args.values() {
            collect_answer_refs_from_value(value, &mut bindings);
        }
        if let Some(repeat_for) = &step.repeat_for {
            collect_answer_refs_from_str(repeat_for, &mut bindings);
        }
        if let Some(condition) = &step.when {
            collect_answer_refs_from_str(condition, &mut bindings);
        }
    }
    bindings.into_iter().collect()
}

fn collect_answer_refs_from_value(value: &serde_json::Value, bindings: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::String(text) => collect_answer_refs_from_str(text, bindings),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_answer_refs_from_value(item, bindings);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                collect_answer_refs_from_value(item, bindings);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn collect_answer_refs_from_str(text: &str, bindings: &mut BTreeSet<String>) {
    let mut remainder = text;
    while let Some(start) = remainder.find("answers.") {
        let after_prefix = &remainder[start + "answers.".len()..];
        let end = after_prefix
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
            .unwrap_or(after_prefix.len());
        if end > 0 {
            bindings.insert(after_prefix[..end].to_string());
        }
        remainder = &after_prefix[end..];
    }
}

fn is_mutating_verb(verb: &str) -> bool {
    let verb_tail = verb.rsplit('.').next().unwrap_or(verb);
    matches!(
        verb_tail,
        "add-product"
            | "assign-role"
            | "bind-service-options"
            | "cancel-data-request"
            | "cancel-slice"
            | "compile-data-request"
            | "constrain-option-values"
            | "create"
            | "declare-eligibility"
            | "declare-fanout-rule"
            | "declare-option"
            | "define"
            | "deprecate"
            | "dispatch-ready-slices"
            | "draft"
            | "ensure"
            | "override-option"
            | "publish"
            | "remove-product"
            | "remove-role"
            | "request-onboarding"
            | "request-onboarding-batch"
            | "retire"
            | "submit-for-review"
            | "update"
    )
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn answer_kind_id(kind: &AnswerKind) -> &'static str {
    match kind {
        AnswerKind::String => "string",
        AnswerKind::Boolean => "boolean",
        AnswerKind::List => "list",
        AnswerKind::EntityRef => "entity_ref",
        AnswerKind::Enum => "enum",
    }
}

fn arg_type_id(kind: ArgType) -> &'static str {
    match kind {
        ArgType::String => "string",
        ArgType::Integer => "integer",
        ArgType::Decimal => "decimal",
        ArgType::Boolean => "boolean",
        ArgType::Date => "date",
        ArgType::Timestamp => "timestamp",
        ArgType::Uuid => "uuid",
        ArgType::UuidArray => "uuid_array",
        ArgType::UuidList => "uuid_list",
        ArgType::Json => "json",
        ArgType::Lookup => "lookup",
        ArgType::StringList => "string_list",
        ArgType::Map => "map",
        ArgType::SymbolRef => "symbol_ref",
        ArgType::Object => "object",
    }
}

fn behavior_id(behavior: VerbBehavior) -> &'static str {
    match behavior {
        VerbBehavior::Crud => "crud",
        VerbBehavior::Plugin => "plugin",
        VerbBehavior::GraphQuery => "graph_query",
        VerbBehavior::Durable => "durable",
    }
}

fn crud_operation_id(operation: CrudOperation) -> &'static str {
    match operation {
        CrudOperation::Insert => "insert",
        CrudOperation::Select => "select",
        CrudOperation::Update => "update",
        CrudOperation::Delete => "delete",
        CrudOperation::Upsert => "upsert",
        CrudOperation::Link => "link",
        CrudOperation::Unlink => "unlink",
        CrudOperation::RoleLink => "role_link",
        CrudOperation::RoleUnlink => "role_unlink",
        CrudOperation::ListByFk => "list_by_fk",
        CrudOperation::ListParties => "list_parties",
        CrudOperation::SelectWithJoin => "select_with_join",
        CrudOperation::EntityCreate => "entity_create",
        CrudOperation::EntityUpsert => "entity_upsert",
    }
}

fn return_type_id(return_type: ReturnTypeConfig) -> &'static str {
    match return_type {
        ReturnTypeConfig::Uuid => "uuid",
        ReturnTypeConfig::String => "string",
        ReturnTypeConfig::Record => "record",
        ReturnTypeConfig::RecordSet => "record_set",
        ReturnTypeConfig::Affected => "affected",
        ReturnTypeConfig::Void => "void",
        ReturnTypeConfig::EntityQueryResult => "entity_query_result",
        ReturnTypeConfig::TemplateInvokeResult => "template_invoke_result",
        ReturnTypeConfig::TemplateBatchResult => "template_batch_result",
        ReturnTypeConfig::BatchControlResult => "batch_control_result",
        ReturnTypeConfig::BatchResult => "batch_result",
        ReturnTypeConfig::GraphResult => "graph_result",
        ReturnTypeConfig::PathResult => "path_result",
        ReturnTypeConfig::ViewState => "view_state",
        ReturnTypeConfig::LayoutResult => "layout_result",
        ReturnTypeConfig::SelectionInfo => "selection_info",
        ReturnTypeConfig::Object => "object",
    }
}

fn resolution_mode_id(mode: ResolutionMode) -> &'static str {
    match mode {
        ResolutionMode::Reference => "reference",
        ResolutionMode::Entity => "entity",
    }
}

fn normalize_binding_key(value: &str) -> String {
    value.replace('-', "_")
}

fn workspace_id(workspace: &WorkspaceKind) -> String {
    match workspace {
        WorkspaceKind::ProductMaintenance => "product_maintenance",
        WorkspaceKind::Catalogue => "catalogue",
        WorkspaceKind::Deal => "deal",
        WorkspaceKind::Cbu => "cbu",
        WorkspaceKind::Kyc => "kyc",
        WorkspaceKind::InstrumentMatrix => "instrument_matrix",
        WorkspaceKind::OnBoarding => "onboarding_request",
        WorkspaceKind::SemOsMaintenance => "semos_maintenance",
        WorkspaceKind::LifecycleResources => "lifecycle_resources",
        WorkspaceKind::BookingPrincipal => "booking_principal",
    }
    .to_string()
}

fn stable_json_hash(value: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(value).context("serializing projection hash material")?;
    let hash = Sha256::digest(bytes);
    Ok(format!("{hash:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_config_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config")
    }

    #[test]
    fn slice1_projection_contains_expected_packs_only() {
        let projection = build_slice1_acp_registry_projection(repo_config_root()).unwrap();
        let pack_ids = projection
            .packs
            .iter()
            .map(|pack| pack.pack_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            pack_ids,
            vec![
                "cbu-maintenance",
                "onboarding-request",
                "product-service-taxonomy"
            ]
        );
        assert_eq!(projection.pack_count, 3);
        assert!(projection.diagnostics.is_empty());
        assert_eq!(
            projection
                .diagnostic_taxonomy
                .iter()
                .map(|entry| entry.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "acp_ambiguous_pack",
                "acp_unsupported_macro_tier",
                "acp_forbidden_verb",
                "acp_missing_binding",
                "acp_legacy_route_bait"
            ]
        );
    }

    #[test]
    fn slice1_projection_lifts_pack_templates_as_workbook_plans() {
        let projection = build_slice1_acp_registry_projection(repo_config_root()).unwrap();
        let plan_ids = projection
            .packs
            .iter()
            .flat_map(|pack| pack.workbook_plans.iter())
            .map(|plan| plan.plan_id.as_str())
            .collect::<BTreeSet<_>>();

        assert!(plan_ids.contains("cbu-maintenance.create-cbu"));
        assert!(plan_ids.contains("cbu-maintenance.add-entity-and-role"));
        assert!(plan_ids.contains("onboarding-request.standard-onboarding-handoff"));
        assert!(plan_ids.contains("product-service-taxonomy.product-first-taxonomy"));
        assert!(plan_ids.contains("product-service-taxonomy.service-first-taxonomy"));
        assert!(plan_ids.contains("product-service-taxonomy.resource-first-taxonomy"));
        assert_eq!(projection.workbook_plan_count, 6);
    }

    #[test]
    fn workbook_plans_preserve_binding_sources() {
        let projection = build_slice1_acp_registry_projection(repo_config_root()).unwrap();
        let handoff = projection
            .packs
            .iter()
            .flat_map(|pack| pack.workbook_plans.iter())
            .find(|plan| plan.plan_id == "onboarding-request.standard-onboarding-handoff")
            .unwrap();

        assert_eq!(
            handoff.required_bindings,
            vec!["cbu_id", "contract_id", "deal_id", "product_id"]
        );
        assert_eq!(
            handoff.optional_bindings,
            vec!["notes", "requested_by", "target_live_date"]
        );
        assert_eq!(handoff.steps.len(), 5);
        assert!(handoff.plan_hash.len() == 64);
    }

    #[test]
    fn slice1_projection_includes_verb_binding_metadata() {
        let projection = build_slice1_acp_registry_projection(repo_config_root()).unwrap();

        assert_eq!(projection.verb_binding_count, 71);

        let add_product = find_binding(&projection, "cbu.add-product");
        let cbu_id = add_product
            .args
            .iter()
            .find(|arg| arg.name == "cbu-id")
            .unwrap();
        assert!(cbu_id.required);
        assert_eq!(cbu_id.arg_type, "uuid");
        assert_eq!(cbu_id.binding_source, "verb_lookup");
        assert_eq!(
            cbu_id
                .lookup
                .as_ref()
                .and_then(|lookup| lookup.entity_type.as_deref()),
            Some("cbu")
        );

        let product = add_product
            .args
            .iter()
            .find(|arg| arg.name == "product")
            .unwrap();
        assert_eq!(product.binding_source, "verb_lookup");
        assert_eq!(
            product.lookup.as_ref().map(|lookup| lookup.table.as_str()),
            Some("products")
        );
    }

    #[test]
    fn pack_questions_are_joined_to_verb_args_by_normalized_field() {
        let projection = build_slice1_acp_registry_projection(repo_config_root()).unwrap();
        let request_onboarding = find_binding(&projection, "deal.request-onboarding");

        let cbu_id = request_onboarding
            .args
            .iter()
            .find(|arg| arg.name == "cbu-id")
            .unwrap();
        assert_eq!(cbu_id.binding_source, "pack_question");
        assert_eq!(cbu_id.pack_question_field.as_deref(), Some("cbu_id"));
        assert!(cbu_id
            .pack_question_prompt
            .as_deref()
            .unwrap()
            .contains("existing CBU"));

        let target_live_date = request_onboarding
            .args
            .iter()
            .find(|arg| arg.name == "target-live-date")
            .unwrap();
        assert_eq!(target_live_date.binding_source, "pack_question");
        assert_eq!(
            target_live_date.pack_question_field.as_deref(),
            Some("target_live_date")
        );
    }

    #[test]
    fn slice1_projection_includes_entity_grain_effects() {
        let projection = build_slice1_acp_registry_projection(repo_config_root()).unwrap();

        assert_eq!(projection.verb_effect_count, 78);

        let cbu_pack = find_pack(&projection, "cbu-maintenance");
        let create = find_effect(cbu_pack, "cbu.create", "allowed");
        assert_eq!(create.side_effects.as_deref(), Some("state_write"));
        assert_eq!(create.produces_entity_grain.as_deref(), Some("cbu"));
        assert!(create.write_entity_grains.contains(&"cbu".to_string()));
        assert!(create.read_entity_grains.contains(&"entity".to_string()));
        assert!(create
            .read_entity_grains
            .contains(&"jurisdiction".to_string()));

        let taxonomy_pack = find_pack(&projection, "product-service-taxonomy");
        let product_list = find_effect(taxonomy_pack, "product.list", "allowed");
        assert_eq!(product_list.side_effects.as_deref(), Some("facts_only"));
        assert!(product_list
            .read_entity_grains
            .contains(&"product".to_string()));
        assert!(product_list.write_entity_grains.is_empty());

        let forbidden_create = find_effect(taxonomy_pack, "cbu.create", "forbidden");
        assert_eq!(forbidden_create.behavior, "plugin");
        assert!(forbidden_create
            .write_entity_grains
            .contains(&"cbu".to_string()));
    }

    #[test]
    fn slice1_projection_tiers_pack_macro_references() {
        let projection = build_slice1_acp_registry_projection(repo_config_root()).unwrap();

        assert_eq!(projection.macro_tier_count, 21);

        let cbu_pack = find_pack(&projection, "cbu-maintenance");
        assert_eq!(cbu_pack.macro_tiers.len(), 21);
        assert_eq!(count_macro_tier(cbu_pack, "project"), 18);
        assert_eq!(count_macro_tier(cbu_pack, "lift"), 3);
        assert_eq!(count_macro_tier(cbu_pack, "quarantine"), 0);

        let direct = find_macro_tier(cbu_pack, "struct.lux.pe.scsp");
        assert_eq!(direct.tier, "project");
        assert_eq!(direct.reason, "registry_macro_projectable");
        assert!(direct.expands_to_verbs.contains(&"cbu.create".to_string()));
        assert!(direct.invokes_macros.is_empty());

        let composite = find_macro_tier(cbu_pack, "struct.ie.hedge.icav");
        assert_eq!(composite.tier, "lift");
        assert_eq!(composite.reason, "registry_macro_uses_nested_invocations");
        assert!(composite
            .invokes_macros
            .contains(&"struct.ie.aif.icav".to_string()));

        assert!(find_pack(&projection, "onboarding-request")
            .macro_tiers
            .is_empty());
        assert!(find_pack(&projection, "product-service-taxonomy")
            .macro_tiers
            .is_empty());
    }

    #[test]
    fn slice1_projection_includes_policy_metadata() {
        let projection = build_slice1_acp_registry_projection(repo_config_root()).unwrap();

        let cbu_pack = find_pack(&projection, "cbu-maintenance");
        let create = find_effect(cbu_pack, "cbu.create", "allowed");
        assert_eq!(create.policy.policy_grade, "policy_gap");
        assert!(create.policy.requires_confirmation);
        assert!(create.policy.hitl_required);
        assert!(create.policy.dry_run_required);
        assert!(!create.policy.dry_run_supported);
        assert!(create
            .policy
            .refusal_conditions
            .contains(&"dry_run_metadata_missing".to_string()));

        let taxonomy_pack = find_pack(&projection, "product-service-taxonomy");
        let product_list = find_effect(taxonomy_pack, "product.list", "allowed");
        assert_eq!(product_list.policy.policy_grade, "read_only");
        assert!(!product_list.policy.requires_confirmation);
        assert!(!product_list.policy.dry_run_required);

        let onboarding_pack = find_pack(&projection, "onboarding-request");
        let dispatch_ready_slices = find_effect(
            onboarding_pack,
            "onboarding.dispatch-ready-slices",
            "allowed",
        );
        assert_eq!(
            dispatch_ready_slices.side_effects.as_deref(),
            Some("outbox_write")
        );
        assert_eq!(dispatch_ready_slices.policy.policy_grade, "policy_gap");
        assert!(dispatch_ready_slices.policy.requires_confirmation);
        assert!(dispatch_ready_slices.policy.hitl_required);
        assert!(dispatch_ready_slices.policy.dry_run_required);

        let forbidden_create = find_effect(taxonomy_pack, "cbu.create", "forbidden");
        assert_eq!(forbidden_create.policy.policy_grade, "refusal");
        assert!(forbidden_create.policy.hitl_required);
        assert!(forbidden_create
            .policy
            .refusal_conditions
            .contains(&"pack_forbidden".to_string()));

        let direct_macro = find_macro_tier(cbu_pack, "struct.lux.pe.scsp");
        assert_eq!(direct_macro.policy.policy_grade, "policy_gap");
        assert!(direct_macro.policy.dry_run_required);

        let composite_macro = find_macro_tier(cbu_pack, "struct.ie.hedge.icav");
        assert_eq!(composite_macro.policy.policy_grade, "refusal");
        assert!(composite_macro
            .policy
            .refusal_conditions
            .contains(&"nested_macro_requires_lift_before_execution".to_string()));

        let handoff = find_plan(
            &projection,
            "onboarding-request.standard-onboarding-handoff",
        );
        assert_eq!(handoff.policy.policy_grade, "policy_gap");
        assert!(handoff.policy.hitl_required);
        assert!(handoff.policy.dry_run_required);
    }

    #[test]
    fn projection_hash_is_stable_for_same_inputs() {
        let first = build_slice1_acp_registry_projection(repo_config_root()).unwrap();
        let second = build_slice1_acp_registry_projection(repo_config_root()).unwrap();

        assert_eq!(first.projection_hash, second.projection_hash);
        assert_eq!(first.projection_hash.len(), 64);
    }

    #[test]
    fn projection_json_bytes_are_stable_for_same_inputs() {
        let first = build_slice1_acp_registry_projection(repo_config_root()).unwrap();
        let second = build_slice1_acp_registry_projection(repo_config_root()).unwrap();
        let first_bytes = serde_json::to_vec_pretty(&first).unwrap();
        let second_bytes = serde_json::to_vec_pretty(&second).unwrap();

        assert_eq!(first_bytes, second_bytes);
    }

    #[test]
    fn verb_configs_have_explicit_arg_contracts() {
        let verbs_dir = repo_config_root().join("verbs");
        let mut missing = Vec::new();

        for entry in walk_yaml_files(&verbs_dir) {
            let yaml = std::fs::read_to_string(&entry).unwrap();
            let value = serde_yaml::from_str::<serde_yaml::Value>(&yaml).unwrap();
            let Some(domains) = value.get("domains").and_then(serde_yaml::Value::as_mapping) else {
                continue;
            };
            for (domain, body) in domains {
                let domain = domain.as_str().unwrap_or("<unknown-domain>");
                let Some(verbs) = body.get("verbs").and_then(serde_yaml::Value::as_mapping) else {
                    continue;
                };
                for (verb, config) in verbs {
                    let verb = verb.as_str().unwrap_or("<unknown-verb>");
                    let has_args = config
                        .as_mapping()
                        .map(|mapping| mapping.contains_key("args"))
                        .unwrap_or(false);
                    if !has_args {
                        missing.push(format!("{}:{}.{verb}", entry.display(), domain));
                    }
                }
            }
        }

        assert!(missing.is_empty(), "missing args entries: {missing:?}");
    }

    fn find_binding<'a>(
        projection: &'a AcpRegistryProjection,
        verb: &str,
    ) -> &'a AcpVerbBindingProjection {
        projection
            .packs
            .iter()
            .flat_map(|pack| pack.verb_bindings.iter())
            .find(|binding| binding.verb == verb)
            .unwrap()
    }

    fn find_pack<'a>(
        projection: &'a AcpRegistryProjection,
        pack_id: &str,
    ) -> &'a AcpRegistryPackProjection {
        projection
            .packs
            .iter()
            .find(|pack| pack.pack_id == pack_id)
            .unwrap()
    }

    fn find_effect<'a>(
        pack: &'a AcpRegistryPackProjection,
        verb: &str,
        exposure: &str,
    ) -> &'a AcpVerbEffectProjection {
        pack.verb_effects
            .iter()
            .find(|effect| effect.verb == verb && effect.exposure == exposure)
            .unwrap()
    }

    fn find_macro_tier<'a>(
        pack: &'a AcpRegistryPackProjection,
        macro_id: &str,
    ) -> &'a AcpMacroTierProjection {
        pack.macro_tiers
            .iter()
            .find(|tier| tier.macro_id == macro_id)
            .unwrap()
    }

    fn find_plan<'a>(
        projection: &'a AcpRegistryProjection,
        plan_id: &str,
    ) -> &'a AcpWorkbookPlanProjection {
        projection
            .packs
            .iter()
            .flat_map(|pack| pack.workbook_plans.iter())
            .find(|plan| plan.plan_id == plan_id)
            .unwrap()
    }

    fn count_macro_tier(pack: &AcpRegistryPackProjection, tier: &str) -> usize {
        pack.macro_tiers
            .iter()
            .filter(|projection| projection.tier == tier)
            .count()
    }

    fn walk_yaml_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        let mut dirs = vec![dir.to_path_buf()];
        while let Some(current) = dirs.pop() {
            for entry in std::fs::read_dir(&current).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else if matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("yaml" | "yml")
                ) {
                    files.push(path);
                }
            }
        }
        files.sort();
        files
    }
}
