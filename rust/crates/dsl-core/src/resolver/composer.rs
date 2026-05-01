use super::{
    compute_version_hash, ResolvedSlot, ResolvedSource, ResolvedTemplate, ResolvedTransition,
    ResolverProvenance, ShapeRef, SlotProvenance, WorkspaceId,
};
use crate::config::dag::{
    load_dags_from_dir, ClosureType, CompletenessAssertionConfig as DagCompletenessAssertionConfig,
    Dag, EligibilityConstraint, LoadedDag, Slot as DagSlot, SlotStateMachine,
};
use crate::resolver::shape_rule::{
    load_shape_rules_from_dir, LoadedShapeRule, SlotGateMetadataRefinement, StructuralFacts,
};
use anyhow::{Context, Result};
use sem_os_core::constellation_map_def as core_map;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(String),
    #[error("constellation not found: {0}")]
    ConstellationNotFound(String),
    #[error("shape rule not found: {0}")]
    ShapeRuleNotFound(String),
    #[error("ambiguous vector composition for slot {slot} field {field}: replacement and additive values are both authored in {shape}")]
    AmbiguousVectorComposition {
        slot: String,
        field: String,
        shape: String,
    },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct LoadedConstellationMap {
    pub source_path: PathBuf,
    pub body: core_map::ConstellationMapDefBody,
}

#[derive(Debug, Clone)]
pub struct ResolverInputs {
    pub dag_taxonomies: BTreeMap<String, LoadedDag>,
    pub constellation_maps: BTreeMap<String, LoadedConstellationMap>,
    pub shape_rules: BTreeMap<String, LoadedShapeRule>,
    pub seed_root: PathBuf,
}

impl ResolverInputs {
    pub fn from_seed_root(seed_root: impl Into<PathBuf>) -> Result<Self> {
        let seed_root = seed_root.into();
        let dag_taxonomies = load_dags_from_dir(&seed_root.join("dag_taxonomies"))?;
        let constellation_maps =
            load_constellation_maps_from_dir(&seed_root.join("constellation_maps"))?;
        let shape_rules = load_shape_rules_from_dir(&seed_root.join("shape_rules"))?;
        Ok(Self {
            dag_taxonomies,
            constellation_maps,
            shape_rules,
            seed_root,
        })
    }

    pub fn from_workspace_config_dir(config_dir: impl Into<PathBuf>) -> Result<Self> {
        Self::from_seed_root(config_dir.into().join("sem_os_seeds"))
    }

    pub fn default_from_cargo_manifest() -> Result<Self> {
        Self::from_workspace_config_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config"),
        )
    }

    pub fn legacy_constellation_stack(
        &self,
        constellation_id: &str,
    ) -> Result<Vec<core_map::ConstellationMapDefBody>, ResolveError> {
        let target = self
            .constellation_maps
            .get(constellation_id)
            .ok_or_else(|| ResolveError::ConstellationNotFound(constellation_id.to_string()))?;

        let mut stack = Vec::new();
        if is_cbu_business_plane(&target.body) {
            push_if_present(&mut stack, &self.constellation_maps, "group.ownership");
        }
        stack.push(target.body.clone());
        if is_cbu_business_plane(&target.body) {
            push_if_present(&mut stack, &self.constellation_maps, "kyc.onboarding");
        }
        Ok(stack)
    }
}

#[derive(Debug, Deserialize)]
struct SeedConstellationMap {
    constellation: String,
    #[serde(default)]
    description: Option<String>,
    jurisdiction: String,
    #[serde(default)]
    slots: BTreeMap<String, core_map::SlotDef>,
}

pub fn load_constellation_maps_from_dir(
    dir: &Path,
) -> Result<BTreeMap<String, LoadedConstellationMap>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {dir:?}"))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read constellation map {path:?}"))?;
        let seed: SeedConstellationMap = serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse constellation map {path:?}"))?;
        let body = core_map::ConstellationMapDefBody {
            fqn: seed.constellation.clone(),
            constellation: seed.constellation,
            description: seed.description,
            jurisdiction: seed.jurisdiction,
            slots: seed.slots,
        };
        out.insert(
            body.constellation.clone(),
            LoadedConstellationMap {
                source_path: path,
                body,
            },
        );
    }
    Ok(out)
}

pub fn resolve_template(
    composite_shape: impl Into<ShapeRef>,
    workspace: impl Into<WorkspaceId>,
    inputs: &ResolverInputs,
) -> Result<ResolvedTemplate, ResolveError> {
    let composite_shape = composite_shape.into();
    let workspace = workspace.into();
    let loaded_dag = inputs
        .dag_taxonomies
        .get(&workspace)
        .ok_or_else(|| ResolveError::WorkspaceNotFound(workspace.clone()))?;
    let leaf = inputs
        .constellation_maps
        .get(&composite_shape)
        .ok_or_else(|| ResolveError::ConstellationNotFound(composite_shape.clone()))?;

    let legacy_stack = inputs.legacy_constellation_stack(&composite_shape)?;
    let shape_chain = inputs.shape_rule_chain(&composite_shape)?;
    let structural_facts = compose_structural_facts(&shape_chain);
    let mut slot_ids = BTreeSet::new();
    for slot in &loaded_dag.dag.slots {
        slot_ids.insert(slot.id.clone());
    }
    for slot_id in leaf.body.slots.keys() {
        slot_ids.insert(slot_id.clone());
    }
    for rule in &shape_chain {
        for slot_id in rule.body.slots.keys() {
            slot_ids.insert(slot_id.clone());
        }
    }

    let slots = slot_ids
        .into_iter()
        .map(|slot_id| {
            compose_slot(
                &slot_id,
                loaded_dag.dag.slots.iter().find(|slot| slot.id == slot_id),
                leaf.body.slots.get(&slot_id),
                &shape_chain,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let transitions = compose_transitions(&loaded_dag.dag);

    let mut paths = vec![loaded_dag.source_path.as_path(), leaf.source_path.as_path()];
    for loaded in &legacy_stack {
        if let Some(found) = inputs.constellation_maps.get(&loaded.constellation) {
            paths.push(found.source_path.as_path());
        }
    }
    for rule in &shape_chain {
        paths.push(rule.source_path.as_path());
    }
    let version = compute_version_hash(&paths, &composite_shape, &workspace);

    Ok(ResolvedTemplate {
        workspace,
        composite_shape,
        structural_facts,
        slots,
        transitions,
        version,
        generated_at: "compute-on-read".to_string(),
        generated_from: ResolverProvenance {
            dag_paths: vec![loaded_dag.source_path.to_string_lossy().to_string()],
            constellation_paths: paths
                .into_iter()
                .skip(1)
                .filter(|path| {
                    !path
                        .components()
                        .any(|component| component.as_os_str().to_string_lossy() == "shape_rules")
                })
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            shape_rule_paths: shape_chain
                .iter()
                .map(|rule| rule.source_path.to_string_lossy().to_string())
                .collect(),
            legacy_constellation_stack: legacy_stack,
        },
    })
}

fn compose_structural_facts(shape_chain: &[&LoadedShapeRule]) -> StructuralFacts {
    let mut out = StructuralFacts::default();
    for rule in shape_chain {
        let facts = &rule.body.structural_facts;
        if facts.jurisdiction.is_some() {
            out.jurisdiction = facts.jurisdiction.clone();
        }
        if facts.structure_type.is_some() {
            out.structure_type = facts.structure_type.clone();
        }
        if facts.trading_profile_type.is_some() {
            out.trading_profile_type = facts.trading_profile_type.clone();
        }
        append_unique(
            &mut out.allowed_structure_types,
            &facts.allowed_structure_types,
        );
        append_unique(&mut out.document_bundles, &facts.document_bundles);
        append_unique(&mut out.required_roles, &facts.required_roles);
        append_unique(&mut out.optional_roles, &facts.optional_roles);
        append_unique(&mut out.deferred_roles, &facts.deferred_roles);
    }
    out
}

fn append_unique(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        if !target.contains(value) {
            target.push(value.clone());
        }
    }
}

fn compose_slot(
    id: &str,
    dag_slot: Option<&DagSlot>,
    constellation_slot: Option<&core_map::SlotDef>,
    shape_chain: &[&LoadedShapeRule],
) -> Result<ResolvedSlot, ResolveError> {
    let mut provenance = SlotProvenance::default();

    let mut slot = ResolvedSlot {
        id: id.to_string(),
        state_machine: dag_state_machine_id(dag_slot).or_else(|| {
            source(
                &mut provenance,
                "state_machine",
                ResolvedSource::ConstellationMap,
            );
            constellation_slot.and_then(|slot| slot.state_machine.clone())
        }),
        predicate_bindings: dag_slot
            .and_then(|slot| match &slot.state_machine {
                Some(SlotStateMachine::Structured(machine)) => {
                    source(
                        &mut provenance,
                        "predicate_bindings",
                        ResolvedSource::DagTaxonomy,
                    );
                    Some(machine.predicate_bindings.clone())
                }
                _ => None,
            })
            .unwrap_or_default(),
        table: constellation_slot.and_then(|slot| {
            source(&mut provenance, "table", ResolvedSource::ConstellationMap);
            slot.table.clone()
        }),
        pk: constellation_slot.and_then(|slot| {
            source(&mut provenance, "pk", ResolvedSource::ConstellationMap);
            slot.pk.clone()
        }),
        join: constellation_slot.and_then(|slot| {
            source(&mut provenance, "join", ResolvedSource::ConstellationMap);
            slot.join.clone()
        }),
        entity_kinds: constellation_slot
            .map(|slot| {
                source(
                    &mut provenance,
                    "entity_kinds",
                    ResolvedSource::ConstellationMap,
                );
                slot.entity_kinds.clone()
            })
            .unwrap_or_default(),
        cardinality: constellation_slot.map(|slot| {
            source(
                &mut provenance,
                "cardinality",
                ResolvedSource::ConstellationMap,
            );
            slot.cardinality
        }),
        depends_on: constellation_slot
            .map(|slot| {
                source(
                    &mut provenance,
                    "depends_on",
                    ResolvedSource::ConstellationMap,
                );
                slot.depends_on.clone()
            })
            .unwrap_or_default(),
        placeholder: constellation_slot.and_then(|slot| {
            source(
                &mut provenance,
                "placeholder",
                ResolvedSource::ConstellationMap,
            );
            slot.placeholder.clone()
        }),
        overlays: constellation_slot
            .map(|slot| {
                source(
                    &mut provenance,
                    "overlays",
                    ResolvedSource::ConstellationMap,
                );
                slot.overlays.clone()
            })
            .unwrap_or_default(),
        edge_overlays: constellation_slot
            .map(|slot| {
                source(
                    &mut provenance,
                    "edge_overlays",
                    ResolvedSource::ConstellationMap,
                );
                slot.edge_overlays.clone()
            })
            .unwrap_or_default(),
        verbs: constellation_slot
            .map(|slot| {
                source(&mut provenance, "verbs", ResolvedSource::ConstellationMap);
                slot.verbs.clone()
            })
            .unwrap_or_default(),
        children: constellation_slot
            .map(|slot| {
                source(
                    &mut provenance,
                    "children",
                    ResolvedSource::ConstellationMap,
                );
                slot.children.clone()
            })
            .unwrap_or_default(),
        max_depth: constellation_slot.and_then(|slot| {
            source(
                &mut provenance,
                "max_depth",
                ResolvedSource::ConstellationMap,
            );
            slot.max_depth
        }),
        closure: dag_slot
            .and_then(|slot| slot.closure.clone())
            .inspect(|_| {
                source(&mut provenance, "closure", ResolvedSource::DagTaxonomy);
            })
            .or_else(|| {
                constellation_slot.and_then(|slot| {
                    source(&mut provenance, "closure", ResolvedSource::ConstellationMap);
                    convert_closure(slot.closure.as_ref())
                })
            }),
        eligibility: dag_slot
            .and_then(|slot| slot.eligibility.clone())
            .inspect(|_| {
                source(&mut provenance, "eligibility", ResolvedSource::DagTaxonomy);
            })
            .or_else(|| {
                constellation_slot.and_then(|slot| {
                    source(
                        &mut provenance,
                        "eligibility",
                        ResolvedSource::ConstellationMap,
                    );
                    convert_eligibility(slot.eligibility.as_ref())
                })
            }),
        cardinality_max: dag_slot
            .and_then(|slot| slot.cardinality_max)
            .inspect(|_| {
                source(
                    &mut provenance,
                    "cardinality_max",
                    ResolvedSource::DagTaxonomy,
                );
            })
            .or_else(|| {
                constellation_slot.and_then(|slot| {
                    source(
                        &mut provenance,
                        "cardinality_max",
                        ResolvedSource::ConstellationMap,
                    );
                    slot.cardinality_max
                })
            }),
        entry_state: dag_slot
            .and_then(|slot| slot.entry_state.clone())
            .inspect(|_| {
                source(&mut provenance, "entry_state", ResolvedSource::DagTaxonomy);
            })
            .or_else(|| {
                constellation_slot.and_then(|slot| {
                    source(
                        &mut provenance,
                        "entry_state",
                        ResolvedSource::ConstellationMap,
                    );
                    slot.entry_state.clone()
                })
            }),
        attachment_predicates: gate_vec(
            &mut provenance,
            "attachment_predicates",
            dag_slot.map(|slot| &slot.attachment_predicates),
            constellation_slot.map(|slot| &slot.attachment_predicates),
        ),
        addition_predicates: gate_vec(
            &mut provenance,
            "addition_predicates",
            dag_slot.map(|slot| &slot.addition_predicates),
            constellation_slot.map(|slot| &slot.addition_predicates),
        ),
        aggregate_breach_checks: gate_vec(
            &mut provenance,
            "aggregate_breach_checks",
            dag_slot.map(|slot| &slot.aggregate_breach_checks),
            constellation_slot.map(|slot| &slot.aggregate_breach_checks),
        ),
        role_guard: dag_slot
            .and_then(|slot| slot.role_guard.clone())
            .inspect(|_| {
                source(&mut provenance, "role_guard", ResolvedSource::DagTaxonomy);
            })
            .or_else(|| {
                constellation_slot.and_then(|slot| {
                    source(
                        &mut provenance,
                        "role_guard",
                        ResolvedSource::ConstellationMap,
                    );
                    convert_role_guard(slot.role_guard.as_ref())
                })
            }),
        justification_required: dag_slot
            .and_then(|slot| slot.justification_required)
            .or_else(|| constellation_slot.and_then(|slot| slot.justification_required)),
        audit_class: dag_slot
            .and_then(|slot| slot.audit_class.clone())
            .or_else(|| constellation_slot.and_then(|slot| slot.audit_class.clone())),
        completeness_assertion: dag_slot
            .and_then(|slot| {
                slot.completeness_assertion
                    .as_ref()
                    .map(convert_dag_completeness)
            })
            .or_else(|| constellation_slot.and_then(|slot| slot.completeness_assertion.clone())),
        provenance,
    };

    for rule in shape_chain {
        if let Some(refinement) = rule.body.slots.get(id) {
            apply_slot_refinement(&mut slot, refinement, &rule.body.shape)?;
        }
    }

    Ok(slot)
}

impl ResolverInputs {
    fn shape_rule_chain(
        &self,
        composite_shape: &str,
    ) -> Result<Vec<&LoadedShapeRule>, ResolveError> {
        let Some(leaf) = self.shape_rules.get(composite_shape) else {
            return Ok(Vec::new());
        };

        let mut out = Vec::new();
        self.push_shape_rule_ancestors(leaf, &mut out)?;
        out.push(leaf);
        Ok(out)
    }

    fn push_shape_rule_ancestors<'a>(
        &'a self,
        rule: &'a LoadedShapeRule,
        out: &mut Vec<&'a LoadedShapeRule>,
    ) -> Result<(), ResolveError> {
        for ancestor in &rule.body.extends {
            let loaded = self
                .shape_rules
                .get(ancestor)
                .ok_or_else(|| ResolveError::ShapeRuleNotFound(ancestor.clone()))?;
            self.push_shape_rule_ancestors(loaded, out)?;
            out.push(loaded);
        }
        Ok(())
    }
}

fn apply_slot_refinement(
    slot: &mut ResolvedSlot,
    refinement: &SlotGateMetadataRefinement,
    shape: &str,
) -> Result<(), ResolveError> {
    if let Some(value) = &refinement.closure {
        slot.closure = Some(value.clone());
        source(&mut slot.provenance, "closure", ResolvedSource::ShapeRule);
    }
    if let Some(value) = &refinement.eligibility {
        slot.eligibility = Some(value.clone());
        source(
            &mut slot.provenance,
            "eligibility",
            ResolvedSource::ShapeRule,
        );
    }
    if let Some(value) = refinement.cardinality_max {
        slot.cardinality_max = Some(value);
        source(
            &mut slot.provenance,
            "cardinality_max",
            ResolvedSource::ShapeRule,
        );
    }
    if let Some(value) = &refinement.entry_state {
        slot.entry_state = Some(value.clone());
        source(
            &mut slot.provenance,
            "entry_state",
            ResolvedSource::ShapeRule,
        );
    }
    apply_vector_refinement(
        &slot.id,
        shape,
        "attachment_predicates",
        &mut slot.attachment_predicates,
        &refinement.attachment_predicates,
        &refinement.additive_attachment_predicates,
        &mut slot.provenance,
    )?;
    apply_vector_refinement(
        &slot.id,
        shape,
        "addition_predicates",
        &mut slot.addition_predicates,
        &refinement.addition_predicates,
        &refinement.additive_addition_predicates,
        &mut slot.provenance,
    )?;
    apply_vector_refinement(
        &slot.id,
        shape,
        "aggregate_breach_checks",
        &mut slot.aggregate_breach_checks,
        &refinement.aggregate_breach_checks,
        &refinement.additive_aggregate_breach_checks,
        &mut slot.provenance,
    )?;
    if let Some(value) = &refinement.role_guard {
        slot.role_guard = Some(value.clone());
        source(
            &mut slot.provenance,
            "role_guard",
            ResolvedSource::ShapeRule,
        );
    }
    if let Some(value) = refinement.justification_required {
        slot.justification_required = Some(value);
        source(
            &mut slot.provenance,
            "justification_required",
            ResolvedSource::ShapeRule,
        );
    }
    if let Some(value) = &refinement.audit_class {
        slot.audit_class = Some(value.clone());
        source(
            &mut slot.provenance,
            "audit_class",
            ResolvedSource::ShapeRule,
        );
    }
    if let Some(value) = &refinement.completeness_assertion {
        slot.completeness_assertion = Some(value.clone());
        source(
            &mut slot.provenance,
            "completeness_assertion",
            ResolvedSource::ShapeRule,
        );
    }
    Ok(())
}

fn apply_vector_refinement(
    slot_id: &str,
    shape: &str,
    field: &str,
    target: &mut Vec<String>,
    replacement: &[String],
    additive: &[String],
    provenance: &mut SlotProvenance,
) -> Result<(), ResolveError> {
    if !replacement.is_empty() && !additive.is_empty() {
        return Err(ResolveError::AmbiguousVectorComposition {
            slot: slot_id.to_string(),
            field: field.to_string(),
            shape: shape.to_string(),
        });
    }
    if !replacement.is_empty() {
        *target = replacement.to_vec();
        source(provenance, field, ResolvedSource::ShapeRule);
    }
    if !additive.is_empty() {
        target.extend(additive.iter().cloned());
        source(provenance, field, ResolvedSource::ShapeRule);
    }
    Ok(())
}

fn source(provenance: &mut SlotProvenance, field: &str, source: ResolvedSource) {
    provenance.field_sources.insert(field.to_string(), source);
}

fn dag_state_machine_id(slot: Option<&DagSlot>) -> Option<String> {
    slot.and_then(|slot| match &slot.state_machine {
        Some(SlotStateMachine::Structured(machine)) => Some(machine.id.clone()),
        Some(SlotStateMachine::Reference(reference)) => Some(reference.clone()),
        None => None,
    })
}

fn compose_transitions(dag: &Dag) -> Vec<ResolvedTransition> {
    let mut out = Vec::new();
    for slot in &dag.slots {
        let Some(SlotStateMachine::Structured(machine)) = &slot.state_machine else {
            continue;
        };
        for transition in &machine.transitions {
            let destination_green_when = machine
                .states
                .iter()
                .find(|state| state.id == transition.to)
                .and_then(|state| state.green_when.clone());
            out.push(ResolvedTransition {
                slot_id: slot.id.clone(),
                from: serde_yaml::to_string(&transition.from)
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                to: transition.to.clone(),
                via: transition.via.as_ref().map(|via| {
                    serde_yaml::to_string(via)
                        .unwrap_or_default()
                        .trim()
                        .to_string()
                }),
                destination_green_when,
            });
        }
    }
    out
}

fn gate_vec(
    provenance: &mut SlotProvenance,
    field: &str,
    dag: Option<&Vec<String>>,
    constellation: Option<&Vec<String>>,
) -> Vec<String> {
    if let Some(values) = dag.filter(|values| !values.is_empty()) {
        source(provenance, field, ResolvedSource::DagTaxonomy);
        return values.clone();
    }
    if let Some(values) = constellation.filter(|values| !values.is_empty()) {
        source(provenance, field, ResolvedSource::ConstellationMap);
        return values.clone();
    }
    Vec::new()
}

fn convert_closure(value: Option<&core_map::ClosureType>) -> Option<ClosureType> {
    Some(match value? {
        core_map::ClosureType::Open => ClosureType::Open,
        core_map::ClosureType::ClosedBounded => ClosureType::ClosedBounded,
        core_map::ClosureType::ClosedUnbounded => ClosureType::ClosedUnbounded,
    })
}

fn convert_eligibility(
    value: Option<&core_map::EligibilityConstraint>,
) -> Option<EligibilityConstraint> {
    Some(match value? {
        core_map::EligibilityConstraint::EntityKinds { entity_kinds } => {
            EligibilityConstraint::EntityKinds {
                entity_kinds: entity_kinds.clone(),
            }
        }
        core_map::EligibilityConstraint::ShapeTaxonomyPosition {
            shape_taxonomy_position,
        } => EligibilityConstraint::ShapeTaxonomyPosition {
            shape_taxonomy_position: shape_taxonomy_position.clone(),
        },
    })
}

fn convert_role_guard(
    value: Option<&core_map::RoleGuard>,
) -> Option<crate::config::dag::RoleGuard> {
    value.map(|guard| crate::config::dag::RoleGuard {
        any_of: guard.any_of.clone(),
        all_of: guard.all_of.clone(),
    })
}

fn convert_dag_completeness(
    value: &DagCompletenessAssertionConfig,
) -> core_map::CompletenessAssertionConfig {
    core_map::CompletenessAssertionConfig {
        predicate: value.predicate.clone(),
        description: value.description.clone(),
        extra: value.extra.clone(),
    }
}

fn is_cbu_business_plane(body: &core_map::ConstellationMapDefBody) -> bool {
    body.slots
        .get("cbu")
        .is_some_and(|slot| slot.cardinality == core_map::Cardinality::Root)
}

fn push_if_present(
    stack: &mut Vec<core_map::ConstellationMapDefBody>,
    maps: &BTreeMap<String, LoadedConstellationMap>,
    id: &str,
) {
    if let Some(map) = maps.get(id) {
        stack.push(map.body.clone());
    }
}
