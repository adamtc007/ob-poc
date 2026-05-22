//! Resolver skeleton for SemOS DAG/constellation composition.

pub mod manifest;
mod version;

pub use manifest::{ManifestOptions, ResolverManifest, SlotManifestRow};
pub use version::{compute_version_hash, VersionHash};

use crate::config::dag::{ClosureType, EligibilityConstraint, RoleGuard};
use dsl_types::constellation_map_def::{
    AuditClass, Cardinality, CompletenessAssertionConfig, JoinDef, SlotDef,
};
use std::collections::BTreeMap;

pub use dsl_types::resolver_facts::StructuralFacts;

pub type WorkspaceId = String;
pub type ShapeRef = String;
pub type SlotId = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSource {
    DagTaxonomy,
    ConstellationMap,
    ShapeRule,
    Default,
}

#[derive(Debug, Clone, Default)]
pub struct SlotProvenance {
    pub field_sources: BTreeMap<String, ResolvedSource>,
}

#[derive(Debug, Clone)]
pub struct ResolverProvenance {
    pub dag_paths: Vec<String>,
    pub constellation_paths: Vec<String>,
    pub shape_rule_paths: Vec<String>,
    pub legacy_constellation_stack:
        Vec<dsl_types::constellation_map_def::ConstellationMapDefBody>,
}

#[derive(Debug, Clone)]
pub struct ResolvedTemplate {
    pub workspace: WorkspaceId,
    pub composite_shape: ShapeRef,
    pub structural_facts: StructuralFacts,
    pub slots: Vec<ResolvedSlot>,
    pub transitions: Vec<ResolvedTransition>,
    pub version: VersionHash,
    pub generated_at: String,
    pub generated_from: ResolverProvenance,
}

impl ResolvedTemplate {
    pub fn slot(&self, id: &str) -> Option<&ResolvedSlot> {
        self.slots.iter().find(|slot| slot.id == id)
    }

    pub fn slot_mut(&mut self, id: &str) -> Option<&mut ResolvedSlot> {
        self.slots.iter_mut().find(|slot| slot.id == id)
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedSlot {
    pub id: SlotId,
    pub state_machine: Option<String>,
    pub predicate_bindings: Vec<crate::config::dag::PredicateBinding>,
    pub table: Option<String>,
    pub pk: Option<String>,
    pub join: Option<JoinDef>,
    pub entity_kinds: Vec<String>,
    pub cardinality: Option<Cardinality>,
    pub depends_on: Vec<dsl_types::constellation_map_def::DependencyEntry>,
    pub placeholder: Option<String>,
    pub overlays: Vec<String>,
    pub edge_overlays: Vec<String>,
    pub verbs: BTreeMap<String, dsl_types::constellation_map_def::VerbPaletteEntry>,
    pub children: BTreeMap<String, SlotDef>,
    pub max_depth: Option<usize>,
    pub closure: Option<ClosureType>,
    pub eligibility: Option<EligibilityConstraint>,
    pub cardinality_max: Option<u64>,
    pub entry_state: Option<String>,
    pub attachment_predicates: Vec<String>,
    pub addition_predicates: Vec<String>,
    pub aggregate_breach_checks: Vec<String>,
    pub role_guard: Option<RoleGuard>,
    pub justification_required: Option<bool>,
    pub audit_class: Option<AuditClass>,
    pub completeness_assertion: Option<CompletenessAssertionConfig>,
    pub provenance: SlotProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTransition {
    pub slot_id: SlotId,
    pub from: String,
    pub to: String,
    pub via: Option<String>,
    pub destination_green_when: Option<String>,
}
