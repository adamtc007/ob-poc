//! SRDEF Registry Loader
//!
//! Loads ServiceResourceDefinition (SRDEF) configurations from YAML files
//! and syncs them to the database.

use anyhow::{Context, Result};
use sem_os_ontology::service_resource_def::{
    ServiceResourceAttributeRequirement, ServiceResourceDefBody, ServiceResourceDimensions,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::sem_reg::ids::object_id_for;
use crate::sem_reg::{ChangeType, ObjectType, SnapshotMeta, SnapshotStore};

// =============================================================================
// YAML CONFIG TYPES
// =============================================================================

/// Root of an SRDEF YAML file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrdefConfigFile {
    pub domain: String,
    pub description: Option<String>,
    pub srdefs: Vec<SrdefConfig>,
}

/// Individual SRDEF configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrdefConfig {
    pub code: String,
    pub name: String,
    pub resource_type: String,
    pub purpose: Option<String>,
    pub provisioning_strategy: String,
    pub owner: String,

    #[serde(default)]
    pub triggered_by_services: Vec<String>,

    #[serde(default)]
    pub attributes: Vec<SrdefAttributeConfig>,

    #[serde(default)]
    pub depends_on: Vec<String>,

    #[serde(default)]
    pub per_market: bool,

    #[serde(default)]
    pub per_currency: bool,

    #[serde(default)]
    pub per_counterparty: bool,

    #[serde(default)]
    pub application_binding: Option<SrdefApplicationBindingConfig>,
}

/// Attribute requirement in SRDEF config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrdefAttributeConfig {
    pub id: String,
    pub requirement: String,

    #[serde(default)]
    pub source_policy: Vec<String>,

    #[serde(default)]
    pub constraints: Option<JsonValue>,

    #[serde(default)]
    pub evidence_policy: Option<JsonValue>,

    pub default: Option<JsonValue>,
    pub condition: Option<String>,
    pub description: Option<String>,
}

/// Optional L4 application binding policy for an SRDEF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrdefApplicationBindingConfig {
    pub application_id: Option<Uuid>,
    pub application_instance_id: Option<Uuid>,

    #[serde(default)]
    pub require_live_binding: bool,
}

// =============================================================================
// LOADED SRDEF (in-memory representation)
// =============================================================================

/// A fully loaded SRDEF with resolved attribute UUIDs
#[derive(Debug, Clone)]
pub struct LoadedSrdef {
    pub srdef_id: String,
    pub code: String,
    pub name: String,
    pub resource_type: String,
    pub purpose: Option<String>,
    pub provisioning_strategy: String,
    pub owner: String,
    pub triggered_by_services: Vec<String>,
    pub attributes: Vec<LoadedSrdefAttribute>,
    pub depends_on: Vec<String>,
    pub per_market: bool,
    pub per_currency: bool,
    pub per_counterparty: bool,
    pub resource_id: Option<Uuid>,
    pub application_binding: Option<LoadedSrdefApplicationBinding>,
}

/// Loaded attribute with resolved UUID
#[derive(Debug, Clone)]
pub struct LoadedSrdefAttribute {
    pub attr_id: String,
    pub attr_uuid: Option<Uuid>,
    pub requirement: String,
    pub source_policy: Vec<String>,
    pub constraints: JsonValue,
    pub evidence_policy: JsonValue,
    pub default_value: Option<JsonValue>,
    pub condition: Option<String>,
    pub description: Option<String>,
}

/// Loaded optional L4 application binding policy for an SRDEF.
#[derive(Debug, Clone)]
pub struct LoadedSrdefApplicationBinding {
    pub application_id: Option<Uuid>,
    pub application_instance_id: Option<Uuid>,
    pub require_live_binding: bool,
}

// =============================================================================
// SRDEF REGISTRY
// =============================================================================

/// In-memory registry of loaded SRDEFs
#[derive(Debug, Default)]
pub struct SrdefRegistry {
    /// SRDEFs indexed by srdef_id
    pub srdefs: HashMap<String, LoadedSrdef>,

    /// Service → SRDEF mapping (which SRDEFs are triggered by which services)
    pub service_triggers: HashMap<String, Vec<String>>,

    /// Dependency graph
    pub dependencies: HashMap<String, Vec<String>>,
}

impl SrdefRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get an SRDEF by ID
    pub fn get(&self, srdef_id: &str) -> Option<&LoadedSrdef> {
        self.srdefs.get(srdef_id)
    }

    /// Get all SRDEFs triggered by a service
    pub fn get_by_service(&self, service_code: &str) -> Vec<&LoadedSrdef> {
        self.service_triggers
            .get(service_code)
            .map(|ids| ids.iter().filter_map(|id| self.srdefs.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get dependencies for an SRDEF
    pub fn get_dependencies(&self, srdef_id: &str) -> Vec<&LoadedSrdef> {
        self.dependencies
            .get(srdef_id)
            .map(|ids| ids.iter().filter_map(|id| self.srdefs.get(id)).collect())
            .unwrap_or_default()
    }

    /// Topological sort of SRDEFs (dependencies first)
    pub fn topo_sort(&self, srdef_ids: &[String]) -> Result<Vec<String>> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut temp_visited = std::collections::HashSet::new();

        for id in srdef_ids {
            self.topo_visit(id, &mut result, &mut visited, &mut temp_visited)?;
        }

        Ok(result)
    }

    fn topo_visit(
        &self,
        id: &str,
        result: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
        temp_visited: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        if temp_visited.contains(id) {
            anyhow::bail!("Circular dependency detected involving {}", id);
        }
        if visited.contains(id) {
            return Ok(());
        }

        temp_visited.insert(id.to_string());

        if let Some(deps) = self.dependencies.get(id) {
            for dep in deps {
                self.topo_visit(dep, result, visited, temp_visited)?;
            }
        }

        temp_visited.remove(id);
        visited.insert(id.to_string());
        result.push(id.to_string());

        Ok(())
    }
}

// =============================================================================
// LOADER
// =============================================================================

/// Loads SRDEF configurations from YAML files
pub struct SrdefLoader {
    config_dir: std::path::PathBuf,
}

impl SrdefLoader {
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
        }
    }

    /// Load all SRDEF configs from the config directory
    pub fn load_all(&self) -> Result<SrdefRegistry> {
        let mut registry = SrdefRegistry::new();

        // Find all YAML files
        let yaml_files: Vec<_> = std::fs::read_dir(&self.config_dir)
            .context("Failed to read SRDEF config directory")?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "yaml" || ext == "yml")
                    .unwrap_or(false)
            })
            .collect();

        info!("Found {} SRDEF config files", yaml_files.len());

        for entry in yaml_files {
            let path = entry.path();
            match self.load_file(&path) {
                Ok(config) => {
                    info!(
                        "Loaded {} SRDEFs from {} (domain: {})",
                        config.srdefs.len(),
                        path.display(),
                        config.domain
                    );

                    for srdef_config in config.srdefs {
                        let loaded = self.config_to_loaded(&config.domain, &srdef_config);
                        let srdef_id = loaded.srdef_id.clone();

                        // Build service trigger index
                        for service in &loaded.triggered_by_services {
                            registry
                                .service_triggers
                                .entry(service.clone())
                                .or_default()
                                .push(srdef_id.clone());
                        }

                        // Build dependency graph
                        registry
                            .dependencies
                            .insert(srdef_id.clone(), loaded.depends_on.clone());

                        registry.srdefs.insert(srdef_id, loaded);
                    }
                }
                Err(e) => {
                    warn!("Failed to load SRDEF config {}: {}", path.display(), e);
                }
            }
        }

        info!(
            "Loaded {} total SRDEFs into registry",
            registry.srdefs.len()
        );
        Ok(registry)
    }

    /// Load a single YAML file
    fn load_file(&self, path: &Path) -> Result<SrdefConfigFile> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))
    }

    /// Convert config to loaded representation
    fn config_to_loaded(&self, _domain: &str, config: &SrdefConfig) -> LoadedSrdef {
        let srdef_id = format!(
            "SRDEF::{}::{}::{}",
            config.owner, config.resource_type, config.code
        );

        let attributes = config
            .attributes
            .iter()
            .map(|attr| LoadedSrdefAttribute {
                attr_id: attr.id.clone(),
                attr_uuid: None, // Will be resolved when syncing to DB
                requirement: attr.requirement.clone(),
                source_policy: attr.source_policy.clone(),
                constraints: attr.constraints.clone().unwrap_or(json!({})),
                evidence_policy: attr.evidence_policy.clone().unwrap_or(json!({})),
                default_value: attr.default.clone(),
                condition: attr.condition.clone(),
                description: attr.description.clone(),
            })
            .collect();

        LoadedSrdef {
            srdef_id,
            code: config.code.clone(),
            name: config.name.clone(),
            resource_type: config.resource_type.clone(),
            purpose: config.purpose.clone(),
            provisioning_strategy: config.provisioning_strategy.clone(),
            owner: config.owner.clone(),
            triggered_by_services: config.triggered_by_services.clone(),
            attributes,
            depends_on: config.depends_on.clone(),
            per_market: config.per_market,
            per_currency: config.per_currency,
            per_counterparty: config.per_counterparty,
            resource_id: None,
            application_binding: config.application_binding.as_ref().map(|binding| {
                LoadedSrdefApplicationBinding {
                    application_id: binding.application_id,
                    application_instance_id: binding.application_instance_id,
                    require_live_binding: binding.require_live_binding,
                }
            }),
        }
    }

    /// Sync loaded SRDEFs to the database
    pub async fn sync_to_database(
        &self,
        pool: &PgPool,
        registry: &SrdefRegistry,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        for (srdef_id, srdef) in &registry.srdefs {
            let srdef_snapshot = srdef_snapshot(srdef);
            let srdef_snapshot_text = serde_json::to_string(&srdef_snapshot)?;
            let srdef_snapshot_hash = blake3::hash(srdef_snapshot_text.as_bytes()).to_hex();
            let owner_principal_fqn = format!("resource_owner:{}", srdef.owner);
            let binding_policy = binding_policy(srdef);
            let srdef_snapshot_id =
                publish_service_resource_def_snapshot(pool, srdef, &binding_policy).await?;
            let l4_binding_required = srdef.provisioning_strategy == "request"
                && srdef
                    .application_binding
                    .as_ref()
                    .is_some_and(|binding| binding.require_live_binding);
            let bound_application_id = srdef
                .application_binding
                .as_ref()
                .and_then(|binding| binding.application_id);
            let bound_application_instance_id = srdef
                .application_binding
                .as_ref()
                .and_then(|binding| binding.application_instance_id);

            self.ensure_resource_owner_principal(pool, srdef).await?;

            // Check if exists
            let existing: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE srdef_id = $1"#,
            )
            .bind(srdef_id)
            .fetch_optional(pool)
            .await?;

            let depends_on_json = json!(srdef.depends_on);

            if let Some((resource_id,)) = existing {
                // Update existing
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".service_resource_types
                    SET name = $1,
                        description = $2,
                        owner = $3,
                        resource_code = $4,
                        resource_type = $5,
                        resource_purpose = $6,
                        provisioning_strategy = $7,
                        depends_on = $8,
                        per_market = $9,
                        per_currency = $10,
                        per_counterparty = $11,
                        owner_principal_fqn = $12,
                        srdef_lineage = 'yaml',
                        srdef_snapshot = $13,
                        srdef_snapshot_hash = $14,
                        srdef_snapshot_id = $15,
                        binding_policy = $16,
                        l4_binding_required = $17,
                        bound_application_id = $18,
                        bound_application_instance_id = $19,
                        srdef_synced_at = NOW(),
                        updated_at = NOW()
                    WHERE resource_id = $20
                    "#,
                )
                .bind(&srdef.name)
                .bind(&srdef.purpose)
                .bind(&srdef.owner)
                .bind(&srdef.code)
                .bind(&srdef.resource_type)
                .bind(&srdef.purpose)
                .bind(&srdef.provisioning_strategy)
                .bind(&depends_on_json)
                .bind(srdef.per_market)
                .bind(srdef.per_currency)
                .bind(srdef.per_counterparty)
                .bind(&owner_principal_fqn)
                .bind(&srdef_snapshot)
                .bind(srdef_snapshot_hash.as_str())
                .bind(srdef_snapshot_id)
                .bind(&binding_policy)
                .bind(l4_binding_required)
                .bind(bound_application_id)
                .bind(bound_application_instance_id)
                .bind(resource_id)
                .execute(pool)
                .await?;

                result.updated += 1;
                debug!("Updated SRDEF: {}", srdef_id);
            } else {
                // Insert new
                let resource_id = Uuid::new_v4();
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".service_resource_types
                        (resource_id, name, description, owner, resource_code, resource_type,
                         resource_purpose, provisioning_strategy, depends_on,
                         per_market, per_currency, per_counterparty, is_active,
                         owner_principal_fqn, srdef_lineage, srdef_snapshot,
                         srdef_snapshot_hash, srdef_snapshot_id, binding_policy,
                         l4_binding_required, bound_application_id, bound_application_instance_id,
                         srdef_synced_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, TRUE,
                            $13, 'yaml', $14, $15, $16, $17, $18, $19, $20, NOW())
                    "#,
                )
                .bind(resource_id)
                .bind(&srdef.name)
                .bind(&srdef.purpose)
                .bind(&srdef.owner)
                .bind(&srdef.code)
                .bind(&srdef.resource_type)
                .bind(&srdef.purpose)
                .bind(&srdef.provisioning_strategy)
                .bind(&depends_on_json)
                .bind(srdef.per_market)
                .bind(srdef.per_currency)
                .bind(srdef.per_counterparty)
                .bind(&owner_principal_fqn)
                .bind(&srdef_snapshot)
                .bind(srdef_snapshot_hash.as_str())
                .bind(srdef_snapshot_id)
                .bind(&binding_policy)
                .bind(l4_binding_required)
                .bind(bound_application_id)
                .bind(bound_application_instance_id)
                .execute(pool)
                .await?;

                result.inserted += 1;
                debug!("Inserted SRDEF: {}", srdef_id);
            }

            // Sync attribute requirements
            let attr_summary = self
                .sync_attribute_requirements(pool, srdef_id, &srdef.attributes)
                .await?;
            let lifecycle_status =
                if attr_summary.missing_attribute_defs > 0 || attr_summary.conflicts > 0 {
                    "gaps_found"
                } else {
                    "complete"
                };
            sqlx::query(
                r#"
                UPDATE "ob-poc".service_resource_types
                SET lifecycle_status = $1,
                    attribute_gap_count = $2,
                    attribute_conflict_count = $3,
                    updated_at = NOW()
                WHERE srdef_id = $4
                "#,
            )
            .bind(lifecycle_status)
            .bind(attr_summary.missing_attribute_defs as i32)
            .bind(attr_summary.conflicts as i32)
            .bind(srdef_id)
            .execute(pool)
            .await?;
        }

        info!(
            "SRDEF sync complete: {} inserted, {} updated",
            result.inserted, result.updated
        );
        Ok(result)
    }

    /// Sync attribute requirements for an SRDEF
    async fn sync_attribute_requirements(
        &self,
        pool: &PgPool,
        srdef_id: &str,
        attributes: &[LoadedSrdefAttribute],
    ) -> Result<AttributeSyncSummary> {
        let mut summary = AttributeSyncSummary::default();
        let mut seen = HashSet::new();
        // Get resource_id for this SRDEF
        let resource_id: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE srdef_id = $1"#,
        )
        .bind(srdef_id)
        .fetch_optional(pool)
        .await?;

        let Some((resource_id,)) = resource_id else {
            warn!("Cannot sync attributes: SRDEF {} not found in DB", srdef_id);
            return Ok(summary);
        };

        for (idx, attr) in attributes.iter().enumerate() {
            if !seen.insert(attr.attr_id.clone()) {
                summary.conflicts += 1;
                warn!(
                    attribute_id = %attr.attr_id,
                    srdef_id = %srdef_id,
                    "Duplicate attribute requirement in SRDEF, keeping first occurrence",
                );
                continue;
            }

            // Look up attribute UUID by name/id
            let attr_uuid: Option<Uuid> =
                sqlx::query_scalar::<_, Option<Uuid>>(r#"
                    SELECT COALESCE(
                        (SELECT object_id FROM sem_reg.v_active_attribute_defs WHERE fqn = $1 LIMIT 1),
                        (SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1)
                    ) AS uuid
                "#)
                    .bind(&attr.attr_id)
                    .fetch_one(pool)
                    .await?;

            let Some(attr_uuid) = attr_uuid else {
                summary.missing_attribute_defs += 1;
                warn!(
                    attribute_id = %attr.attr_id,
                    srdef_id = %srdef_id,
                    "Attribute not found in SemOS or registry, skipping",
                );
                continue;
            };

            let source_policy = json!(attr.source_policy);

            // Upsert attribute requirement
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".resource_attribute_requirements
                    (requirement_id, resource_id, attribute_id, requirement_type,
                     source_policy, constraints, default_value, condition_expression,
                     evidence_policy, is_mandatory, display_order, requirement_status,
                     conflict_reason)
                VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9,
                        $10, 'synced', NULL)
                ON CONFLICT (resource_id, attribute_id) DO UPDATE SET
                    requirement_type = EXCLUDED.requirement_type,
                    source_policy = EXCLUDED.source_policy,
                    constraints = EXCLUDED.constraints,
                    default_value = EXCLUDED.default_value,
                    condition_expression = EXCLUDED.condition_expression,
                    evidence_policy = EXCLUDED.evidence_policy,
                    is_mandatory = EXCLUDED.is_mandatory,
                    display_order = EXCLUDED.display_order,
                    requirement_status = EXCLUDED.requirement_status,
                    conflict_reason = EXCLUDED.conflict_reason
                "#,
            )
            .bind(resource_id)
            .bind(attr_uuid)
            .bind(&attr.requirement)
            .bind(&source_policy)
            .bind(&attr.constraints)
            .bind(attr.default_value.as_ref().map(|v| v.to_string()))
            .bind(&attr.condition)
            .bind(&attr.evidence_policy)
            .bind(attr.requirement == "required")
            .bind(idx as i32)
            .execute(pool)
            .await?;
            summary.synced += 1;
        }

        Ok(summary)
    }

    async fn ensure_resource_owner_principal(
        &self,
        pool: &PgPool,
        srdef: &LoadedSrdef,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".resource_owner_principals
                (owner_principal_fqn, owner_system, display_name,
                 principal_kind, principal_capabilities, dispatch_enabled)
            VALUES ($1, $2, $2, 'resource_owner', '["resource_owner"]'::jsonb, TRUE)
            ON CONFLICT (owner_principal_fqn) DO UPDATE
            SET owner_system = EXCLUDED.owner_system,
                display_name = COALESCE("ob-poc".resource_owner_principals.display_name, EXCLUDED.display_name),
                principal_kind = 'resource_owner',
                principal_capabilities = EXCLUDED.principal_capabilities,
                updated_at = NOW()
            "#,
        )
        .bind(format!("resource_owner:{}", srdef.owner))
        .bind(&srdef.owner)
        .execute(pool)
        .await?;
        Ok(())
    }
}

/// Result of syncing SRDEFs to database
#[derive(Debug, Default)]
pub struct SyncResult {
    pub inserted: usize,
    pub updated: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Default)]
struct AttributeSyncSummary {
    synced: usize,
    missing_attribute_defs: usize,
    conflicts: usize,
}

fn binding_policy(srdef: &LoadedSrdef) -> JsonValue {
    srdef
        .application_binding
        .as_ref()
        .map(|binding| {
            json!({
                "application_id": binding.application_id,
                "application_instance_id": binding.application_instance_id,
                "require_live_binding": binding.require_live_binding,
            })
        })
        .unwrap_or_else(|| json!({}))
}

fn srdef_snapshot(srdef: &LoadedSrdef) -> JsonValue {
    json!({
        "srdef_id": srdef.srdef_id,
        "code": srdef.code,
        "name": srdef.name,
        "resource_type": srdef.resource_type,
        "purpose": srdef.purpose,
        "provisioning_strategy": srdef.provisioning_strategy,
        "owner": srdef.owner,
        "triggered_by_services": srdef.triggered_by_services,
        "attributes": srdef.attributes.iter().map(|attr| {
            json!({
                "id": attr.attr_id,
                "requirement": attr.requirement,
                "source_policy": attr.source_policy,
                "constraints": attr.constraints,
                "evidence_policy": attr.evidence_policy,
                "default": attr.default_value,
                "condition": attr.condition,
                "description": attr.description,
            })
        }).collect::<Vec<_>>(),
        "depends_on": srdef.depends_on,
        "per_market": srdef.per_market,
        "per_currency": srdef.per_currency,
        "per_counterparty": srdef.per_counterparty,
        "application_binding": binding_policy(srdef),
    })
}

async fn publish_service_resource_def_snapshot(
    pool: &PgPool,
    srdef: &LoadedSrdef,
    binding_policy: &JsonValue,
) -> Result<Uuid> {
    let body = ServiceResourceDefBody {
        srdef_id: srdef.srdef_id.clone(),
        code: srdef.code.clone(),
        name: srdef.name.clone(),
        resource_type: srdef.resource_type.clone(),
        purpose: srdef.purpose.clone(),
        provisioning_strategy: srdef.provisioning_strategy.clone(),
        owner_principal_fqn: format!("resource_owner:{}", srdef.owner),
        triggered_by_services: srdef.triggered_by_services.clone(),
        attributes: srdef
            .attributes
            .iter()
            .map(|attr| ServiceResourceAttributeRequirement {
                attr_id: attr.attr_id.clone(),
                requirement: attr.requirement.clone(),
                source_policy: attr.source_policy.clone(),
                constraints: attr.constraints.clone(),
                evidence_policy: attr.evidence_policy.clone(),
                default_value: attr.default_value.clone(),
                condition: attr.condition.clone(),
                description: attr.description.clone(),
            })
            .collect(),
        depends_on: srdef.depends_on.clone(),
        dimensions: ServiceResourceDimensions {
            per_market: srdef.per_market,
            per_currency: srdef.per_currency,
            per_counterparty: srdef.per_counterparty,
        },
        binding_policy: binding_policy.clone(),
    };
    let definition = serde_json::to_value(&body)?;
    let object_id = object_id_for(ObjectType::ServiceResourceDef, &srdef.srdef_id);

    if let Some(active) =
        SnapshotStore::resolve_active(pool, ObjectType::ServiceResourceDef, object_id).await?
    {
        if active.definition == definition {
            return Ok(active.snapshot_id);
        }
        SnapshotStore::supersede_snapshot(pool, active.snapshot_id).await?;
        let mut meta = SnapshotMeta::new_operational(
            ObjectType::ServiceResourceDef,
            object_id,
            "service-resource.sync-definitions",
        );
        meta.predecessor_id = Some(active.snapshot_id);
        meta.change_type = ChangeType::NonBreaking;
        return SnapshotStore::insert_snapshot(pool, &meta, &definition, None).await;
    }

    let meta = SnapshotMeta::new_operational(
        ObjectType::ServiceResourceDef,
        object_id,
        "service-resource.sync-definitions",
    );
    SnapshotStore::insert_snapshot(pool, &meta, &definition, None).await
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Load SRDEFs from the default config directory
pub fn load_srdefs_from_config() -> Result<SrdefRegistry> {
    let config_dir =
        std::env::var("SRDEF_CONFIG_DIR").unwrap_or_else(|_| "config/srdefs".to_string());

    let loader = SrdefLoader::new(&config_dir);
    loader.load_all()
}

/// Load and sync SRDEFs to database
pub async fn load_and_sync_srdefs(pool: &PgPool) -> Result<(SrdefRegistry, SyncResult)> {
    let config_dir =
        std::env::var("SRDEF_CONFIG_DIR").unwrap_or_else(|_| "config/srdefs".to_string());

    let loader = SrdefLoader::new(&config_dir);
    let registry = loader.load_all()?;
    let sync_result = loader.sync_to_database(pool, &registry).await?;

    Ok((registry, sync_result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_custody_srdef() {
        let yaml = r#"
domain: CUSTODY
description: Test
srdefs:
  - code: test_account
    name: Test Account
    resource_type: Account
    provisioning_strategy: create
    owner: CUSTODY
    triggered_by_services:
      - TEST_SERVICE
    attributes:
      - id: test_attr
        requirement: required
        source_policy: [cbu, manual]
"#;

        let config: SrdefConfigFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.domain, "CUSTODY");
        assert_eq!(config.srdefs.len(), 1);
        assert_eq!(config.srdefs[0].code, "test_account");
        assert_eq!(config.srdefs[0].attributes.len(), 1);
    }

    #[test]
    fn test_srdef_id_generation() {
        let loader = SrdefLoader::new("/tmp");
        let config = SrdefConfig {
            code: "custody_securities".to_string(),
            name: "Securities Custody Account".to_string(),
            resource_type: "Account".to_string(),
            purpose: Some("Hold securities".to_string()),
            provisioning_strategy: "request".to_string(),
            owner: "CUSTODY".to_string(),
            triggered_by_services: vec![],
            attributes: vec![],
            depends_on: vec![],
            per_market: true,
            per_currency: false,
            per_counterparty: false,
            application_binding: None,
        };

        let loaded = loader.config_to_loaded("CUSTODY", &config);
        assert_eq!(
            loaded.srdef_id,
            "SRDEF::CUSTODY::Account::custody_securities"
        );
    }

    #[test]
    fn test_parse_evidence_and_application_binding() {
        let yaml = r#"
domain: CUSTODY
srdefs:
  - code: governed_account
    name: Governed Account
    resource_type: Account
    provisioning_strategy: request
    owner: CUSTODY
    application_binding:
      application_id: "018f79f2-148e-7f2a-9e0d-7fa58a1d2000"
      application_instance_id: "018f79f2-148e-7f2a-9e0d-7fa58a1d2001"
      require_live_binding: true
    attributes:
      - id: tax_jurisdiction
        requirement: required
        source_policy: [entity]
        evidence_policy:
          requires_document: true
"#;

        let config: SrdefConfigFile = serde_yaml::from_str(yaml).unwrap();
        let srdef = &config.srdefs[0];
        assert!(srdef
            .application_binding
            .as_ref()
            .is_some_and(|binding| binding.require_live_binding));
        assert_eq!(
            srdef.attributes[0]
                .evidence_policy
                .as_ref()
                .and_then(|policy| policy.get("requires_document"))
                .and_then(JsonValue::as_bool),
            Some(true)
        );
    }

    #[test]
    fn test_topo_sort() {
        let mut registry = SrdefRegistry::new();

        // A depends on B, B depends on C
        registry
            .dependencies
            .insert("A".to_string(), vec!["B".to_string()]);
        registry
            .dependencies
            .insert("B".to_string(), vec!["C".to_string()]);
        registry.dependencies.insert("C".to_string(), vec![]);

        let sorted = registry.topo_sort(&["A".to_string()]).unwrap();
        assert_eq!(sorted, vec!["C", "B", "A"]);
    }
}
