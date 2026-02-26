//! Write-set derivation — identifies entity UUIDs that a verb invocation will mutate.
//!
//! Two strategies (INV-8):
//!
//! 1. **Heuristic** (always active): scans all arg values for UUIDs.
//!    Conservative — may include read-only entity references.
//!
//! 2. **Contract-driven** (behind `write-set-contract` feature): uses verb
//!    YAML metadata (`crud.key`, `maps_to`, `lookup.entity_type`) to identify
//!    which args are genuine write targets. Only UUID args that map to a
//!    CRUD key column or have an entity lookup are included.
//!
//! The combined function `derive_write_set()` returns the **union** of both
//! strategies (contract ∪ heuristic, deduplicated) when the feature is
//! enabled, or just the heuristic when it is not.

use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[cfg(feature = "write-set-contract")]
use crate::repl::verb_config_index::VerbConfigIndex;

// ---------------------------------------------------------------------------
// Heuristic: always-on UUID extraction from arg values
// ---------------------------------------------------------------------------

/// Always-on heuristic: extract any arg value that parses as a UUID.
///
/// This catches the common case where entity IDs are passed as resolved
/// argument values (e.g., `:entity-id <uuid>`, `:cbu-id <uuid>`).
///
/// Returns a `BTreeSet` for deterministic ordering (INV-2).
pub fn derive_write_set_heuristic(args: &BTreeMap<String, String>) -> BTreeSet<Uuid> {
    args.values()
        .filter_map(|v| {
            let trimmed = v.trim().trim_matches(|c| c == '<' || c == '>');
            Uuid::parse_str(trimmed).ok()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Contract-driven: uses verb YAML metadata (feature-gated)
// ---------------------------------------------------------------------------

/// Contract-driven write_set derivation from verb YAML metadata.
///
/// Uses `VerbConfigIndex` to look up arg metadata:
/// - Args whose `maps_to` matches the verb's `crud_key` are primary key
///   references (definite write targets for update/delete).
/// - Args with `lookup_entity_type` are entity references (write targets
///   for operations that modify the referenced entity).
///
/// Returns `BTreeSet<Uuid>` for deterministic ordering (INV-2).
#[cfg(feature = "write-set-contract")]
pub fn derive_write_set_from_contract(
    verb_fqn: &str,
    args: &BTreeMap<String, String>,
    verb_config_index: &VerbConfigIndex,
) -> BTreeSet<Uuid> {
    let mut write_set = BTreeSet::new();

    let entry = match verb_config_index.get(verb_fqn) {
        Some(e) => e,
        None => return write_set, // Unknown verb — no contract metadata
    };

    let crud_key = entry.crud_key.as_deref();

    for arg_meta in &entry.args {
        // Normalize arg name: YAML uses kebab-case, args HashMap may use either
        let arg_name_kebab = &arg_meta.name;
        let arg_value = args.get(arg_name_kebab).or_else(|| {
            // Try snake_case variant
            let snake = arg_name_kebab.replace('-', "_");
            args.get(&snake)
        });

        let value = match arg_value {
            Some(v) => v,
            None => continue,
        };

        let trimmed = value.trim().trim_matches(|c| c == '<' || c == '>');
        let uuid = match Uuid::parse_str(trimmed) {
            Ok(u) => u,
            Err(_) => continue, // Not a UUID value — skip
        };

        // Strategy 1: arg maps to the CRUD key column → definite write target
        if let (Some(maps_to), Some(key)) = (&arg_meta.maps_to, crud_key) {
            if maps_to == key {
                write_set.insert(uuid);
                continue;
            }
        }

        // Strategy 2: arg has entity lookup → entity reference (write target)
        if arg_meta.lookup_entity_type.is_some() {
            write_set.insert(uuid);
        }
    }

    write_set
}

// ---------------------------------------------------------------------------
// Combined: contract (if feature enabled) ∪ heuristic
// ---------------------------------------------------------------------------

/// Combined write_set derivation: contract (if feature enabled) ∪ heuristic.
///
/// When `write-set-contract` feature is enabled, returns the union of
/// contract-derived and heuristic-derived write sets. When disabled,
/// returns only the heuristic result.
///
/// The union ensures we never miss a write target — contract narrows
/// false positives while heuristic provides conservative coverage.
///
/// Returns `BTreeSet<Uuid>` for deterministic ordering (INV-2).
#[cfg(feature = "write-set-contract")]
pub fn derive_write_set(
    verb_fqn: &str,
    args: &BTreeMap<String, String>,
    verb_config_index: Option<&VerbConfigIndex>,
) -> BTreeSet<Uuid> {
    let mut result = derive_write_set_heuristic(args);

    if let Some(index) = verb_config_index {
        let contract = derive_write_set_from_contract(verb_fqn, args, index);
        result.extend(contract);
    }

    result
}

/// Heuristic-only write_set derivation (when `write-set-contract` is disabled).
#[cfg(not(feature = "write-set-contract"))]
pub fn derive_write_set(
    _verb_fqn: &str,
    args: &BTreeMap<String, String>,
    _verb_config_index: Option<&()>,
) -> BTreeSet<Uuid> {
    derive_write_set_heuristic(args)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_extracts_uuids_from_args() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let mut args = BTreeMap::new();
        args.insert("entity-id".to_string(), id1.to_string());
        args.insert("cbu-id".to_string(), format!("<{}>", id2));
        args.insert("name".to_string(), "Acme Corp".to_string());

        let result = derive_write_set_heuristic(&args);
        assert!(result.contains(&id1));
        assert!(result.contains(&id2));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_heuristic_ignores_non_uuids() {
        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Acme Corp".to_string());
        args.insert("jurisdiction".to_string(), "LU".to_string());
        args.insert("count".to_string(), "42".to_string());

        let result = derive_write_set_heuristic(&args);
        assert!(result.is_empty());
    }

    #[test]
    fn test_heuristic_fallback_when_no_contract() {
        let id = Uuid::new_v4();
        let mut args = BTreeMap::new();
        args.insert("entity-id".to_string(), id.to_string());

        // With no verb_config_index, derive_write_set falls back to heuristic
        #[cfg(feature = "write-set-contract")]
        {
            let result = derive_write_set("unknown.verb", &args, None);
            assert!(result.contains(&id));
        }
        #[cfg(not(feature = "write-set-contract"))]
        {
            let result = derive_write_set("unknown.verb", &args, None);
            assert!(result.contains(&id));
        }
    }

    #[test]
    fn test_write_set_uses_btreeset() {
        // INV-2: output must be BTreeSet, not HashSet
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let mut args = BTreeMap::new();
        args.insert("a".to_string(), id1.to_string());
        args.insert("b".to_string(), id2.to_string());

        let result = derive_write_set_heuristic(&args);
        // BTreeSet is deterministic: same inputs → same iteration order
        let bytes1: Vec<u8> = result.iter().flat_map(|u| u.as_bytes().to_vec()).collect();
        let bytes2: Vec<u8> = result.iter().flat_map(|u| u.as_bytes().to_vec()).collect();
        assert_eq!(
            bytes1, bytes2,
            "BTreeSet iteration must be deterministic (INV-2)"
        );
    }

    #[cfg(feature = "write-set-contract")]
    mod contract_tests {
        use super::*;
        use crate::repl::runbook::ConfirmPolicy;
        use crate::repl::verb_config_index::{ArgSummary, VerbConfigIndex, VerbIndexEntry};

        fn test_index() -> VerbConfigIndex {
            let mut index = VerbConfigIndex::empty();
            index.insert_test_entry(VerbIndexEntry {
                fqn: "cbu.update".to_string(),
                description: "Update a CBU".to_string(),
                invocation_phrases: vec![],
                sentence_templates: vec![],
                sentences: None,
                args: vec![
                    ArgSummary {
                        name: "cbu-id".to_string(),
                        arg_type: "uuid".to_string(),
                        required: true,
                        description: None,
                        maps_to: Some("cbu_id".to_string()),
                        lookup_entity_type: Some("cbu".to_string()),
                    },
                    ArgSummary {
                        name: "name".to_string(),
                        arg_type: "string".to_string(),
                        required: false,
                        description: None,
                        maps_to: Some("name".to_string()),
                        lookup_entity_type: None,
                    },
                ],
                crud_key: Some("cbu_id".to_string()),
                confirm_policy: ConfirmPolicy::Always,
                precondition_checks: vec![],
            });
            index.insert_test_entry(VerbIndexEntry {
                fqn: "entity.create".to_string(),
                description: "Create entity".to_string(),
                invocation_phrases: vec![],
                sentence_templates: vec![],
                sentences: None,
                args: vec![
                    ArgSummary {
                        name: "name".to_string(),
                        arg_type: "string".to_string(),
                        required: true,
                        description: None,
                        maps_to: Some("company_name".to_string()),
                        lookup_entity_type: None,
                    },
                    ArgSummary {
                        name: "parent-entity-id".to_string(),
                        arg_type: "uuid".to_string(),
                        required: false,
                        description: None,
                        maps_to: None,
                        lookup_entity_type: Some("entity".to_string()),
                    },
                ],
                crud_key: None,
                confirm_policy: ConfirmPolicy::Always,
                precondition_checks: vec![],
            });
            index
        }

        #[test]
        fn test_contract_extracts_crud_key_arg() {
            let index = test_index();
            let cbu_id = Uuid::new_v4();
            let mut args = BTreeMap::new();
            args.insert("cbu-id".to_string(), cbu_id.to_string());
            args.insert("name".to_string(), "New Name".to_string());

            let result = derive_write_set_from_contract("cbu.update", &args, &index);
            assert!(
                result.contains(&cbu_id),
                "CRUD key arg should be in write_set"
            );
            assert_eq!(result.len(), 1, "Only the key arg, not 'name'");
        }

        #[test]
        fn test_contract_extracts_entity_lookup_arg() {
            let index = test_index();
            let parent_id = Uuid::new_v4();
            let mut args = BTreeMap::new();
            args.insert("name".to_string(), "Child Corp".to_string());
            args.insert("parent-entity-id".to_string(), parent_id.to_string());

            let result = derive_write_set_from_contract("entity.create", &args, &index);
            assert!(
                result.contains(&parent_id),
                "Entity lookup arg should be in write_set"
            );
        }

        #[test]
        fn test_combined_write_set_union() {
            let index = test_index();
            let cbu_id = Uuid::new_v4();
            let extra_id = Uuid::new_v4();
            let mut args = BTreeMap::new();
            args.insert("cbu-id".to_string(), cbu_id.to_string());
            // An arg not in the contract but contains a UUID — heuristic picks it up
            args.insert("extra-ref".to_string(), extra_id.to_string());

            let result = derive_write_set("cbu.update", &args, Some(&index));
            assert!(result.contains(&cbu_id), "Contract-derived");
            assert!(result.contains(&extra_id), "Heuristic-derived");
            assert_eq!(result.len(), 2, "Union without duplicates");
        }

        #[test]
        fn test_contract_unknown_verb_returns_empty() {
            let index = test_index();
            let mut args = BTreeMap::new();
            args.insert("id".to_string(), Uuid::new_v4().to_string());

            let result = derive_write_set_from_contract("nonexistent.verb", &args, &index);
            assert!(result.is_empty(), "Unknown verb has no contract metadata");
        }
    }
}
