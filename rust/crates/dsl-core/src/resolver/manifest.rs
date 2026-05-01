use super::{ResolvedSlot, ResolvedTemplate};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default)]
pub struct ManifestOptions {
    pub required_slots: Option<BTreeSet<String>>,
}

impl ManifestOptions {
    pub fn pilot_lux_sicav() -> Self {
        Self {
            required_slots: Some(
                [
                    "cbu",
                    "entity_proper_person",
                    "entity_limited_company_ubo",
                    "manco",
                    "share_class",
                    "cbu_evidence",
                    "management_company",
                    "depositary",
                    "investment_manager",
                    "mandate",
                    "administrator",
                    "auditor",
                ]
                .into_iter()
                .map(str::to_string)
                .collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotManifestRow {
    pub slot_id: String,
    pub required: bool,
    pub has_all_required_gate_metadata: bool,
    pub missing_fields: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolverManifest {
    pub workspace: String,
    pub composite_shape: String,
    pub version: String,
    pub slots_resolved: usize,
    pub slots_with_all_required_gate_metadata: usize,
    pub slots_with_missing_gate_metadata: usize,
    pub rows: Vec<SlotManifestRow>,
}

impl ResolverManifest {
    pub fn from_template(template: &ResolvedTemplate, options: &ManifestOptions) -> Self {
        let rows = template
            .slots
            .iter()
            .map(|slot| row_for_slot(slot, options))
            .collect::<Vec<_>>();
        let required_rows = rows.iter().filter(|row| row.required).collect::<Vec<_>>();
        let slots_with_all_required_gate_metadata = required_rows
            .iter()
            .filter(|row| row.has_all_required_gate_metadata)
            .count();
        let slots_with_missing_gate_metadata = required_rows
            .iter()
            .filter(|row| !row.has_all_required_gate_metadata)
            .count();

        Self {
            workspace: template.workspace.clone(),
            composite_shape: template.composite_shape.clone(),
            version: template.version.to_string(),
            slots_resolved: template.slots.len(),
            slots_with_all_required_gate_metadata,
            slots_with_missing_gate_metadata,
            rows,
        }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "ResolvedTemplate manifest for (workspace={}, shape={})\n",
            self.workspace, self.composite_shape
        ));
        out.push_str(&format!("Version hash: {}\n", self.version));
        out.push_str(&format!("Slots resolved: {}\n", self.slots_resolved));
        out.push_str(&format!(
            "Slots with all required gate metadata: {}\n",
            self.slots_with_all_required_gate_metadata
        ));
        out.push_str(&format!(
            "Slots with missing gate metadata: {}\n\n",
            self.slots_with_missing_gate_metadata
        ));
        out.push_str("slot_id,required,missing_fields\n");
        for row in &self.rows {
            if row.required || !row.missing_fields.is_empty() {
                out.push_str(&format!(
                    "{},{},{}\n",
                    row.slot_id,
                    row.required,
                    if row.missing_fields.is_empty() {
                        "-".to_string()
                    } else {
                        row.missing_fields.join("|")
                    }
                ));
            }
        }
        out
    }
}

fn row_for_slot(slot: &ResolvedSlot, options: &ManifestOptions) -> SlotManifestRow {
    let required = options
        .required_slots
        .as_ref()
        .map(|slots| slots.contains(&slot.id))
        .unwrap_or(true);
    let mut missing_fields = Vec::new();

    if required {
        if slot.closure.is_none() {
            missing_fields.push("closure".to_string());
        }
        if needs_eligibility(slot) && slot.eligibility.is_none() {
            missing_fields.push("eligibility".to_string());
        }
        if needs_entry_state(slot) && slot.entry_state.is_none() {
            missing_fields.push("entry_state".to_string());
        }
        if needs_cardinality_max(slot) && slot.cardinality_max.is_none() {
            missing_fields.push("cardinality_max".to_string());
        }
    }

    SlotManifestRow {
        slot_id: slot.id.clone(),
        required,
        has_all_required_gate_metadata: missing_fields.is_empty(),
        missing_fields,
    }
}

fn needs_eligibility(slot: &ResolvedSlot) -> bool {
    !slot.entity_kinds.is_empty()
        || matches!(
            slot.id.as_str(),
            "cbu" | "entity_proper_person" | "entity_limited_company_ubo" | "manco"
        )
}

fn needs_entry_state(slot: &ResolvedSlot) -> bool {
    slot.state_machine.is_some() || slot.id == "cbu"
}

fn needs_cardinality_max(slot: &ResolvedSlot) -> bool {
    matches!(
        slot.closure,
        Some(crate::config::dag::ClosureType::ClosedBounded)
    ) && slot.id != "mandate"
}
