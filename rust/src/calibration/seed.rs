//! Scenario seed construction from live runtime metadata.

use anyhow::{anyhow, Result};
use sem_os_client::{inprocess::InProcessClient, SemOsClient};
use sem_os_core::authoring::agent_mode::AgentMode;
use sem_os_core::context_resolution::{
    ContextResolutionRequest, DiscoveryContext, EvidenceMode, SubjectRef,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::agent::learning::embedder::{CandleEmbedder, Embedder};
use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
use crate::agent::verb_surface::{
    compute_session_verb_surface, SessionVerbSurface, VerbSurfaceContext, VerbSurfaceFailPolicy,
};
use crate::dsl_v2::execution::runtime_registry_arc;
use crate::sem_os_runtime::constellation_runtime::{
    compute_map_revision, load_builtin_constellation_map,
};
use crate::sem_reg::abac::ActorContext;
use crate::sem_reg::agent::mcp_tools::build_sem_os_service;
use crate::sem_reg::types::Classification;
use crate::traceability::Phase2Service;

use super::types::{
    CalibrationExecutionShape, CalibrationScenario, ConfusionRisk, ExcludedNeighbour,
    GoldUtterance, GovernanceStatus, NearNeighbourVerb,
};

/// Build a calibration scenario from live runtime metadata.
///
/// # Examples
/// ```rust,no_run
/// use ob_poc::calibration::{build_scenario_seed, CalibrationExecutionShape};
///
/// # async fn demo(pool: sqlx::PgPool, embedder: ob_poc::agent::learning::embedder::CandleEmbedder) -> anyhow::Result<()> {
/// let _scenario = build_scenario_seed(
///     &pool,
///     &embedder,
///     "cbu-status",
///     "struct.lux.ucits.sicav",
///     "cbu",
///     "ACTIVE",
///     "cbu.get-status",
///     vec![],
///     CalibrationExecutionShape::Singleton,
///     0.08,
/// )
/// .await?;
/// # Ok(())
/// # }
/// ```
#[allow(clippy::too_many_arguments)]
pub async fn build_scenario_seed(
    pool: &PgPool,
    embedder: &CandleEmbedder,
    scenario_name: &str,
    template_id: &str,
    target_entity_type: &str,
    target_entity_state: &str,
    target_verb: &str,
    linked_entity_states: Vec<(String, String)>,
    execution_shape: CalibrationExecutionShape,
    margin_threshold: f32,
) -> Result<CalibrationScenario> {
    let _map = load_builtin_constellation_map(template_id)
        .map_err(|err| anyhow!("load_builtin_constellation_map({template_id}): {err}"))?;
    let yaml = built_in_map_yaml(template_id)?;
    let template_revision = compute_map_revision(&yaml);
    let signature = compute_situation_signature(
        target_entity_type,
        target_entity_state,
        &linked_entity_states,
    );
    let operational_phase = derive_operational_phase(&signature);
    let signature_hash = compute_situation_signature_hash(&signature);
    let legal_verbs = compute_live_legal_verb_set(
        pool,
        target_verb,
        target_entity_type,
        target_entity_state,
        &operational_phase,
    )
    .await?;
    let verb_taxonomy_tag = classify_live_verb_metadata(target_verb);
    let near_neighbour_verbs =
        build_neighbour_set(embedder, target_verb, &legal_verbs, margin_threshold).await?;

    Ok(CalibrationScenario {
        scenario_id: Uuid::new_v4(),
        scenario_name: scenario_name.to_string(),
        created_by: "calibration.seed_builder".to_string(),
        governance_status: GovernanceStatus::Draft,
        constellation_template_id: template_id.to_string(),
        constellation_template_version: template_revision,
        situation_signature: signature,
        situation_signature_hash: Some(signature_hash),
        operational_phase,
        target_entity_type: target_entity_type.to_string(),
        target_entity_state: target_entity_state.to_string(),
        linked_entity_states,
        target_verb: target_verb.to_string(),
        legal_verb_set_snapshot: legal_verbs,
        verb_taxonomy_tag,
        excluded_neighbours: Vec::<ExcludedNeighbour>::new(),
        near_neighbour_verbs,
        expected_margin_threshold: margin_threshold,
        execution_shape,
        gold_utterances: Vec::<GoldUtterance>::new(),
        admitted_synthetic_set_id: None,
    })
}

/// Compute the canonical situation-signature string used by scenario seeds.
///
/// # Examples
/// ```rust
/// use ob_poc::calibration::compute_situation_signature;
///
/// let signature = compute_situation_signature("cbu", "ACTIVE", &[("kyc".into(), "OPEN".into())]);
/// assert_eq!(signature, "cbu:ACTIVE|kyc:OPEN");
/// ```
pub fn compute_situation_signature(
    entity_type: &str,
    entity_state: &str,
    linked: &[(String, String)],
) -> String {
    let mut parts = vec![format!("{entity_type}:{entity_state}")];
    for (entity_type, entity_state) in linked {
        parts.push(format!("{entity_type}:{entity_state}"));
    }
    parts.sort();
    parts.join("|")
}

/// Derive a coarse operational phase label from a scenario signature.
///
/// # Examples
/// ```rust
/// use ob_poc::calibration::derive_operational_phase;
///
/// assert_eq!(derive_operational_phase("cbu:ACTIVE|kyc:OPEN"), "KYCBlocked");
/// ```
pub fn derive_operational_phase(signature: &str) -> String {
    if signature.contains("cbu:DRAFT") || signature.contains("cbu:DISCOVERED") {
        "EarlyOnboarding".to_string()
    } else if signature.contains("kyc:OPEN") && signature.contains("cbu:ACTIVE") {
        "KYCBlocked".to_string()
    } else if signature.contains("cbu:VALIDATED") {
        "PreActivation".to_string()
    } else if signature.contains("cbu:ACTIVE") {
        "Active".to_string()
    } else if signature.contains("cbu:TERMINATED") {
        "Terminated".to_string()
    } else {
        "Unknown".to_string()
    }
}

fn compute_situation_signature_hash(signature: &str) -> i64 {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(signature.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    i64::from_be_bytes(bytes)
}

async fn compute_live_legal_verb_set(
    pool: &PgPool,
    target_verb: &str,
    target_entity_type: &str,
    target_entity_state: &str,
    operational_phase: &str,
) -> Result<Vec<String>> {
    let actor = calibration_actor();
    let principal =
        sem_os_core::principal::Principal::in_process(&actor.actor_id, actor.roles.clone());
    let core_actor: sem_os_core::abac::ActorContext = {
        let json = serde_json::to_value(&actor).expect("ActorContext serializes");
        serde_json::from_value(json).expect("ActorContext round-trips")
    };
    let stage_focus = infer_stage_focus(
        target_verb,
        target_entity_type,
        target_entity_state,
        operational_phase,
    );
    let service = build_sem_os_service(pool);
    let client = InProcessClient::new(service);
    let subject = persisted_subject_for_seed(pool, target_entity_type, target_entity_state).await?;
    let request = ContextResolutionRequest {
        subject,
        intent_summary: Some(format!(
            "calibration seed for {target_entity_type} in state {target_entity_state}"
        )),
        raw_utterance: None,
        actor: core_actor,
        goals: stage_focus_goals(stage_focus),
        constraints: Default::default(),
        evidence_mode: evidence_mode_for_focus(stage_focus),
        point_in_time: None,
        entity_kind: Some(target_entity_type.to_string()),
        entity_confidence: Some(1.0),
        discovery: DiscoveryContext::default(),
    };
    let response = client
        .resolve_context(&principal, request)
        .await
        .map_err(|error| anyhow!("resolve calibration SemOS context: {error}"))?;
    let envelope = SemOsContextEnvelope::from_resolution(&response);
    let surface = compute_live_session_surface(
        &envelope,
        target_verb,
        target_entity_type,
        target_entity_state,
        operational_phase,
    );
    let phase2 = Phase2Service::evaluate_from_envelope(envelope);
    if !phase2.is_available {
        anyhow::bail!("SemOS context resolution unavailable for calibration seed");
    }

    let mut verbs = surface.allowed_fqns().into_iter().collect::<Vec<_>>();
    verbs.sort();
    verbs.dedup();
    Ok(verbs)
}

async fn persisted_subject_for_seed(
    pool: &PgPool,
    target_entity_type: &str,
    target_entity_state: &str,
) -> Result<SubjectRef> {
    if target_entity_type.eq_ignore_ascii_case("cbu") {
        let cbu_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT cbu_id
            FROM "ob-poc".cbus
            WHERE status = $1
            ORDER BY created_at DESC NULLS LAST, cbu_id DESC
            LIMIT 1
            "#,
        )
        .bind(target_entity_state)
        .fetch_optional(pool)
        .await?;

        if let Some(cbu_id) = cbu_id {
            return Ok(SubjectRef::EntityId(cbu_id));
        }
    }

    Ok(SubjectRef::TaskId(Uuid::new_v4()))
}

fn compute_live_session_surface(
    envelope: &SemOsContextEnvelope,
    target_verb: &str,
    target_entity_type: &str,
    target_entity_state: &str,
    operational_phase: &str,
) -> SessionVerbSurface {
    let surface_ctx = VerbSurfaceContext {
        agent_mode: AgentMode::default(),
        stage_focus: infer_stage_focus(
            target_verb,
            target_entity_type,
            target_entity_state,
            operational_phase,
        ),
        envelope,
        fail_policy: VerbSurfaceFailPolicy::FailClosed,
        entity_state: Some(target_entity_state),
        has_group_scope: true,
        is_infrastructure_scope: false,
        composite_state: None,
    };
    compute_session_verb_surface(&surface_ctx)
}

fn calibration_actor() -> ActorContext {
    ActorContext {
        actor_id: "calibration.seed_builder".to_string(),
        roles: vec!["admin".to_string(), "ops".to_string()],
        department: Some("calibration".to_string()),
        clearance: Some(Classification::Restricted),
        jurisdictions: vec!["*".to_string()],
    }
}

fn infer_stage_focus<'a>(
    target_verb: &'a str,
    target_entity_type: &'a str,
    target_entity_state: &'a str,
    operational_phase: &'a str,
) -> Option<&'static str> {
    if target_entity_type == "cbu" && is_read_style_cbu_verb(target_verb) {
        return Some("semos-calibration");
    }
    if matches!(
        target_entity_type,
        "document" | "requirement" | "ubo" | "screening"
    ) || operational_phase == "KYCBlocked"
    {
        Some("semos-kyc")
    } else if target_entity_type == "cbu"
        && matches!(
            target_entity_state,
            "DISCOVERED" | "VALIDATION_FAILED" | "VALIDATION_PENDING" | "VALIDATED"
        )
    {
        Some("semos-onboarding")
    } else if matches!(
        target_entity_type,
        "deal" | "contract" | "billing" | "product" | "registry"
    ) {
        Some("semos-data-management")
    } else {
        None
    }
}

fn is_read_style_cbu_verb(target_verb: &str) -> bool {
    matches!(target_verb, "cbu.read" | "cbu.list" | "cbu.parties")
}

fn stage_focus_goals(stage_focus: Option<&str>) -> Vec<String> {
    match stage_focus {
        Some("semos-calibration") => Vec::new(),
        Some("semos-data-management") | Some("semos-data") => vec![
            "data-management".to_string(),
            "data".to_string(),
            "deal".to_string(),
            "onboarding".to_string(),
            "kyc".to_string(),
            "navigation".to_string(),
        ],
        Some(s) if s.starts_with("semos-") => {
            vec![s.strip_prefix("semos-").unwrap_or_default().to_string()]
        }
        _ => Vec::new(),
    }
}

fn evidence_mode_for_focus(stage_focus: Option<&str>) -> EvidenceMode {
    if matches!(stage_focus, Some("semos-data-management" | "semos-data")) {
        EvidenceMode::Exploratory
    } else {
        EvidenceMode::default()
    }
}

fn classify_live_verb_metadata(target_verb: &str) -> String {
    let registry = runtime_registry_arc();
    registry
        .get_by_name(target_verb)
        .map(|verb| {
            if matches!(
                verb.harm_class,
                Some(crate::dsl_v2::config::types::HarmClass::ReadOnly)
            ) {
                "read".to_string()
            } else if verb.harm_class.is_some() {
                "write".to_string()
            } else {
                "other".to_string()
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

async fn build_neighbour_set(
    embedder: &CandleEmbedder,
    target_verb: &str,
    legal_verbs: &[String],
    _margin_threshold: f32,
) -> Result<Vec<NearNeighbourVerb>> {
    let target_embedding = embedder.embed_target(target_verb).await?;
    let mut neighbours = Vec::new();
    for verb in legal_verbs {
        if verb == target_verb {
            continue;
        }
        let verb_embedding = embedder.embed_target(verb).await?;
        let distance = cosine_distance(&target_embedding, &verb_embedding);
        if distance < 0.40 {
            let confusion_risk = if distance < 0.15 {
                ConfusionRisk::High
            } else if distance < 0.25 {
                ConfusionRisk::Medium
            } else {
                ConfusionRisk::Low
            };
            neighbours.push(NearNeighbourVerb {
                verb_id: verb.clone(),
                expected_embedding_distance: distance,
                confusion_risk,
                distinguishing_signals: vec![format!("semantic_distance:{distance:.3}")],
            });
        }
    }
    neighbours.sort_by(|left, right| {
        left.expected_embedding_distance
            .partial_cmp(&right.expected_embedding_distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    neighbours.truncate(8);
    Ok(neighbours)
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    1.0 - (dot / (norm_a * norm_b))
}

fn built_in_map_yaml(template_id: &str) -> Result<String> {
    let filename = format!("{}.yaml", template_id.replace(['.', '-'], "_"));
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/sem_os_seeds/constellation_maps")
        .join(filename);
    std::fs::read_to_string(&path).map_err(|error| {
        anyhow!(
            "read built-in constellation map '{}': {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{derive_operational_phase, infer_stage_focus, stage_focus_goals};

    #[test]
    fn infer_stage_focus_prefers_kyc_when_operational_phase_is_blocked() {
        let phase = derive_operational_phase("cbu:ACTIVE|kyc:OPEN");
        assert_eq!(
            infer_stage_focus("cbu.update", "cbu", "ACTIVE", &phase),
            Some("semos-kyc")
        );
    }

    #[test]
    fn infer_stage_focus_maps_early_cbu_states_to_onboarding() {
        assert_eq!(
            infer_stage_focus("cbu.update", "cbu", "VALIDATION_PENDING", "EarlyOnboarding"),
            Some("semos-onboarding")
        );
    }

    #[test]
    fn infer_stage_focus_maps_read_style_cbu_verbs_to_calibration_focus() {
        assert_eq!(
            infer_stage_focus("cbu.read", "cbu", "DISCOVERED", "EarlyOnboarding"),
            Some("semos-calibration")
        );
    }

    #[test]
    fn calibration_focus_emits_no_goals() {
        assert!(stage_focus_goals(Some("semos-calibration")).is_empty());
    }
}
