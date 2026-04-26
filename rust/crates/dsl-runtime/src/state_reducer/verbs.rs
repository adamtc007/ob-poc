use std::collections::HashMap;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sem_os_core::constellation_map_def::{ConstellationMapDefBody, SlotDef, SlotType};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::ast::{
    BlockReason, BlockedVerb, BlockedWhyResult, ConditionEvaluation, ConsistencyWarning,
    DerivationTrace, EvalScope, OverrideInfo, RuleEvaluation, SlotOverlayData, SlotRecord,
    SlotReduceResult,
};
use super::eval::ConditionEvaluator;
use super::overrides::{
    create_override, get_active_override, list_active_overrides, revoke_override,
    CreateOverrideRequest, StateOverride,
};
use super::{fetch_slot_overlays, ValidatedStateMachine};

/// Compute reducer state for a single slot.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool, sm: &dsl_runtime::state_reducer::ValidatedStateMachine) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::handle_state_derive;
///
/// let _ = handle_state_derive(
///     pool,
///     Uuid::new_v4(),
///     Uuid::new_v4(),
///     "entity.primary",
///     None,
///     sm,
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_state_derive(
    pool: &PgPool,
    cbu_id: Uuid,
    entity_id: Uuid,
    slot_path: &str,
    case_id: Option<Uuid>,
    state_machine: &ValidatedStateMachine,
) -> Result<SlotReduceResult> {
    let mut overlays = fetch_slot_overlays(pool, cbu_id, entity_id, case_id).await?;
    let override_entry = get_active_override(pool, cbu_id, case_id, slot_path).await?;
    let scope = build_eval_scope(pool, cbu_id, case_id).await?;
    overlays.scope = scope.as_scope_data();
    let result = reduce_slot(state_machine, slot_path, &overlays, override_entry)?;
    persist_reducer_state(
        pool,
        &infer_entity_type(state_machine, slot_path),
        entity_id,
        &result,
    )
    .await?;
    Ok(result)
}

/// Compute a full reducer trace for a single slot.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool, sm: &dsl_runtime::state_reducer::ValidatedStateMachine) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::handle_state_diagnose;
///
/// let _ = handle_state_diagnose(
///     pool,
///     Uuid::new_v4(),
///     Uuid::new_v4(),
///     "entity.primary",
///     None,
///     sm,
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_state_diagnose(
    pool: &PgPool,
    cbu_id: Uuid,
    entity_id: Uuid,
    slot_path: &str,
    case_id: Option<Uuid>,
    state_machine: &ValidatedStateMachine,
) -> Result<DerivationTrace> {
    let mut overlays = fetch_slot_overlays(pool, cbu_id, entity_id, case_id).await?;
    let override_entry = get_active_override(pool, cbu_id, case_id, slot_path).await?;
    let scope = build_eval_scope(pool, cbu_id, case_id).await?;
    overlays.scope = scope.as_scope_data();
    diagnose_slot(
        state_machine,
        entity_id,
        slot_path,
        &overlays,
        override_entry,
    )
}

/// Compute reducer state for all discovered slots in a constellation.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool, sm: &dsl_runtime::state_reducer::ValidatedStateMachine) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::handle_state_derive_all;
///
/// let _results = handle_state_derive_all(pool, Uuid::new_v4(), None, sm).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_state_derive_all(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    state_machine: &ValidatedStateMachine,
) -> Result<Vec<SlotReduceResult>> {
    if let Ok(map) = load_sem_os_builtin_constellation_map("struct.lux.ucits.sicav") {
        let contexts =
            discover_sem_os_slot_contexts(pool, cbu_id, case_id, &map, &state_machine.name)
                .await
                .unwrap_or_default();
        if !contexts.is_empty() {
            return evaluate_slot_contexts(pool, cbu_id, case_id, state_machine, contexts).await;
        }
    }
    let contexts = load_slot_contexts(pool, cbu_id, case_id).await?;
    evaluate_slot_contexts(pool, cbu_id, case_id, state_machine, contexts).await
}

/// Explain why a verb is blocked for the current slot state.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool, sm: &dsl_runtime::state_reducer::ValidatedStateMachine) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::handle_state_blocked_why;
///
/// let _ = handle_state_blocked_why(
///     pool,
///     Uuid::new_v4(),
///     Uuid::new_v4(),
///     "entity.primary",
///     "case.approve",
///     None,
///     sm,
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_state_blocked_why(
    pool: &PgPool,
    cbu_id: Uuid,
    entity_id: Uuid,
    slot_path: &str,
    verb: &str,
    case_id: Option<Uuid>,
    state_machine: &ValidatedStateMachine,
) -> Result<BlockedWhyResult> {
    let result =
        handle_state_derive(pool, cbu_id, entity_id, slot_path, case_id, state_machine).await?;
    let blocked = !result.available_verbs.iter().any(|item| item == verb);
    let reasons = if blocked {
        blocked_verbs_with_reasons(state_machine, &result.effective_state)
            .into_iter()
            .find(|item| item.verb == verb)
            .map(|item| item.reasons)
            .unwrap_or_else(|| {
                vec![BlockReason {
                    message: format!(
                        "verb '{verb}' is not available from reducer state '{}'",
                        result.effective_state
                    ),
                }]
            })
    } else {
        Vec::new()
    };

    Ok(BlockedWhyResult {
        blocked,
        verb: verb.to_string(),
        reasons,
    })
}

/// Run consistency checks across all discovered slots in a constellation.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool, sm: &dsl_runtime::state_reducer::ValidatedStateMachine) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::handle_state_check_consistency;
///
/// let _warnings = handle_state_check_consistency(pool, Uuid::new_v4(), None, sm).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_state_check_consistency(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    state_machine: &ValidatedStateMachine,
) -> Result<Vec<ConsistencyWarning>> {
    let contexts = if let Ok(map) = load_sem_os_builtin_constellation_map("struct.lux.ucits.sicav")
    {
        let discovered =
            discover_sem_os_slot_contexts(pool, cbu_id, case_id, &map, &state_machine.name)
                .await
                .unwrap_or_default();
        if discovered.is_empty() {
            load_slot_contexts(pool, cbu_id, case_id).await?
        } else {
            discovered
        }
    } else {
        load_slot_contexts(pool, cbu_id, case_id).await?
    };
    let evaluated = evaluate_slot_contexts(pool, cbu_id, case_id, state_machine, contexts).await?;
    Ok(evaluated
        .into_iter()
        .flat_map(|result| {
            result
                .consistency_warnings
                .into_iter()
                .map(move |warning| ConsistencyWarning {
                    slot_path: result.slot_path.clone(),
                    detail: Some(format!("computed_state={}", result.computed_state)),
                    warning,
                })
        })
        .collect())
}

/// Create a manual override for a slot.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool, sm: &dsl_runtime::state_reducer::ValidatedStateMachine) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::handle_state_override;
///
/// let _ = handle_state_override(
///     pool,
///     Uuid::new_v4(),
///     None,
///     "entity_kyc_lifecycle",
///     "entity.primary",
///     Uuid::new_v4(),
///     "approved",
///     "manual steward decision",
///     "compliance",
///     None,
///     None,
///     sm,
/// ).await?;
/// # Ok(())
/// # }
/// ```
#[allow(clippy::too_many_arguments)]
pub async fn handle_state_override(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    constellation_type: &str,
    slot_path: &str,
    entity_id: Uuid,
    override_state: &str,
    justification: &str,
    authority: &str,
    expires_at: Option<DateTime<Utc>>,
    conditions: Option<&str>,
    state_machine: &ValidatedStateMachine,
) -> Result<StateOverride> {
    if !state_machine
        .states
        .iter()
        .any(|state| state == override_state)
    {
        return Err(anyhow!(
            "'{}' is not a valid reducer state in '{}'",
            override_state,
            state_machine.name
        ));
    }

    let current =
        handle_state_derive(pool, cbu_id, entity_id, slot_path, case_id, state_machine).await?;
    let req = CreateOverrideRequest {
        cbu_id,
        case_id,
        constellation_type: constellation_type.to_string(),
        slot_path: slot_path.to_string(),
        computed_state: current.computed_state,
        override_state: override_state.to_string(),
        justification: justification.to_string(),
        authority: authority.to_string(),
        conditions: conditions.map(ToString::to_string),
        reducer_revision: state_machine.reducer_revision.clone(),
        expires_at,
    };
    create_override(pool, req).await
}

/// Revoke an override.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::handle_state_revoke_override;
///
/// handle_state_revoke_override(pool, Uuid::new_v4(), "operator", "superseded").await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_state_revoke_override(
    pool: &PgPool,
    override_id: Uuid,
    revoked_by: &str,
    reason: &str,
) -> Result<()> {
    revoke_override(pool, override_id, revoked_by, reason).await
}

/// List active overrides for a CBU.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::handle_state_list_overrides;
///
/// let _ = handle_state_list_overrides(pool, Uuid::new_v4()).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_state_list_overrides(
    pool: &PgPool,
    cbu_id: Uuid,
) -> Result<Vec<StateOverride>> {
    list_active_overrides(pool, cbu_id).await
}

/// Reduce a slot from already-fetched overlays.
///
/// # Examples
/// ```rust
/// use std::collections::HashMap;
/// use dsl_runtime::state_reducer::{load_builtin_state_machine, reduce_slot, ScopeData, SlotOverlayData};
///
/// let machine = load_builtin_state_machine("entity_kyc_lifecycle").unwrap();
/// let overlays = SlotOverlayData {
///     sources: HashMap::new(),
///     scope: ScopeData { fields: serde_json::json!({}) },
///     slots: vec![],
/// };
/// let result = reduce_slot(&machine, "entity.primary", &overlays, None).unwrap();
/// assert_eq!(result.slot_path, "entity.primary");
/// ```
pub fn reduce_slot(
    state_machine: &ValidatedStateMachine,
    slot_path: &str,
    overlays: &SlotOverlayData,
    override_entry: Option<StateOverride>,
) -> super::ReducerResult<SlotReduceResult> {
    let mut evaluator =
        ConditionEvaluator::new(&state_machine.eval_order, &state_machine.conditions);
    let condition_results = evaluator.evaluate_all(overlays)?;
    let computed_state = super::evaluate_rules(&state_machine.rules, &condition_results)?;
    let effective_state = override_entry
        .as_ref()
        .map(|entry| entry.override_state.clone())
        .unwrap_or_else(|| computed_state.clone());
    let consistency_warnings =
        compute_consistency_warnings(&state_machine.rules, &computed_state, &condition_results);

    Ok(SlotReduceResult {
        slot_path: slot_path.to_string(),
        computed_state,
        effective_state: effective_state.clone(),
        override_entry,
        available_verbs: available_verbs_for_state(state_machine, &effective_state),
        blocked_verbs: blocked_verbs_with_reasons(state_machine, &effective_state),
        consistency_warnings,
        reducer_revision: state_machine.reducer_revision.clone(),
    })
}

/// Diagnose a slot from already-fetched overlays and return a full trace.
///
/// # Examples
/// ```rust
/// use std::collections::HashMap;
/// use uuid::Uuid;
/// use dsl_runtime::state_reducer::{diagnose_slot, load_builtin_state_machine, ScopeData, SlotOverlayData};
///
/// let machine = load_builtin_state_machine("entity_kyc_lifecycle").unwrap();
/// let overlays = SlotOverlayData {
///     sources: HashMap::new(),
///     scope: ScopeData { fields: serde_json::json!({}) },
///     slots: vec![],
/// };
/// let trace = diagnose_slot(&machine, Uuid::new_v4(), "entity.primary", &overlays, None).unwrap();
/// assert_eq!(trace.slot_path, "entity.primary");
/// ```
pub fn diagnose_slot(
    state_machine: &ValidatedStateMachine,
    entity_id: Uuid,
    slot_path: &str,
    overlays: &SlotOverlayData,
    override_entry: Option<StateOverride>,
) -> Result<DerivationTrace> {
    let mut evaluator =
        ConditionEvaluator::new(&state_machine.eval_order, &state_machine.conditions);
    let results = evaluator.evaluate_all(overlays)?;
    let conditions_evaluated = state_machine
        .eval_order
        .iter()
        .map(|name| ConditionEvaluation {
            name: name.clone(),
            result: results.get(name).copied().unwrap_or(false),
        })
        .collect::<Vec<_>>();

    let (computed_state, rules_evaluated) = evaluate_rules_traced(&state_machine.rules, &results)
        .map_err(|err| anyhow!(err.to_string()))?;
    let effective_state = override_entry
        .as_ref()
        .map(|entry| entry.override_state.clone())
        .unwrap_or_else(|| computed_state.clone());
    let blocked_verbs = blocked_verbs_with_reasons(state_machine, &effective_state);
    let consistency_warnings =
        compute_consistency_warnings(&state_machine.rules, &computed_state, &results);

    Ok(DerivationTrace {
        reducer_revision: state_machine.reducer_revision.clone(),
        slot_path: slot_path.to_string(),
        entity_id: Some(entity_id),
        state_machine: state_machine.name.clone(),
        computed_state,
        override_entry: override_entry.as_ref().map(|entry| OverrideInfo {
            override_state: entry.override_state.clone(),
            authority: entry.authority.clone(),
            justification: entry.justification.clone(),
            expires_at: entry.expires_at,
        }),
        effective_state: effective_state.clone(),
        conditions_evaluated,
        rules_evaluated,
        available_verbs: available_verbs_for_state(state_machine, &effective_state),
        blocked_verbs,
        consistency_warnings,
    })
}

pub(crate) fn available_verbs_for_state(
    state_machine: &ValidatedStateMachine,
    state: &str,
) -> Vec<String> {
    state_machine
        .transitions
        .iter()
        .filter(|transition| transition.from == state)
        .flat_map(|transition| transition.verbs.iter().cloned())
        .collect()
}

pub(crate) fn blocked_verbs_with_reasons(
    state_machine: &ValidatedStateMachine,
    current_state: &str,
) -> Vec<BlockedVerb> {
    state_machine
        .transitions
        .iter()
        .filter(|transition| transition.from != current_state)
        .flat_map(|transition| {
            transition.verbs.iter().map(|verb| BlockedVerb {
                verb: verb.clone(),
                reasons: vec![BlockReason {
                    message: format!(
                        "transition requires current state '{}' but slot is '{}'",
                        transition.from, current_state
                    ),
                }],
            })
        })
        .collect()
}

pub(crate) async fn build_eval_scope(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
) -> Result<EvalScope> {
    let mut scope = EvalScope {
        cbu_id: Some(cbu_id),
        case_id,
        case_status: None,
        fields: HashMap::new(),
    };

    if let Some(case_id) = case_id {
        if let Some(status) = sqlx::query_scalar::<_, String>(
            r#"
            SELECT status
            FROM "ob-poc".cases
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_optional(pool)
        .await?
        {
            scope.case_status = Some(status);
        }
    }

    Ok(scope)
}

pub async fn build_eval_scope_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
) -> Result<EvalScope> {
    let mut scope = EvalScope {
        cbu_id: Some(cbu_id),
        case_id,
        case_status: None,
        fields: HashMap::new(),
    };

    if let Some(case_id) = case_id {
        if let Some(status) = sqlx::query_scalar::<_, String>(
            r#"
            SELECT status
            FROM "ob-poc".cases
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&mut **tx)
        .await?
        {
            scope.case_status = Some(status);
        }
    }

    Ok(scope)
}

fn evaluate_rules_traced(
    rules: &[super::RuleDef],
    results: &HashMap<String, bool>,
) -> super::ReducerResult<(String, Vec<RuleEvaluation>)> {
    let mut trace = Vec::new();
    for rule in rules {
        let requires_ok = rule
            .requires
            .iter()
            .all(|name| results.get(name).copied().unwrap_or(false));
        let excludes_ok = rule
            .excludes
            .iter()
            .all(|name| !results.get(name).copied().unwrap_or(false));
        let matched = requires_ok && excludes_ok;
        trace.push(RuleEvaluation {
            state: rule.state.clone(),
            matched,
        });
        if matched {
            return Ok((rule.state.clone(), trace));
        }
    }

    Err(super::ReducerError::Evaluation(
        "no reducer rule matched the evaluated condition set".into(),
    ))
}

fn compute_consistency_warnings(
    rules: &[super::RuleDef],
    computed_state: &str,
    results: &HashMap<String, bool>,
) -> Vec<String> {
    rules
        .iter()
        .find(|rule| rule.state == computed_state)
        .and_then(|rule| rule.consistency_check.as_ref())
        .filter(|check| !results.get(&check.warn_unless).copied().unwrap_or(false))
        .map(|check| vec![check.warning.clone()])
        .unwrap_or_default()
}

#[derive(Clone)]
struct SlotContext {
    entity_id: Uuid,
    slot_path: String,
    slot_type: String,
    cardinality: String,
}

fn load_sem_os_builtin_constellation_map(name: &str) -> Result<ConstellationMapDefBody> {
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(anyhow!("invalid built-in constellation map '{name}'"));
    }

    let filename = format!("{}.yaml", name.replace(['.', '-'], "_"));
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/sem_os_seeds/constellation_maps")
        .join(filename);
    let yaml = std::fs::read_to_string(&path)?;
    Ok(serde_yaml::from_str(&yaml)?)
}

async fn discover_sem_os_slot_contexts(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map: &ConstellationMapDefBody,
    state_machine_name: &str,
) -> Result<Vec<SlotContext>> {
    let mut discovered_slots = Vec::new();
    collect_sem_os_slots(&mut discovered_slots, &[], &map.slots, state_machine_name);
    let mut contexts = Vec::new();
    for (qualified_slot, slot) in discovered_slots {
        contexts.extend(
            sem_os_slot_contexts_for_slot(pool, cbu_id, case_id, &qualified_slot, &slot).await?,
        );
    }
    Ok(contexts)
}

fn collect_sem_os_slots(
    out: &mut Vec<(String, SlotDef)>,
    prefix: &[String],
    slots: &std::collections::BTreeMap<String, SlotDef>,
    state_machine_name: &str,
) {
    for (slot_name, slot) in slots {
        let mut path = prefix.to_vec();
        path.push(slot_name.clone());
        let qualified_slot = path.join(".");

        if slot.state_machine.as_deref() == Some(state_machine_name) {
            out.push((qualified_slot.clone(), slot.clone()));
        }

        if !slot.children.is_empty() {
            collect_sem_os_slots(out, &path, &slot.children, state_machine_name);
        }
    }
}

async fn sem_os_slot_contexts_for_slot(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    qualified_slot: &str,
    slot: &SlotDef,
) -> Result<Vec<SlotContext>> {
    match slot.slot_type {
        SlotType::Entity => sem_os_entity_slot_contexts(pool, cbu_id, qualified_slot, slot).await,
        SlotType::Case => {
            if let Some(case_id) = case_id {
                Ok(vec![SlotContext {
                    entity_id: case_id,
                    slot_path: qualified_slot.to_string(),
                    slot_type: "case".to_string(),
                    cardinality: format!("{:?}", slot.cardinality).to_ascii_lowercase(),
                }])
            } else {
                Ok(Vec::new())
            }
        }
        _ => Ok(Vec::new()),
    }
}

async fn sem_os_entity_slot_contexts(
    pool: &PgPool,
    cbu_id: Uuid,
    qualified_slot: &str,
    slot: &SlotDef,
) -> Result<Vec<SlotContext>> {
    let Some(join) = slot.join.as_ref() else {
        return Ok(Vec::new());
    };

    let via = checked_ident(&join.via)?;
    let parent_fk = checked_ident(&join.parent_fk)?;
    let child_fk = checked_ident(&join.child_fk)?;
    let mut query = format!(
        r#"SELECT DISTINCT "{child_fk}" AS entity_id FROM "ob-poc"."{via}" WHERE "{parent_fk}" = $1"#
    );
    let rows = if let (Some(filter_column), Some(filter_value)) =
        (join.filter_column.as_ref(), join.filter_value.as_ref())
    {
        let filter_column = checked_ident(filter_column)?;
        query.push_str(&format!(
            r#" AND "{filter_column}" = $2 ORDER BY "{child_fk}""#
        ));
        sqlx::query_scalar::<_, Uuid>(&query)
            .bind(cbu_id)
            .bind(filter_value)
            .fetch_all(pool)
            .await?
    } else {
        query.push_str(&format!(r#" ORDER BY "{child_fk}""#));
        sqlx::query_scalar::<_, Uuid>(&query)
            .bind(cbu_id)
            .fetch_all(pool)
            .await?
    };

    Ok(rows
        .into_iter()
        .map(|entity_id| SlotContext {
            entity_id,
            slot_path: qualified_slot.to_string(),
            slot_type: "entity".to_string(),
            cardinality: format!("{:?}", slot.cardinality).to_ascii_lowercase(),
        })
        .collect())
}

fn checked_ident(value: &str) -> Result<&str> {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        Ok(value)
    } else {
        Err(anyhow!(
            "unsupported identifier '{value}' in Sem OS slot discovery"
        ))
    }
}

async fn load_slot_contexts(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
) -> Result<Vec<SlotContext>> {
    let entity_ids = if let Some(case_id) = case_id {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT DISTINCT entity_id
            FROM "ob-poc".entity_workstreams
            WHERE case_id = $1
            ORDER BY entity_id
            "#,
        )
        .bind(case_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT DISTINCT entity_id
            FROM "ob-poc".cbu_entity_roles
            WHERE cbu_id = $1
            ORDER BY entity_id
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?
    };

    Ok(entity_ids
        .into_iter()
        .map(|entity_id| SlotContext {
            entity_id,
            slot_path: format!("entity.{entity_id}"),
            slot_type: String::from("entity"),
            cardinality: String::from("mandatory"),
        })
        .collect())
}

async fn evaluate_slot_contexts(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    state_machine: &ValidatedStateMachine,
    contexts: Vec<SlotContext>,
) -> Result<Vec<SlotReduceResult>> {
    let scope = build_eval_scope(pool, cbu_id, case_id).await?;
    let mut loaded = Vec::with_capacity(contexts.len());
    for context in contexts {
        let mut overlays = fetch_slot_overlays(pool, cbu_id, context.entity_id, case_id).await?;
        overlays.scope = scope.as_scope_data();
        let override_entry = get_active_override(pool, cbu_id, case_id, &context.slot_path).await?;
        let result = reduce_slot(state_machine, &context.slot_path, &overlays, override_entry)?;
        loaded.push((context, overlays, result));
    }

    let slot_records = loaded
        .iter()
        .map(|(context, _, result)| SlotRecord {
            slot_type: context.slot_type.clone(),
            cardinality: context.cardinality.clone(),
            effective_state: result.effective_state.clone(),
            computed_state: result.computed_state.clone(),
        })
        .collect::<Vec<_>>();

    let mut evaluated = Vec::with_capacity(loaded.len());
    for (context, mut overlays, _) in loaded {
        overlays.slots = slot_records.clone();
        let override_entry = get_active_override(pool, cbu_id, case_id, &context.slot_path).await?;
        let result = reduce_slot(state_machine, &context.slot_path, &overlays, override_entry)?;
        persist_reducer_state(
            pool,
            &infer_entity_type(state_machine, &context.slot_path),
            context.entity_id,
            &result,
        )
        .await?;
        evaluated.push(result);
    }

    Ok(evaluated)
}

async fn persist_reducer_state(
    pool: &PgPool,
    entity_type: &str,
    entity_id: Uuid,
    result: &SlotReduceResult,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sem_reg.reducer_states (
            entity_type, entity_id, current_state, lane, phase, computed_at
        ) VALUES (
            $1, $2, $3, $4, $5, now()
        )
        ON CONFLICT (entity_type, entity_id)
        DO UPDATE SET
            current_state = EXCLUDED.current_state,
            lane = EXCLUDED.lane,
            phase = EXCLUDED.phase,
            computed_at = EXCLUDED.computed_at
        "#,
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(&result.effective_state)
    .bind(Option::<String>::None)
    .bind(Some(result.computed_state.clone()))
    .execute(pool)
    .await?;
    Ok(())
}

fn infer_entity_type(state_machine: &ValidatedStateMachine, slot_path: &str) -> String {
    if state_machine.name.starts_with("entity_") {
        "entity".to_string()
    } else if state_machine.name.starts_with("kyc_case_") {
        "kyc-case".to_string()
    } else if state_machine.name.starts_with("ubo_") {
        "ubo".to_string()
    } else {
        slot_path.split('.').next().unwrap_or("entity").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{checked_ident, collect_sem_os_slots};
    use sem_os_core::constellation_map_def::ConstellationMapDefBody;

    #[test]
    fn sem_os_slot_collection_finds_nested_state_machine_slots() {
        let map: ConstellationMapDefBody = serde_yaml::from_str(
            r#"
fqn: demo
constellation: demo
jurisdiction: LU
slots:
  case:
    type: case
    table: cases
    pk: case_id
    cardinality: optional
    state_machine: kyc_case_lifecycle
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        cardinality: optional
        state_machine: tollgate_machine
  manager:
    type: entity
    table: entities
    pk: entity_id
    cardinality: mandatory
    state_machine: entity_kyc_lifecycle
"#,
        )
        .unwrap();

        let mut discovered = Vec::new();
        collect_sem_os_slots(&mut discovered, &[], &map.slots, "tollgate_machine");
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].0, "case.tollgate");
    }

    #[test]
    fn checked_ident_rejects_unsafe_identifiers() {
        assert!(checked_ident("cbu_entity_roles").is_ok());
        assert!(checked_ident("cbu-entity-roles").is_err());
        assert!(checked_ident("entity roles").is_err());
    }
}
