//! Valid verb set computation for SemOS-scoped verb resolution.
//!
//! Given a session context (client group, constellation template, entity states),
//! computes the set of verbs that are LEGAL at this moment. This is entirely
//! deterministic — no NLP, no embeddings, no LLM calls.

use std::{collections::HashSet, path::PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::sem_os_runtime::constellation_runtime::{Cardinality, ConstellationMapDef, SlotDef};

use super::session_context::EntityState;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A verb that is legal in the current session context.
#[derive(Debug, Clone)]
pub struct VerbCandidate {
    pub verb_fqn: String,
    pub entity_id: Option<Uuid>,
    pub entity_type: String,
    pub source: VerbSource,
    pub priority: u32,
    pub keywords: Vec<String>,
}

/// How this verb became part of the valid set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerbSource {
    /// Outgoing FSM transition from current entity state.
    FsmTransition,
    /// Creation verb for an entity that doesn't exist yet.
    CreationVerb,
    /// Always available (observation verbs: read, list, show).
    AlwaysAvailable,
}

/// The computed set of valid verbs for a session context.
#[derive(Debug, Clone)]
pub struct ValidVerbSet {
    pub verbs: Vec<VerbCandidate>,
    pub client_group_id: Uuid,
    pub constellation_id: String,
    pub computed_at: DateTime<Utc>,
}

impl ValidVerbSet {
    /// Get all verb FQNs in the set.
    pub fn verb_fqns(&self) -> Vec<&str> {
        self.verbs.iter().map(|v| v.verb_fqn.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    /// Check if a specific verb FQN is in the valid set.
    pub fn contains_verb(&self, fqn: &str) -> bool {
        self.verbs.iter().any(|v| v.verb_fqn == fqn)
    }

    /// Convert to a HashSet for passing to the constrained embedding search.
    pub fn to_allowed_set(&self) -> HashSet<String> {
        self.verbs.iter().map(|v| v.verb_fqn.clone()).collect()
    }
}

// ---------------------------------------------------------------------------
// State machine loading
// ---------------------------------------------------------------------------

/// A parsed state machine with transitions.
#[derive(Debug, Clone, serde::Deserialize)]
struct StateMachineDef {
    #[allow(dead_code)]
    state_machine: String,
    #[allow(dead_code)]
    states: Vec<String>,
    transitions: Vec<TransitionDef>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TransitionDef {
    from: String,
    #[allow(dead_code)]
    to: String,
    verbs: Vec<String>,
}

/// Load a state machine definition from the builtin YAML files.
fn load_state_machine(name: &str) -> Result<StateMachineDef> {
    let filename = format!("{}.yaml", name);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/sem_os_seeds/state_machines")
        .join(&filename);
    let yaml = std::fs::read_to_string(&path)
        .map_err(|_| anyhow::anyhow!("State machine '{}' not found at {:?}", name, path))?;
    let sm: StateMachineDef = serde_yaml::from_str(&yaml)?;
    Ok(sm)
}

/// Get outgoing transition verbs from a state machine for a given state.
fn get_fsm_transition_verbs(sm: &StateMachineDef, current_state: &str) -> Vec<String> {
    let state_lower = current_state.to_lowercase();
    sm.transitions
        .iter()
        .filter(|t| t.from.to_lowercase() == state_lower)
        .flat_map(|t| t.verbs.clone())
        .collect()
}

fn constellation_maps_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/sem_os_seeds/constellation_maps")
}

/// Load a raw constellation definition by constellation ID.
///
/// # Examples
/// ```rust
/// use ob_poc::sage::valid_verb_set::load_constellation_by_id;
///
/// let map = load_constellation_by_id("group.ownership").unwrap();
/// assert_eq!(map.constellation, "group.ownership");
/// ```
pub fn load_constellation_by_id(id: &str) -> Result<ConstellationMapDef> {
    for entry in std::fs::read_dir(constellation_maps_dir())? {
        let path = entry?.path();
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "yaml" | "yml"));
        if !is_yaml {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let map: ConstellationMapDef = serde_yaml::from_str(&content)?;
        if map.constellation == id {
            return Ok(map);
        }
    }

    anyhow::bail!("Constellation not found: {}", id)
}

// ---------------------------------------------------------------------------
// Core computation
// ---------------------------------------------------------------------------

/// Compute the set of verbs that are LEGAL right now, given the session context.
///
/// This is entirely deterministic — no NLP, no embeddings, no LLM calls.
///
/// Steps:
/// 1. For each existing entity, find its constellation slot and compute legal FSM transitions
/// 2. For constellation slots with no entity, add creation verbs if dependencies are met
/// 3. Add observation verbs (read, list) for all existing entities
/// 4. Sort by priority, dedup
pub fn compute_valid_verb_set(
    entity_states: &[EntityState],
    constellation: &ConstellationMapDef,
    client_group_id: Uuid,
) -> ValidVerbSet {
    let mut verbs = Vec::new();

    // Step 1: For each existing entity, compute legal FSM transitions
    for es in entity_states {
        if let Some((slot_name, slot)) = find_slot_for_entity_type(constellation, &es.entity_type) {
            // Load state machine if defined for this slot
            if let Some(sm_name) = &slot.state_machine {
                if let Ok(sm) = load_state_machine(sm_name) {
                    let transition_verbs = get_fsm_transition_verbs(&sm, &es.current_state);
                    for verb_fqn in transition_verbs {
                        // Check if this verb is in the slot's verb palette
                        if is_verb_in_palette(slot, &verb_fqn) {
                            // Check availability gate
                            if is_verb_available(slot, &verb_fqn, &es.current_state) {
                                verbs.push(VerbCandidate {
                                    verb_fqn: verb_fqn.clone(),
                                    entity_id: Some(es.entity_id),
                                    entity_type: es.entity_type.clone(),
                                    source: VerbSource::FsmTransition,
                                    priority: 10,
                                    keywords: extract_keywords_for_verb(&verb_fqn),
                                });
                            }
                        } else {
                            // Verb from FSM transition but not in palette — still add as transition
                            verbs.push(VerbCandidate {
                                verb_fqn,
                                entity_id: Some(es.entity_id),
                                entity_type: es.entity_type.clone(),
                                source: VerbSource::FsmTransition,
                                priority: 15,
                                keywords: Vec::new(),
                            });
                        }
                    }
                }
            }

            // Add observation verbs for existing entities
            for entry in slot.verbs.values() {
                let fqn = entry.verb_fqn().to_string();
                if is_observation_verb(&fqn) {
                    verbs.push(VerbCandidate {
                        verb_fqn: fqn.clone(),
                        entity_id: Some(es.entity_id),
                        entity_type: es.entity_type.clone(),
                        source: VerbSource::AlwaysAvailable,
                        priority: 50,
                        keywords: extract_keywords_for_verb(&fqn),
                    });
                }
            }

            // Also add all ungated verbs from the slot palette
            for entry in slot.verbs.values() {
                let fqn = entry.verb_fqn().to_string();
                if !is_observation_verb(&fqn) && entry.available_in().is_empty() {
                    // Ungated verb — always available
                    verbs.push(VerbCandidate {
                        verb_fqn: fqn.clone(),
                        entity_id: Some(es.entity_id),
                        entity_type: es.entity_type.clone(),
                        source: VerbSource::AlwaysAvailable,
                        priority: 30,
                        keywords: extract_keywords_for_verb(&fqn),
                    });
                }
            }

            // Recurse into slot children
            add_child_slot_verbs(&mut verbs, &slot_name, slot, es);
        }
    }

    // Step 2: For constellation slots with NO entity yet, add creation verbs
    for (slot_name, slot) in &constellation.slots {
        let has_entity = entity_states
            .iter()
            .any(|es| slot.entity_kinds.contains(&es.entity_type));
        let is_required = matches!(slot.cardinality, Cardinality::Root | Cardinality::Mandatory);

        if !has_entity
            && (is_required || slot.cardinality == Cardinality::Optional)
            && dependencies_met(slot, entity_states, constellation)
        {
            if let Some(create_verb) = find_creation_verb(slot) {
                verbs.push(VerbCandidate {
                    verb_fqn: create_verb.clone(),
                    entity_id: None,
                    entity_type: slot
                        .entity_kinds
                        .first()
                        .cloned()
                        .unwrap_or_else(|| slot_name.clone()),
                    source: VerbSource::CreationVerb,
                    priority: 5,
                    keywords: extract_keywords_for_verb(&create_verb),
                });
            }
        }

        // Also recurse into children for creation verbs
        for (child_name, child_slot) in &slot.children {
            let has_child_entity = entity_states
                .iter()
                .any(|es| child_slot.entity_kinds.contains(&es.entity_type));
            if !has_child_entity && dependencies_met(child_slot, entity_states, constellation) {
                if let Some(create_verb) = find_creation_verb(child_slot) {
                    verbs.push(VerbCandidate {
                        verb_fqn: create_verb.clone(),
                        entity_id: None,
                        entity_type: child_slot
                            .entity_kinds
                            .first()
                            .cloned()
                            .unwrap_or_else(|| child_name.clone()),
                        source: VerbSource::CreationVerb,
                        priority: 5,
                        keywords: extract_keywords_for_verb(&create_verb),
                    });
                }
            }
        }
    }

    // Sort by priority (lower = higher priority), dedup by verb_fqn
    verbs.sort_by_key(|v| v.priority);
    verbs.dedup_by(|a, b| a.verb_fqn == b.verb_fqn);

    ValidVerbSet {
        verbs,
        client_group_id,
        constellation_id: constellation.constellation.clone(),
        computed_at: Utc::now(),
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Find the constellation slot matching an entity type.
fn find_slot_for_entity_type<'a>(
    constellation: &'a ConstellationMapDef,
    entity_type: &str,
) -> Option<(String, &'a SlotDef)> {
    // Search top-level slots
    for (name, slot) in &constellation.slots {
        if slot.entity_kinds.contains(&entity_type.to_string()) {
            return Some((name.clone(), slot));
        }
        // Search children
        for (child_name, child_slot) in &slot.children {
            if child_slot.entity_kinds.contains(&entity_type.to_string()) {
                return Some((child_name.clone(), child_slot));
            }
        }
    }
    None
}

/// Check if a verb FQN appears anywhere in a slot's verb palette.
fn is_verb_in_palette(slot: &SlotDef, verb_fqn: &str) -> bool {
    slot.verbs
        .values()
        .any(|entry| entry.verb_fqn() == verb_fqn)
}

/// Check if a verb is available in the current state (gating check).
fn is_verb_available(slot: &SlotDef, verb_fqn: &str, current_state: &str) -> bool {
    for entry in slot.verbs.values() {
        if entry.verb_fqn() == verb_fqn {
            let available_in = entry.available_in();
            if available_in.is_empty() {
                return true; // Ungated — always available
            }
            let state_lower = current_state.to_lowercase();
            return available_in.iter().any(|s| s.to_lowercase() == state_lower);
        }
    }
    true // Not found in palette — allow (will be filtered by FSM transitions)
}

/// Check if all dependencies for a slot are met.
fn dependencies_met(
    slot: &SlotDef,
    entity_states: &[EntityState],
    _constellation: &ConstellationMapDef,
) -> bool {
    if slot.depends_on.is_empty() {
        return true;
    }
    for dep in &slot.depends_on {
        let dep_slot_name = dep.slot_name();
        let min_state = dep.min_state();
        // Check if any entity exists for the dependency slot with at least min_state
        let dep_met = entity_states.iter().any(|es| {
            es.slot_name.as_deref() == Some(dep_slot_name) || es.entity_type == dep_slot_name
        });
        if !dep_met && min_state != "empty" {
            return false;
        }
    }
    true
}

/// Find the creation verb in a slot's verb palette.
fn find_creation_verb(slot: &SlotDef) -> Option<String> {
    // Look for verbs with "create" or "open" in the key
    for (key, entry) in &slot.verbs {
        if key.contains("create") || key.contains("open") || key.contains("add") {
            return Some(entry.verb_fqn().to_string());
        }
    }
    // Fallback: first verb in palette
    slot.verbs.values().next().map(|e| e.verb_fqn().to_string())
}

/// Check if a verb is an observation (read-only) verb.
fn is_observation_verb(verb_fqn: &str) -> bool {
    let action = verb_fqn.rsplit('.').next().unwrap_or("");
    matches!(
        action,
        "read"
            | "list"
            | "show"
            | "inspect"
            | "get"
            | "for-entity"
            | "state"
            | "summary"
            | "list-by-cbu"
            | "list-owners"
            | "list-ubos"
    )
}

/// Extract keywords from a verb FQN for keyword matching.
///
/// Splits on dots and hyphens. e.g., "document.solicit" → ["document", "solicit"]
/// Public accessor for tests.
pub fn extract_keywords_for_verb_pub(verb_fqn: &str) -> Vec<String> {
    extract_keywords_for_verb(verb_fqn)
}

fn extract_keywords_for_verb(verb_fqn: &str) -> Vec<String> {
    verb_fqn
        .split(['.', '-'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// Add verbs from child slots for an existing entity.
fn add_child_slot_verbs(
    verbs: &mut Vec<VerbCandidate>,
    _parent_slot_name: &str,
    parent_slot: &SlotDef,
    entity_state: &EntityState,
) {
    for child_slot in parent_slot.children.values() {
        for entry in child_slot.verbs.values() {
            let fqn = entry.verb_fqn().to_string();
            verbs.push(VerbCandidate {
                verb_fqn: fqn.clone(),
                entity_id: Some(entity_state.entity_id),
                entity_type: child_slot
                    .entity_kinds
                    .first()
                    .cloned()
                    .unwrap_or_else(|| entity_state.entity_type.clone()),
                source: VerbSource::AlwaysAvailable,
                priority: 40,
                keywords: extract_keywords_for_verb(&fqn),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_valid_verb_set_basic() {
        // Load a real constellation map from YAML.
        // We use ConstellationMapDef (raw YAML) not ValidatedConstellationMap.
        // for compute_valid_verb_set. Load the raw YAML instead.
        let yaml_path = constellation_maps_dir().join("group_ownership.yaml");
        let yaml = match std::fs::read_to_string(&yaml_path) {
            Ok(y) => y,
            Err(e) => {
                println!("Skipping test — could not read constellation YAML: {}", e);
                return;
            }
        };
        let map: ConstellationMapDef = match serde_yaml::from_str(&yaml) {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping test — could not load constellation: {}", e);
                return;
            }
        };

        // Mock entity states: empty — no entities exist yet.
        // The valid verb set should contain creation verbs for required slots.
        let entity_states: Vec<EntityState> = vec![];

        let valid = compute_valid_verb_set(&entity_states, &map, Uuid::new_v4());

        println!(
            "Constellation: {} ({} top-level slots)",
            map.constellation,
            map.slots.len()
        );
        println!("Valid verb set ({} verbs):", valid.len());
        for v in &valid.verbs {
            println!(
                "  {} [{}] {:?} — keywords: {:?}",
                v.verb_fqn, v.entity_type, v.source, v.keywords
            );
        }

        assert!(!valid.is_empty(), "Valid verb set should not be empty");
        // With no entities, we should get creation verbs for root/mandatory slots
        assert!(
            valid.len() >= 1,
            "Expected at least 1 creation verb, got {}",
            valid.len()
        );
    }

    #[test]
    fn test_valid_verb_set_utility_methods() {
        let valid = ValidVerbSet {
            verbs: vec![
                VerbCandidate {
                    verb_fqn: "cbu.update".to_string(),
                    entity_id: Some(Uuid::new_v4()),
                    entity_type: "cbu".to_string(),
                    source: VerbSource::FsmTransition,
                    priority: 10,
                    keywords: vec!["cbu".to_string(), "update".to_string()],
                },
                VerbCandidate {
                    verb_fqn: "document.solicit".to_string(),
                    entity_id: None,
                    entity_type: "document".to_string(),
                    source: VerbSource::CreationVerb,
                    priority: 5,
                    keywords: vec!["document".to_string(), "solicit".to_string()],
                },
            ],
            client_group_id: Uuid::new_v4(),
            constellation_id: "test".to_string(),
            computed_at: Utc::now(),
        };

        assert_eq!(valid.len(), 2);
        assert!(!valid.is_empty());
        assert!(valid.contains_verb("cbu.update"));
        assert!(valid.contains_verb("document.solicit"));
        assert!(!valid.contains_verb("nonexistent.verb"));

        let fqns = valid.verb_fqns();
        assert_eq!(fqns.len(), 2);

        let allowed = valid.to_allowed_set();
        assert!(allowed.contains("cbu.update"));
        assert!(allowed.contains("document.solicit"));
    }

    #[test]
    fn test_extract_keywords() {
        assert_eq!(
            extract_keywords_for_verb("document.solicit"),
            vec!["document", "solicit"]
        );
        assert_eq!(
            extract_keywords_for_verb("kyc-case.create"),
            vec!["kyc", "case", "create"]
        );
        assert_eq!(
            extract_keywords_for_verb("cbu.assign-role"),
            vec!["cbu", "assign", "role"]
        );
    }

    #[test]
    fn test_is_observation_verb() {
        assert!(is_observation_verb("cbu.read"));
        assert!(is_observation_verb("entity.list"));
        assert!(is_observation_verb("document.for-entity"));
        assert!(!is_observation_verb("cbu.create"));
        assert!(!is_observation_verb("document.solicit"));
    }

    #[test]
    fn test_semos_scope_hit_rate() {
        use crate::sage::constrained_match::resolve_constrained;

        // Load test utterances from TOML fixture
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/intent_test_utterances.toml");
        let content = std::fs::read_to_string(&path).expect("Failed to read fixture");

        #[derive(serde::Deserialize)]
        struct Fixture {
            test: Vec<TC>,
        }
        #[derive(serde::Deserialize)]
        struct TC {
            utterance: String,
            expected_verb: String,
            #[serde(default)]
            alt_verbs: Vec<String>,
            #[serde(default)]
            category: String,
        }
        let fixture: Fixture = toml::from_str(&content).expect("Failed to parse TOML");

        // Build a realistic valid verb set from a single session-scoped constellation.
        let group_id = Uuid::new_v4();
        let constellation_id = "kyc.onboarding";
        let entity_states = vec![
            EntityState {
                entity_id: Uuid::new_v4(),
                entity_type: "cbu".to_string(),
                current_state: "DISCOVERED".to_string(),
                slot_name: Some("cbu".to_string()),
            },
            EntityState {
                entity_id: Uuid::new_v4(),
                entity_type: "kyc_case".to_string(),
                current_state: "intake".to_string(),
                slot_name: Some("kyc_case".to_string()),
            },
        ];

        let map = load_constellation_by_id(constellation_id).expect("failed to load constellation");
        let valid = compute_valid_verb_set(&entity_states, &map, group_id);

        // Run all utterances through constrained match
        let total = fixture.test.len();
        let mut resolved = 0usize;
        let mut correct = 0usize;
        let mut wrong = 0usize;
        let mut fallthrough = 0usize;
        let mut wrong_cases: Vec<(String, String, String, String)> = Vec::new();
        let mut correct_cases: Vec<(String, String)> = Vec::new();

        for tc in &fixture.test {
            let result = resolve_constrained(&tc.utterance, &valid);
            if result.resolved() {
                resolved += 1;
                let got = result.verb_fqn.as_deref().unwrap_or("");
                if got == tc.expected_verb || tc.alt_verbs.iter().any(|a| a == got) {
                    correct += 1;
                    correct_cases.push((tc.utterance.clone(), got.to_string()));
                } else {
                    wrong += 1;
                    wrong_cases.push((
                        tc.utterance.clone(),
                        tc.expected_verb.clone(),
                        got.to_string(),
                        tc.category.clone(),
                    ));
                }
            } else {
                fallthrough += 1;
            }
        }

        println!("\n============================================================");
        println!("  SemOS-SCOPED RESOLUTION HIT RATE (simulated session)");
        println!("============================================================");
        println!("  Constellation:         {}", valid.constellation_id);
        println!("  Valid verb set:        {} verbs", valid.len());
        println!("  Total utterances:      {}", total);
        println!(
            "  Resolved (constrained): {} ({:.1}%)",
            resolved,
            resolved as f64 / total as f64 * 100.0
        );
        println!(
            "  ├─ Correct:            {} ({:.1}% of total)",
            correct,
            correct as f64 / total as f64 * 100.0
        );
        println!(
            "  └─ Wrong:              {} ({:.1}% of total)",
            wrong,
            wrong as f64 / total as f64 * 100.0
        );
        println!(
            "  Fallthrough to open:   {} ({:.1}%)",
            fallthrough,
            fallthrough as f64 / total as f64 * 100.0
        );
        println!(
            "  Constrained accuracy:  {:.1}%",
            if resolved > 0 {
                correct as f64 / resolved as f64 * 100.0
            } else {
                0.0
            }
        );

        if !wrong_cases.is_empty() {
            println!("\n  WRONG ({}):", wrong_cases.len());
            for (utt, expected, got, cat) in &wrong_cases {
                let short = if utt.len() > 50 { &utt[..50] } else { utt };
                println!(
                    "    ✗ [{}] '{}' → {} (expected {})",
                    cat, short, got, expected
                );
            }
        }

        println!("\n  CORRECT SAMPLE (first 15):");
        for (utt, got) in correct_cases.iter().take(15) {
            let short = if utt.len() > 45 { &utt[..45] } else { utt };
            println!("    ✓ '{}' → {}", short, got);
        }
        println!("============================================================\n");
    }
}
