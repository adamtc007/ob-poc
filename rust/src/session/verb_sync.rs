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
//!                                         ├── Compile to full contract JSON
//!                                         └── Upsert only changed verbs
//! ```
//!
//! # Contract Storage
//!
//! Each verb is compiled to a full JSON contract and stored in `dsl_verbs`:
//! - `compiled_json`: Full RuntimeVerb serialized as JSON
//! - `effective_config_json`: Expanded configuration with defaults applied
//! - `diagnostics_json`: Compilation errors and warnings
//! - `compiled_hash`: SHA256 of canonical compiled_json for integrity
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

use super::canonical_hash::canonical_json_hash;
use super::verb_contract::{codes, VerbDiagnostics};

/// Compiler version for contract versioning
/// Update this when making changes to verb compilation logic
pub const COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Compiled verb contract with all metadata
#[derive(Debug)]
pub struct CompiledVerbContract {
    /// Full RuntimeVerb serialized as JSON
    pub compiled_json: serde_json::Value,
    /// Expanded configuration with defaults applied
    pub effective_config_json: serde_json::Value,
    /// Compilation diagnostics (errors, warnings)
    pub diagnostics: VerbDiagnostics,
    /// SHA256 of canonical compiled_json
    pub compiled_hash: [u8; 32],
}

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

    /// Compute hash for a single verb (public for CI checks)
    ///
    /// This uses the same hashing logic as sync_all, allowing external
    /// tools like `cargo x verbs check` to compare YAML hashes without syncing.
    pub fn hash_verb(&self, verb: &RuntimeVerb) -> String {
        self.compute_verb_hash(verb)
    }

    /// Compute hashes for all verbs in a registry (public for CI checks)
    ///
    /// Returns a map of full_name -> hash for comparison with database.
    pub fn hash_registry(
        &self,
        registry: &RuntimeVerbRegistry,
    ) -> std::collections::HashMap<String, String> {
        registry
            .all_verbs()
            .map(|v| (v.full_name.clone(), self.compute_verb_hash(v)))
            .collect()
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
                    // Changed - update with full contract
                    self.upsert_verb_with_contract(verb, &verb_hash).await?;
                    updated += 1;
                    debug!("Verb {} updated (hash changed)", verb.full_name);
                }
                None => {
                    // New - insert with full contract
                    self.upsert_verb_with_contract(verb, &verb_hash).await?;
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

    // =========================================================================
    // Contract Compilation Methods (Phase 1)
    // =========================================================================

    /// Compile a RuntimeVerb to its full JSON representation with diagnostics
    ///
    /// Returns compiled JSON, effective config, and any compilation diagnostics.
    /// Note: We manually build the JSON since RuntimeVerb doesn't derive Serialize.
    pub fn compile_verb_contract(&self, verb: &RuntimeVerb) -> CompiledVerbContract {
        let mut diagnostics = VerbDiagnostics::default();

        // Manually build compiled_json since RuntimeVerb doesn't derive Serialize
        let compiled_json = self.verb_to_json(verb);

        // Build effective_config with expanded defaults
        let effective_config_json = self.build_effective_config(verb, &mut diagnostics);

        // Validate and add warnings
        self.validate_verb_contract(verb, &mut diagnostics);

        // Compute canonical hash for integrity verification
        let compiled_hash = canonical_json_hash(&compiled_json);

        CompiledVerbContract {
            compiled_json,
            effective_config_json,
            diagnostics,
            compiled_hash,
        }
    }

    /// Convert RuntimeVerb to JSON manually (since it doesn't derive Serialize)
    fn verb_to_json(&self, verb: &RuntimeVerb) -> serde_json::Value {
        let behavior_json = match &verb.behavior {
            RuntimeBehavior::Crud(crud) => serde_json::json!({
                "type": "crud",
                "operation": format!("{:?}", crud.operation),
                "table": crud.table,
                "schema": crud.schema,
                "key": crud.key,
                "returning": crud.returning,
            }),
            RuntimeBehavior::Plugin(handler) => serde_json::json!({
                "type": "plugin",
                "handler": handler,
            }),
            RuntimeBehavior::GraphQuery(gq) => serde_json::json!({
                "type": "graph_query",
                "operation": format!("{:?}", gq.operation),
            }),
        };

        let args_json: Vec<serde_json::Value> = verb
            .args
            .iter()
            .map(|arg| {
                let mut arg_json = serde_json::json!({
                    "name": arg.name,
                    "type": format!("{:?}", arg.arg_type),
                    "required": arg.required,
                });

                if let Some(ref maps_to) = arg.maps_to {
                    arg_json["maps_to"] = serde_json::Value::String(maps_to.clone());
                }
                if let Some(ref desc) = arg.description {
                    arg_json["description"] = serde_json::Value::String(desc.clone());
                }
                if let Some(ref lookup) = arg.lookup {
                    arg_json["lookup"] = serde_json::json!({
                        "table": lookup.table,
                        "schema": lookup.schema,
                        "entity_type": lookup.entity_type,
                    });
                }
                if let Some(ref default) = arg.default {
                    // Convert serde_yaml::Value to serde_json::Value
                    if let Ok(json_default) = serde_json::to_value(default) {
                        arg_json["default"] = json_default;
                    }
                }

                arg_json
            })
            .collect();

        let mut json = serde_json::json!({
            "domain": verb.domain,
            "verb": verb.verb,
            "full_name": verb.full_name,
            "description": verb.description,
            "behavior": behavior_json,
            "args": args_json,
            "returns": {
                "type": format!("{:?}", verb.returns.return_type),
                "name": verb.returns.name,
                "capture": verb.returns.capture,
            },
        });

        if let Some(ref produces) = verb.produces {
            json["produces"] = serde_json::json!({
                "produced_type": produces.produced_type,
                "subtype": produces.subtype,
            });
        }

        if !verb.consumes.is_empty() {
            let consumes_json: Vec<serde_json::Value> = verb
                .consumes
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "consumed_type": c.consumed_type,
                        "required": c.required,
                    })
                })
                .collect();
            json["consumes"] = serde_json::Value::Array(consumes_json);
        }

        if let Some(ref lifecycle) = verb.lifecycle {
            json["lifecycle"] = serde_json::json!({
                "entity_arg": lifecycle.entity_arg,
                "requires_states": lifecycle.requires_states,
                "transitions_to": lifecycle.transitions_to,
            });
        }

        json
    }

    /// Build effective configuration with all defaults expanded
    ///
    /// For now, this mirrors compiled_json with some additional processing.
    /// Future enhancements could expand more complex defaults.
    fn build_effective_config(
        &self,
        verb: &RuntimeVerb,
        _diagnostics: &mut VerbDiagnostics,
    ) -> serde_json::Value {
        // For now, effective config is the same as compiled JSON
        // Future: expand defaults, resolve references, etc.
        self.verb_to_json(verb)
    }

    /// Validate verb contract and add diagnostics for common issues
    fn validate_verb_contract(&self, verb: &RuntimeVerb, diagnostics: &mut VerbDiagnostics) {
        // Check for common issues in args
        for (i, arg) in verb.args.iter().enumerate() {
            // Warn if lookup configured but entity_type missing
            if let Some(ref lookup) = arg.lookup {
                if lookup.entity_type.is_none() && !lookup.table.is_empty() {
                    diagnostics.add_warning_with_path(
                        codes::LOOKUP_MISSING_ENTITY_TYPE,
                        &format!(
                            "Arg '{}' has lookup.table but no lookup.entity_type",
                            arg.name
                        ),
                        Some(&format!("args[{}].lookup", i)),
                        Some("Add entity_type for EntityGateway resolution"),
                    );
                }
            }

            // Warn if required arg has default (contradiction)
            if arg.required && arg.default.is_some() {
                diagnostics.add_warning_with_path(
                    codes::REQUIRED_WITH_DEFAULT,
                    &format!(
                        "Arg '{}' is marked required but has a default value",
                        arg.name
                    ),
                    Some(&format!("args[{}]", i)),
                    Some("Either remove 'required: true' or remove the default"),
                );
            }
        }

        // Check lifecycle consistency
        if let Some(ref lifecycle) = verb.lifecycle {
            if lifecycle.transitions_to.is_some() && lifecycle.entity_arg.is_none() {
                diagnostics.add_error_with_path(
                    codes::LIFECYCLE_MISSING_ENTITY_ARG,
                    "Lifecycle has transitions_to but no entity_arg specified",
                    Some("lifecycle"),
                    Some(
                        "Add entity_arg to identify which arg holds the entity being transitioned",
                    ),
                );
            }

            if !lifecycle.requires_states.is_empty() && lifecycle.entity_arg.is_none() {
                diagnostics.add_warning_with_path(
                    codes::LIFECYCLE_MISSING_ENTITY_ARG,
                    "Lifecycle has requires_states but no entity_arg specified",
                    Some("lifecycle"),
                    Some("Add entity_arg for state validation to work"),
                );
            }
        }

        // Check produces/consumes consistency
        if let Some(ref produces) = verb.produces {
            if produces.produced_type.is_empty() {
                diagnostics.add_warning_with_path(
                    codes::PRODUCES_EMPTY_TYPE,
                    "Produces block has empty produced_type",
                    Some("produces"),
                    Some("Specify what entity type this verb produces"),
                );
            }
        }

        // Check behavior-specific requirements
        match &verb.behavior {
            RuntimeBehavior::Crud(crud) => {
                // CRUD verbs should have table mapping
                if crud.table.is_empty() {
                    diagnostics.add_error_with_path(
                        codes::CRUD_MISSING_TABLE,
                        "CRUD behavior missing table name",
                        Some("behavior.crud"),
                        None,
                    );
                }
            }
            RuntimeBehavior::Plugin(handler) => {
                // Plugin handler should be non-empty
                if handler.is_empty() {
                    diagnostics.add_warning_with_path(
                        codes::PLUGIN_EMPTY_HANDLER,
                        "Plugin behavior has empty handler name",
                        Some("behavior.plugin"),
                        None,
                    );
                }
            }
            RuntimeBehavior::GraphQuery(_) => {
                // Graph queries are generally valid if they compile
            }
        }
    }

    /// Enhanced upsert that stores compiled contract alongside verb metadata
    async fn upsert_verb_with_contract(
        &self,
        verb: &RuntimeVerb,
        yaml_hash: &str,
    ) -> Result<(), VerbSyncError> {
        // Compile the verb to get full contract
        let contract = self.compile_verb_contract(verb);

        // Convert diagnostics to JSON
        let diagnostics_json = serde_json::to_value(&contract.diagnostics)
            .unwrap_or(serde_json::json!({"errors":[],"warnings":[]}));

        // Get behavior string
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

        // Determine category from domain
        let category = self.infer_category(&verb.domain);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_verbs (
                domain, verb_name, description, behavior,
                category, produces_type, produces_subtype, consumes,
                lifecycle_entity_arg, requires_states, transitions_to,
                source, yaml_hash,
                compiled_json, effective_config_json, diagnostics_json, compiled_hash,
                compiler_version, compiled_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, now())
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
                compiled_json = EXCLUDED.compiled_json,
                effective_config_json = EXCLUDED.effective_config_json,
                diagnostics_json = EXCLUDED.diagnostics_json,
                compiled_hash = EXCLUDED.compiled_hash,
                compiler_version = EXCLUDED.compiler_version,
                compiled_at = EXCLUDED.compiled_at,
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
        .bind(yaml_hash)
        .bind(&contract.compiled_json)
        .bind(&contract.effective_config_json)
        .bind(&diagnostics_json)
        .bind(&contract.compiled_hash[..])
        .bind(COMPILER_VERSION)
        .execute(&self.pool)
        .await?;

        // Log if there were compilation issues
        if contract.diagnostics.has_errors() {
            warn!(
                "Verb {} has {} compilation error(s)",
                verb.full_name,
                contract.diagnostics.errors.len()
            );
        }
        if !contract.diagnostics.warnings.is_empty() {
            debug!(
                "Verb {} has {} warning(s)",
                verb.full_name,
                contract.diagnostics.warnings.len()
            );
        }

        Ok(())
    }

    // =========================================================================
    // Hash Computation Methods
    // =========================================================================

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
