//! Registration helpers for the boundary-side pack provider hooks.
//!
//! Phase 3D of capability-crate restructure (2026-05-13). The pack
//! catalogue's authoritative source is eventually SemOS-via-MCP; today
//! it's fed from disk. This module wires the two function-pointer
//! provider hooks in `ob_poc_boundary::pack_projection` so the
//! boundary-side runtime path (`acp_dag_semantic::semantic_index` and
//! `acp_registry_projection::build_slice1_acp_registry_projection`) can
//! resolve manifests without an in-crate `crate::journey::*` import.
//!
//! Production callers should invoke [`register_pack_providers`] exactly
//! once during app startup, before any path that exercises
//! `semantic_index()` or `build_slice1_acp_registry_projection()`.
//!
//! Test code reaches the same surface through boundary's
//! `#[cfg(test)] ensure_test_provider_registered()` fixture — it
//! mirrors `project_pack` below so behaviour stays in sync.
//!
//! The projection logic deliberately duplicates the structure of the
//! test fixture in `ob-poc-boundary/src/pack_projection.rs`. Boundary's
//! test fixture serves as the contract for what the integrator must
//! produce; tests will catch any drift.

use std::path::{Path, PathBuf};

use ob_poc_boundary::acp_dag_semantic::{
    workspace_context_name, AcpDagSemanticPackContext, AcpDagSemanticPackProgressSignal,
    AcpDagSemanticPackQuestion, AcpDagSemanticPackRiskPolicy, AcpDagSemanticPackSection,
    AcpDagSemanticPackTemplate, AcpDagSemanticPackTemplateStep,
};
use ob_poc_boundary::pack_projection::{
    set_pack_manifest_provider, set_pack_projection_provider, PackIndexing, PackProjection,
};
use ob_poc_journey::pack::load_packs_from_dir;
use ob_poc_types::journey::pack_types::PackManifest;

/// Default packs directory used by the projection provider. Override via
/// `OBPOC_PACKS_DIR` env var.
fn packs_dir() -> PathBuf {
    std::env::var("OBPOC_PACKS_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("rust/config/packs"))
}

/// Provider entry-point invoked once by `acp_dag_semantic::semantic_index()`.
fn provide_pack_projections() -> Result<Vec<PackProjection>, String> {
    let dir = packs_dir();
    let packs = load_packs_from_dir(&dir).map_err(|error| error.to_string())?;
    Ok(packs
        .into_iter()
        .map(|(manifest, hash)| project_pack(&manifest, hash))
        .collect())
}

/// Provider entry-point invoked by
/// `acp_registry_projection::build_slice1_acp_registry_projection`.
fn provide_pack_manifests(config_root: &Path) -> Result<Vec<(PackManifest, String)>, String> {
    load_packs_from_dir(&config_root.join("packs")).map_err(|error| error.to_string())
}

/// Wire both boundary-side provider hooks. Safe to call more than once;
/// `OnceLock` semantics make the first registration the winner.
pub fn register_pack_providers() {
    let _ = set_pack_projection_provider(provide_pack_projections);
    let _ = set_pack_manifest_provider(provide_pack_manifests);
}

/// Project a single `PackManifest` (plus its content hash) into the
/// boundary-side `PackProjection`. Mirrors boundary's
/// `#[cfg(test)] test_project_pack` exactly — if the two diverge, the
/// boundary test fixture catches it.
fn project_pack(manifest: &PackManifest, hash: String) -> PackProjection {
    let mut phrase_set = std::collections::BTreeSet::new();
    phrase_set.insert(manifest.id.clone());
    phrase_set.insert(manifest.name.clone());
    for phrase in &manifest.invocation_phrases {
        phrase_set.insert(phrase.clone());
    }
    for workspace in &manifest.workspaces {
        phrase_set.insert(workspace_context_name(workspace).replace('_', " "));
    }
    let indexing = PackIndexing {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        hash: hash.clone(),
        phrases: phrase_set.into_iter().collect(),
        allowed_verbs: manifest.allowed_verbs.iter().cloned().collect(),
    };

    let context = AcpDagSemanticPackContext {
        pack_id: manifest.id.clone(),
        pack_name: manifest.name.clone(),
        pack_version: manifest.version.clone(),
        pack_hash: hash,
        score: 0.0,
        matched_phrase: None,
        description: manifest.description.clone(),
        invocation_phrases: manifest.invocation_phrases.clone(),
        workspaces: manifest
            .workspaces
            .iter()
            .map(workspace_context_name)
            .collect(),
        required_context: manifest.required_context.clone(),
        optional_context: manifest.optional_context.clone(),
        allowed_verbs: manifest.allowed_verbs.clone(),
        allowed_verb_count: manifest.allowed_verbs.len(),
        forbidden_verbs: manifest.forbidden_verbs.clone(),
        risk_policy: AcpDagSemanticPackRiskPolicy {
            require_confirm_before_execute: manifest.risk_policy.require_confirm_before_execute,
            max_steps_without_confirm: manifest.risk_policy.max_steps_without_confirm,
        },
        required_questions: manifest
            .required_questions
            .iter()
            .map(|question| AcpDagSemanticPackQuestion {
                field: question.field.clone(),
                prompt: question.prompt.clone(),
                answer_kind: format!("{:?}", question.answer_kind),
                options_source: question.options_source.clone(),
                default: question.default.clone(),
                ask_when: question.ask_when.clone(),
            })
            .collect(),
        optional_questions: manifest
            .optional_questions
            .iter()
            .map(|question| AcpDagSemanticPackQuestion {
                field: question.field.clone(),
                prompt: question.prompt.clone(),
                answer_kind: format!("{:?}", question.answer_kind),
                options_source: question.options_source.clone(),
                default: question.default.clone(),
                ask_when: question.ask_when.clone(),
            })
            .collect(),
        stop_rules: manifest.stop_rules.clone(),
        templates: manifest
            .templates
            .iter()
            .map(|template| AcpDagSemanticPackTemplate {
                template_id: template.template_id.clone(),
                when_to_use: template.when_to_use.clone(),
                steps: template
                    .steps
                    .iter()
                    .map(|step| AcpDagSemanticPackTemplateStep {
                        verb: step.verb.clone(),
                        args: step
                            .args
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect(),
                        repeat_for: step.repeat_for.clone(),
                        when: step.when.clone(),
                        execution_mode: step.execution_mode.clone(),
                    })
                    .collect(),
            })
            .collect(),
        pack_summary_template: manifest.pack_summary_template.clone(),
        section_layout: manifest
            .section_layout
            .iter()
            .map(|section| AcpDagSemanticPackSection {
                title: section.title.clone(),
                verb_prefixes: section.verb_prefixes.clone(),
            })
            .collect(),
        definition_of_done: manifest.definition_of_done.clone(),
        progress_signals: manifest
            .progress_signals
            .iter()
            .map(|signal| AcpDagSemanticPackProgressSignal {
                signal: signal.signal.clone(),
                description: signal.description.clone(),
            })
            .collect(),
        handoff_target: manifest.handoff_target.clone(),
    };

    PackProjection { indexing, context }
}
