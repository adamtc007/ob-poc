//! Step 4: Infer entity types and relationships from schema + verb cross-references.
//!
//! Groups `AttributeCandidate`s by source table into `EntityTypeCandidate`s.
//! Classifies FK relationships by edge class (Structural, Reference, Association, Temporal).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

use super::schema_extract::{ForeignKeyExtract, TableExtract};
use super::xref::{AttributeCandidate, ColumnClassification};

/// A candidate entity type inferred from a database table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeCandidate {
    /// Schema name
    pub schema: String,
    /// Table name (used as entity type name basis)
    pub table: String,
    /// Inferred fully qualified name (e.g. "entity.legal_entity", "cbu.cbus")
    pub fqn: String,
    /// Domain derived from schema/table
    pub domain: String,
    /// Human-readable name (derived from table name)
    pub display_name: String,
    /// Attribute FQNs belonging to this entity
    pub attribute_fqns: Vec<String>,
    /// Primary key columns
    pub primary_keys: Vec<String>,
    /// Lifecycle states (from CHECK constraints, if detected)
    pub lifecycle_states: Vec<String>,
    /// Number of verb-connected attributes
    pub verb_connected_count: usize,
    /// Number of operational orphan attributes
    pub orphan_count: usize,
}

/// A candidate relationship inferred from a foreign key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipCandidate {
    /// Inferred FQN (e.g. "relationship.cbus_to_entities")
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Source table schema
    pub source_schema: String,
    /// Source table
    pub source_table: String,
    /// Source column (FK column)
    pub source_column: String,
    /// Target table schema
    pub target_schema: String,
    /// Target table
    pub target_table: String,
    /// Target column
    pub target_column: String,
    /// FK constraint name
    pub constraint_name: String,
    /// Edge classification
    pub edge_class: EdgeClass,
    /// Inferred cardinality
    pub cardinality: InferredCardinality,
}

/// Semantic classification of a FK relationship edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeClass {
    /// FK to parent/owner (same domain) — e.g. cbus.apex_entity_id → entities.entity_id
    Structural,
    /// FK to document/evidence table
    Reference,
    /// FK to lookup/code table
    Association,
    /// Time-bounded join (table has effective_from/effective_until)
    Temporal,
}

/// Inferred cardinality from FK structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InferredCardinality {
    OneToOne,
    OneToMany,
    ManyToMany,
}

/// Infer entity types by grouping attribute candidates by source table.
pub fn infer_entity_types(
    candidates: &[AttributeCandidate],
    tables: &[TableExtract],
) -> Vec<EntityTypeCandidate> {
    // Group candidates by (schema, table)
    let mut grouped: BTreeMap<(String, String), Vec<&AttributeCandidate>> = BTreeMap::new();
    for c in candidates {
        grouped
            .entry((c.schema.clone(), c.table.clone()))
            .or_default()
            .push(c);
    }

    // Build PK lookup from tables
    let pk_map: BTreeMap<(String, String), Vec<String>> = tables
        .iter()
        .map(|t| {
            (
                (t.schema.clone(), t.table_name.clone()),
                t.primary_keys.clone(),
            )
        })
        .collect();

    // Build lifecycle states from table column patterns
    let lifecycle_map = detect_lifecycle_states(tables);

    let mut entity_types = Vec::new();

    for ((schema, table), attrs) in &grouped {
        let domain = infer_domain(schema, table);
        let display_name = table_to_display_name(table);
        let fqn = format!("{}.{}", domain, table);

        let mut attribute_fqns = Vec::new();
        let mut verb_connected_count = 0;
        let mut orphan_count = 0;

        for attr in attrs {
            let attr_fqn = format!("{}.{}.{}", domain, table, attr.column);
            attribute_fqns.push(attr_fqn);

            match attr.classification {
                ColumnClassification::VerbConnected => verb_connected_count += 1,
                ColumnClassification::OperationalOrphan => orphan_count += 1,
                _ => {}
            }
        }

        let primary_keys = pk_map
            .get(&(schema.clone(), table.clone()))
            .cloned()
            .unwrap_or_default();

        let lifecycle_states = lifecycle_map
            .get(&(schema.clone(), table.clone()))
            .cloned()
            .unwrap_or_default();

        entity_types.push(EntityTypeCandidate {
            schema: schema.clone(),
            table: table.clone(),
            fqn,
            domain,
            display_name,
            attribute_fqns,
            primary_keys,
            lifecycle_states,
            verb_connected_count,
            orphan_count,
        });
    }

    entity_types
}

/// Infer relationships from foreign keys on extracted tables.
pub fn infer_relationships(tables: &[TableExtract]) -> Vec<RelationshipCandidate> {
    // Build a set of all table names for lookup table detection
    let all_tables: HashSet<String> = tables.iter().map(|t| t.table_name.clone()).collect();

    // Build column name sets per table for temporal detection
    let table_columns: BTreeMap<(String, String), HashSet<String>> = tables
        .iter()
        .map(|t| {
            let cols: HashSet<String> = t.columns.iter().map(|c| c.name.clone()).collect();
            ((t.schema.clone(), t.table_name.clone()), cols)
        })
        .collect();

    let mut relationships = Vec::new();

    for table in tables {
        for fk in &table.foreign_keys {
            let edge_class = classify_edge(
                &table.schema,
                &table.table_name,
                &fk.target_schema,
                &fk.target_table,
                &table_columns,
                &all_tables,
            );

            let cardinality = infer_cardinality(fk, table);

            let fqn = format!("relationship.{}_to_{}", table.table_name, fk.target_table);
            let name = format!(
                "{} → {}",
                table_to_display_name(&table.table_name),
                table_to_display_name(&fk.target_table)
            );

            relationships.push(RelationshipCandidate {
                fqn,
                name,
                source_schema: table.schema.clone(),
                source_table: table.table_name.clone(),
                source_column: fk.from_column.clone(),
                target_schema: fk.target_schema.clone(),
                target_table: fk.target_table.clone(),
                target_column: fk.target_column.clone(),
                constraint_name: fk.constraint_name.clone(),
                edge_class,
                cardinality,
            });
        }
    }

    relationships.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    relationships
}

/// Classify a FK edge by pattern matching.
fn classify_edge(
    source_schema: &str,
    source_table: &str,
    target_schema: &str,
    target_table: &str,
    table_columns: &BTreeMap<(String, String), HashSet<String>>,
    _all_tables: &HashSet<String>,
) -> EdgeClass {
    // Temporal: source table has effective_from/effective_until columns
    if let Some(cols) = table_columns.get(&(source_schema.to_string(), source_table.to_string())) {
        let has_temporal = cols.iter().any(|c| {
            let lower = c.to_lowercase();
            lower == "effective_from"
                || lower == "effective_until"
                || lower == "valid_from"
                || lower == "valid_until"
        });
        if has_temporal {
            return EdgeClass::Temporal;
        }
    }

    // Reference: target is a document/evidence table
    let target_lower = target_table.to_lowercase();
    if target_lower.contains("document")
        || target_lower.contains("evidence")
        || target_lower.contains("attachment")
        || target_lower.contains("observation")
    {
        return EdgeClass::Reference;
    }

    // Structural: same schema (parent/child relationship)
    if source_schema == target_schema {
        // Check for common structural patterns
        let source_lower = source_table.to_lowercase();
        if target_lower.contains("entities")
            || target_lower.contains("cbus")
            || source_lower.starts_with(&target_lower)
            || source_lower.ends_with("_items")
            || source_lower.ends_with("_lines")
            || source_lower.ends_with("_entries")
        {
            return EdgeClass::Structural;
        }
    }

    // Default: Association
    EdgeClass::Association
}

/// Infer cardinality from FK and unique constraint structure.
fn infer_cardinality(fk: &ForeignKeyExtract, table: &TableExtract) -> InferredCardinality {
    // If the FK column IS the primary key or has a unique constraint → OneToOne
    if table.primary_keys.len() == 1 && table.primary_keys[0] == fk.from_column {
        return InferredCardinality::OneToOne;
    }

    // If the FK column is part of a unique constraint → OneToOne
    for uc in &table.unique_constraints {
        if uc.len() == 1 && uc[0] == fk.from_column {
            return InferredCardinality::OneToOne;
        }
    }

    // If the FK column is part of a composite PK → ManyToMany (junction table pattern)
    if table.primary_keys.len() > 1 && table.primary_keys.contains(&fk.from_column) {
        return InferredCardinality::ManyToMany;
    }

    // Default: OneToMany
    InferredCardinality::OneToMany
}

/// Detect lifecycle states from table columns that look like status/state enums.
fn detect_lifecycle_states(tables: &[TableExtract]) -> BTreeMap<(String, String), Vec<String>> {
    let mut result = BTreeMap::new();

    for table in tables {
        let status_cols: Vec<&str> = table
            .columns
            .iter()
            .filter(|c| {
                let lower = c.name.to_lowercase();
                lower == "status" || lower == "state" || lower.ends_with("_status")
            })
            .map(|c| c.name.as_str())
            .collect();

        if !status_cols.is_empty() {
            // We note that lifecycle states exist but can't extract CHECK constraint values
            // from information_schema alone. We record the column names as a hint.
            let hints: Vec<String> = status_cols.iter().map(|c| format!("has_{}", c)).collect();
            result.insert((table.schema.clone(), table.table_name.clone()), hints);
        }
    }

    result
}

/// Infer domain from schema name.
fn infer_domain(schema: &str, _table: &str) -> String {
    match schema {
        "ob-poc" => "ob_poc".to_string(),
        "kyc" => "kyc".to_string(),
        "sem_reg" => "sem_reg".to_string(),
        other => other.replace('-', "_"),
    }
}

/// Convert table name to human-readable display name.
fn table_to_display_name(table: &str) -> String {
    table
        .replace('_', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_to_display_name() {
        assert_eq!(table_to_display_name("cbus"), "Cbus");
        assert_eq!(table_to_display_name("legal_entities"), "Legal Entities");
        assert_eq!(
            table_to_display_name("trading_profile_instruments"),
            "Trading Profile Instruments"
        );
    }

    #[test]
    fn test_infer_domain() {
        assert_eq!(infer_domain("ob-poc", "cbus"), "ob_poc");
        assert_eq!(infer_domain("kyc", "cases"), "kyc");
        assert_eq!(infer_domain("sem_reg", "snapshots"), "sem_reg");
    }

    #[test]
    fn test_edge_class_temporal() {
        let mut cols = HashSet::new();
        cols.insert("effective_from".to_string());
        cols.insert("effective_until".to_string());
        cols.insert("entity_id".to_string());

        let mut table_columns = BTreeMap::new();
        table_columns.insert(("ob-poc".to_string(), "entity_roles".to_string()), cols);

        let result = classify_edge(
            "ob-poc",
            "entity_roles",
            "ob-poc",
            "entities",
            &table_columns,
            &HashSet::new(),
        );
        assert_eq!(result, EdgeClass::Temporal);
    }

    #[test]
    fn test_edge_class_reference() {
        let result = classify_edge(
            "ob-poc",
            "kyc_cases",
            "ob-poc",
            "document_versions",
            &BTreeMap::new(),
            &HashSet::new(),
        );
        assert_eq!(result, EdgeClass::Reference);
    }

    #[test]
    fn test_edge_class_default_association() {
        let result = classify_edge(
            "ob-poc",
            "trading_profiles",
            "kyc",
            "master_jurisdictions",
            &BTreeMap::new(),
            &HashSet::new(),
        );
        assert_eq!(result, EdgeClass::Association);
    }
}
