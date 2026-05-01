//! DAG taxonomy YAML loader + typed structs (R-2a, v1.3 foundation).
//!
//! Loads `rust/config/sem_os_seeds/dag_taxonomies/*.yaml` into typed Rust
//! structs. Covers the full v1.2 DAG schema (`overall_lifecycle:`,
//! `slots:`, `cross_slot_constraints:`, `product_module_gates:`,
//! `prune_cascade_rules:`, `prune_pre_validation:`) AND the v1.3
//! extensions (cross_workspace_constraints, derived_cross_workspace_state,
//! parent_slot/state_dependency, expected_lifetime, dual_lifecycle,
//! periodic_review_cadence, evidence_types, category_gated).
//!
//! Structs are permissive on deserialize — extra YAML keys are tolerated
//! so that DAG authors can experiment without breaking the loader
//! (matches pack_loader's rollout philosophy).
//!
//! See `docs/todo/catalogue-platform-refinement-v1_3.md` for the
//! authoritative schema definition.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Value as YamlValue;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

// =============================================================================
// TOP-LEVEL DAG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Dag {
    #[serde(default)]
    pub version: String,
    pub workspace: String,
    pub dag_id: String,

    #[serde(default)]
    pub overall_lifecycle: Option<OverallLifecycle>,

    #[serde(default)]
    pub slots: Vec<Slot>,

    #[serde(default)]
    pub cross_slot_constraints: Vec<CrossSlotConstraint>,

    // --- v1.3 additions ---
    #[serde(default)]
    pub cross_workspace_constraints: Vec<CrossWorkspaceConstraint>,

    #[serde(default)]
    pub derived_cross_workspace_state: Vec<DerivedCrossWorkspaceState>,

    #[serde(default)]
    pub evidence_types: Vec<EvidenceType>,

    // --- existing sections ---
    #[serde(default)]
    pub product_module_gates: Option<ProductModuleGates>,

    #[serde(default)]
    pub out_of_scope: Vec<String>,

    #[serde(default)]
    pub prune_cascade_rules: Vec<PruneCascadeRule>,

    #[serde(default)]
    pub prune_pre_validation: Option<PrunePreValidation>,
}

// =============================================================================
// OVERALL LIFECYCLE (v1.2-2)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OverallLifecycle {
    pub id: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub derived: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub phases: Vec<Phase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Phase {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub derivation: Option<Derivation>,
    /// Verbs that can drive progression out of this phase. Tolerant to
    /// list form (canonical) OR free-text string (documentation escape
    /// used in some workspaces, e.g. KYC "remediation" phase).
    #[serde(default)]
    pub progression_verbs: ProgressionVerbs,
    #[serde(default)]
    pub next_phase: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ProgressionVerbs {
    List(Vec<String>),
    Text(String),
}

impl Default for ProgressionVerbs {
    fn default() -> Self {
        ProgressionVerbs::List(Vec::new())
    }
}

/// Phase derivation clause. Tolerant to free-form strings (current v1.2
/// style) and structured workspace-slot-state entries (v1.3 aggregate
/// derivation). A single clause may mix both.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Derivation {
    #[serde(default)]
    pub all_of: Vec<DerivationCondition>,
    #[serde(default)]
    pub any_of: Vec<DerivationCondition>,
}

/// A derivation condition — either a free-text SQL-like predicate (v1.2)
/// or a structured workspace/slot/state reference (v1.3 Mode B).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DerivationCondition {
    Raw(String),
    Structured(StructuredDerivationCondition),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredDerivationCondition {
    pub workspace: String,
    pub slot: String,
    #[serde(default)]
    pub state: Option<StateSelector>,
    #[serde(default)]
    pub predicate: Option<String>,
}

/// State selector — a single state ID or a set of allowable states.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum StateSelector {
    Single(String),
    Set(Vec<String>),
}

/// Closure semantics for a composite slot.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClosureType {
    Open,
    ClosedBounded,
    ClosedUnbounded,
}

/// Candidate eligibility constraint for attaching or populating a slot.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum EligibilityConstraint {
    /// v1: constrain by authored entity kinds.
    EntityKinds { entity_kinds: Vec<String> },
    /// v2: constrain by typed shape taxonomy position.
    ShapeTaxonomyPosition { shape_taxonomy_position: String },
}

/// Role guard metadata for discretionary gate enforcement.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RoleGuard {
    #[serde(default)]
    pub any_of: Vec<String>,

    #[serde(default)]
    pub all_of: Vec<String>,
}

/// Audit classification for discretionary gate outcomes.
pub type AuditClass = String;

/// Completeness assertion metadata for open slots.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletenessAssertionConfig {
    #[serde(default)]
    pub predicate: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, YamlValue>,
}

// =============================================================================
// SLOTS
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Slot {
    pub id: String,

    #[serde(default)]
    pub stateless: bool,

    #[serde(default)]
    pub rationale: Option<String>,

    /// Per-slot state machine (absent for stateless slots).
    #[serde(default)]
    pub state_machine: Option<SlotStateMachine>,

    // v1.2 product gating
    #[serde(default)]
    pub requires_products: Vec<String>,

    // Phase 1.5B gate metadata
    #[serde(default)]
    pub closure: Option<ClosureType>,

    #[serde(default)]
    pub eligibility: Option<EligibilityConstraint>,

    #[serde(default)]
    pub cardinality_max: Option<u64>,

    #[serde(default)]
    pub entry_state: Option<String>,

    #[serde(default)]
    pub attachment_predicates: Vec<String>,

    #[serde(default)]
    pub addition_predicates: Vec<String>,

    #[serde(default)]
    pub aggregate_breach_checks: Vec<String>,

    #[serde(
        default,
        rename = "+attachment_predicates",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additive_attachment_predicates: Vec<String>,

    #[serde(
        default,
        rename = "+addition_predicates",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additive_addition_predicates: Vec<String>,

    #[serde(
        default,
        rename = "+aggregate_breach_checks",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additive_aggregate_breach_checks: Vec<String>,

    #[serde(default)]
    pub role_guard: Option<RoleGuard>,

    #[serde(default)]
    pub justification_required: Option<bool>,

    #[serde(default)]
    pub audit_class: Option<AuditClass>,

    #[serde(default)]
    pub completeness_assertion: Option<CompletenessAssertionConfig>,

    // v1.3 additions
    #[serde(default)]
    pub parent_slot: Option<ParentSlot>,

    #[serde(default)]
    pub state_dependency: Option<StateDependency>,

    #[serde(default)]
    pub dual_lifecycle: Vec<DualLifecycle>,

    #[serde(default)]
    pub periodic_review_cadence: Option<PeriodicReviewCadence>,

    #[serde(default)]
    pub category_gated: Option<CategoryGated>,

    /// Explicit opt-out of the SUSPENDED-universal lint (V1.3-4).
    #[serde(default)]
    pub suspended_state_exempt: bool,

    /// Additional free-form metadata retained for forward compatibility
    /// (e.g. `product_gates:`, `cross_workspace_gate:` used by KYC DAG).
    #[serde(flatten)]
    pub extra: BTreeMap<String, YamlValue>,
}

/// Slot-level state machine. `state_machine:` may be a full block OR a
/// string reference ("reconcile-existing" form) — we accept both.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SlotStateMachine {
    Structured(Box<StateMachine>),
    Reference(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateMachine {
    pub id: String,
    #[serde(default)]
    pub source_entity: Option<String>,
    #[serde(default)]
    pub state_column: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub description: Option<String>,

    /// Predicate entity bindings used by `green_when` evaluation.
    ///
    /// These bindings tell the Frontier/evaluator how names that appear in
    /// state predicates map onto substrate rows. The field is optional so
    /// existing DAG YAML remains loadable while authored predicates are
    /// migrated from free text to machine evaluation.
    #[serde(default)]
    pub predicate_bindings: Vec<PredicateBinding>,

    #[serde(default)]
    pub states: Vec<StateDef>,

    #[serde(default)]
    pub transitions: Vec<TransitionDef>,

    #[serde(default)]
    pub terminal_states: Vec<String>,

    // v1.3 additions
    #[serde(default)]
    pub expected_lifetime: Option<ExpectedLifetime>,

    /// Ownership label for the lifecycle — governance/KYC artefact per
    /// OQ-3 resolution (2026-04-24). Runtime does NOT enforce.
    #[serde(default)]
    pub owner: Option<String>,

    #[serde(default)]
    pub note: Option<String>,

    /// Tolerate additional author-supplied metadata (description fields,
    /// etc.) without failing the load.
    #[serde(flatten)]
    pub extra: BTreeMap<String, YamlValue>,
}

/// Substrate binding for one entity kind named by a `green_when` predicate.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PredicateBinding {
    /// Predicate entity kind as authored, e.g. `red_flag`.
    pub entity: String,

    /// Whether this binding names a concrete substrate carrier or a DAG-level entity.
    #[serde(default)]
    pub source_kind: PredicateBindingSourceKind,

    /// Backing table or view, e.g. `"ob-poc".red_flags`.
    #[serde(default)]
    pub source_entity: Option<String>,

    /// Column carrying the entity's lifecycle state.
    #[serde(default)]
    pub state_column: Option<String>,

    /// Column carrying a scalar value for attribute predicates.
    #[serde(default)]
    pub value_column: Option<String>,

    /// Primary key or stable identifier column for diagnostics.
    #[serde(default)]
    pub id_column: Option<String>,

    /// Human-readable scope label, e.g. `attached_to this UBO`.
    #[serde(default)]
    pub scope: Option<String>,

    /// Column on this entity that points back to the parent/current row.
    #[serde(default)]
    pub parent_key: Option<String>,

    /// Column on the parent/current row used by `parent_key`.
    #[serde(default)]
    pub child_key: Option<String>,

    /// Required-universe binding for `every required <entity> exists`.
    #[serde(default)]
    pub required_universe: Option<PredicateRequiredUniverse>,

    /// Whether shape rules may replace this binding rather than tighten it.
    #[serde(default)]
    pub replaceable_by_shape: bool,

    /// Forward-compatible metadata for schema-specific binding details.
    #[serde(flatten)]
    pub extra: BTreeMap<String, YamlValue>,
}

/// Universe of required rows for a `required` predicate binding.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PredicateRequiredUniverse {
    /// Whether the required universe is a concrete carrier or a DAG-level set.
    #[serde(default)]
    pub source_kind: PredicateBindingSourceKind,

    /// Table or view that declares required entities.
    pub source_entity: String,

    /// Identifier column in the required-universe source.
    #[serde(default)]
    pub id_column: Option<String>,

    /// Column identifying the required entity kind.
    #[serde(default)]
    pub required_column: Option<String>,

    /// Column linking the required universe to the current parent row.
    #[serde(default)]
    pub parent_key: Option<String>,

    /// Column on the current parent row used by `parent_key`.
    #[serde(default)]
    pub child_key: Option<String>,

    /// Forward-compatible metadata for domain-specific required-set rules.
    #[serde(flatten)]
    pub extra: BTreeMap<String, YamlValue>,
}

/// Source kind for a predicate binding.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PredicateBindingSourceKind {
    /// Concrete substrate table/view binding.
    #[default]
    Substrate,

    /// DAG-declared entity whose carrier may be resolved later.
    DagEntity,

    /// DSL/runtime fact produced by a verb or resolver.
    DslFact,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedLifetime {
    ShortLived,
    LongLived,
    Ephemeral,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateDef {
    pub id: String,
    #[serde(default)]
    pub entry: bool,
    #[serde(default)]
    pub description: Option<String>,
    /// v1.4 (D-016, 2026-04-29): the green-switch predicate. State is
    /// "green" when this predicate holds — entity in this state is
    /// considered satisfied/ready. Verbs that move INTO this state are
    /// expected to have made changes that flip the predicate true; the
    /// destination state's green_when IS the postcondition of the
    /// transition. Optional: states without a green_when are permissive
    /// (no entry test).
    #[serde(default)]
    pub green_when: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransitionDef {
    pub from: YamlValue, // accepts "STATE" or "(STATE_A, STATE_B)" string or list
    pub to: String,
    #[serde(default)]
    pub via: Option<YamlValue>, // accepts single verb or list
    #[serde(default)]
    pub precondition: Option<String>,
    #[serde(default)]
    pub args: Option<YamlValue>,
}

// =============================================================================
// CROSS-SLOT CONSTRAINTS (v1.2)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrossSlotConstraint {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub rule: Option<String>,
    #[serde(default)]
    pub severity: Severity,

    #[serde(default)]
    pub v1_3_candidate: bool,

    #[serde(default)]
    pub enforced_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Informational,
}

// =============================================================================
// V1.3-1 CROSS-WORKSPACE CONSTRAINTS (Mode A blocking)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrossWorkspaceConstraint {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,

    pub source_workspace: String,
    pub source_slot: String,

    #[serde(default)]
    pub source_state: Option<StateSelector>,

    #[serde(default)]
    pub source_predicate: Option<String>,

    pub target_workspace: String,
    pub target_slot: String,

    /// e.g. "STATE_A -> STATE_B" or "* -> STATE_B"
    pub target_transition: String,

    #[serde(default)]
    pub severity: Severity,

    /// Whether shape rules may structurally replace this constraint.
    #[serde(default)]
    pub replaceable_by_shape: bool,
}

// =============================================================================
// V1.3-2 DERIVED CROSS-WORKSPACE STATE (Mode B aggregation / tollgate)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DerivedCrossWorkspaceState {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,

    pub host_workspace: String,
    pub host_slot: String,
    pub host_state: String,

    pub derivation: Derivation,

    #[serde(default)]
    pub exposure: Option<ExposureConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExposureConfig {
    #[serde(default)]
    pub visible_as: Option<Visibility>,

    /// Per OQ-2 resolution (2026-04-24): cache scope = session /
    /// workspace-context. This flag is retained for future granularity
    /// but the runtime default is session-cached unless explicitly false.
    #[serde(default = "default_cacheable")]
    pub cacheable: bool,
}

fn default_cacheable() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    FirstClassState,
    Annotation,
}

// =============================================================================
// V1.3-3 PARENT SLOT + STATE DEPENDENCY (Mode C hierarchy)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParentSlot {
    #[serde(default)]
    pub workspace: Option<String>, // defaults to same workspace
    pub slot: String,
    #[serde(default)]
    pub join: Option<ParentJoin>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParentJoin {
    #[serde(default)]
    pub via: Option<String>,
    #[serde(default)]
    pub parent_fk: Option<String>,
    #[serde(default)]
    pub child_fk: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateDependency {
    #[serde(default)]
    pub cascade_rules: Vec<CascadeRule>,
    #[serde(default)]
    pub severity: Severity,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CascadeRule {
    pub parent_state: String,
    pub child_allowed_states: Vec<String>,
    #[serde(default)]
    pub cascade_on_parent_transition: bool,
    #[serde(default)]
    pub default_child_state_on_cascade: Option<String>,
}

// =============================================================================
// V1.3-5 DUAL LIFECYCLE
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DualLifecycle {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Governance artefact — does NOT enforce at runtime (OQ-3).
    #[serde(default)]
    pub owner: Option<String>,
    pub junction_state_from_primary: String,
    #[serde(default)]
    pub states: Vec<StateDef>,
    #[serde(default)]
    pub transitions: Vec<TransitionDef>,
    #[serde(default)]
    pub terminal_states: Vec<String>,
}

// =============================================================================
// V1.3-6 PERIODIC REVIEW CADENCE + EVIDENCE TYPE VALIDITY WINDOWS
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeriodicReviewCadence {
    pub base_window: String, // ISO 8601 duration: "P1Y", "P2Y", etc.
    #[serde(default)]
    pub risk_tiered_overrides: Vec<RiskTierOverride>,
    #[serde(default)]
    pub review_scope: Option<ReviewScope>,
    #[serde(default)]
    pub scheduler_hook: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskTierOverride {
    pub risk_tier: String,
    pub window: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewScope {
    Full,
    Partial,
}

/// An evidence-type reference entry. `validity_window` may be `"once"`
/// (special value — no refresh) or an ISO 8601 duration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvidenceType {
    pub id: String,
    pub validity_window: String,
}

// =============================================================================
// V1.3-8 CATEGORY GATED
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CategoryGated {
    pub category_column: String,
    pub category_source: String, // table name, e.g. "cbus"
    #[serde(default)]
    pub activated_by: Vec<String>,
    #[serde(default)]
    pub deactivated_by: Vec<String>,
    #[serde(default)]
    pub lifecycle_variant_map: HashMap<String, String>,
}

// =============================================================================
// PRODUCT MODULE GATES (v1.2)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProductModuleGates {
    #[serde(default)]
    pub always_on: Vec<String>,
    #[serde(default)]
    pub conditionally_on: Vec<ConditionalGate>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConditionalGate {
    pub slot: String,
    #[serde(default)]
    pub activated_by: Vec<String>,
    #[serde(default)]
    pub rationale: Option<String>,
}

// =============================================================================
// PRUNE SEMANTICS (v1.2-4)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PruneCascadeRule {
    pub id: String,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub cascades_to: Vec<PruneCascadeTarget>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PruneCascadeTarget {
    pub target: String,
    #[serde(default)]
    pub rule: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrunePreValidation {
    #[serde(default)]
    pub required_verbs: Vec<String>,
    #[serde(default)]
    pub abort_conditions: Vec<String>,
}

// =============================================================================
// LOADER
// =============================================================================

#[derive(Debug, Clone)]
pub struct LoadedDag {
    pub source_path: PathBuf,
    pub dag: Dag,
}

/// Load every `*.yaml` file in the DAG taxonomies directory. Returns a
/// map keyed by `workspace` name. Malformed files surface an error —
/// we're stricter than pack_loader because DAG YAML is authoritative
/// architectural input.
pub fn load_dags_from_dir(dags_dir: &Path) -> Result<BTreeMap<String, LoadedDag>> {
    let mut out = BTreeMap::new();
    let entries = fs::read_dir(dags_dir)
        .with_context(|| format!("cannot read DAG taxonomies dir {dags_dir:?}"))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("cannot read DAG taxonomy {path:?}"))?;
        let dag: Dag = serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse DAG taxonomy {path:?}"))?;
        out.insert(
            dag.workspace.clone(),
            LoadedDag {
                source_path: path,
                dag,
            },
        );
    }
    Ok(out)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_dag_parses() {
        let yaml = r#"
version: "1.0"
workspace: example
dag_id: example_dag
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(dag.workspace, "example");
        assert_eq!(dag.dag_id, "example_dag");
    }

    #[test]
    fn predicate_bindings_parse_on_state_machine() {
        let yaml = r#"
version: "1.0"
workspace: example
dag_id: example_dag
slots:
  - id: clearance
    state_machine:
      id: clearance_lifecycle
      source_entity: '"ob-poc".booking_principal_clearances'
      state_column: clearance_status
      predicate_bindings:
        - entity: screening_check
          source_kind: substrate
          source_entity: '"ob-poc".screenings'
          state_column: status
          id_column: screening_id
          scope: attached_to this clearance
          parent_key: clearance_id
          child_key: id
        - entity: evidence_requirement
          source_kind: substrate
          source_entity: '"ob-poc".ubo_evidence'
          state_column: verification_status
          required_universe:
            source_kind: substrate
            source_entity: '"sem_reg".evidence_requirements'
            id_column: requirement_id
            required_column: evidence_kind
            parent_key: case_id
            child_key: case_id
      states:
        - id: PENDING
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        let SlotStateMachine::Structured(machine) =
            dag.slots[0].state_machine.as_ref().expect("state machine")
        else {
            panic!("expected structured state machine");
        };

        assert_eq!(machine.predicate_bindings.len(), 2);
        assert_eq!(machine.predicate_bindings[0].entity, "screening_check");
        assert_eq!(
            machine.predicate_bindings[0].source_kind,
            PredicateBindingSourceKind::Substrate
        );
        assert_eq!(
            machine.predicate_bindings[0].source_entity.as_deref(),
            Some("\"ob-poc\".screenings")
        );
        assert_eq!(
            machine.predicate_bindings[1]
                .required_universe
                .as_ref()
                .map(|binding| binding.source_entity.as_str()),
            Some("\"sem_reg\".evidence_requirements")
        );
    }

    #[test]
    fn cross_workspace_constraint_parses() {
        let yaml = r#"
workspace: deal
dag_id: deal_dag
cross_workspace_constraints:
  - id: deal_contracted_requires_kyc_approved
    description: "Deal needs KYC"
    source_workspace: kyc
    source_slot: kyc_case
    source_state: APPROVED
    target_workspace: deal
    target_slot: deal
    target_transition: "KYC_CLEARANCE -> CONTRACTED"
    severity: error
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(dag.cross_workspace_constraints.len(), 1);
        let c = &dag.cross_workspace_constraints[0];
        assert_eq!(c.source_workspace, "kyc");
        assert_eq!(c.target_workspace, "deal");
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn derived_cross_workspace_state_with_tollgate_parses() {
        let yaml = r#"
workspace: cbu
dag_id: cbu_dag
derived_cross_workspace_state:
  - id: cbu_operationally_active
    description: "Tollgate"
    host_workspace: cbu
    host_slot: cbu
    host_state: operationally_active
    derivation:
      all_of:
        - { workspace: kyc, slot: kyc_case, state: APPROVED }
        - { workspace: deal, slot: deal, state: [CONTRACTED, ONBOARDING, ACTIVE] }
        - { workspace: cbu, slot: cbu_evidence, predicate: "all verified" }
    exposure:
      visible_as: first_class_state
      cacheable: true
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(dag.derived_cross_workspace_state.len(), 1);
        let d = &dag.derived_cross_workspace_state[0];
        assert_eq!(d.host_state, "operationally_active");
        assert_eq!(d.derivation.all_of.len(), 3);
    }

    #[test]
    fn slot_with_parent_and_state_dependency() {
        let yaml = r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    parent_slot:
      workspace: cbu
      slot: cbu
      join:
        via: cbu_entity_relationships
        parent_fk: parent_cbu_id
        child_fk: child_cbu_id
    state_dependency:
      cascade_rules:
        - parent_state: SUSPENDED
          child_allowed_states: [SUSPENDED]
          cascade_on_parent_transition: true
          default_child_state_on_cascade: SUSPENDED
      severity: error
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        let slot = &dag.slots[0];
        assert!(slot.parent_slot.is_some());
        let dep = slot.state_dependency.as_ref().unwrap();
        assert_eq!(dep.cascade_rules.len(), 1);
        assert_eq!(dep.cascade_rules[0].parent_state, "SUSPENDED");
    }

    #[test]
    fn slot_with_category_gated() {
        let yaml = r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: investor
    stateless: false
    category_gated:
      category_column: cbu_category
      category_source: cbus
      activated_by: [FUND_MANDATE]
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        let gate = dag.slots[0].category_gated.as_ref().unwrap();
        assert_eq!(gate.activated_by, vec!["FUND_MANDATE"]);
    }

    #[test]
    fn dual_lifecycle_parses() {
        let yaml = r#"
workspace: deal
dag_id: deal_dag
slots:
  - id: deal
    stateless: false
    state_machine:
      id: deal_commercial_lifecycle
      owner: "sales+BAC"
      expected_lifetime: long_lived
      states:
        - id: PROSPECT
          entry: true
        - id: CONTRACTED
      transitions:
        - from: PROSPECT
          to: CONTRACTED
    dual_lifecycle:
      - id: deal_operational_lifecycle
        owner: ops
        junction_state_from_primary: CONTRACTED
        states:
          - id: ONBOARDING
            entry: true
          - id: OFFBOARDED
        terminal_states: [OFFBOARDED]
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        let slot = &dag.slots[0];
        let dual = &slot.dual_lifecycle;
        assert_eq!(dual.len(), 1);
        assert_eq!(dual[0].junction_state_from_primary, "CONTRACTED");
        assert_eq!(dual[0].owner.as_deref(), Some("ops"));
    }

    #[test]
    fn periodic_review_cadence_parses() {
        let yaml = r#"
workspace: kyc
dag_id: kyc_dag
slots:
  - id: kyc_case
    stateless: false
    periodic_review_cadence:
      base_window: "P2Y"
      risk_tiered_overrides:
        - risk_tier: HIGH
          window: "P1Y"
      review_scope: full
evidence_types:
  - id: sanctions_screening
    validity_window: "P14D"
  - id: corporate_formation_docs
    validity_window: once
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        let cadence = dag.slots[0].periodic_review_cadence.as_ref().unwrap();
        assert_eq!(cadence.base_window, "P2Y");
        assert_eq!(cadence.risk_tiered_overrides[0].window, "P1Y");
        assert_eq!(dag.evidence_types.len(), 2);
    }

    #[test]
    fn suspended_exempt_parses() {
        let yaml = r#"
workspace: kyc
dag_id: kyc_dag
slots:
  - id: kyc_case
    stateless: false
    suspended_state_exempt: true
    state_machine:
      id: kyc_case_lifecycle
      expected_lifetime: long_lived
      states:
        - id: INTAKE
          entry: true
        - id: APPROVED
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        assert!(dag.slots[0].suspended_state_exempt);
        let sm = match dag.slots[0].state_machine.as_ref().unwrap() {
            SlotStateMachine::Structured(sm) => sm,
            _ => panic!("expected structured"),
        };
        assert_eq!(sm.expected_lifetime, Some(ExpectedLifetime::LongLived));
    }

    #[test]
    fn state_machine_reference_form_parses() {
        // "reconcile-existing" form uses a string reference
        let yaml = r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: client_group
    stateless: false
    state_machine: "(reconcile-existing — see instrument_matrix_dag.yaml)"
"#;
        let dag: Dag = serde_yaml::from_str(yaml).expect("parse");
        let sm = dag.slots[0].state_machine.as_ref().unwrap();
        assert!(matches!(sm, SlotStateMachine::Reference(_)));
    }
}
