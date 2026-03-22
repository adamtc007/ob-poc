//! Harness helpers that run calibration utterances through the live agent service.

use anyhow::{anyhow, Context, Result};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use crate::api::agent_service::AgentService;
use crate::api::session::BoundEntity as ApiBoundEntity;
use crate::calibration::{CalibrationScenario, FixtureStateSnapshot};
use crate::dsl_v2::runtime_registry::runtime_registry;
use crate::sem_reg::abac::ActorContext;
use crate::session::{BoundEntity, UnifiedSession};
use crate::traceability::{UtteranceTraceRecord, UtteranceTraceRepository};

/// Fixture set used by a calibration run.
pub struct CalibrationFixtures {
    pub session: UnifiedSession,
    pub entities: HashMap<String, FixtureEntity>,
}

/// One known fixture entity.
#[derive(Debug, Clone)]
pub struct FixtureEntity {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub current_state: String,
}

impl CalibrationFixtures {
    /// Build an empty fixture set around a session.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::calibration::CalibrationFixtures;
    /// use ob_poc::session::UnifiedSession;
    ///
    /// let fixtures = CalibrationFixtures::new(UnifiedSession::new());
    /// assert!(fixtures.entities.is_empty());
    /// ```
    pub fn new(session: UnifiedSession) -> Self {
        Self {
            session,
            entities: HashMap::new(),
        }
    }

    /// Prime the session with synthetic bound entities derived from one scenario.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::calibration::{CalibrationExecutionShape, CalibrationFixtures, CalibrationScenario, GovernanceStatus};
    /// use ob_poc::session::UnifiedSession;
    /// use uuid::Uuid;
    ///
    /// let scenario = CalibrationScenario {
    ///     scenario_id: Uuid::nil(),
    ///     scenario_name: "demo".into(),
    ///     created_by: "test".into(),
    ///     governance_status: GovernanceStatus::Draft,
    ///     constellation_template_id: "demo".into(),
    ///     constellation_template_version: "v1".into(),
    ///     situation_signature: "entity:ACTIVE".into(),
    ///     situation_signature_hash: Some(1),
    ///     operational_phase: "Active".into(),
    ///     target_entity_type: "entity".into(),
    ///     target_entity_state: "ACTIVE".into(),
    ///     linked_entity_states: vec![("case".into(), "OPEN".into())],
    ///     target_verb: "entity.read".into(),
    ///     legal_verb_set_snapshot: vec![],
    ///     verb_taxonomy_tag: "read".into(),
    ///     excluded_neighbours: vec![],
    ///     near_neighbour_verbs: vec![],
    ///     expected_margin_threshold: 0.1,
    ///     execution_shape: CalibrationExecutionShape::Singleton,
    ///     gold_utterances: vec![],
    ///     admitted_synthetic_set_id: None,
    /// };
    /// let mut fixtures = CalibrationFixtures::new(UnifiedSession::new());
    /// fixtures.prime_for_scenario(&scenario);
    /// assert!(!fixtures.entities.is_empty());
    /// ```
    pub fn prime_for_scenario(&mut self, scenario: &CalibrationScenario) {
        let target_entity = synthetic_entity(
            &scenario.target_entity_type,
            &scenario.target_entity_state,
            &scenario.scenario_name,
        );
        self.entities
            .insert("target".to_string(), target_entity.clone());
        self.session
            .set_binding("target", bound_entity(&target_entity, "Target Entity"));

        if scenario.target_entity_type == "cbu" {
            self.session.context.active_cbu = Some(api_bound_entity(&target_entity, "Active CBU"));
        }

        for (index, (entity_type, entity_state)) in scenario.linked_entity_states.iter().enumerate()
        {
            let fixture = synthetic_entity(
                entity_type,
                entity_state,
                &format!("{} linked {}", scenario.scenario_name, index + 1),
            );
            let key = format!("linked_{}", index + 1);
            self.session.set_binding(
                &key,
                bound_entity(&fixture, &format!("{entity_type} {entity_state}")),
            );
            self.entities.insert(key, fixture);
        }
    }

    /// Replace synthetic placeholders with persisted fixture subjects when available.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationFixtures;
    /// use ob_poc::session::UnifiedSession;
    ///
    /// # async fn demo(pool: sqlx::PgPool, scenario: ob_poc::calibration::CalibrationScenario) -> anyhow::Result<()> {
    /// let mut fixtures = CalibrationFixtures::new(UnifiedSession::new());
    /// fixtures.hydrate_persisted_subjects(&pool, &scenario).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn hydrate_persisted_subjects(
        &mut self,
        pool: &PgPool,
        scenario: &CalibrationScenario,
    ) -> Result<()> {
        if scenario.target_entity_type.eq_ignore_ascii_case("cbu") {
            self.hydrate_persisted_cbu(pool, &scenario.target_entity_state)
                .await?;
        }
        Ok(())
    }

    /// Apply any lifecycle transition implied by the resolved trace verb.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::calibration::CalibrationFixtures;
    /// use ob_poc::session::UnifiedSession;
    ///
    /// let fixtures = CalibrationFixtures::new(UnifiedSession::new());
    /// assert!(fixtures.snapshot_state().is_empty());
    /// ```
    pub fn apply_trace_transition(&mut self, trace: &UtteranceTraceRecord) {
        let Some(resolved_verb) = trace.resolved_verb.as_deref() else {
            return;
        };
        let Some(target_state) = runtime_registry()
            .get_by_name(resolved_verb)
            .and_then(|verb| verb.lifecycle.as_ref())
            .and_then(|lifecycle| lifecycle.transitions_to.as_ref())
            .cloned()
        else {
            return;
        };

        let Some(binding_key) = self.resolve_transition_binding(resolved_verb) else {
            return;
        };
        let Some(entity) = self.entities.get_mut(&binding_key) else {
            return;
        };
        entity.current_state = target_state;
        let display_name = display_name_for_binding(&binding_key, entity);
        self.session
            .set_binding(&binding_key, bound_entity(entity, &display_name));
        if binding_key == "target" && entity.entity_type == "cbu" {
            self.session.context.active_cbu = Some(api_bound_entity(entity, "Active CBU"));
        }
    }

    /// Snapshot the current synthetic fixture state for persistence and reporting.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::calibration::CalibrationFixtures;
    /// use ob_poc::session::UnifiedSession;
    ///
    /// let fixtures = CalibrationFixtures::new(UnifiedSession::new());
    /// assert!(fixtures.snapshot_state().is_empty());
    /// ```
    pub fn snapshot_state(&self) -> Vec<FixtureStateSnapshot> {
        let mut rows: Vec<_> = self
            .entities
            .iter()
            .map(|(binding_key, entity)| FixtureStateSnapshot {
                binding_key: binding_key.clone(),
                entity_id: entity.entity_id,
                entity_type: entity.entity_type.clone(),
                current_state: entity.current_state.clone(),
            })
            .collect();
        rows.sort_by(|left, right| left.binding_key.cmp(&right.binding_key));
        rows
    }

    fn resolve_transition_binding(&self, resolved_verb: &str) -> Option<String> {
        let runtime_verb = runtime_registry().get_by_name(resolved_verb)?;
        if runtime_verb.subject_kinds.is_empty() {
            return self
                .entities
                .contains_key("target")
                .then(|| "target".to_string());
        }

        if self.entities.get("target").is_some_and(|entity| {
            matches_subject_kind(&runtime_verb.subject_kinds, &entity.entity_type)
        }) {
            return Some("target".to_string());
        }

        let mut matches = self
            .entities
            .iter()
            .filter(|(_, entity)| {
                matches_subject_kind(&runtime_verb.subject_kinds, &entity.entity_type)
            })
            .map(|(binding_key, _)| binding_key.clone());
        let first = matches.next()?;
        if matches.next().is_none() {
            Some(first)
        } else {
            None
        }
    }

    async fn hydrate_persisted_cbu(&mut self, pool: &PgPool, target_state: &str) -> Result<()> {
        let row = sqlx::query(
            r#"
            SELECT cbu_id, name
            FROM "ob-poc".cbus
            WHERE status = $1
            ORDER BY created_at DESC NULLS LAST, cbu_id DESC
            LIMIT 1
            "#,
        )
        .bind(target_state)
        .fetch_optional(pool)
        .await
        .context("load persisted calibration CBU fixture")?;

        let Some(row) = row else {
            return Ok(());
        };

        let entity = FixtureEntity {
            entity_id: row.get("cbu_id"),
            entity_type: "cbu".to_string(),
            current_state: target_state.to_string(),
        };
        let display_name: String = row.get("name");
        self.entities.insert("target".to_string(), entity.clone());
        self.session.entity_id = Some(entity.entity_id);
        self.session.add_cbu(entity.entity_id);
        self.session
            .set_binding("target", bound_entity(&entity, &display_name));
        self.session
            .set_binding("cbu", bound_entity(&entity, &display_name));
        self.session.context.last_cbu_id = Some(entity.entity_id);
        self.session.context.dominant_entity_id = Some(entity.entity_id);
        if !self.session.context.cbu_ids.contains(&entity.entity_id) {
            self.session.context.cbu_ids.push(entity.entity_id);
        }
        self.session
            .context
            .set_binding("cbu", entity.entity_id, "cbu", &display_name);
        self.session
            .context
            .set_binding("target", entity.entity_id, "cbu", &display_name);
        self.session
            .context
            .named_refs
            .insert("cbu_id".to_string(), entity.entity_id);
        self.session
            .context
            .set_active_cbu(entity.entity_id, &display_name);
        Ok(())
    }
}

fn synthetic_entity(entity_type: &str, state: &str, seed_name: &str) -> FixtureEntity {
    FixtureEntity {
        entity_id: Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            format!("calibration:{entity_type}:{state}:{seed_name}").as_bytes(),
        ),
        entity_type: entity_type.to_string(),
        current_state: state.to_string(),
    }
}

fn bound_entity(entity: &FixtureEntity, display_name: &str) -> BoundEntity {
    BoundEntity {
        id: entity.entity_id,
        entity_type: entity.entity_type.clone(),
        display_name: display_name.to_string(),
    }
}

fn api_bound_entity(entity: &FixtureEntity, display_name: &str) -> ApiBoundEntity {
    ApiBoundEntity {
        id: entity.entity_id,
        entity_type: entity.entity_type.clone(),
        display_name: display_name.to_string(),
    }
}

fn display_name_for_binding(binding_key: &str, entity: &FixtureEntity) -> String {
    if binding_key == "target" {
        "Target Entity".to_string()
    } else {
        format!("{} {}", entity.entity_type, entity.current_state)
    }
}

fn matches_subject_kind(subject_kinds: &[String], entity_type: &str) -> bool {
    subject_kinds
        .iter()
        .any(|subject_kind| subject_kind.eq_ignore_ascii_case(entity_type))
}

/// Execute one utterance through the live chat pipeline and return its trace ID.
///
/// # Examples
/// ```rust,no_run
/// use ob_poc::calibration::{execute_calibration_utterance, CalibrationFixtures};
///
/// # async fn demo(service: ob_poc::api::agent_service::AgentService, actor: ob_poc::sem_reg::abac::ActorContext, pool: sqlx::PgPool) -> anyhow::Result<()> {
/// let mut fixtures = CalibrationFixtures::new(ob_poc::session::UnifiedSession::new());
/// let _trace_id =
///     execute_calibration_utterance(&service, &mut fixtures, &actor, "show me the case", &pool)
///         .await?;
/// # Ok(())
/// # }
/// ```
pub async fn execute_calibration_utterance(
    service: &AgentService,
    fixtures: &mut CalibrationFixtures,
    actor: &ActorContext,
    utterance_text: &str,
    pool: &PgPool,
) -> Result<Uuid> {
    let request = ob_poc_types::ChatRequest {
        message: utterance_text.to_string(),
        cbu_id: None,
        disambiguation_response: None,
    };
    service
        .process_chat(&mut fixtures.session, &request, actor.clone())
        .await
        .map_err(|error| anyhow!(error))
        .context("execute calibration utterance through live chat pipeline")?;
    let trace_id = fixtures
        .session
        .last_trace_id
        .ok_or_else(|| anyhow!("chat pipeline completed without a trace_id"))?;
    let repository = UtteranceTraceRepository::new(pool.clone());
    repository
        .set_synthetic(trace_id, true)
        .await
        .context("mark calibration trace synthetic")?;
    Ok(trace_id)
}

/// Load a persisted trace record by trace ID.
///
/// # Examples
/// ```rust,no_run
/// use ob_poc::calibration::load_trace;
/// use uuid::Uuid;
///
/// # async fn demo(pool: sqlx::PgPool) -> anyhow::Result<()> {
/// let _trace = load_trace(&pool, Uuid::new_v4()).await?;
/// # Ok(())
/// # }
/// ```
pub async fn load_trace(pool: &PgPool, trace_id: Uuid) -> Result<UtteranceTraceRecord> {
    let repo = UtteranceTraceRepository::new(pool.clone());
    repo.get(trace_id)
        .await?
        .ok_or_else(|| anyhow!("trace not found: {}", trace_id))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::CalibrationFixtures;
    use crate::calibration::{CalibrationExecutionShape, CalibrationScenario, GovernanceStatus};
    use crate::session::UnifiedSession;
    use crate::traceability::{SurfaceVersions, TraceKind, TraceOutcome, UtteranceTraceRecord};

    fn scenario() -> CalibrationScenario {
        CalibrationScenario {
            scenario_id: Uuid::nil(),
            scenario_name: "demo".into(),
            created_by: "test".into(),
            governance_status: GovernanceStatus::Draft,
            constellation_template_id: "demo".into(),
            constellation_template_version: "v1".into(),
            situation_signature: "cbu:DISCOVERED".into(),
            situation_signature_hash: Some(1),
            operational_phase: "Active".into(),
            target_entity_type: "cbu".into(),
            target_entity_state: "DISCOVERED".into(),
            linked_entity_states: vec![],
            target_verb: "cbu.read".into(),
            legal_verb_set_snapshot: vec![],
            verb_taxonomy_tag: "read".into(),
            excluded_neighbours: vec![],
            near_neighbour_verbs: vec![],
            expected_margin_threshold: 0.1,
            execution_shape: CalibrationExecutionShape::Singleton,
            gold_utterances: vec![],
            admitted_synthetic_set_id: None,
        }
    }

    fn trace_with_resolved_verb(resolved_verb: String) -> UtteranceTraceRecord {
        UtteranceTraceRecord {
            trace_id: Uuid::new_v4(),
            utterance_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            correlation_id: None,
            trace_kind: TraceKind::Original,
            parent_trace_id: None,
            timestamp: Utc::now(),
            raw_utterance: "demo".into(),
            is_synthetic: true,
            outcome: TraceOutcome::ExecutedSuccessfully,
            halt_reason_code: None,
            halt_phase: None,
            resolved_verb: Some(resolved_verb),
            plane: None,
            polarity: None,
            execution_shape_kind: None,
            fallback_invoked: false,
            fallback_reason_code: None,
            situation_signature_hash: Some(1),
            template_id: None,
            template_version: None,
            surface_versions: SurfaceVersions::default(),
            trace_payload: serde_json::json!({}),
        }
    }

    #[test]
    fn snapshot_state_returns_sorted_fixture_rows() {
        let mut fixtures = CalibrationFixtures::new(UnifiedSession::new());
        fixtures.prime_for_scenario(&scenario());
        let state = fixtures.snapshot_state();
        assert_eq!(state.len(), 1);
        assert_eq!(state[0].binding_key, "target");
        assert_eq!(state[0].entity_type, "cbu");
    }

    #[test]
    fn apply_trace_transition_updates_target_state_when_registry_has_transition() {
        let Some(runtime_verb) = crate::dsl_v2::runtime_registry::runtime_registry()
            .all_verbs()
            .find(|verb| {
                verb.lifecycle
                    .as_ref()
                    .and_then(|lifecycle| lifecycle.transitions_to.as_ref())
                    .is_some()
                    && verb
                        .subject_kinds
                        .iter()
                        .any(|kind| kind.eq_ignore_ascii_case("cbu"))
            })
        else {
            return;
        };

        let mut fixtures = CalibrationFixtures::new(UnifiedSession::new());
        fixtures.prime_for_scenario(&scenario());
        let target_state = runtime_verb
            .lifecycle
            .as_ref()
            .and_then(|lifecycle| lifecycle.transitions_to.as_ref())
            .expect("transition state")
            .clone();

        fixtures.apply_trace_transition(&trace_with_resolved_verb(runtime_verb.full_name.clone()));

        assert_eq!(
            fixtures
                .entities
                .get("target")
                .map(|entity| entity.current_state.as_str()),
            Some(target_state.as_str())
        );
    }
}
