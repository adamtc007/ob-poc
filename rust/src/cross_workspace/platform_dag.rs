//! Level 0 Platform DAG — derived on-the-fly from shared atom registry
//! + Level 1 verb footprints (reads_from).
//!
//! NOT persisted. Computed at boot and after registry changes.
//! At O(50) shared atoms × O(1,464) verbs, this is trivially fast.
//!
//! See: docs/architecture/cross-workspace-state-consistency-v0.4.md §3.2

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::cross_workspace::types::SharedAtomDef;

// ── Types ────────────────────────────────────────────────────────────

/// A directed edge in the Level 0 Platform DAG.
///
/// Represents: "workspace `consumer_workspace` consumes shared atom
/// `atom_path` owned by `owner_workspace`".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEdge {
    pub atom_path: String,
    pub atom_id: uuid::Uuid,
    pub owner_workspace: String,
    pub consumer_workspace: String,
    /// Verb FQNs in the consumer workspace that read from the atom's backing table.
    pub consumer_verbs: Vec<String>,
}

/// The complete Level 0 Platform DAG — a computed view of cross-workspace
/// data dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformDag {
    pub edges: Vec<PlatformEdge>,
    /// Number of active shared atoms in the registry.
    pub atom_count: usize,
    /// Number of unique consuming workspaces.
    pub consumer_workspace_count: usize,
}

impl PlatformDag {
    /// Get all consumers of a specific shared atom.
    pub fn consumers_of(&self, atom_path: &str) -> Vec<&PlatformEdge> {
        self.edges
            .iter()
            .filter(|e| e.atom_path == atom_path)
            .collect()
    }

    /// Get all atoms consumed by a specific workspace.
    pub fn atoms_consumed_by(&self, workspace: &str) -> Vec<&PlatformEdge> {
        self.edges
            .iter()
            .filter(|e| e.consumer_workspace == workspace)
            .collect()
    }

    /// Check if a workspace consumes any shared atoms.
    pub fn is_consumer(&self, workspace: &str) -> bool {
        self.edges.iter().any(|e| e.consumer_workspace == workspace)
    }
}

// ── Derivation ───────────────────────────────────────────────────────

/// Verb footprint entry — a verb's declared reads_from tables.
#[derive(Debug, Clone)]
pub struct VerbFootprint {
    pub verb_fqn: String,
    pub domain: String,
    pub reads_from: Vec<String>,
}

/// Derive the Level 0 Platform DAG from shared atoms and verb footprints.
///
/// For each active shared atom:
///   1. Resolve the atom's backing table (from atom_path → table mapping)
///   2. Find all verbs in non-owning workspaces that `reads_from` that table
///   3. Create a platform edge for each consumer workspace
///
/// This is O(atoms × verbs) — trivially fast at ~50 × ~1,464.
pub fn derive_platform_dag(
    atoms: &[SharedAtomDef],
    verb_footprints: &[VerbFootprint],
    domain_to_workspace: &HashMap<String, String>,
) -> PlatformDag {
    let mut edges = Vec::new();
    let mut consumer_workspaces = HashSet::new();

    // Map atom_path to its backing table.
    // Convention: "entity.lei" → "entities", "cbu.fund_structure_type" → "cbus"
    let atom_table_map = build_atom_table_map(atoms);

    for atom in atoms {
        let backing_table = match atom_table_map.get(&atom.atom_path) {
            Some(t) => t,
            None => continue,
        };

        // Group consumer verbs by workspace
        let mut workspace_verbs: HashMap<String, Vec<String>> = HashMap::new();

        for vf in verb_footprints {
            if !vf.reads_from.iter().any(|t| t == backing_table) {
                continue;
            }

            // Resolve verb's workspace from its domain
            let verb_workspace = match domain_to_workspace.get(&vf.domain) {
                Some(w) => w.clone(),
                None => continue,
            };

            // Skip if same as owner workspace
            if verb_workspace == atom.owner_workspace {
                continue;
            }

            workspace_verbs
                .entry(verb_workspace)
                .or_default()
                .push(vf.verb_fqn.clone());
        }

        for (consumer_ws, verbs) in workspace_verbs {
            consumer_workspaces.insert(consumer_ws.clone());
            edges.push(PlatformEdge {
                atom_path: atom.atom_path.clone(),
                atom_id: atom.id,
                owner_workspace: atom.owner_workspace.clone(),
                consumer_workspace: consumer_ws,
                consumer_verbs: verbs,
            });
        }
    }

    PlatformDag {
        atom_count: atoms.len(),
        consumer_workspace_count: consumer_workspaces.len(),
        edges,
    }
}

/// Map atom_path to backing table name.
///
/// Convention: the first segment of the atom path maps to a table:
/// - "entity.*" → "entities"
/// - "cbu.*" → "cbus"
/// - "client-group.*" → "client_group"
fn build_atom_table_map(atoms: &[SharedAtomDef]) -> HashMap<String, String> {
    atoms
        .iter()
        .map(|a| {
            let table = match a.atom_path.split('.').next().unwrap_or("") {
                "entity" => "entities".to_string(),
                "cbu" => "cbus".to_string(),
                "client-group" => "client_group".to_string(),
                "product" => "products".to_string(),
                "service" => "services".to_string(),
                other => other.to_string(),
            };
            (a.atom_path.clone(), table)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_atom(path: &str, owner: &str) -> SharedAtomDef {
        SharedAtomDef {
            id: Uuid::new_v4(),
            atom_path: path.to_string(),
            display_name: path.to_string(),
            owner_workspace: owner.to_string(),
            owner_constellation_family: format!("{owner}_workspace"),
            lifecycle_status: crate::cross_workspace::types::SharedAtomLifecycle::Active,
            validation_rule: None,
            created_at: Utc::now(),
            activated_at: Some(Utc::now()),
            updated_at: Utc::now(),
        }
    }

    fn make_footprint(fqn: &str, domain: &str, reads: &[&str]) -> VerbFootprint {
        VerbFootprint {
            verb_fqn: fqn.to_string(),
            domain: domain.to_string(),
            reads_from: reads.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_derive_platform_dag_basic() {
        let atoms = vec![make_atom("entity.lei", "kyc")];

        let footprints = vec![
            make_footprint("cbu.create", "cbu", &["entities", "cbus"]),
            make_footprint("kyc.verify", "kyc", &["entities"]),
            make_footprint("deal.create", "deal", &["deals"]),
            make_footprint("onboarding.setup", "onboarding", &["entities", "cbus"]),
        ];

        let mut domain_ws = HashMap::new();
        domain_ws.insert("cbu".to_string(), "cbu".to_string());
        domain_ws.insert("kyc".to_string(), "kyc".to_string());
        domain_ws.insert("deal".to_string(), "deal".to_string());
        domain_ws.insert("onboarding".to_string(), "onboarding".to_string());

        let dag = derive_platform_dag(&atoms, &footprints, &domain_ws);

        assert_eq!(dag.atom_count, 1);
        // cbu and onboarding read "entities" but are not "kyc" (owner)
        assert_eq!(dag.edges.len(), 2);
        assert_eq!(dag.consumer_workspace_count, 2);

        let consumers: Vec<&str> = dag
            .edges
            .iter()
            .map(|e| e.consumer_workspace.as_str())
            .collect();
        assert!(consumers.contains(&"cbu"));
        assert!(consumers.contains(&"onboarding"));
        // kyc is owner, not consumer
        assert!(!consumers.contains(&"kyc"));
        // deal doesn't read entities
        assert!(!consumers.contains(&"deal"));
    }

    #[test]
    fn test_consumers_of_query() {
        let atoms = vec![
            make_atom("entity.lei", "kyc"),
            make_atom("cbu.fund_structure_type", "cbu"),
        ];

        let footprints = vec![make_footprint(
            "onboarding.setup",
            "onboarding",
            &["entities", "cbus"],
        )];

        let mut domain_ws = HashMap::new();
        domain_ws.insert("onboarding".to_string(), "onboarding".to_string());

        let dag = derive_platform_dag(&atoms, &footprints, &domain_ws);

        let lei_consumers = dag.consumers_of("entity.lei");
        assert_eq!(lei_consumers.len(), 1);
        assert_eq!(lei_consumers[0].consumer_workspace, "onboarding");

        let fund_consumers = dag.consumers_of("cbu.fund_structure_type");
        assert_eq!(fund_consumers.len(), 1);
    }

    #[test]
    fn test_empty_dag() {
        let dag = derive_platform_dag(&[], &[], &HashMap::new());
        assert_eq!(dag.atom_count, 0);
        assert_eq!(dag.edges.len(), 0);
        assert_eq!(dag.consumer_workspace_count, 0);
    }
}
