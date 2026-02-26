//! Step 5: Assemble OnboardingManifest from extraction pipeline outputs.
//!
//! Combines verb extracts, attribute candidates, entity type candidates,
//! and relationship candidates into a single serializable manifest with
//! wiring completeness metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use super::entity_infer::{EntityTypeCandidate, RelationshipCandidate};
use super::verb_extract::VerbExtract;
use super::xref::{AttributeCandidate, ColumnClassification, XrefResult};

/// Complete onboarding manifest produced by the extraction pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingManifest {
    /// When the manifest was generated
    pub extracted_at: DateTime<Utc>,
    /// Source database identifier
    pub source_db: String,
    /// Number of tables scanned
    pub tables_scanned: usize,
    /// Number of columns scanned
    pub columns_scanned: usize,
    /// Number of verbs extracted
    pub verbs_extracted: usize,

    /// All attribute candidates with classification
    pub attribute_candidates: Vec<AttributeCandidate>,
    /// All verb extracts
    pub verb_extracts: Vec<VerbExtract>,
    /// Inferred entity type candidates
    pub entity_type_candidates: Vec<EntityTypeCandidate>,
    /// Inferred relationship candidates
    pub relationship_candidates: Vec<RelationshipCandidate>,

    // ── Classification summary ───────────────────────────────
    /// Columns referenced by ≥1 verb
    pub verb_connected_attrs: usize,
    /// Standard housekeeping columns (not seeded)
    pub framework_columns: usize,
    /// Schema columns with no verb references (seeded as orphans)
    pub operational_orphans: usize,
    /// Completely unreferenced columns (not seeded)
    pub dead_schema: usize,

    // ── Wiring completeness ──────────────────────────────────
    /// Verbs where all I/O columns are mapped
    pub verbs_fully_wired: usize,
    /// Verbs where some but not all I/O columns are mapped
    pub verbs_partially_wired: usize,
    /// Verbs with no schema column mappings
    pub verbs_unwired: usize,
    /// Overall wiring percentage (target ≥ 80%)
    pub wiring_pct: f32,
}

/// Assemble a manifest from pipeline outputs.
pub fn assemble_manifest(
    source_db: &str,
    tables_scanned: usize,
    columns_scanned: usize,
    verb_extracts: Vec<VerbExtract>,
    xref_result: XrefResult,
    entity_type_candidates: Vec<EntityTypeCandidate>,
    relationship_candidates: Vec<RelationshipCandidate>,
) -> OnboardingManifest {
    let (verbs_fully_wired, verbs_partially_wired, verbs_unwired, wiring_pct) =
        compute_wiring_metrics(&verb_extracts, &xref_result.candidates);

    OnboardingManifest {
        extracted_at: Utc::now(),
        source_db: source_db.to_string(),
        tables_scanned,
        columns_scanned,
        verbs_extracted: verb_extracts.len(),
        attribute_candidates: xref_result.candidates,
        verb_extracts,
        entity_type_candidates,
        relationship_candidates,
        verb_connected_attrs: xref_result.verb_connected,
        framework_columns: xref_result.framework,
        operational_orphans: xref_result.operational_orphans,
        dead_schema: xref_result.dead_schema,
        verbs_fully_wired,
        verbs_partially_wired,
        verbs_unwired,
        wiring_pct,
    }
}

/// Compute verb wiring metrics.
///
/// A verb is "fully wired" if it has schema touchpoints AND all its `maps_to` args
/// can be found in the schema. Verbs with side_effects or produces that reference
/// real tables are considered wired even without `maps_to` args (plugin pattern).
///
/// A verb is "partially wired" if some but not all `maps_to` args are mapped,
/// OR it has side_effects but no `maps_to` confirmation.
///
/// A verb is "unwired" if it has zero schema touchpoints — no maps_to, no
/// side_effects referencing known tables, no produces referencing known tables.
fn compute_wiring_metrics(
    verbs: &[VerbExtract],
    candidates: &[AttributeCandidate],
) -> (usize, usize, usize, f32) {
    // Build set of known (table, column) pairs from candidates
    let known_columns: HashSet<(String, String)> = candidates
        .iter()
        .filter(|c| c.classification == ColumnClassification::VerbConnected)
        .map(|c| (c.table.clone(), c.column.clone()))
        .collect();

    // Build set of known table names from ALL candidates (not just verb-connected)
    let known_tables: HashSet<&str> = candidates.iter().map(|c| c.table.as_str()).collect();

    let mut fully_wired = 0;
    let mut partially_wired = 0;
    let mut unwired = 0;

    for verb in verbs {
        let mapped_args: Vec<&str> = verb
            .inputs
            .iter()
            .filter_map(|i| i.maps_to.as_deref())
            .collect();

        // Check if verb has side_effects referencing known tables
        let has_known_side_effects = verb
            .side_effects
            .iter()
            .any(|e| known_tables.contains(e.table.as_str()));

        // Check if verb produces an entity type that maps to a known table
        let has_known_produces = verb
            .output
            .as_ref()
            .map(|o| known_tables.contains(o.produced_type.as_str()))
            .unwrap_or(false);

        let has_schema_touchpoint = has_known_side_effects || has_known_produces;

        if mapped_args.is_empty() {
            // No maps_to args — classify based on other schema touchpoints
            if has_schema_touchpoint {
                // Plugin/template verb with side_effects or produces → wired
                fully_wired += 1;
            } else {
                // Pure graph query, internal, or truly unwired
                unwired += 1;
            }
            continue;
        }

        // Has maps_to args — check how many resolve to known columns
        let verb_tables: Vec<&str> = verb.side_effects.iter().map(|e| e.table.as_str()).collect();

        let mapped_count = mapped_args
            .iter()
            .filter(|col| {
                verb_tables
                    .iter()
                    .any(|table| known_columns.contains(&(table.to_string(), col.to_string())))
            })
            .count();

        if mapped_count == mapped_args.len() {
            fully_wired += 1;
        } else if mapped_count > 0 || has_schema_touchpoint {
            partially_wired += 1;
        } else {
            unwired += 1;
        }
    }

    let total = fully_wired + partially_wired + unwired;
    let wiring_pct = if total > 0 {
        (fully_wired as f32 / total as f32) * 100.0
    } else {
        0.0
    };

    (fully_wired, partially_wired, unwired, wiring_pct)
}

/// Write manifest to a JSON file.
pub fn write_manifest(manifest: &OnboardingManifest, path: &Path) -> Result<(), anyhow::Error> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(manifest)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Read manifest from a JSON file.
pub fn read_manifest(path: &Path) -> Result<OnboardingManifest, anyhow::Error> {
    let json = std::fs::read_to_string(path)?;
    let manifest: OnboardingManifest = serde_json::from_str(&json)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serde_round_trip() {
        let manifest = OnboardingManifest {
            extracted_at: Utc::now(),
            source_db: "test_db".into(),
            tables_scanned: 10,
            columns_scanned: 100,
            verbs_extracted: 50,
            attribute_candidates: vec![],
            verb_extracts: vec![],
            entity_type_candidates: vec![],
            relationship_candidates: vec![],
            verb_connected_attrs: 60,
            framework_columns: 20,
            operational_orphans: 15,
            dead_schema: 5,
            verbs_fully_wired: 30,
            verbs_partially_wired: 10,
            verbs_unwired: 10,
            wiring_pct: 60.0,
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let back: OnboardingManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tables_scanned, 10);
        assert_eq!(back.verbs_extracted, 50);
        assert_eq!(back.wiring_pct, 60.0);
    }

    #[test]
    fn test_wiring_metrics_empty() {
        let (fw, pw, uw, pct) = compute_wiring_metrics(&[], &[]);
        assert_eq!(fw, 0);
        assert_eq!(pw, 0);
        assert_eq!(uw, 0);
        assert_eq!(pct, 0.0);
    }
}
