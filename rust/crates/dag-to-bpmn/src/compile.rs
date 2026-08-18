//! Public entry point: `Dag` + slot id -> validated BPMN template.

use dsl_lowering::JourneySpec;
use dsl_types::{Dag, SlotStateMachine};

use crate::emit::emit_dsl_source;
use crate::error::{DagToBpmnError, ShapeError};

/// A compiled BPMN workflow template: the emitted bpmn-lite DSL source text,
/// and the `JourneySpec` that `dsl_migrate_verify::compile_to_spec` produced
/// from it — proof the template is structurally valid BPMN, not just
/// well-formed DSL syntax.
#[derive(Debug, Clone)]
pub struct CompiledTemplate {
    /// Human-readable bpmn-lite DSL source. This *is* the workflow
    /// template artifact.
    pub dsl_source: String,
    /// The validated, lowered process definition.
    pub spec: JourneySpec,
}

/// Compile `dag.slots[slot_id].state_machine` into a BPMN workflow
/// template.
///
/// `process_name` is passed straight through to `dsl_lowering::lower` and
/// becomes `JourneySpec::name`.
pub fn compile_slot(
    dag: &Dag,
    slot_id: &str,
    process_name: &str,
) -> Result<CompiledTemplate, DagToBpmnError> {
    let slot = dag
        .slots
        .iter()
        .find(|s| s.id == slot_id)
        .ok_or_else(|| ShapeError::SlotNotFound {
            slot_id: slot_id.to_string(),
        })?;

    let state_machine = match &slot.state_machine {
        None => {
            return Err(ShapeError::SlotHasNoStateMachine {
                slot_id: slot_id.to_string(),
            }
            .into())
        }
        Some(SlotStateMachine::Reference(reference)) => {
            return Err(ShapeError::SlotStateMachineIsReference {
                slot_id: slot_id.to_string(),
                reference: reference.clone(),
            }
            .into())
        }
        Some(SlotStateMachine::Structured(sm)) => sm.as_ref(),
    };

    let dsl_source = emit_dsl_source(slot_id, dag, state_machine)?;

    let spec = dsl_migrate_verify::compile_to_spec(&dsl_source, process_name)
        .map_err(DagToBpmnError::Pipeline)?;

    Ok(CompiledTemplate { dsl_source, spec })
}
