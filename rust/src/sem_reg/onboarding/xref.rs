//! Step 3: Cross-reference verbs ↔ schema columns and classify.
//!
//! For each verb's inputs/outputs/side-effects, match to schema columns.
//! Classify every column into one of four categories:
//! - `VerbConnected` — referenced by ≥1 verb I/O or side-effect
//! - `Framework` — standard housekeeping columns (created_at, id, version)
//! - `OperationalOrphan` — in schema, no verb touches it, not framework
//! - `DeadSchema` — no verb, no application code references (flagged for cleanup)

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::schema_extract::{ForeignKeyExtract, TableExtract};
use super::verb_extract::{SideEffectOp, VerbExtract};

/// Classification of a schema column relative to verb coverage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnClassification {
    /// Referenced by ≥1 verb I/O or side-effect — first-class AttributeDef.
    VerbConnected,
    /// Standard framework column (created_at, id, version) — NOT seeded.
    Framework,
    /// In schema, no verb touches it, not framework — seeded with `verb_orphan=true`.
    OperationalOrphan,
    /// No verb, no application code references — flagged for cleanup, NOT seeded.
    DeadSchema,
}

/// A candidate attribute produced by cross-referencing verbs with schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeCandidate {
    /// Schema name
    pub schema: String,
    /// Table name
    pub table: String,
    /// Column name
    pub column: String,
    /// SQL data type
    pub sql_type: String,
    /// Whether nullable
    pub is_nullable: bool,
    /// Classification
    pub classification: ColumnClassification,
    /// Verb FQNs that reference this column
    pub verb_refs: Vec<String>,
    /// How the verb references this column
    pub verb_ref_kinds: Vec<VerbRefKind>,
    /// Whether this column is a primary key
    pub is_primary_key: bool,
    /// Foreign key relationship (if any)
    pub foreign_key: Option<ForeignKeyExtract>,
    /// Default value expression
    pub default_value: Option<String>,
}

/// How a verb references a column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VerbRefKind {
    /// Verb argument `maps_to` points to this column
    ArgMapping,
    /// Verb lookup config references this table
    LookupTable,
    /// Verb CRUD config targets this table
    CrudTable,
    /// VerbLifecycle `writes_tables` / `reads_tables`
    LifecycleTable,
    /// VerbProduces `produced_type` maps to this table by convention
    ProducesType,
}

/// Result of cross-referencing verbs with schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrefResult {
    /// All attribute candidates
    pub candidates: Vec<AttributeCandidate>,
    /// Summary counts
    pub verb_connected: usize,
    pub framework: usize,
    pub operational_orphans: usize,
    pub dead_schema: usize,
}

/// Cross-reference verb extracts against schema extracts.
///
/// Produces classified `AttributeCandidate` for every column in every table.
pub fn cross_reference(verbs: &[VerbExtract], tables: &[TableExtract]) -> XrefResult {
    // Build verb→table reference index
    let verb_table_refs = build_verb_table_refs(verbs);

    // Build verb→column reference index
    let verb_column_refs = build_verb_column_refs(verbs);

    // Build set of tables referenced by verb "produces" (produced_type → table name)
    let produced_tables = build_produced_tables(verbs);

    let mut candidates = Vec::new();

    for table in tables {
        let table_key = format!("{}.{}", table.schema, table.table_name);
        let table_key_no_schema = &table.table_name;

        // Check if any verb references this table
        let table_verb_refs: Vec<(&str, VerbRefKind)> = verb_table_refs
            .get(table_key.as_str())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .chain(
                verb_table_refs
                    .get(table_key_no_schema.as_str())
                    .cloned()
                    .unwrap_or_default(),
            )
            .chain(if produced_tables.contains(table_key_no_schema.as_str()) {
                // Find which verb produces this type
                verbs
                    .iter()
                    .filter(|v| {
                        v.output
                            .as_ref()
                            .map(|o| o.produced_type == *table_key_no_schema)
                            .unwrap_or(false)
                    })
                    .map(|v| (v.fqn.as_str(), VerbRefKind::ProducesType))
                    .collect::<Vec<_>>()
            } else {
                vec![]
            })
            .collect();

        let pk_set: HashSet<&str> = table.primary_keys.iter().map(|s| s.as_str()).collect();
        let fk_map: HashMap<&str, &ForeignKeyExtract> = table
            .foreign_keys
            .iter()
            .map(|fk| (fk.from_column.as_str(), fk))
            .collect();

        for col in &table.columns {
            // Check for direct column-level verb references
            let col_key = format!("{}.{}.{}", table.schema, table.table_name, col.name);
            let col_verb_refs: Vec<(&str, VerbRefKind)> = verb_column_refs
                .get(col_key.as_str())
                .cloned()
                .unwrap_or_default();

            // Merge table-level and column-level verb refs
            let all_verb_refs: Vec<(&str, VerbRefKind)> = table_verb_refs
                .iter()
                .cloned()
                .chain(col_verb_refs.into_iter())
                .collect();

            let verb_fqns: Vec<String> = all_verb_refs
                .iter()
                .map(|(fqn, _)| fqn.to_string())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            let verb_ref_kinds: Vec<VerbRefKind> = all_verb_refs
                .iter()
                .map(|(_, kind)| kind.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            let is_pk = pk_set.contains(col.name.as_str());
            let foreign_key = fk_map.get(col.name.as_str()).copied().cloned();

            // Classify
            let classification = classify_column(&col.name, is_pk, !verb_fqns.is_empty());

            candidates.push(AttributeCandidate {
                schema: table.schema.clone(),
                table: table.table_name.clone(),
                column: col.name.clone(),
                sql_type: col.sql_type.clone(),
                is_nullable: col.is_nullable,
                classification,
                verb_refs: verb_fqns,
                verb_ref_kinds,
                is_primary_key: is_pk,
                foreign_key,
                default_value: col.default_value.clone(),
            });
        }
    }

    let verb_connected = candidates
        .iter()
        .filter(|c| c.classification == ColumnClassification::VerbConnected)
        .count();
    let framework = candidates
        .iter()
        .filter(|c| c.classification == ColumnClassification::Framework)
        .count();
    let operational_orphans = candidates
        .iter()
        .filter(|c| c.classification == ColumnClassification::OperationalOrphan)
        .count();
    let dead_schema = candidates
        .iter()
        .filter(|c| c.classification == ColumnClassification::DeadSchema)
        .count();

    XrefResult {
        candidates,
        verb_connected,
        framework,
        operational_orphans,
        dead_schema,
    }
}

/// Classify a column based on its name, PK status, and verb connectivity.
fn classify_column(name: &str, is_pk: bool, has_verb_refs: bool) -> ColumnClassification {
    if has_verb_refs {
        return ColumnClassification::VerbConnected;
    }

    if is_framework_column(name, is_pk) {
        return ColumnClassification::Framework;
    }

    // No verb references and not framework → operational orphan
    // (We treat all non-framework, non-verb-connected columns as orphans rather than
    //  dead schema, since we'd need code analysis to distinguish. Orphans are seeded
    //  with verb_orphan=true flag so they're visible in the registry.)
    ColumnClassification::OperationalOrphan
}

/// Framework column detection via regex-like pattern matching.
///
/// Framework columns are standard housekeeping columns that don't need
/// their own AttributeDef in the semantic registry.
fn is_framework_column(name: &str, is_pk: bool) -> bool {
    let lower = name.to_lowercase();

    // Timestamp audit columns
    if matches!(
        lower.as_str(),
        "created_at"
            | "updated_at"
            | "modified_at"
            | "deleted_at"
            | "created_on"
            | "updated_on"
            | "modified_on"
            | "deleted_on"
    ) {
        return true;
    }

    // Author audit columns
    if matches!(
        lower.as_str(),
        "created_by" | "updated_by" | "modified_by" | "deleted_by"
    ) {
        return true;
    }

    // Primary key identity columns (only when they ARE the PK)
    if is_pk && matches!(lower.as_str(), "id" | "uuid") {
        return true;
    }

    // Version tracking
    if matches!(lower.as_str(), "version" | "row_version") {
        return true;
    }

    false
}

/// Build an index of verb→table references from verb side effects.
///
/// Returns a map of `table_key → Vec<(verb_fqn, ref_kind)>`.
fn build_verb_table_refs(verbs: &[VerbExtract]) -> HashMap<&str, Vec<(&str, VerbRefKind)>> {
    let mut refs: HashMap<&str, Vec<(&str, VerbRefKind)>> = HashMap::new();

    for verb in verbs {
        // From side effects (CRUD table + lifecycle tables)
        for effect in &verb.side_effects {
            let table_key: &str = &effect.table;
            let kind = match effect.operation {
                SideEffectOp::Read => VerbRefKind::LifecycleTable,
                _ => VerbRefKind::CrudTable,
            };
            refs.entry(table_key).or_default().push((&verb.fqn, kind));

            // Also index with schema prefix if available
            if let Some(ref schema) = effect.schema {
                // We can't return a reference to a computed string,
                // so we skip schema-qualified indexing here.
                // The caller does the schema-qualified lookup separately.
                let _ = schema;
            }
        }

        // From lookup configs on inputs
        for input in &verb.inputs {
            if let Some(ref lookup_table) = input.lookup_table {
                refs.entry(lookup_table.as_str())
                    .or_default()
                    .push((&verb.fqn, VerbRefKind::LookupTable));
            }
        }
    }

    refs
}

/// Build an index of verb→column references from verb arg `maps_to`.
///
/// Returns a map of `"schema.table.column" → Vec<(verb_fqn, ref_kind)>`.
fn build_verb_column_refs(verbs: &[VerbExtract]) -> HashMap<String, Vec<(&str, VerbRefKind)>> {
    let mut refs: HashMap<String, Vec<(&str, VerbRefKind)>> = HashMap::new();

    for verb in verbs {
        // Find the CRUD table for this verb (for resolving maps_to)
        let crud_table = verb
            .side_effects
            .iter()
            .find(|e| {
                matches!(
                    e.operation,
                    SideEffectOp::Insert
                        | SideEffectOp::Update
                        | SideEffectOp::Upsert
                        | SideEffectOp::Write
                )
            })
            .map(|e| {
                let schema = e.schema.as_deref().unwrap_or("ob-poc");
                (schema, e.table.as_str())
            });

        for input in &verb.inputs {
            if let Some(ref maps_to) = input.maps_to {
                if let Some((schema, table)) = crud_table {
                    let col_key = format!("{}.{}.{}", schema, table, maps_to);
                    refs.entry(col_key)
                        .or_default()
                        .push((&verb.fqn, VerbRefKind::ArgMapping));
                }
            }
        }
    }

    refs
}

/// Build set of table names that are referenced by verb `produces.produced_type`.
fn build_produced_tables(verbs: &[VerbExtract]) -> HashSet<&str> {
    verbs
        .iter()
        .filter_map(|v| v.output.as_ref().map(|o| o.produced_type.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_framework_column() {
        assert!(is_framework_column("created_at", false));
        assert!(is_framework_column("updated_at", false));
        assert!(is_framework_column("created_by", false));
        assert!(is_framework_column("version", false));
        assert!(is_framework_column("row_version", false));

        // PK id columns
        assert!(is_framework_column("id", true));
        assert!(is_framework_column("uuid", true));
        assert!(!is_framework_column("id", false)); // Not PK → not framework

        // Non-framework
        assert!(!is_framework_column("name", false));
        assert!(!is_framework_column("jurisdiction", false));
        assert!(!is_framework_column("entity_id", false));
    }

    #[test]
    fn test_classify_column() {
        assert_eq!(
            classify_column("name", false, true),
            ColumnClassification::VerbConnected
        );
        assert_eq!(
            classify_column("created_at", false, false),
            ColumnClassification::Framework
        );
        assert_eq!(
            classify_column("some_column", false, false),
            ColumnClassification::OperationalOrphan
        );
    }
}
