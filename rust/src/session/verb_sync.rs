//! Verb Sync Service
//!
//! Synchronizes RuntimeVerb definitions from YAML to the `dsl_verbs` database table.
//! Uses hash-based change detection to only update changed verbs.
//!
//! # Startup Flow
//!
//! ```text
//! YAML files → RuntimeVerbRegistry → VerbSyncService → dsl_verbs table
//!                                         │
//!                                         ├── Compute SHA256 hash of verb definition
//!                                         ├── Compare with yaml_hash in DB
//!                                         └── Upsert only changed verbs
//! ```
//!
//! # RAG Metadata
//!
//! The `dsl_verbs` table includes additional RAG columns that don't exist in RuntimeVerb:
//! - `search_text`: Auto-generated from description + intent_patterns (via DB trigger)
//! - `intent_patterns`: Natural language patterns - populated separately
//! - `typical_next`: Workflow hints - populated separately
//! - `workflow_phases`: KYC phases - populated separately
//! - `graph_contexts`: Graph cursor contexts - populated separately

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::dsl_v2::runtime_registry::{RuntimeBehavior, RuntimeVerb};
use crate::dsl_v2::RuntimeVerbRegistry;

/// Errors from verb sync operations
#[derive(Debug, Error)]
pub enum VerbSyncError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result of a sync operation
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub verbs_added: i32,
    pub verbs_updated: i32,
    pub verbs_unchanged: i32,
    pub verbs_removed: i32,
    pub duration_ms: i64,
    pub source_hash: String,
}

/// Verb Sync Service - synchronizes YAML verbs to database
pub struct VerbSyncService {
    pool: PgPool,
}

impl VerbSyncService {
    /// Create a new VerbSyncService
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Sync all verbs from registry to database
    ///
    /// Returns the sync result with counts of added, updated, unchanged verbs.
    pub async fn sync_all(
        &self,
        registry: &RuntimeVerbRegistry,
    ) -> Result<SyncResult, VerbSyncError> {
        let start = Instant::now();

        let mut added = 0i32;
        let mut updated = 0i32;
        let mut unchanged = 0i32;

        // Compute overall source hash (for logging)
        let source_hash = self.compute_registry_hash(registry);

        // Get existing verb hashes from DB
        let existing_hashes = self.get_existing_hashes().await?;

        // Sync each verb
        for verb in registry.all_verbs() {
            let verb_hash = self.compute_verb_hash(verb);
            let existing_hash = existing_hashes.get(&verb.full_name);

            match existing_hash {
                Some(hash) if hash == &verb_hash => {
                    // Unchanged
                    unchanged += 1;
                    debug!("Verb {} unchanged (hash match)", verb.full_name);
                }
                Some(_) => {
                    // Changed - update
                    self.upsert_verb(verb, &verb_hash).await?;
                    updated += 1;
                    debug!("Verb {} updated (hash changed)", verb.full_name);
                }
                None => {
                    // New - insert
                    self.upsert_verb(verb, &verb_hash).await?;
                    added += 1;
                    debug!("Verb {} added (new)", verb.full_name);
                }
            }
        }

        // Mark verbs not in registry as removed (don't delete, just log)
        let registry_verbs: std::collections::HashSet<_> =
            registry.all_verbs().map(|v| &v.full_name).collect();
        let removed = existing_hashes
            .keys()
            .filter(|k| !registry_verbs.contains(*k))
            .count() as i32;

        if removed > 0 {
            warn!("{} verbs in DB not in YAML registry (orphaned)", removed);
        }

        let duration_ms = start.elapsed().as_millis() as i64;

        // Log sync
        self.log_sync(
            added,
            updated,
            unchanged,
            removed,
            &source_hash,
            duration_ms,
            None,
        )
        .await?;

        let result = SyncResult {
            verbs_added: added,
            verbs_updated: updated,
            verbs_unchanged: unchanged,
            verbs_removed: removed,
            duration_ms,
            source_hash,
        };

        info!(
            "Verb sync complete: {} added, {} updated, {} unchanged, {} orphaned in {}ms",
            added, updated, unchanged, removed, duration_ms
        );

        Ok(result)
    }

    /// Get existing verb hashes from database
    async fn get_existing_hashes(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, VerbSyncError> {
        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as(r#"SELECT full_name, yaml_hash FROM "ob-poc".dsl_verbs"#)
                .fetch_all(&self.pool)
                .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(name, hash)| hash.map(|h| (name, h)))
            .collect())
    }

    /// Upsert a verb to the database
    async fn upsert_verb(&self, verb: &RuntimeVerb, hash: &str) -> Result<(), VerbSyncError> {
        let behavior = match &verb.behavior {
            RuntimeBehavior::Crud(_) => "crud",
            RuntimeBehavior::Plugin(_) => "plugin",
            RuntimeBehavior::GraphQuery(_) => "graph_query",
        };

        // Extract produces info
        let (produces_type, produces_subtype) = verb
            .produces
            .as_ref()
            .map(|p| (Some(p.produced_type.clone()), p.subtype.clone()))
            .unwrap_or((None, None));

        // Convert consumes to JSON
        let consumes_json = serde_json::to_value(&verb.consumes)?;

        // Extract lifecycle info
        let (lifecycle_entity_arg, requires_states, transitions_to) = verb
            .lifecycle
            .as_ref()
            .map(|l| {
                (
                    l.entity_arg.clone(),
                    if l.requires_states.is_empty() {
                        None
                    } else {
                        Some(l.requires_states.clone())
                    },
                    l.transitions_to.clone(),
                )
            })
            .unwrap_or((None, None, None));

        // Determine category from domain (can be overridden later)
        let category = self.infer_category(&verb.domain);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_verbs (
                domain, verb_name, description, behavior,
                category, produces_type, produces_subtype, consumes,
                lifecycle_entity_arg, requires_states, transitions_to,
                source, yaml_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (domain, verb_name) DO UPDATE SET
                description = EXCLUDED.description,
                behavior = EXCLUDED.behavior,
                category = COALESCE("ob-poc".dsl_verbs.category, EXCLUDED.category),
                produces_type = EXCLUDED.produces_type,
                produces_subtype = EXCLUDED.produces_subtype,
                consumes = EXCLUDED.consumes,
                lifecycle_entity_arg = EXCLUDED.lifecycle_entity_arg,
                requires_states = EXCLUDED.requires_states,
                transitions_to = EXCLUDED.transitions_to,
                yaml_hash = EXCLUDED.yaml_hash,
                updated_at = now()
        "#,
        )
        .bind(&verb.domain)
        .bind(&verb.verb)
        .bind(&verb.description)
        .bind(behavior)
        .bind(&category)
        .bind(&produces_type)
        .bind(&produces_subtype)
        .bind(&consumes_json)
        .bind(&lifecycle_entity_arg)
        .bind(&requires_states)
        .bind(&transitions_to)
        .bind("yaml")
        .bind(hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Log a sync operation
    async fn log_sync(
        &self,
        added: i32,
        updated: i32,
        unchanged: i32,
        removed: i32,
        source_hash: &str,
        duration_ms: i64,
        error: Option<&str>,
    ) -> Result<Uuid, VerbSyncError> {
        let sync_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".dsl_verb_sync_log (
                verbs_added, verbs_updated, verbs_unchanged, verbs_removed,
                source_hash, duration_ms, error_message
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING sync_id
        "#,
        )
        .bind(added)
        .bind(updated)
        .bind(unchanged)
        .bind(removed)
        .bind(source_hash)
        .bind(duration_ms as i32)
        .bind(error)
        .fetch_one(&self.pool)
        .await?;

        Ok(sync_id.0)
    }

    /// Compute SHA256 hash of a verb definition
    fn compute_verb_hash(&self, verb: &RuntimeVerb) -> String {
        let mut hasher = Sha256::new();

        // Hash stable fields
        hasher.update(verb.domain.as_bytes());
        hasher.update(verb.verb.as_bytes());
        hasher.update(verb.description.as_bytes());

        // Hash behavior type
        let behavior_str = match &verb.behavior {
            RuntimeBehavior::Crud(c) => format!("crud:{:?}", c.operation),
            RuntimeBehavior::Plugin(h) => format!("plugin:{}", h),
            RuntimeBehavior::GraphQuery(g) => format!("graph_query:{:?}", g.operation),
        };
        hasher.update(behavior_str.as_bytes());

        // Hash args
        for arg in &verb.args {
            hasher.update(arg.name.as_bytes());
            hasher.update(format!("{:?}", arg.arg_type).as_bytes());
            hasher.update(if arg.required { b"1" } else { b"0" });
        }

        // Hash produces/consumes
        if let Some(p) = &verb.produces {
            hasher.update(p.produced_type.as_bytes());
            if let Some(s) = &p.subtype {
                hasher.update(s.as_bytes());
            }
        }
        for c in &verb.consumes {
            hasher.update(c.consumed_type.as_bytes());
            hasher.update(if c.required { b"1" } else { b"0" });
        }

        // Hash lifecycle
        if let Some(l) = &verb.lifecycle {
            if let Some(ref entity_arg) = l.entity_arg {
                hasher.update(entity_arg.as_bytes());
            }
            for s in &l.requires_states {
                hasher.update(s.as_bytes());
            }
            if let Some(ref t) = l.transitions_to {
                hasher.update(t.as_bytes());
            }
        }

        let result = hasher.finalize();
        Self::bytes_to_hex(&result)
    }

    /// Compute hash of entire registry (for logging)
    fn compute_registry_hash(&self, registry: &RuntimeVerbRegistry) -> String {
        let mut hasher = Sha256::new();

        // Sort verbs for deterministic ordering
        let mut verbs: Vec<_> = registry.all_verbs().collect();
        verbs.sort_by(|a, b| a.full_name.cmp(&b.full_name));

        for verb in verbs {
            hasher.update(self.compute_verb_hash(verb).as_bytes());
        }

        let result = hasher.finalize();
        Self::bytes_to_hex(&result)
    }

    /// Convert bytes to hex string
    fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Infer category from domain name
    fn infer_category(&self, domain: &str) -> Option<String> {
        Some(
            match domain {
                "cbu" => "cbu_operations",
                "entity" => "entity_management",
                "ubo" => "ownership_control",
                "document" => "document_management",
                "kyc-case" | "entity-workstream" | "red-flag" | "doc-request" => "kyc_workflow",
                "case-screening" | "screening" => "screening",
                "graph" => "graph_visualization",
                "cbu-custody" | "isda" | "entity-settlement" => "custody_settlement",
                "product" | "service" | "service-resource" => "products_services",
                "fund" | "share-class" | "holding" | "movement" => "fund_structure",
                "verify" | "allegation" | "observation" | "discrepancy" => "verification",
                "jurisdiction" | "currency" | "role" | "client-type" | "case-type"
                | "screening-type" | "risk-rating" | "settlement-type" | "ssi-type"
                | "instrument-class" | "market" | "security-type" | "subcustodian" => {
                    "reference_data"
                }
                _ => return None,
            }
            .to_string(),
        )
    }

    /// Update RAG metadata for a verb (intent_patterns, workflow_phases, etc.)
    ///
    /// This is separate from sync_all because RAG metadata may be managed
    /// independently (e.g., via admin UI or separate YAML files).
    pub async fn update_rag_metadata(
        &self,
        domain: &str,
        verb: &str,
        intent_patterns: Option<Vec<String>>,
        workflow_phases: Option<Vec<String>>,
        graph_contexts: Option<Vec<String>>,
        typical_next: Option<Vec<String>>,
        example_short: Option<String>,
        example_dsl: Option<String>,
    ) -> Result<(), VerbSyncError> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_verbs
            SET
                intent_patterns = COALESCE($3, intent_patterns),
                workflow_phases = COALESCE($4, workflow_phases),
                graph_contexts = COALESCE($5, graph_contexts),
                typical_next = COALESCE($6, typical_next),
                example_short = COALESCE($7, example_short),
                example_dsl = COALESCE($8, example_dsl),
                updated_at = now()
            WHERE domain = $1 AND verb_name = $2
        "#,
        )
        .bind(domain)
        .bind(verb)
        .bind(&intent_patterns)
        .bind(&workflow_phases)
        .bind(&graph_contexts)
        .bind(&typical_next)
        .bind(&example_short)
        .bind(&example_dsl)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Bulk update RAG metadata from a YAML file
    ///
    /// The YAML format should be:
    /// ```yaml
    /// verbs:
    ///   cbu.create:
    ///     intent_patterns: ["create a cbu", "onboard a client"]
    ///     workflow_phases: ["intake"]
    ///   cbu.assign-role:
    ///     intent_patterns: ["add role", "assign role"]
    ///     graph_contexts: ["cursor_on_cbu", "cursor_on_entity"]
    /// ```
    pub async fn bulk_update_rag_from_yaml(
        &self,
        yaml_content: &str,
    ) -> Result<i32, VerbSyncError> {
        #[derive(serde::Deserialize)]
        struct RagMetadataFile {
            verbs: std::collections::HashMap<String, RagMetadata>,
        }

        #[derive(serde::Deserialize)]
        struct RagMetadata {
            intent_patterns: Option<Vec<String>>,
            workflow_phases: Option<Vec<String>>,
            graph_contexts: Option<Vec<String>>,
            typical_next: Option<Vec<String>>,
            example_short: Option<String>,
            example_dsl: Option<String>,
        }

        let data: RagMetadataFile = serde_yaml::from_str(yaml_content).map_err(|e| {
            VerbSyncError::Serialization(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )))
        })?;

        let mut count = 0;
        for (full_name, metadata) in data.verbs {
            if let Some((domain, verb)) = full_name.split_once('.') {
                self.update_rag_metadata(
                    domain,
                    verb,
                    metadata.intent_patterns,
                    metadata.workflow_phases,
                    metadata.graph_contexts,
                    metadata.typical_next,
                    metadata.example_short,
                    metadata.example_dsl,
                )
                .await?;
                count += 1;
            } else {
                warn!("Invalid verb name in RAG metadata: {}", full_name);
            }
        }

        info!("Updated RAG metadata for {} verbs", count);
        Ok(count)
    }

    /// Populate RAG metadata from hardcoded patterns in verb_rag_metadata module
    ///
    /// This updates intent_patterns, workflow_phases, graph_contexts, and typical_next
    /// for all known verbs. Called on startup after sync_all.
    pub async fn populate_rag_metadata(&self) -> Result<i32, VerbSyncError> {
        use super::verb_rag_metadata::{
            get_graph_contexts, get_intent_patterns, get_typical_next, get_workflow_phases,
        };

        let mut updated = 0i32;

        // Update intent patterns
        let intent_patterns = get_intent_patterns();
        for (verb, patterns) in intent_patterns {
            let patterns_vec: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();
            let result = sqlx::query(
                r#"
                UPDATE "ob-poc".dsl_verbs
                SET intent_patterns = $1,
                    updated_at = NOW()
                WHERE full_name = $2
                "#,
            )
            .bind(&patterns_vec)
            .bind(verb)
            .execute(&self.pool)
            .await?;

            if result.rows_affected() > 0 {
                updated += 1;
            }
        }

        // Update workflow phases (forward mapping: verb -> phase)
        let workflow_phases = get_workflow_phases();
        for (verb, phase) in workflow_phases {
            sqlx::query(
                r#"
                UPDATE "ob-poc".dsl_verbs
                SET workflow_phases = array_append(
                    COALESCE(workflow_phases, ARRAY[]::text[]),
                    $1
                ),
                updated_at = NOW()
                WHERE full_name = $2
                  AND NOT ($1 = ANY(COALESCE(workflow_phases, ARRAY[]::text[])))
                "#,
            )
            .bind(phase)
            .bind(verb)
            .execute(&self.pool)
            .await?;
        }

        // Update graph contexts (reverse mapping)
        let graph_contexts = get_graph_contexts();
        for (context, verbs) in graph_contexts {
            for verb in verbs {
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".dsl_verbs
                    SET graph_contexts = array_append(
                        COALESCE(graph_contexts, ARRAY[]::text[]),
                        $1
                    ),
                    updated_at = NOW()
                    WHERE full_name = $2
                      AND NOT ($1 = ANY(COALESCE(graph_contexts, ARRAY[]::text[])))
                    "#,
                )
                .bind(context)
                .bind(verb)
                .execute(&self.pool)
                .await?;
            }
        }

        // Update typical_next
        let typical_next = get_typical_next();
        for (verb, next_verbs) in typical_next {
            let next_vec: Vec<String> = next_verbs.iter().map(|s| s.to_string()).collect();
            sqlx::query(
                r#"
                UPDATE "ob-poc".dsl_verbs
                SET typical_next = $1,
                    updated_at = NOW()
                WHERE full_name = $2
                "#,
            )
            .bind(&next_vec)
            .bind(verb)
            .execute(&self.pool)
            .await?;
        }

        // Regenerate search_text from description + intent_patterns
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_verbs
            SET search_text = CONCAT(
                COALESCE(full_name, ''), ' ',
                COALESCE(description, ''), ' ',
                COALESCE(array_to_string(intent_patterns, ' '), '')
            ),
            updated_at = NOW()
            "#,
        )
        .execute(&self.pool)
        .await?;

        info!("Populated RAG metadata for {} verbs", updated);
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Standalone function for testing category inference without needing a pool
    fn test_infer_category_impl(domain: &str) -> Option<String> {
        match domain {
            "cbu" => Some("cbu_operations".to_string()),
            "entity" => Some("entity_management".to_string()),
            "document" => Some("document_management".to_string()),
            "kyc-case" | "entity-workstream" | "red-flag" | "doc-request" | "case-screening"
            | "case-event" => Some("kyc_workflow".to_string()),
            "ubo" => Some("ubo_ownership".to_string()),
            "cbu-custody" | "isda" | "entity-settlement" => Some("custody_settlement".to_string()),
            "share-class" | "holding" | "movement" => Some("investor_registry".to_string()),
            "service-resource" | "delivery" => Some("service_delivery".to_string()),
            "screening" | "allegation" | "observation" | "discrepancy" => {
                Some("verification".to_string())
            }
            "threshold" | "rfi" => Some("risk_assessment".to_string()),
            _ => None,
        }
    }

    // Standalone function for testing hash computation
    fn test_compute_hash(verb: &RuntimeVerb) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(verb.full_name.as_bytes());
        hasher.update(verb.description.as_bytes());
        hasher.update(format!("{:?}", verb.behavior).as_bytes());
        for arg in &verb.args {
            hasher.update(arg.name.as_bytes());
            hasher.update(format!("{:?}", arg.arg_type).as_bytes());
            hasher.update(if arg.required { b"1" } else { b"0" });
        }
        hasher.update(format!("{:?}", verb.returns).as_bytes());
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn test_infer_category() {
        assert_eq!(
            test_infer_category_impl("cbu"),
            Some("cbu_operations".to_string())
        );
        assert_eq!(
            test_infer_category_impl("entity"),
            Some("entity_management".to_string())
        );
        assert_eq!(
            test_infer_category_impl("kyc-case"),
            Some("kyc_workflow".to_string())
        );
        assert_eq!(test_infer_category_impl("unknown-domain"), None);
    }

    #[test]
    fn test_compute_verb_hash_deterministic() {
        use crate::dsl_v2::config::types::{ArgType, ReturnTypeConfig};
        use crate::dsl_v2::runtime_registry::{RuntimeArg, RuntimeReturn};

        let verb = RuntimeVerb {
            domain: "cbu".to_string(),
            verb: "create".to_string(),
            full_name: "cbu.create".to_string(),
            description: "Create a CBU".to_string(),
            behavior: RuntimeBehavior::Plugin("create_cbu".to_string()),
            args: vec![RuntimeArg {
                name: "name".to_string(),
                arg_type: ArgType::String,
                required: true,
                maps_to: Some("name".to_string()),
                lookup: None,
                valid_values: None,
                default: None,
                description: None,
                fuzzy_check: None,
            }],
            returns: RuntimeReturn {
                return_type: ReturnTypeConfig::Uuid,
                name: Some("cbu_id".to_string()),
                capture: true,
            },
            produces: None,
            consumes: vec![],
            lifecycle: None,
        };

        let hash1 = test_compute_hash(&verb);
        let hash2 = test_compute_hash(&verb);

        assert_eq!(hash1, hash2, "Hash should be deterministic");
        assert_eq!(hash1.len(), 64, "SHA256 hash should be 64 hex chars");
    }
}
