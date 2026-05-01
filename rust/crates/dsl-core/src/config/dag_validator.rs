//! DAG-taxonomy validator (R-2b, v1.3 cross-DAG checks).
//!
//! Takes the set of loaded DAGs (`BTreeMap<workspace_name, LoadedDag>`)
//! and applies the v1.3 architectural checks from
//! `catalogue-platform-refinement-v1_3.md` §3.1:
//!
//! Errors:
//! - CrossWorkspaceConstraintUnresolved
//! - CrossWorkspaceConstraintSelfReference
//! - DerivedCrossWorkspaceStateUnresolved
//! - DerivedCrossWorkspaceStateCycle
//! - ParentSlotUnresolved
//! - StateDependencyInconsistent
//! - DualLifecycleJunctionMissing
//! - CategoryGatedUnresolvedColumn (structural — category_column must be present)
//! - CategoryGatedMutuallyExclusiveGates
//!
//! Warnings:
//! - LongLivedSlotMissingSuspended (V1.3-4 lint)
//! - PeriodicReviewCadenceInconsistent
//! - ValidityWindowWithoutExpiredState
//!
//! Core checks are pure function library — no DB. Directory convenience
//! helpers are limited to reading authored YAML into those pure checks.

use crate::config::dag::*;
use crate::config::predicate::{parse_green_when, EntityRef, EntitySetRef, Predicate};
use crate::resolver::{ResolvedSlot, ResolvedTemplate};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

// ---------------------------------------------------------------------------
// Error + warning taxonomy
// ---------------------------------------------------------------------------

/// Location of a DAG-level finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagLocation {
    pub workspace: String,
    pub path: String,
}

impl std::fmt::Display for DagLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}::{}", self.workspace, self.path)
    }
}

/// Structural + well-formedness errors in DAG cross-references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    // V1.3-1 cross_workspace_constraints
    CrossWorkspaceConstraintUnresolved {
        location: DagLocation,
        constraint_id: String,
        unresolved_ref: String,
        reason: String,
    },
    CrossWorkspaceConstraintSelfReference {
        location: DagLocation,
        constraint_id: String,
    },
    // V1.3-2 derived_cross_workspace_state
    DerivedCrossWorkspaceStateUnresolved {
        location: DagLocation,
        derived_id: String,
        unresolved_ref: String,
    },
    DerivedCrossWorkspaceStateCycle {
        cycle_path: Vec<String>, // list of "workspace.slot.state" nodes
    },

    // V1.3-3 parent_slot + state_dependency
    ParentSlotUnresolved {
        location: DagLocation,
        slot_id: String,
        parent_workspace: String,
        parent_slot: String,
    },
    StateDependencyInconsistent {
        location: DagLocation,
        slot_id: String,
        parent_state: String,
        reason: String,
    },

    // V1.3-5 dual_lifecycle
    DualLifecycleJunctionMissing {
        location: DagLocation,
        slot_id: String,
        dual_id: String,
        junction_state: String,
    },

    // V1.3-8 category_gated
    CategoryGatedMutuallyExclusiveGates {
        location: DagLocation,
        slot_id: String,
    },

    // V1.4 green_when predicate parsing
    GreenWhenParseError {
        location: DagLocation,
        slot_id: String,
        state_id: String,
        reason: String,
    },
    GreenWhenUnboundEntity {
        location: DagLocation,
        slot_id: String,
        state_id: String,
        entity_kind: String,
    },
    PredicateBindingCarrierMissing {
        location: DagLocation,
        slot_id: String,
        state_id: String,
        entity_kind: String,
    },

    // Phase 1.5B gate metadata
    OpenClosureMissingCompletenessAssertion {
        location: DagLocation,
        slot_id: String,
    },
    EligibilityEntityKindUnknown {
        location: DagLocation,
        slot_id: String,
        entity_kind: String,
    },
    ExternalValidationContextMissing {
        location: DagLocation,
        slot_id: String,
        field: String,
    },
    EntryStateUnknown {
        location: DagLocation,
        slot_id: String,
        state_machine: String,
        entry_state: String,
    },
    GatePredicateParseError {
        location: DagLocation,
        slot_id: String,
        field: String,
        predicate_index: usize,
        reason: String,
    },
    AdditivePredicateSigilForbidden {
        location: DagLocation,
        slot_id: String,
        field: String,
    },
    SchemaCoordinationSlotFieldDrift {
        location: DagLocation,
        slot_id: String,
        field: String,
        dag_workspace: String,
    },
    SchemaCoordinationStateMachineMismatch {
        location: DagLocation,
        slot_id: String,
        dag_workspace: String,
        dag_state_machine: String,
        constellation_state_machine: String,
    },
    ResolvedClosureUniversalQuantifierInvalid {
        location: DagLocation,
        slot_id: String,
        field: String,
        predicate_index: usize,
        quantified_slot: String,
        closure: ClosureType,
    },
    SchemaCoordinationParseError {
        location: DagLocation,
        reason: String,
    },
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CrossWorkspaceConstraintUnresolved {
                location,
                constraint_id,
                unresolved_ref,
                reason,
            } => write!(
                f,
                "{location}: cross_workspace_constraint '{constraint_id}' references \
                 '{unresolved_ref}' — {reason}"
            ),
            Self::CrossWorkspaceConstraintSelfReference {
                location,
                constraint_id,
            } => write!(
                f,
                "{location}: cross_workspace_constraint '{constraint_id}' has \
                 source_workspace == target_workspace — use intra-DAG \
                 cross_slot_constraints instead"
            ),
            Self::DerivedCrossWorkspaceStateUnresolved {
                location,
                derived_id,
                unresolved_ref,
            } => write!(
                f,
                "{location}: derived_cross_workspace_state '{derived_id}' has \
                 unresolved derivation ref '{unresolved_ref}'"
            ),
            Self::DerivedCrossWorkspaceStateCycle { cycle_path } => write!(
                f,
                "derived_cross_workspace_state cycle detected: {}",
                cycle_path.join(" -> ")
            ),
            Self::ParentSlotUnresolved {
                location,
                slot_id,
                parent_workspace,
                parent_slot,
            } => write!(
                f,
                "{location}: slot '{slot_id}' parent_slot \
                 '{parent_workspace}.{parent_slot}' is not declared"
            ),
            Self::StateDependencyInconsistent {
                location,
                slot_id,
                parent_state,
                reason,
            } => write!(
                f,
                "{location}: slot '{slot_id}' state_dependency references \
                 parent_state '{parent_state}' — {reason}"
            ),
            Self::DualLifecycleJunctionMissing {
                location,
                slot_id,
                dual_id,
                junction_state,
            } => write!(
                f,
                "{location}: slot '{slot_id}' dual_lifecycle '{dual_id}' \
                 junction_state_from_primary '{junction_state}' is not a state \
                 in the slot's primary state_machine"
            ),
            Self::CategoryGatedMutuallyExclusiveGates { location, slot_id } => write!(
                f,
                "{location}: slot '{slot_id}' category_gated has both activated_by \
                 and deactivated_by — mutually exclusive"
            ),
            Self::GreenWhenParseError {
                location,
                slot_id,
                state_id,
                reason,
            } => write!(
                f,
                "{location}: slot '{slot_id}' state '{state_id}' has invalid \
                 green_when predicate — {reason}"
            ),
            Self::GreenWhenUnboundEntity {
                location,
                slot_id,
                state_id,
                entity_kind,
            } => write!(
                f,
                "{location}: slot '{slot_id}' state '{state_id}' references \
                 green_when entity '{entity_kind}' without a predicate_bindings entry"
            ),
            Self::PredicateBindingCarrierMissing {
                location,
                slot_id,
                state_id,
                entity_kind,
            } => write!(
                f,
                "{location}: slot '{slot_id}' state '{state_id}' references \
                 green_when entity '{entity_kind}' whose predicate binding has no carrier"
            ),
            Self::OpenClosureMissingCompletenessAssertion { location, slot_id } => write!(
                f,
                "{location}: slot '{slot_id}' has closure=open but no \
                 completeness_assertion"
            ),
            Self::EligibilityEntityKindUnknown {
                location,
                slot_id,
                entity_kind,
            } => write!(
                f,
                "{location}: slot '{slot_id}' eligibility references unknown \
                 entity kind '{entity_kind}'"
            ),
            Self::ExternalValidationContextMissing {
                location,
                slot_id,
                field,
            } => write!(
                f,
                "{location}: slot '{slot_id}' cannot validate '{field}' because \
                 external validation context is missing"
            ),
            Self::EntryStateUnknown {
                location,
                slot_id,
                state_machine,
                entry_state,
            } => write!(
                f,
                "{location}: slot '{slot_id}' entry_state '{entry_state}' is not a \
                 state in state_machine '{state_machine}'"
            ),
            Self::GatePredicateParseError {
                location,
                slot_id,
                field,
                predicate_index,
                reason,
            } => write!(
                f,
                "{location}: slot '{slot_id}' {field}[{predicate_index}] is not a \
                 valid predicate — {reason}"
            ),
            Self::AdditivePredicateSigilForbidden {
                location,
                slot_id,
                field,
            } => write!(
                f,
                "{location}: slot '{slot_id}' uses '{field}', but + vector \
                 composition is only valid in shape-rule files"
            ),
            Self::SchemaCoordinationSlotFieldDrift {
                location,
                slot_id,
                field,
                dag_workspace,
            } => write!(
                f,
                "{location}: slot '{slot_id}' also exists in DAG workspace \
                 '{dag_workspace}' and both schemas declare '{field}'. Error in \
                 Phase 2 unless explicitly documented as known-deferred."
            ),
            Self::SchemaCoordinationStateMachineMismatch {
                location,
                slot_id,
                dag_workspace,
                dag_state_machine,
                constellation_state_machine,
            } => write!(
                f,
                "{location}: slot '{slot_id}' also exists in DAG workspace \
                 '{dag_workspace}' with state_machine '{dag_state_machine}', but \
                 constellation declares '{constellation_state_machine}'. Error in \
                 Phase 2 unless explicitly documented as known-deferred."
            ),
            Self::ResolvedClosureUniversalQuantifierInvalid {
                location,
                slot_id,
                field,
                predicate_index,
                quantified_slot,
                closure,
            } => write!(
                f,
                "{location}: slot '{slot_id}' {field}[{predicate_index}] universally \
                 quantifies over '{quantified_slot}' with closure={closure:?}; use an \
                 aggregate-only predicate form for unbounded/open slots"
            ),
            Self::SchemaCoordinationParseError { location, reason } => {
                write!(
                    f,
                    "{location}: failed to parse constellation map — {reason}"
                )
            }
        }
    }
}

/// Policy / lint warnings — advisory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagWarning {
    // V1.3-4
    LongLivedSlotMissingSuspended {
        location: DagLocation,
        slot_id: String,
    },

    // V1.3-6
    PeriodicReviewCadenceWithoutRereviewTransition {
        location: DagLocation,
        slot_id: String,
    },
    ValidityWindowWithoutExpiredState {
        location: DagLocation,
        evidence_type_id: String,
    },

    // Phase 1.5B schema-coordination warning surface (D-011).
    SchemaCoordinationSlotFieldDrift {
        location: DagLocation,
        slot_id: String,
        field: String,
        dag_workspace: String,
    },
    SchemaCoordinationStateMachineMismatch {
        location: DagLocation,
        slot_id: String,
        dag_workspace: String,
        dag_state_machine: String,
        constellation_state_machine: String,
    },
}

impl std::fmt::Display for DagWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LongLivedSlotMissingSuspended { location, slot_id } => write!(
                f,
                "{location}: slot '{slot_id}' is expected_lifetime=long_lived but \
                 state_machine has no SUSPENDED state. Declare SUSPENDED (bidirectional \
                 from preceding operational state) or mark suspended_state_exempt: true \
                 with a rationale comment. (V1.3-4)"
            ),
            Self::PeriodicReviewCadenceWithoutRereviewTransition { location, slot_id } => {
                write!(
                    f,
                    "{location}: slot '{slot_id}' declares periodic_review_cadence but \
                     the state machine has no transition that re-enters an earlier \
                     review state. Cadence will have no runtime effect. (V1.3-6)"
                )
            }
            Self::ValidityWindowWithoutExpiredState {
                location,
                evidence_type_id,
            } => write!(
                f,
                "{location}: evidence_type '{evidence_type_id}' has a validity_window \
                 but the evidence state machine has no EXPIRED state. \
                 Validity-window expiration has no state to transition into. (V1.3-6)"
            ),
            Self::SchemaCoordinationSlotFieldDrift {
                location,
                slot_id,
                field,
                dag_workspace,
            } => write!(
                f,
                "{location}: constellation slot '{slot_id}' also sets gate metadata \
                 field '{field}' declared by DAG workspace '{dag_workspace}'. \
                 Warning only until Phase 2 per D-011."
            ),
            Self::SchemaCoordinationStateMachineMismatch {
                location,
                slot_id,
                dag_workspace,
                dag_state_machine,
                constellation_state_machine,
            } => write!(
                f,
                "{location}: constellation slot '{slot_id}' references state_machine \
                 '{constellation_state_machine}' but DAG workspace '{dag_workspace}' \
                 declares '{dag_state_machine}'. Warning only until Phase 2 per D-011."
            ),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DagValidationReport {
    pub errors: Vec<DagError>,
    pub warnings: Vec<DagWarning>,
}

impl DagValidationReport {
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}

/// Optional external indexes for validator checks whose data lives outside DAG YAML.
#[derive(Debug, Default, Clone)]
pub struct DagValidationContext {
    pub known_entity_kinds: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SchemaCoordinationKnownDeferred {
    SlotFieldDrift {
        source_name: String,
        slot_id: String,
        field: String,
        dag_workspace: String,
    },
    StateMachineMismatch {
        source_name: String,
        slot_id: String,
        dag_workspace: String,
        dag_state_machine: String,
        constellation_state_machine: String,
    },
}

// ---------------------------------------------------------------------------
// Validator entry point
// ---------------------------------------------------------------------------

/// Validate all loaded DAGs. Takes the full map so cross-DAG references
/// (cross_workspace_constraints, derived_cross_workspace_state, parent_slot
/// into other workspaces) can be resolved.
pub fn validate_dags(loaded: &BTreeMap<String, LoadedDag>) -> DagValidationReport {
    validate_dags_with_context(loaded, &DagValidationContext::default())
}

/// Validate all loaded DAGs using optional external context.
pub fn validate_dags_with_context(
    loaded: &BTreeMap<String, LoadedDag>,
    context: &DagValidationContext,
) -> DagValidationReport {
    let mut report = DagValidationReport::default();

    // Index: workspace -> (slot_id -> Slot ref) and state_machine state_ids.
    let index = SlotIndex::build(loaded);

    for (ws, ld) in loaded {
        validate_cross_workspace_constraints(ws, &ld.dag, &index, &mut report);
        validate_derived_cross_workspace_state_refs(ws, &ld.dag, &index, &mut report);
        validate_parent_slots(ws, &ld.dag, &index, &mut report);
        validate_dual_lifecycles(ws, &ld.dag, &mut report);
        validate_category_gated(ws, &ld.dag, &mut report);
        validate_green_when_predicates(ws, &ld.dag, &mut report);
        validate_gate_metadata(ws, &ld.dag, context, &mut report);

        // Warnings
        validate_long_lived_suspended_convention(ws, &ld.dag, &mut report);
        validate_periodic_review_cadence(ws, &ld.dag, &mut report);
        validate_validity_window_expired_state(ws, &ld.dag, &mut report);
    }

    // Cycle detection across all derivations (requires full index).
    detect_derivation_cycles(loaded, &mut report);

    report
}

/// Validate gate metadata after DAG, constellation-map, and shape-rule composition.
pub fn validate_resolved_template_gate_metadata(
    template: &ResolvedTemplate,
    context: &DagValidationContext,
) -> DagValidationReport {
    let mut report = DagValidationReport::default();
    let slot_closures = template
        .slots
        .iter()
        .flat_map(|slot| {
            let Some(closure) = slot.closure.clone() else {
                return Vec::new();
            };
            let mut entries = vec![(slot.id.clone(), closure.clone())];
            entries.extend(
                slot.predicate_bindings
                    .iter()
                    .map(|binding| (binding.entity.clone(), closure.clone())),
            );
            entries
        })
        .collect::<HashMap<_, _>>();

    for slot in &template.slots {
        let location = DagLocation {
            workspace: template.workspace.clone(),
            path: format!(
                "resolved_template.{}.slots.{}",
                template.composite_shape, slot.id
            ),
        };

        validate_resolved_open_closure(&location, slot, &mut report);
        validate_resolved_eligibility(&location, slot, context, &mut report);
        validate_resolved_predicate_vector(
            &location,
            slot,
            "attachment_predicates",
            &slot.attachment_predicates,
            &slot_closures,
            &mut report,
        );
        validate_resolved_predicate_vector(
            &location,
            slot,
            "addition_predicates",
            &slot.addition_predicates,
            &slot_closures,
            &mut report,
        );
        validate_resolved_predicate_vector(
            &location,
            slot,
            "aggregate_breach_checks",
            &slot.aggregate_breach_checks,
            &slot_closures,
            &mut report,
        );

        if let Some(predicate) = slot
            .completeness_assertion
            .as_ref()
            .and_then(|assertion| assertion.predicate.as_ref())
        {
            validate_resolved_predicate_text(
                &location,
                slot,
                "completeness_assertion.predicate",
                0,
                predicate,
                &slot_closures,
                &mut report,
            );
        }
    }

    report
}

/// Parse `config/ontology/entity_taxonomy.yaml`-style YAML into known entity kinds.
pub fn entity_kinds_from_taxonomy_yaml(yaml: &str) -> Result<HashSet<String>, serde_yaml::Error> {
    #[derive(serde::Deserialize)]
    struct EntityTaxonomy {
        #[serde(default)]
        entities: BTreeMap<String, serde_yaml::Value>,
    }

    let parsed: EntityTaxonomy = serde_yaml::from_str(yaml)?;
    Ok(parsed.entities.into_keys().collect())
}

/// Validate one constellation map's schema coordination against loaded DAGs.
///
/// This intentionally parses a lightweight raw YAML shape instead of depending
/// on `sem_os_core::constellation_map_def`, keeping `dsl-core` dependency-free.
pub fn validate_constellation_map_schema_coordination(
    loaded: &BTreeMap<String, LoadedDag>,
    source_name: &str,
    yaml: &str,
) -> DagValidationReport {
    let mut report = DagValidationReport::default();
    let map = match serde_yaml::from_str::<RawConstellationMap>(yaml) {
        Ok(map) => map,
        Err(err) => {
            report.errors.push(DagError::SchemaCoordinationParseError {
                location: DagLocation {
                    workspace: "constellation".to_string(),
                    path: source_name.to_string(),
                },
                reason: err.to_string(),
            });
            return report;
        }
    };

    validate_raw_constellation_map_schema_coordination(loaded, source_name, &map, &mut report);
    report
}

/// Validate a directory of constellation map YAML files against loaded DAGs.
pub fn validate_constellation_map_dir_schema_coordination(
    loaded: &BTreeMap<String, LoadedDag>,
    dir: &Path,
) -> std::io::Result<DagValidationReport> {
    let mut report = DagValidationReport::default();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "yaml" | "yml"));
        if !is_yaml {
            continue;
        }
        let contents = std::fs::read_to_string(&path)?;
        let source_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        let map_report =
            validate_constellation_map_schema_coordination(loaded, source_name, &contents);
        report.errors.extend(map_report.errors);
        report.warnings.extend(map_report.warnings);
    }
    Ok(report)
}

pub fn validate_constellation_map_dir_schema_coordination_strict(
    loaded: &BTreeMap<String, LoadedDag>,
    dir: &Path,
    known_deferred: &[SchemaCoordinationKnownDeferred],
) -> std::io::Result<DagValidationReport> {
    let mut report = validate_constellation_map_dir_schema_coordination(loaded, dir)?;
    harden_schema_coordination_warnings(&mut report, known_deferred);
    Ok(report)
}

pub fn harden_schema_coordination_warnings(
    report: &mut DagValidationReport,
    known_deferred: &[SchemaCoordinationKnownDeferred],
) {
    let known_deferred = known_deferred.iter().cloned().collect::<HashSet<_>>();
    let mut retained_warnings = Vec::new();
    for warning in std::mem::take(&mut report.warnings) {
        match schema_coordination_known_deferred_key(&warning) {
            Some(key) if known_deferred.contains(&key) => retained_warnings.push(warning),
            Some(_) => report
                .errors
                .push(schema_coordination_warning_to_error(warning)),
            None => retained_warnings.push(warning),
        }
    }
    report.warnings = retained_warnings;
}

fn schema_coordination_known_deferred_key(
    warning: &DagWarning,
) -> Option<SchemaCoordinationKnownDeferred> {
    match warning {
        DagWarning::SchemaCoordinationSlotFieldDrift {
            location,
            slot_id,
            field,
            dag_workspace,
        } => Some(SchemaCoordinationKnownDeferred::SlotFieldDrift {
            source_name: schema_coordination_source_name(location),
            slot_id: slot_id.clone(),
            field: field.clone(),
            dag_workspace: dag_workspace.clone(),
        }),
        DagWarning::SchemaCoordinationStateMachineMismatch {
            location,
            slot_id,
            dag_workspace,
            dag_state_machine,
            constellation_state_machine,
        } => Some(SchemaCoordinationKnownDeferred::StateMachineMismatch {
            source_name: schema_coordination_source_name(location),
            slot_id: slot_id.clone(),
            dag_workspace: dag_workspace.clone(),
            dag_state_machine: dag_state_machine.clone(),
            constellation_state_machine: constellation_state_machine.clone(),
        }),
        DagWarning::LongLivedSlotMissingSuspended { .. }
        | DagWarning::PeriodicReviewCadenceWithoutRereviewTransition { .. }
        | DagWarning::ValidityWindowWithoutExpiredState { .. } => None,
    }
}

fn schema_coordination_warning_to_error(warning: DagWarning) -> DagError {
    match warning {
        DagWarning::SchemaCoordinationSlotFieldDrift {
            location,
            slot_id,
            field,
            dag_workspace,
        } => DagError::SchemaCoordinationSlotFieldDrift {
            location,
            slot_id,
            field,
            dag_workspace,
        },
        DagWarning::SchemaCoordinationStateMachineMismatch {
            location,
            slot_id,
            dag_workspace,
            dag_state_machine,
            constellation_state_machine,
        } => DagError::SchemaCoordinationStateMachineMismatch {
            location,
            slot_id,
            dag_workspace,
            dag_state_machine,
            constellation_state_machine,
        },
        DagWarning::LongLivedSlotMissingSuspended { .. }
        | DagWarning::PeriodicReviewCadenceWithoutRereviewTransition { .. }
        | DagWarning::ValidityWindowWithoutExpiredState { .. } => {
            unreachable!("only schema-coordination warnings are promoted")
        }
    }
}

fn schema_coordination_source_name(location: &DagLocation) -> String {
    location
        .path
        .split_once(":slots.")
        .map(|(source, _)| source)
        .unwrap_or(&location.path)
        .to_string()
}

// ---------------------------------------------------------------------------
// Index — workspace -> slot -> state set
// ---------------------------------------------------------------------------

struct SlotIndex {
    /// workspace -> slot_id -> (has_state_machine, states, transitions_count)
    slots: HashMap<String, HashMap<String, SlotSummary>>,
}

#[derive(Default)]
struct SlotSummary {
    has_state_machine: bool,
    states: HashSet<String>,
    transitions_count: usize,
    expected_lifetime: Option<ExpectedLifetime>,
    suspended_state_exempt: bool,
}

impl SlotIndex {
    fn build(loaded: &BTreeMap<String, LoadedDag>) -> Self {
        let mut slots = HashMap::new();
        for (ws, ld) in loaded {
            let mut ws_slots = HashMap::new();
            for slot in &ld.dag.slots {
                let mut summary = SlotSummary::default();
                if let Some(sm) = &slot.state_machine {
                    match sm {
                        SlotStateMachine::Structured(sm) => {
                            summary.has_state_machine = true;
                            for st in &sm.states {
                                summary.states.insert(st.id.clone());
                            }
                            summary.transitions_count = sm.transitions.len();
                            summary.expected_lifetime = sm.expected_lifetime.clone();
                        }
                        SlotStateMachine::Reference(_) => {
                            summary.has_state_machine = true;
                        }
                    }
                }
                // V1.3-5: dual_lifecycle states belong to the same slot
                // conceptually — index them too.
                for dual in &slot.dual_lifecycle {
                    for st in &dual.states {
                        summary.states.insert(st.id.clone());
                    }
                }
                summary.suspended_state_exempt = slot.suspended_state_exempt;
                ws_slots.insert(slot.id.clone(), summary);
            }
            slots.insert(ws.clone(), ws_slots);
        }
        SlotIndex { slots }
    }

    fn workspace_exists(&self, ws: &str) -> bool {
        self.slots.contains_key(ws)
    }

    fn slot_exists(&self, ws: &str, slot: &str) -> bool {
        self.slots.get(ws).is_some_and(|m| m.contains_key(slot))
    }

    fn state_exists(&self, ws: &str, slot: &str, state: &str) -> bool {
        self.slots
            .get(ws)
            .and_then(|m| m.get(slot))
            .is_some_and(|s| s.states.is_empty() || s.states.contains(state))
        // Empty states means "reference-only state machine" — skip strict check.
    }
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

fn validate_cross_workspace_constraints(
    workspace: &str,
    dag: &Dag,
    index: &SlotIndex,
    report: &mut DagValidationReport,
) {
    for c in &dag.cross_workspace_constraints {
        let loc = DagLocation {
            workspace: workspace.to_string(),
            path: format!("cross_workspace_constraints[{}]", c.id),
        };

        // Self-reference check
        if c.source_workspace == c.target_workspace {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintSelfReference {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                });
        }

        // state + predicate may be declared together — they compose
        // with AND semantics (state narrows WHICH state, predicate
        // narrows WHICH row).

        // Source workspace exists?
        if !index.workspace_exists(&c.source_workspace) {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintUnresolved {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                    unresolved_ref: format!("workspace:{}", c.source_workspace),
                    reason: "workspace not loaded".to_string(),
                });
            continue;
        }

        // Source slot exists?
        if !index.slot_exists(&c.source_workspace, &c.source_slot) {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintUnresolved {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                    unresolved_ref: format!("{}.{}", c.source_workspace, c.source_slot),
                    reason: "slot not declared in source workspace".to_string(),
                });
            continue;
        }

        // Source state exists in source slot?
        if let Some(state_sel) = &c.source_state {
            let states: Vec<String> = match state_sel {
                StateSelector::Single(s) => vec![s.clone()],
                StateSelector::Set(v) => v.clone(),
            };
            for s in states {
                if !index.state_exists(&c.source_workspace, &c.source_slot, &s) {
                    report
                        .errors
                        .push(DagError::CrossWorkspaceConstraintUnresolved {
                            location: loc.clone(),
                            constraint_id: c.id.clone(),
                            unresolved_ref: format!(
                                "{}.{}::{}",
                                c.source_workspace, c.source_slot, s
                            ),
                            reason: "state not in source slot's state machine".to_string(),
                        });
                }
            }
        }

        // Target workspace/slot exist?
        if !index.workspace_exists(&c.target_workspace) {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintUnresolved {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                    unresolved_ref: format!("target workspace:{}", c.target_workspace),
                    reason: "target workspace not loaded".to_string(),
                });
        } else if !index.slot_exists(&c.target_workspace, &c.target_slot) {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintUnresolved {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                    unresolved_ref: format!("{}.{}", c.target_workspace, c.target_slot),
                    reason: "target slot not declared".to_string(),
                });
        }
    }
}

fn validate_derived_cross_workspace_state_refs(
    workspace: &str,
    dag: &Dag,
    index: &SlotIndex,
    report: &mut DagValidationReport,
) {
    for d in &dag.derived_cross_workspace_state {
        let loc = DagLocation {
            workspace: workspace.to_string(),
            path: format!("derived_cross_workspace_state[{}]", d.id),
        };

        // Host exists?
        if !index.slot_exists(&d.host_workspace, &d.host_slot) {
            report
                .errors
                .push(DagError::DerivedCrossWorkspaceStateUnresolved {
                    location: loc.clone(),
                    derived_id: d.id.clone(),
                    unresolved_ref: format!("host {}.{}", d.host_workspace, d.host_slot),
                });
        }

        // Each derivation condition resolvable
        for cond in d.derivation.all_of.iter().chain(d.derivation.any_of.iter()) {
            if let DerivationCondition::Structured(s) = cond {
                if !index.slot_exists(&s.workspace, &s.slot) {
                    report
                        .errors
                        .push(DagError::DerivedCrossWorkspaceStateUnresolved {
                            location: loc.clone(),
                            derived_id: d.id.clone(),
                            unresolved_ref: format!("{}.{}", s.workspace, s.slot),
                        });
                    continue;
                }
                if let Some(state_sel) = &s.state {
                    let states: Vec<String> = match state_sel {
                        StateSelector::Single(s) => vec![s.clone()],
                        StateSelector::Set(v) => v.clone(),
                    };
                    for st in states {
                        if !index.state_exists(&s.workspace, &s.slot, &st) {
                            report
                                .errors
                                .push(DagError::DerivedCrossWorkspaceStateUnresolved {
                                    location: loc.clone(),
                                    derived_id: d.id.clone(),
                                    unresolved_ref: format!("{}.{}::{}", s.workspace, s.slot, st),
                                });
                        }
                    }
                }
            }
        }
    }
}

fn validate_parent_slots(
    workspace: &str,
    dag: &Dag,
    index: &SlotIndex,
    report: &mut DagValidationReport,
) {
    for slot in &dag.slots {
        if let Some(parent) = &slot.parent_slot {
            let parent_ws = parent.workspace.as_deref().unwrap_or(workspace);
            let loc = DagLocation {
                workspace: workspace.to_string(),
                path: format!("slots.{}.parent_slot", slot.id),
            };
            if !index.slot_exists(parent_ws, &parent.slot) {
                report.errors.push(DagError::ParentSlotUnresolved {
                    location: loc.clone(),
                    slot_id: slot.id.clone(),
                    parent_workspace: parent_ws.to_string(),
                    parent_slot: parent.slot.clone(),
                });
                continue;
            }

            // state_dependency consistency
            if let Some(dep) = &slot.state_dependency {
                for rule in &dep.cascade_rules {
                    if !index.state_exists(parent_ws, &parent.slot, &rule.parent_state) {
                        report.errors.push(DagError::StateDependencyInconsistent {
                            location: loc.clone(),
                            slot_id: slot.id.clone(),
                            parent_state: rule.parent_state.clone(),
                            reason: "parent_state is not in parent slot's state machine"
                                .to_string(),
                        });
                    }
                }
            }
        }
    }
}

fn validate_dual_lifecycles(workspace: &str, dag: &Dag, report: &mut DagValidationReport) {
    for slot in &dag.slots {
        let Some(sm) = slot.state_machine.as_ref() else {
            continue;
        };
        let SlotStateMachine::Structured(sm) = sm else {
            continue; // can't validate reference form
        };

        let primary_states: HashSet<&str> = sm.states.iter().map(|s| s.id.as_str()).collect();

        for dual in &slot.dual_lifecycle {
            if !primary_states.contains(dual.junction_state_from_primary.as_str()) {
                report.errors.push(DagError::DualLifecycleJunctionMissing {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("slots.{}.dual_lifecycle", slot.id),
                    },
                    slot_id: slot.id.clone(),
                    dual_id: dual.id.clone(),
                    junction_state: dual.junction_state_from_primary.clone(),
                });
            }
        }
    }
}

fn validate_category_gated(workspace: &str, dag: &Dag, report: &mut DagValidationReport) {
    for slot in &dag.slots {
        let Some(cg) = &slot.category_gated else {
            continue;
        };
        if !cg.activated_by.is_empty() && !cg.deactivated_by.is_empty() {
            report
                .errors
                .push(DagError::CategoryGatedMutuallyExclusiveGates {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("slots.{}.category_gated", slot.id),
                    },
                    slot_id: slot.id.clone(),
                });
        }
    }
}

fn validate_green_when_predicates(workspace: &str, dag: &Dag, report: &mut DagValidationReport) {
    for slot in &dag.slots {
        let Some(SlotStateMachine::Structured(machine)) = slot.state_machine.as_ref() else {
            continue;
        };
        for state in &machine.states {
            let Some(predicate) = &state.green_when else {
                continue;
            };
            if predicate.trim().is_empty() {
                continue;
            }
            let location = DagLocation {
                workspace: workspace.to_string(),
                path: format!("slots.{}.states.{}.green_when", slot.id, state.id),
            };
            let ast = match parse_green_when(predicate) {
                Ok(ast) => ast,
                Err(err) => {
                    report.errors.push(DagError::GreenWhenParseError {
                        location,
                        slot_id: slot.id.clone(),
                        state_id: state.id.clone(),
                        reason: err.to_string(),
                    });
                    continue;
                }
            };

            let bound_entities: HashSet<&str> = machine
                .predicate_bindings
                .iter()
                .map(|binding| binding.entity.as_str())
                .collect();
            let slot_ids = dag
                .slots
                .iter()
                .map(|slot| slot.id.as_str())
                .collect::<HashSet<_>>();
            let mut referenced_entities = HashSet::new();
            collect_predicate_entity_refs(&ast, &mut referenced_entities);
            for entity_kind in referenced_entities {
                if !bound_entities.contains(entity_kind.as_str()) {
                    report.errors.push(DagError::GreenWhenUnboundEntity {
                        location: location.clone(),
                        slot_id: slot.id.clone(),
                        state_id: state.id.clone(),
                        entity_kind,
                    });
                    continue;
                }
                let binding = machine
                    .predicate_bindings
                    .iter()
                    .find(|binding| binding.entity == entity_kind)
                    .expect("bound entity already checked");
                let has_carrier = binding.source_entity.is_some()
                    || (binding.source_kind == PredicateBindingSourceKind::DagEntity
                        && slot_ids.contains(binding.entity.as_str()));
                if !has_carrier {
                    report
                        .errors
                        .push(DagError::PredicateBindingCarrierMissing {
                            location: location.clone(),
                            slot_id: slot.id.clone(),
                            state_id: state.id.clone(),
                            entity_kind,
                        });
                }
            }
        }
    }
}

fn validate_gate_metadata(
    workspace: &str,
    dag: &Dag,
    context: &DagValidationContext,
    report: &mut DagValidationReport,
) {
    for slot in &dag.slots {
        let location = DagLocation {
            workspace: workspace.to_string(),
            path: format!("slots.{}", slot.id),
        };

        if slot.closure == Some(ClosureType::Open) && slot.completeness_assertion.is_none() {
            report
                .errors
                .push(DagError::OpenClosureMissingCompletenessAssertion {
                    location: location.clone(),
                    slot_id: slot.id.clone(),
                });
        }

        if let Some(EligibilityConstraint::EntityKinds { entity_kinds }) = &slot.eligibility {
            if context.known_entity_kinds.is_empty() {
                report
                    .errors
                    .push(DagError::ExternalValidationContextMissing {
                        location: location.clone(),
                        slot_id: slot.id.clone(),
                        field: "eligibility".to_string(),
                    });
            } else {
                for entity_kind in entity_kinds {
                    if !context.known_entity_kinds.contains(entity_kind) {
                        report.errors.push(DagError::EligibilityEntityKindUnknown {
                            location: location.clone(),
                            slot_id: slot.id.clone(),
                            entity_kind: entity_kind.clone(),
                        });
                    }
                }
            }
        }

        validate_entry_state(&location, slot, report);

        validate_gate_predicate_vector(
            &location,
            &slot.id,
            "attachment_predicates",
            &slot.attachment_predicates,
            report,
        );
        validate_gate_predicate_vector(
            &location,
            &slot.id,
            "addition_predicates",
            &slot.addition_predicates,
            report,
        );
        validate_gate_predicate_vector(
            &location,
            &slot.id,
            "aggregate_breach_checks",
            &slot.aggregate_breach_checks,
            report,
        );

        reject_additive_predicate_vector(
            &location,
            &slot.id,
            "+attachment_predicates",
            &slot.additive_attachment_predicates,
            report,
        );
        reject_additive_predicate_vector(
            &location,
            &slot.id,
            "+addition_predicates",
            &slot.additive_addition_predicates,
            report,
        );
        reject_additive_predicate_vector(
            &location,
            &slot.id,
            "+aggregate_breach_checks",
            &slot.additive_aggregate_breach_checks,
            report,
        );
    }
}

fn validate_entry_state(location: &DagLocation, slot: &Slot, report: &mut DagValidationReport) {
    let Some(entry_state) = &slot.entry_state else {
        return;
    };
    let Some(SlotStateMachine::Structured(machine)) = &slot.state_machine else {
        return;
    };
    if machine.states.iter().any(|state| state.id == *entry_state) {
        return;
    }

    report.errors.push(DagError::EntryStateUnknown {
        location: location.clone(),
        slot_id: slot.id.clone(),
        state_machine: machine.id.clone(),
        entry_state: entry_state.clone(),
    });
}

fn validate_gate_predicate_vector(
    location: &DagLocation,
    slot_id: &str,
    field: &str,
    predicates: &[String],
    report: &mut DagValidationReport,
) {
    for (idx, predicate) in predicates.iter().enumerate() {
        if predicate.trim().is_empty() {
            continue;
        }
        if let Err(err) = parse_green_when(predicate) {
            report.errors.push(DagError::GatePredicateParseError {
                location: location.clone(),
                slot_id: slot_id.to_string(),
                field: field.to_string(),
                predicate_index: idx,
                reason: err.to_string(),
            });
        }
    }
}

fn reject_additive_predicate_vector(
    location: &DagLocation,
    slot_id: &str,
    field: &str,
    predicates: &[String],
    report: &mut DagValidationReport,
) {
    if !predicates.is_empty() {
        report
            .errors
            .push(DagError::AdditivePredicateSigilForbidden {
                location: location.clone(),
                slot_id: slot_id.to_string(),
                field: field.to_string(),
            });
    }
}

fn validate_resolved_open_closure(
    location: &DagLocation,
    slot: &ResolvedSlot,
    report: &mut DagValidationReport,
) {
    if slot.closure == Some(ClosureType::Open) && slot.completeness_assertion.is_none() {
        report
            .errors
            .push(DagError::OpenClosureMissingCompletenessAssertion {
                location: location.clone(),
                slot_id: slot.id.clone(),
            });
    }
}

fn validate_resolved_eligibility(
    location: &DagLocation,
    slot: &ResolvedSlot,
    context: &DagValidationContext,
    report: &mut DagValidationReport,
) {
    let Some(EligibilityConstraint::EntityKinds { entity_kinds }) = &slot.eligibility else {
        return;
    };
    if context.known_entity_kinds.is_empty() {
        report
            .errors
            .push(DagError::ExternalValidationContextMissing {
                location: location.clone(),
                slot_id: slot.id.clone(),
                field: "eligibility".to_string(),
            });
        return;
    }

    for entity_kind in entity_kinds {
        if !context.known_entity_kinds.contains(entity_kind) {
            report.errors.push(DagError::EligibilityEntityKindUnknown {
                location: location.clone(),
                slot_id: slot.id.clone(),
                entity_kind: entity_kind.clone(),
            });
        }
    }
}

fn validate_resolved_predicate_vector(
    location: &DagLocation,
    slot: &ResolvedSlot,
    field: &str,
    predicates: &[String],
    slot_closures: &HashMap<String, ClosureType>,
    report: &mut DagValidationReport,
) {
    for (idx, predicate) in predicates.iter().enumerate() {
        validate_resolved_predicate_text(
            location,
            slot,
            field,
            idx,
            predicate,
            slot_closures,
            report,
        );
    }
}

fn validate_resolved_predicate_text(
    location: &DagLocation,
    slot: &ResolvedSlot,
    field: &str,
    predicate_index: usize,
    predicate: &str,
    slot_closures: &HashMap<String, ClosureType>,
    report: &mut DagValidationReport,
) {
    if predicate.trim().is_empty() {
        return;
    }
    let Ok(parsed) = parse_green_when(predicate) else {
        return;
    };

    reject_unbounded_universal_quantifiers(
        location,
        slot,
        field,
        predicate_index,
        &parsed,
        slot_closures,
        report,
    );
}

fn reject_unbounded_universal_quantifiers(
    location: &DagLocation,
    slot: &ResolvedSlot,
    field: &str,
    predicate_index: usize,
    predicate: &Predicate,
    slot_closures: &HashMap<String, ClosureType>,
    report: &mut DagValidationReport,
) {
    match predicate {
        Predicate::And(items) => {
            for item in items {
                reject_unbounded_universal_quantifiers(
                    location,
                    slot,
                    field,
                    predicate_index,
                    item,
                    slot_closures,
                    report,
                );
            }
        }
        Predicate::Every { set, condition } => {
            if let Some(closure @ (ClosureType::ClosedUnbounded | ClosureType::Open)) =
                slot_closures.get(&set.kind)
            {
                report
                    .errors
                    .push(DagError::ResolvedClosureUniversalQuantifierInvalid {
                        location: location.clone(),
                        slot_id: slot.id.clone(),
                        field: field.to_string(),
                        predicate_index,
                        quantified_slot: set.kind.clone(),
                        closure: closure.clone(),
                    });
            }
            reject_unbounded_universal_quantifiers(
                location,
                slot,
                field,
                predicate_index,
                condition,
                slot_closures,
                report,
            );
        }
        Predicate::NoneExists { condition, .. } | Predicate::AtLeastOne { condition, .. } => {
            reject_unbounded_universal_quantifiers(
                location,
                slot,
                field,
                predicate_index,
                condition,
                slot_closures,
                report,
            );
        }
        Predicate::Count { condition, .. } => {
            if let Some(condition) = condition {
                reject_unbounded_universal_quantifiers(
                    location,
                    slot,
                    field,
                    predicate_index,
                    condition,
                    slot_closures,
                    report,
                );
            }
        }
        Predicate::Exists { .. }
        | Predicate::StateIn { .. }
        | Predicate::AttrCmp { .. }
        | Predicate::Obtained { .. } => {}
    }
}

fn collect_predicate_entity_refs(predicate: &Predicate, out: &mut HashSet<String>) {
    match predicate {
        Predicate::And(items) => {
            for item in items {
                collect_predicate_entity_refs(item, out);
            }
        }
        Predicate::Exists { entity }
        | Predicate::StateIn { entity, .. }
        | Predicate::AttrCmp { entity, .. }
        | Predicate::Obtained { entity, .. } => collect_entity_ref(entity, out),
        Predicate::Every { set, condition }
        | Predicate::NoneExists { set, condition }
        | Predicate::AtLeastOne { set, condition } => {
            collect_entity_set_ref(set, out);
            collect_predicate_entity_refs(condition, out);
        }
        Predicate::Count { set, condition, .. } => {
            collect_entity_set_ref(set, out);
            if let Some(condition) = condition {
                collect_predicate_entity_refs(condition, out);
            }
        }
    }
}

fn collect_entity_ref(entity: &EntityRef, out: &mut HashSet<String>) {
    match entity {
        EntityRef::This => {}
        EntityRef::Named(kind) | EntityRef::Parent(kind) => {
            out.insert(kind.clone());
        }
        EntityRef::Scoped { kind, .. } => {
            out.insert(kind.clone());
        }
    }
}

fn collect_entity_set_ref(set: &EntitySetRef, out: &mut HashSet<String>) {
    out.insert(set.kind.clone());
}

fn validate_long_lived_suspended_convention(
    workspace: &str,
    dag: &Dag,
    report: &mut DagValidationReport,
) {
    for slot in &dag.slots {
        if slot.suspended_state_exempt {
            continue;
        }
        let Some(sm) = slot.state_machine.as_ref() else {
            continue;
        };
        let SlotStateMachine::Structured(sm) = sm else {
            continue; // reference-form SMs exempt
        };
        if sm.expected_lifetime != Some(ExpectedLifetime::LongLived) {
            continue;
        }
        // V1.3-4: SUSPENDED may live in the primary SM OR in any
        // dual_lifecycle chain — dual-lifecycle is the canonical home
        // for operational states like suspended.
        let has_suspended_primary = sm
            .states
            .iter()
            .any(|s| s.id.eq_ignore_ascii_case("SUSPENDED"));
        let has_suspended_dual = slot.dual_lifecycle.iter().any(|dl| {
            dl.states
                .iter()
                .any(|s| s.id.eq_ignore_ascii_case("SUSPENDED"))
        });
        if !has_suspended_primary && !has_suspended_dual {
            report
                .warnings
                .push(DagWarning::LongLivedSlotMissingSuspended {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("slots.{}.state_machine", slot.id),
                    },
                    slot_id: slot.id.clone(),
                });
        }
    }
}

fn validate_periodic_review_cadence(workspace: &str, dag: &Dag, report: &mut DagValidationReport) {
    for slot in &dag.slots {
        let Some(_cadence) = &slot.periodic_review_cadence else {
            continue;
        };
        let Some(SlotStateMachine::Structured(sm)) = &slot.state_machine else {
            continue;
        };

        // Build: set of states that are reached (appear as `to` in some
        // transition). A re-review transition is any transition whose
        // `to` state ALSO appears as an earlier `to` OR as an entry
        // state — i.e. we can revisit a "review phase" state.
        //
        // Equivalently: the state graph has at least one back-edge /
        // cycle. Simpler heuristic that correctly handles:
        //   APPROVED → EXPIRED → IN_PROGRESS → APPROVED (cycle through
        //   IN_PROGRESS which is re-reviewable).
        //
        // Detect: any state appears as `to` in 2+ transitions (reached
        // from multiple sources — implies revisit) OR the entry state
        // appears as a `to` (direct back-edge).
        let mut to_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for t in &sm.transitions {
            to_counts
                .entry(t.to.as_str())
                .and_modify(|v| *v += 1)
                .or_insert(1);
        }
        let has_back_edge = to_counts.values().any(|&c| c >= 2);

        let entry_state_id: Option<&str> =
            sm.states.iter().find(|s| s.entry).map(|s| s.id.as_str());
        let entry_reached = entry_state_id
            .map(|eid| sm.transitions.iter().any(|t| t.to == eid))
            .unwrap_or(false);

        if !has_back_edge && !entry_reached {
            report
                .warnings
                .push(DagWarning::PeriodicReviewCadenceWithoutRereviewTransition {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("slots.{}.periodic_review_cadence", slot.id),
                    },
                    slot_id: slot.id.clone(),
                });
        }
    }
}

fn validate_validity_window_expired_state(
    workspace: &str,
    dag: &Dag,
    report: &mut DagValidationReport,
) {
    for ev in &dag.evidence_types {
        if ev.validity_window == "once" {
            continue;
        }
        // Heuristic: somewhere in the DAG there should be a slot whose SM
        // has an EXPIRED state. If none, flag warning.
        let any_expired = dag.slots.iter().any(|s| {
            matches!(
                &s.state_machine,
                Some(SlotStateMachine::Structured(sm))
                    if sm.states.iter().any(|st| st.id.eq_ignore_ascii_case("EXPIRED"))
            )
        });
        if !any_expired {
            report
                .warnings
                .push(DagWarning::ValidityWindowWithoutExpiredState {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("evidence_types[{}]", ev.id),
                    },
                    evidence_type_id: ev.id.clone(),
                });
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct RawConstellationMap {
    #[serde(default)]
    constellation: Option<String>,
    #[serde(default)]
    slots: BTreeMap<String, RawConstellationSlot>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct RawConstellationSlot {
    #[serde(default)]
    state_machine: Option<String>,
    #[serde(default)]
    closure: Option<serde_yaml::Value>,
    #[serde(default)]
    eligibility: Option<serde_yaml::Value>,
    #[serde(default)]
    cardinality_max: Option<serde_yaml::Value>,
    #[serde(default)]
    entry_state: Option<serde_yaml::Value>,
    #[serde(default)]
    attachment_predicates: Vec<String>,
    #[serde(default)]
    addition_predicates: Vec<String>,
    #[serde(default)]
    aggregate_breach_checks: Vec<String>,
    #[serde(default, rename = "+attachment_predicates")]
    additive_attachment_predicates: Vec<String>,
    #[serde(default, rename = "+addition_predicates")]
    additive_addition_predicates: Vec<String>,
    #[serde(default, rename = "+aggregate_breach_checks")]
    additive_aggregate_breach_checks: Vec<String>,
    #[serde(default)]
    role_guard: Option<serde_yaml::Value>,
    #[serde(default)]
    justification_required: Option<serde_yaml::Value>,
    #[serde(default)]
    audit_class: Option<serde_yaml::Value>,
    #[serde(default)]
    completeness_assertion: Option<serde_yaml::Value>,
}

fn validate_raw_constellation_map_schema_coordination(
    loaded: &BTreeMap<String, LoadedDag>,
    source_name: &str,
    map: &RawConstellationMap,
    report: &mut DagValidationReport,
) {
    let constellation = map
        .constellation
        .as_deref()
        .unwrap_or("<unknown-constellation>");
    for (slot_id, slot) in &map.slots {
        let location = DagLocation {
            workspace: constellation.to_string(),
            path: format!("{source_name}:slots.{slot_id}"),
        };

        validate_constellation_gate_predicate_vector(
            &location,
            slot_id,
            "attachment_predicates",
            &slot.attachment_predicates,
            report,
        );
        validate_constellation_gate_predicate_vector(
            &location,
            slot_id,
            "addition_predicates",
            &slot.addition_predicates,
            report,
        );
        validate_constellation_gate_predicate_vector(
            &location,
            slot_id,
            "aggregate_breach_checks",
            &slot.aggregate_breach_checks,
            report,
        );
        reject_additive_predicate_vector(
            &location,
            slot_id,
            "+attachment_predicates",
            &slot.additive_attachment_predicates,
            report,
        );
        reject_additive_predicate_vector(
            &location,
            slot_id,
            "+addition_predicates",
            &slot.additive_addition_predicates,
            report,
        );
        reject_additive_predicate_vector(
            &location,
            slot_id,
            "+aggregate_breach_checks",
            &slot.additive_aggregate_breach_checks,
            report,
        );

        for (dag_workspace, ld) in loaded {
            let Some(dag_slot) = ld.dag.slots.iter().find(|dag_slot| dag_slot.id == *slot_id)
            else {
                continue;
            };
            warn_gate_field_drift(&location, slot_id, dag_workspace, dag_slot, slot, report);
            warn_state_machine_mismatch(&location, slot_id, dag_workspace, dag_slot, slot, report);
        }
    }
}

fn validate_constellation_gate_predicate_vector(
    location: &DagLocation,
    slot_id: &str,
    field: &str,
    predicates: &[String],
    report: &mut DagValidationReport,
) {
    validate_gate_predicate_vector(location, slot_id, field, predicates, report);
}

fn warn_state_machine_mismatch(
    location: &DagLocation,
    slot_id: &str,
    dag_workspace: &str,
    dag_slot: &Slot,
    constellation_slot: &RawConstellationSlot,
    report: &mut DagValidationReport,
) {
    let Some(constellation_state_machine) = &constellation_slot.state_machine else {
        return;
    };
    let Some(SlotStateMachine::Structured(dag_state_machine)) = &dag_slot.state_machine else {
        return;
    };
    if dag_state_machine.id != *constellation_state_machine {
        report
            .warnings
            .push(DagWarning::SchemaCoordinationStateMachineMismatch {
                location: location.clone(),
                slot_id: slot_id.to_string(),
                dag_workspace: dag_workspace.to_string(),
                dag_state_machine: dag_state_machine.id.clone(),
                constellation_state_machine: constellation_state_machine.clone(),
            });
    }
}

fn warn_gate_field_drift(
    location: &DagLocation,
    slot_id: &str,
    dag_workspace: &str,
    dag_slot: &Slot,
    constellation_slot: &RawConstellationSlot,
    report: &mut DagValidationReport,
) {
    let checks = [
        (
            "closure",
            dag_slot.closure.is_some(),
            constellation_slot.closure.is_some(),
        ),
        (
            "eligibility",
            dag_slot.eligibility.is_some(),
            constellation_slot.eligibility.is_some(),
        ),
        (
            "cardinality_max",
            dag_slot.cardinality_max.is_some(),
            constellation_slot.cardinality_max.is_some(),
        ),
        (
            "entry_state",
            dag_slot.entry_state.is_some(),
            constellation_slot.entry_state.is_some(),
        ),
        (
            "attachment_predicates",
            !dag_slot.attachment_predicates.is_empty(),
            !constellation_slot.attachment_predicates.is_empty(),
        ),
        (
            "addition_predicates",
            !dag_slot.addition_predicates.is_empty(),
            !constellation_slot.addition_predicates.is_empty(),
        ),
        (
            "aggregate_breach_checks",
            !dag_slot.aggregate_breach_checks.is_empty(),
            !constellation_slot.aggregate_breach_checks.is_empty(),
        ),
        (
            "role_guard",
            dag_slot.role_guard.is_some(),
            constellation_slot.role_guard.is_some(),
        ),
        (
            "justification_required",
            dag_slot.justification_required.is_some(),
            constellation_slot.justification_required.is_some(),
        ),
        (
            "audit_class",
            dag_slot.audit_class.is_some(),
            constellation_slot.audit_class.is_some(),
        ),
        (
            "completeness_assertion",
            dag_slot.completeness_assertion.is_some(),
            constellation_slot.completeness_assertion.is_some(),
        ),
    ];

    for (field, dag_sets_field, constellation_sets_field) in checks {
        if dag_sets_field && constellation_sets_field {
            report
                .warnings
                .push(DagWarning::SchemaCoordinationSlotFieldDrift {
                    location: location.clone(),
                    slot_id: slot_id.to_string(),
                    field: field.to_string(),
                    dag_workspace: dag_workspace.to_string(),
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Cycle detection — DFS over derived_cross_workspace_state graph
// ---------------------------------------------------------------------------

fn detect_derivation_cycles(
    loaded: &BTreeMap<String, LoadedDag>,
    report: &mut DagValidationReport,
) {
    // Build graph: node = "workspace.slot.state" (for declared derived
    // states); edges = "this derived state depends on that (workspace,
    // slot, state)".
    type Node = String;
    let mut edges: HashMap<Node, Vec<Node>> = HashMap::new();
    let mut declared: HashSet<Node> = HashSet::new();

    for (ws, ld) in loaded {
        for d in &ld.dag.derived_cross_workspace_state {
            let node = format!("{}.{}.{}", d.host_workspace, d.host_slot, d.host_state);
            declared.insert(node.clone());
            let deps: Vec<Node> = d
                .derivation
                .all_of
                .iter()
                .chain(d.derivation.any_of.iter())
                .filter_map(|c| match c {
                    DerivationCondition::Structured(s) => {
                        let state_str = match &s.state {
                            Some(StateSelector::Single(s)) => s.clone(),
                            Some(StateSelector::Set(v)) => v.join("|"),
                            None => "<predicate>".to_string(),
                        };
                        Some(format!("{}.{}.{}", s.workspace, s.slot, state_str))
                    }
                    _ => None,
                })
                .collect();
            edges.entry(node).or_default().extend(deps);
            let _ = ws; // silence
        }
    }

    // DFS cycle detection restricted to declared derived states (edges to
    // non-declared nodes are leaves). Owned-node version to keep borrowing
    // simple; graphs are tiny so allocation cost is negligible.
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }
    let mut color: HashMap<Node, Color> = HashMap::new();
    for n in declared.iter() {
        color.insert(n.clone(), Color::White);
    }

    for start in declared.iter() {
        if color.get(start).copied() != Some(Color::White) {
            continue;
        }
        let mut stack: Vec<(Node, usize)> = vec![(start.clone(), 0)];
        let mut path: Vec<Node> = vec![start.clone()];
        color.insert(start.clone(), Color::Gray);
        while let Some((node, idx)) = stack.pop() {
            let neighbours: Vec<Node> = edges.get(&node).cloned().unwrap_or_default();
            if idx < neighbours.len() {
                stack.push((node.clone(), idx + 1));
                let next = neighbours[idx].clone();
                if !declared.contains(&next) {
                    continue; // leaf
                }
                match color.get(&next).copied().unwrap_or(Color::White) {
                    Color::Gray => {
                        let cycle_start =
                            path.iter().position(|n| n == &next).unwrap_or(path.len());
                        let mut cycle_path: Vec<String> = path[cycle_start..].to_vec();
                        cycle_path.push(next);
                        report
                            .errors
                            .push(DagError::DerivedCrossWorkspaceStateCycle { cycle_path });
                    }
                    Color::White => {
                        color.insert(next.clone(), Color::Gray);
                        path.push(next.clone());
                        stack.push((next, 0));
                    }
                    Color::Black => {}
                }
            } else {
                color.insert(node, Color::Black);
                path.pop();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ws_dag(yaml: &str) -> LoadedDag {
        let dag: Dag = serde_yaml::from_str(yaml).unwrap();
        LoadedDag {
            source_path: std::path::PathBuf::new(),
            dag,
        }
    }

    #[test]
    fn unresolved_cross_workspace_source_slot_errors() {
        let deal = ws_dag(
            r#"
workspace: deal
dag_id: deal_dag
cross_workspace_constraints:
  - id: bad_ref
    source_workspace: kyc
    source_slot: does_not_exist
    source_state: APPROVED
    target_workspace: deal
    target_slot: deal
    target_transition: "A -> B"
    severity: error
slots:
  - id: deal
    stateless: false
    state_machine:
      id: deal_lifecycle
      states: [{ id: A, entry: true }, { id: B }]
"#,
        );
        let kyc = ws_dag(
            r#"
workspace: kyc
dag_id: kyc_dag
slots:
  - id: kyc_case
    stateless: false
    state_machine:
      id: kyc_case_lifecycle
      states: [{ id: APPROVED }]
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("deal".to_string(), deal);
        map.insert("kyc".to_string(), kyc);
        let report = validate_dags(&map);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, DagError::CrossWorkspaceConstraintUnresolved { .. })));
    }

    #[test]
    fn self_referencing_cross_workspace_errors() {
        let deal = ws_dag(
            r#"
workspace: deal
dag_id: deal_dag
cross_workspace_constraints:
  - id: self_ref
    source_workspace: deal
    source_slot: deal
    source_state: A
    target_workspace: deal
    target_slot: deal
    target_transition: "A -> B"
slots:
  - id: deal
    stateless: false
    state_machine:
      id: deal_lifecycle
      states: [{ id: A, entry: true }, { id: B }]
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("deal".to_string(), deal);
        let report = validate_dags(&map);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, DagError::CrossWorkspaceConstraintSelfReference { .. })));
    }

    #[test]
    fn parent_slot_unresolved_errors() {
        let cbu = ws_dag(
            r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    parent_slot:
      workspace: cbu
      slot: does_not_exist
    state_dependency:
      cascade_rules:
        - parent_state: SUSPENDED
          child_allowed_states: [SUSPENDED]
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("cbu".to_string(), cbu);
        let report = validate_dags(&map);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, DagError::ParentSlotUnresolved { .. })));
    }

    #[test]
    fn long_lived_missing_suspended_warns() {
        let dag = ws_dag(
            r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    state_machine:
      id: widget_lifecycle
      expected_lifetime: long_lived
      states: [{ id: DRAFT, entry: true }, { id: ACTIVE }, { id: CLOSED }]
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report
            .warnings
            .iter()
            .any(|w| matches!(w, DagWarning::LongLivedSlotMissingSuspended { .. })));
    }

    #[test]
    fn long_lived_exempt_suppresses_warning() {
        let dag = ws_dag(
            r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    suspended_state_exempt: true
    state_machine:
      id: widget_lifecycle
      expected_lifetime: long_lived
      states: [{ id: DRAFT, entry: true }, { id: CLOSED }]
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn dual_lifecycle_missing_junction_errors() {
        let dag = ws_dag(
            r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    state_machine:
      id: primary
      states: [{ id: A, entry: true }]
    dual_lifecycle:
      - id: secondary
        junction_state_from_primary: NOT_IN_PRIMARY
        states: [{ id: B }]
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, DagError::DualLifecycleJunctionMissing { .. })));
    }

    #[test]
    fn category_gated_both_gates_errors() {
        let dag = ws_dag(
            r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    category_gated:
      category_column: cbu_category
      category_source: cbus
      activated_by: [FUND_MANDATE]
      deactivated_by: [RETAIL_CLIENT]
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, DagError::CategoryGatedMutuallyExclusiveGates { .. })));
    }

    #[test]
    fn malformed_green_when_errors() {
        let dag = ws_dag(
            r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    state_machine:
      id: widget_lifecycle
      states:
        - id: DRAFT
          entry: true
        - id: APPROVED
          green_when: "every required"
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, DagError::GreenWhenParseError { .. })));
    }

    #[test]
    fn green_when_unbound_entity_errors() {
        let dag = ws_dag(
            r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    state_machine:
      id: widget_lifecycle
      states:
        - id: DRAFT
          entry: true
        - id: APPROVED
          green_when: "review exists AND review.state = COMPLETE"
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, DagError::GreenWhenUnboundEntity { entity_kind, .. } if entity_kind == "review")));
    }

    #[test]
    fn green_when_bound_entity_is_clean() {
        let dag = ws_dag(
            r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    state_machine:
      id: widget_lifecycle
      predicate_bindings:
        - entity: review
          source_kind: dag_entity
      states:
        - id: DRAFT
          entry: true
        - id: APPROVED
          green_when: "review exists AND review.state = COMPLETE"
  - id: review
    stateless: false
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(
            report.errors.is_empty(),
            "expected no validation errors, got {:#?}",
            report.errors
        );
    }

    #[test]
    fn derivation_cycle_detected() {
        // A -> B -> A cycle in derived states
        let a = ws_dag(
            r#"
workspace: a
dag_id: a_dag
slots:
  - id: main
    stateless: false
    state_machine:
      id: a_sm
      states: [{ id: ready }]
derived_cross_workspace_state:
  - id: a_ready
    host_workspace: a
    host_slot: main
    host_state: ready
    derivation:
      all_of:
        - { workspace: b, slot: main, state: ready }
"#,
        );
        let b = ws_dag(
            r#"
workspace: b
dag_id: b_dag
slots:
  - id: main
    stateless: false
    state_machine:
      id: b_sm
      states: [{ id: ready }]
derived_cross_workspace_state:
  - id: b_ready
    host_workspace: b
    host_slot: main
    host_state: ready
    derivation:
      all_of:
        - { workspace: a, slot: main, state: ready }
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("a".to_string(), a);
        map.insert("b".to_string(), b);
        let report = validate_dags(&map);
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, DagError::DerivedCrossWorkspaceStateCycle { .. })),
            "expected cycle detection, got: {:#?}",
            report.errors
        );
    }

    #[test]
    fn resolved_tollgate_clean() {
        // Full happy path: KYC approved → CBU tollgate derived from KYC+deal
        let kyc = ws_dag(
            r#"
workspace: kyc
dag_id: kyc_dag
slots:
  - id: kyc_case
    stateless: false
    suspended_state_exempt: true
    state_machine:
      id: kyc_case_lifecycle
      states: [{ id: INTAKE, entry: true }, { id: APPROVED }]
"#,
        );
        let deal = ws_dag(
            r#"
workspace: deal
dag_id: deal_dag
slots:
  - id: deal
    stateless: false
    state_machine:
      id: deal_lifecycle
      expected_lifetime: long_lived
      states: [{ id: PROSPECT, entry: true }, { id: CONTRACTED }, { id: ACTIVE }, { id: SUSPENDED }, { id: OFFBOARDED }]
"#,
        );
        let cbu = ws_dag(
            r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    state_machine:
      id: cbu_lifecycle
      expected_lifetime: long_lived
      states: [{ id: DISCOVERED, entry: true }, { id: VALIDATED }, { id: SUSPENDED }, { id: operationally_active }]
derived_cross_workspace_state:
  - id: cbu_operationally_active
    description: "Tollgate aggregate"
    host_workspace: cbu
    host_slot: cbu
    host_state: operationally_active
    derivation:
      all_of:
        - { workspace: kyc, slot: kyc_case, state: APPROVED }
        - { workspace: deal, slot: deal, state: [CONTRACTED, ACTIVE] }
"#,
        );
        let mut map = BTreeMap::new();
        map.insert("kyc".to_string(), kyc);
        map.insert("deal".to_string(), deal);
        map.insert("cbu".to_string(), cbu);
        let report = validate_dags(&map);
        assert!(
            report.is_clean(),
            "expected clean, got: {:#?}",
            report.errors
        );
    }
}
