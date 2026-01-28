use playbook_core::{PlaybookSpec, StepSpec};
use crate::slots::{SlotState, SlotValue};
use std::collections::HashMap;

pub struct LowerResult {
    pub dsl_statements: Vec<String>,
    pub step_to_dsl: HashMap<String, usize>,
    pub missing_slots: Vec<MissingSlot>,
}

pub struct MissingSlot {
    pub name: String,
    pub step_id: String,
    pub step_index: usize,
}

pub fn lower_playbook(
    spec: &PlaybookSpec,
    slots: &SlotState,
) -> LowerResult {
    let mut dsl_statements = Vec::new();
    let mut step_to_dsl = HashMap::new();
    let mut missing_slots = Vec::new();
    
    for (idx, step) in spec.steps.iter().enumerate() {
        let (dsl, missing) = lower_step(step, slots, idx);
        missing_slots.extend(missing);
        step_to_dsl.insert(step.id.clone(), dsl_statements.len());
        dsl_statements.push(dsl);
    }
    
    LowerResult { dsl_statements, step_to_dsl, missing_slots }
}

fn lower_step(step: &StepSpec, slots: &SlotState, idx: usize) -> (String, Vec<MissingSlot>) {
    let mut missing = Vec::new();
    let mut args_str = String::new();
    
    for (key, val) in &step.args {
        let resolved = resolve_value(val, slots, &step.id, idx, &mut missing);
        args_str.push_str(&format!(" :{} {}", key, resolved));
    }
    
    let dsl = format!("({}{})", step.verb, args_str);
    (dsl, missing)
}

fn resolve_value(
    val: &serde_yaml::Value,
    slots: &SlotState,
    step_id: &str,
    step_idx: usize,
    missing: &mut Vec<MissingSlot>,
) -> String {
    match val {
        serde_yaml::Value::String(s) if s.starts_with("${") && s.ends_with("}") => {
            let slot_name = &s[2..s.len()-1];
            match slots.get(slot_name) {
                Some(SlotValue::String(v)) => format!("\"{}\"", v),
                Some(SlotValue::Uuid(u)) => format!("\"{}\"", u),
                None => {
                    missing.push(MissingSlot {
                        name: slot_name.to_string(),
                        step_id: step_id.to_string(),
                        step_index: step_idx,
                    });
                    format!("\"MISSING:{}\"", slot_name)
                }
            }
        }
        serde_yaml::Value::String(s) => format!("\"{}\"", s),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        _ => "nil".to_string(),
    }
}
