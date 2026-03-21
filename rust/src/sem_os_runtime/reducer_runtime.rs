use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::sem_os_runtime::constellation_runtime::{
    RuntimeBlockReason, RuntimeBlockedVerb, RuntimeOverlaySource, RuntimeSlotReduceResult,
    RuntimeStateMachine, RuntimeStateTransition,
};

pub(crate) type ReducerMachineBacking = crate::state_reducer::ValidatedStateMachine;
pub(crate) type RuntimeEvalScope = crate::state_reducer::EvalScope;
pub(crate) type ReducerOverlayData = crate::state_reducer::SlotOverlayData;

pub(crate) struct SlotRuntimeArtifacts {
    pub overlays: ReducerOverlayData,
    pub reducer_result: Option<RuntimeSlotReduceResult>,
}

pub(crate) fn load_runtime_state_machine(
    machine_name: &str,
) -> anyhow::Result<RuntimeStateMachine> {
    crate::state_reducer::load_builtin_state_machine(machine_name)
        .map(runtime_state_machine_from_reducer)
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

pub(crate) async fn build_eval_scope(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
) -> anyhow::Result<RuntimeEvalScope> {
    crate::state_reducer::build_eval_scope_tx(tx, cbu_id, case_id).await
}

pub(crate) async fn fetch_slot_overlays(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    entity_id: Uuid,
    case_id: Option<Uuid>,
) -> anyhow::Result<ReducerOverlayData> {
    crate::state_reducer::fetch_slot_overlays_tx(tx, cbu_id, entity_id, case_id).await
}

pub(crate) async fn get_active_override(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    slot_name: &str,
) -> anyhow::Result<Option<crate::state_reducer::overrides::StateOverride>> {
    crate::state_reducer::get_active_override_tx(tx, cbu_id, case_id, slot_name).await
}

pub(crate) async fn collect_slot_runtime_artifacts(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    entity_id: Uuid,
    slot_name: &str,
    machine: Option<&RuntimeStateMachine>,
    scope: Option<&RuntimeEvalScope>,
) -> anyhow::Result<SlotRuntimeArtifacts> {
    let mut overlays = fetch_slot_overlays(tx, cbu_id, entity_id, case_id).await?;
    let reducer_result = if let (Some(machine), Some(scope)) = (machine, scope) {
        overlays.scope = scope.as_scope_data();
        let override_entry = get_active_override(tx, cbu_id, case_id, slot_name).await?;
        Some(reduce_slot(machine, slot_name, &overlays, override_entry)?)
    } else {
        None
    };

    Ok(SlotRuntimeArtifacts {
        overlays,
        reducer_result,
    })
}

pub(crate) fn reduce_slot(
    machine: &RuntimeStateMachine,
    slot_name: &str,
    overlays: &ReducerOverlayData,
    override_entry: Option<crate::state_reducer::overrides::StateOverride>,
) -> anyhow::Result<RuntimeSlotReduceResult> {
    let reducer_machine = reducer_state_machine_from_runtime(machine);
    crate::state_reducer::reduce_slot(&reducer_machine, slot_name, overlays, override_entry)
        .map(runtime_reduce_result_from_reducer)
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

pub(crate) fn runtime_reduce_result_from_reducer(
    value: crate::state_reducer::SlotReduceResult,
) -> RuntimeSlotReduceResult {
    RuntimeSlotReduceResult {
        slot_path: value.slot_path,
        computed_state: value.computed_state,
        effective_state: value.effective_state,
        available_verbs: value.available_verbs,
        blocked_verbs: value
            .blocked_verbs
            .into_iter()
            .map(|blocked| RuntimeBlockedVerb {
                verb: blocked.verb,
                reasons: blocked
                    .reasons
                    .into_iter()
                    .map(|reason| RuntimeBlockReason {
                        message: reason.message,
                    })
                    .collect(),
            })
            .collect(),
        consistency_warnings: value.consistency_warnings,
        reducer_revision: value.reducer_revision,
    }
}

pub(crate) fn runtime_state_machine_from_reducer(
    machine: ReducerMachineBacking,
) -> RuntimeStateMachine {
    RuntimeStateMachine {
        name: machine.name.clone(),
        states: machine.states.clone(),
        initial: machine.initial.clone(),
        transitions: machine
            .transitions
            .iter()
            .map(|transition| RuntimeStateTransition {
                from: transition.from.clone(),
                to: transition.to.clone(),
                verbs: transition.verbs.clone(),
            })
            .collect(),
        overlay_sources: machine
            .overlay_sources
            .iter()
            .map(|(name, source)| {
                (
                    name.clone(),
                    RuntimeOverlaySource {
                        table: source.table.clone(),
                        join: source.join.clone(),
                        provides: source.provides.clone(),
                        cardinality: source.cardinality.clone(),
                    },
                )
            })
            .collect(),
        reducer_backing: machine,
    }
}

fn reducer_state_machine_from_runtime(machine: &RuntimeStateMachine) -> ReducerMachineBacking {
    machine.reducer_backing.clone()
}
