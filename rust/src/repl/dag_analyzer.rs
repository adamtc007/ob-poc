//! DAG Analyzer for Staged Commands
//!
//! Analyzes dependencies between staged commands and computes execution order.
//! Dependencies arise from:
//! 1. **Output references**: `$1.result`, `$2.entity_ids`
//! 2. **Entity conflicts**: Same entity touched by multiple commands
//!
//! # Transparency
//!
//! If DAG analysis reorders commands, the diff is shown to the user.
//! This maintains the "no magic" principle - user sees exactly what happens.

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::staged_runbook::StagedCommand;

/// Error types for DAG analysis
#[derive(Debug, Clone)]
pub enum DagError {
    /// Cycle detected in dependencies
    CycleDetected {
        /// Command that creates the cycle
        command_id: Uuid,
        /// Path showing the cycle
        cycle_path: Vec<Uuid>,
    },
    /// Invalid output reference
    InvalidOutputRef {
        command_id: Uuid,
        ref_number: i32,
        message: String,
    },
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CycleDetected {
                command_id,
                cycle_path,
            } => {
                write!(
                    f,
                    "Cycle detected at command {}: {:?}",
                    command_id, cycle_path
                )
            }
            Self::InvalidOutputRef {
                command_id,
                ref_number,
                message,
            } => {
                write!(
                    f,
                    "Invalid output ref ${} in command {}: {}",
                    ref_number, command_id, message
                )
            }
        }
    }
}

impl std::error::Error for DagError {}

/// Edge type in the dependency graph
#[derive(Debug, Clone)]
pub enum DependencyEdge {
    /// Output reference ($N.result)
    OutputRef {
        /// The $N reference number
        ref_number: i32,
    },
    /// Entity conflict (same entity, both write)
    EntityConflict { entity_id: Uuid },
}

/// DAG Analyzer
pub struct DagAnalyzer {
    /// Adjacency list: command_id -> [(dependency_command_id, edge_type)]
    edges: HashMap<Uuid, Vec<(Uuid, DependencyEdge)>>,
    /// Command source orders for reference lookup
    source_orders: HashMap<i32, Uuid>,
    /// All command IDs
    commands: Vec<Uuid>,
}

impl DagAnalyzer {
    /// Create a new DAG analyzer from commands
    pub fn new(commands: &[StagedCommand]) -> Self {
        let mut source_orders = HashMap::new();
        let mut command_ids = Vec::new();

        for cmd in commands {
            source_orders.insert(cmd.source_order, cmd.id);
            command_ids.push(cmd.id);
        }

        Self {
            edges: HashMap::new(),
            source_orders,
            commands: command_ids,
        }
    }

    /// Analyze dependencies and build the graph
    pub fn analyze(&mut self, commands: &[StagedCommand]) -> Result<(), DagError> {
        // Clear existing edges
        self.edges.clear();

        for cmd in commands {
            // 1. Detect output references ($N)
            let output_refs = self.detect_output_refs(&cmd.dsl_raw);
            for ref_num in output_refs {
                if let Some(&source_cmd_id) = self.source_orders.get(&ref_num) {
                    if source_cmd_id == cmd.id {
                        return Err(DagError::InvalidOutputRef {
                            command_id: cmd.id,
                            ref_number: ref_num,
                            message: "Self-reference not allowed".to_string(),
                        });
                    }
                    self.add_edge(
                        cmd.id,
                        source_cmd_id,
                        DependencyEdge::OutputRef {
                            ref_number: ref_num,
                        },
                    );
                } else {
                    return Err(DagError::InvalidOutputRef {
                        command_id: cmd.id,
                        ref_number: ref_num,
                        message: format!("No command at line {}", ref_num),
                    });
                }
            }

            // 2. Detect entity conflicts (same entity, earlier command must run first)
            // Only for write operations
            if Self::is_write_verb(&cmd.verb) {
                let cmd_entities: HashSet<_> =
                    cmd.entity_footprint.iter().map(|e| e.entity_id).collect();

                for other in commands {
                    // Only look at earlier commands that also write
                    if other.source_order >= cmd.source_order {
                        continue;
                    }
                    if !Self::is_write_verb(&other.verb) {
                        continue;
                    }

                    let other_entities: HashSet<_> =
                        other.entity_footprint.iter().map(|e| e.entity_id).collect();

                    // Find overlapping entities
                    for entity_id in cmd_entities.intersection(&other_entities) {
                        self.add_edge(
                            cmd.id,
                            other.id,
                            DependencyEdge::EntityConflict {
                                entity_id: *entity_id,
                            },
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Compute execution order via topological sort
    pub fn compute_order(&self) -> Result<Vec<Uuid>, DagError> {
        let mut in_degree: HashMap<Uuid, usize> = HashMap::new();
        let mut reverse_edges: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

        // Initialize in-degree
        for &cmd in &self.commands {
            in_degree.insert(cmd, 0);
        }

        // Count in-degrees
        for (&cmd, deps) in &self.edges {
            for (dep, _) in deps {
                *in_degree.entry(cmd).or_insert(0) += 1;
                reverse_edges.entry(*dep).or_default().push(cmd);
            }
        }

        // Kahn's algorithm
        let mut queue: Vec<Uuid> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&id, _)| id)
            .collect();

        // Sort by source order for deterministic output
        queue.sort_by_key(|id| {
            self.source_orders
                .iter()
                .find(|(_, &v)| v == *id)
                .map(|(&k, _)| k)
                .unwrap_or(0)
        });

        let mut result = Vec::new();
        let mut visited = HashSet::new();

        while let Some(cmd) = queue.pop() {
            if visited.contains(&cmd) {
                continue;
            }
            visited.insert(cmd);
            result.push(cmd);

            if let Some(dependents) = reverse_edges.get(&cmd) {
                for &dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(&dependent) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push(dependent);
                        }
                    }
                }
            }
        }

        // Check for cycle
        if result.len() != self.commands.len() {
            // Find a command in the cycle
            let in_cycle: Vec<_> = self
                .commands
                .iter()
                .filter(|id| !visited.contains(id))
                .cloned()
                .collect();

            return Err(DagError::CycleDetected {
                command_id: in_cycle.first().copied().unwrap_or_default(),
                cycle_path: in_cycle,
            });
        }

        Ok(result)
    }

    /// Compute reorder diff between original and DAG order
    pub fn reorder_diff(&self, commands: &[StagedCommand]) -> Option<ReorderDiff> {
        let dag_order = match self.compute_order() {
            Ok(order) => order,
            Err(_) => return None,
        };

        let original_order: Vec<Uuid> = commands.iter().map(|c| c.id).collect();

        // Check if order changed
        if original_order == dag_order {
            return None;
        }

        // Compute moves
        let mut moves = Vec::new();
        for (new_pos, &cmd_id) in dag_order.iter().enumerate() {
            let old_pos = original_order.iter().position(|&id| id == cmd_id).unwrap();
            if old_pos != new_pos {
                // Find reason for move
                let reason = self
                    .edges
                    .get(&cmd_id)
                    .and_then(|deps| deps.first())
                    .map(|(_, edge)| match edge {
                        DependencyEdge::OutputRef { ref_number } => {
                            format!("depends on ${}", ref_number)
                        }
                        DependencyEdge::EntityConflict { entity_id } => {
                            format!("entity conflict on {}", entity_id)
                        }
                    })
                    .unwrap_or_else(|| "dependency".to_string());

                moves.push(ReorderMove {
                    command_id: cmd_id,
                    from_position: old_pos,
                    to_position: new_pos,
                    reason,
                });
            }
        }

        Some(ReorderDiff {
            original_order,
            reordered: dag_order,
            moves,
        })
    }

    /// Add an edge to the graph
    fn add_edge(&mut self, from: Uuid, to: Uuid, edge: DependencyEdge) {
        self.edges.entry(from).or_default().push((to, edge));
    }

    /// Detect output references ($N) in DSL
    fn detect_output_refs(&self, dsl: &str) -> Vec<i32> {
        let mut refs = Vec::new();
        let chars: Vec<char> = dsl.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() {
                // Parse number after $
                let mut num_str = String::new();
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    num_str.push(chars[j]);
                    j += 1;
                }
                if !num_str.is_empty() {
                    if let Ok(num) = num_str.parse::<i32>() {
                        refs.push(num);
                    }
                }
                i = j;
            } else {
                i += 1;
            }
        }

        refs
    }

    /// Check if a verb is a write operation
    fn is_write_verb(verb: &str) -> bool {
        let write_verbs = [
            "create", "update", "delete", "remove", "add", "assign", "unassign", "set", "clear",
            "import", "sync", "merge", "split", "transfer",
        ];

        let verb_lower = verb.to_lowercase();
        write_verbs.iter().any(|w| verb_lower.contains(w))
    }
}

/// DAG reorder diff
#[derive(Debug, Clone)]
pub struct ReorderDiff {
    pub original_order: Vec<Uuid>,
    pub reordered: Vec<Uuid>,
    pub moves: Vec<ReorderMove>,
}

/// Single reorder move
#[derive(Debug, Clone)]
pub struct ReorderMove {
    pub command_id: Uuid,
    pub from_position: usize,
    pub to_position: usize,
    pub reason: String,
}

impl serde::Serialize for ReorderDiff {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ReorderDiff", 3)?;
        state.serialize_field("original_order", &self.original_order)?;
        state.serialize_field("reordered", &self.reordered)?;
        state.serialize_field("moves", &self.moves)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for ReorderDiff {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Helper {
            original_order: Vec<Uuid>,
            reordered: Vec<Uuid>,
            moves: Vec<ReorderMove>,
        }
        let helper = Helper::deserialize(deserializer)?;
        Ok(Self {
            original_order: helper.original_order,
            reordered: helper.reordered,
            moves: helper.moves,
        })
    }
}

impl serde::Serialize for ReorderMove {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ReorderMove", 4)?;
        state.serialize_field("command_id", &self.command_id)?;
        state.serialize_field("from_position", &self.from_position)?;
        state.serialize_field("to_position", &self.to_position)?;
        state.serialize_field("reason", &self.reason)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for ReorderMove {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Helper {
            command_id: Uuid,
            from_position: usize,
            to_position: usize,
            reason: String,
        }
        let helper = Helper::deserialize(deserializer)?;
        Ok(Self {
            command_id: helper.command_id,
            from_position: helper.from_position,
            to_position: helper.to_position,
            reason: helper.reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::staged_runbook::StagedCommand;

    #[test]
    fn test_detect_output_refs() {
        let analyzer = DagAnalyzer::new(&[]);

        assert_eq!(analyzer.detect_output_refs("$1.result"), vec![1]);
        assert_eq!(analyzer.detect_output_refs("$1 $2 $3"), vec![1, 2, 3]);
        assert_eq!(
            analyzer.detect_output_refs("entity-ids=$1.entity_ids"),
            vec![1]
        );
        assert_eq!(
            analyzer.detect_output_refs("no refs here"),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn test_is_write_verb() {
        assert!(DagAnalyzer::is_write_verb("entity.create"));
        assert!(DagAnalyzer::is_write_verb("cbu.update"));
        assert!(DagAnalyzer::is_write_verb("kyc.delete"));
        assert!(!DagAnalyzer::is_write_verb("entity.list"));
        assert!(!DagAnalyzer::is_write_verb("entity.get"));
    }

    #[test]
    fn test_simple_dag() {
        let cmd1 = StagedCommand::new(
            1,
            "(entity.list)".to_string(),
            "entity.list".to_string(),
            None,
            None,
        );
        let cmd2 = StagedCommand::new(
            2,
            "(entity.get entity-id=$1.result)".to_string(),
            "entity.get".to_string(),
            None,
            None,
        );

        let commands = vec![cmd1.clone(), cmd2.clone()];
        let mut analyzer = DagAnalyzer::new(&commands);
        analyzer.analyze(&commands).unwrap();

        let order = analyzer.compute_order().unwrap();

        // cmd1 must come before cmd2
        let pos1 = order.iter().position(|&id| id == cmd1.id).unwrap();
        let pos2 = order.iter().position(|&id| id == cmd2.id).unwrap();
        assert!(pos1 < pos2);
    }

    #[test]
    fn test_reorder_diff() {
        // Stage in wrong order: $1 reference before the list
        let cmd1 = StagedCommand::new(
            1,
            "(entity.get entity-id=$2.result)".to_string(),
            "entity.get".to_string(),
            None,
            None,
        );
        let cmd2 = StagedCommand::new(
            2,
            "(entity.list)".to_string(),
            "entity.list".to_string(),
            None,
            None,
        );

        let commands = vec![cmd1.clone(), cmd2.clone()];
        let mut analyzer = DagAnalyzer::new(&commands);
        analyzer.analyze(&commands).unwrap();

        let diff = analyzer.reorder_diff(&commands);
        assert!(diff.is_some());

        let diff = diff.unwrap();
        assert_eq!(diff.original_order, vec![cmd1.id, cmd2.id]);
        assert_eq!(diff.reordered, vec![cmd2.id, cmd1.id]);
    }

    #[test]
    fn test_cycle_detection() {
        // Create a cycle: cmd1 refs $2, cmd2 refs $1
        let mut cmd1 = StagedCommand::new(
            1,
            "(entity.get entity-id=$2.result)".to_string(),
            "entity.get".to_string(),
            None,
            None,
        );
        let mut cmd2 = StagedCommand::new(
            2,
            "(entity.get entity-id=$1.result)".to_string(),
            "entity.get".to_string(),
            None,
            None,
        );

        // Manually set IDs for testing
        cmd1.id = Uuid::now_v7();
        cmd2.id = Uuid::now_v7();

        let commands = vec![cmd1.clone(), cmd2.clone()];
        let mut analyzer = DagAnalyzer::new(&commands);
        let result = analyzer.analyze(&commands);

        // Should detect cycle
        assert!(result.is_err() || analyzer.compute_order().is_err());
    }
}
