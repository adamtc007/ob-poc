//! SRDEF Registry Loader
//!
//! Loads ServiceResourceDefinition (SRDEF) configurations from YAML files
//! and syncs them to the database.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};
use uuid::Uuid;

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

    pub default: Option<JsonValue>,
    pub condition: Option<String>,
    pub description: Option<String>,
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
}

/// Loaded attribute with resolved UUID
#[derive(Debug, Clone)]
pub struct LoadedSrdefAttribute {
    pub attr_id: String,
    pub attr_uuid: Option<Uuid>,
    pub requirement: String,
    pub source_policy: Vec<String>,
    pub constraints: JsonValue,
    pub default_value: Option<JsonValue>,
    pub condition: Option<String>,
    pub description: Option<String>,
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
                        updated_at = NOW()
                    WHERE resource_id = $12
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
                .bind(resource_id)
                .execute(pool)
                .await?;

                result.updated += 1;
                debug!("Updated SRDEF: {}", srdef_id);
            } else {
                // Insert new
                let resource_id = Uuid::now_v7();
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".service_resource_types
                        (resource_id, name, description, owner, resource_code, resource_type,
                         resource_purpose, provisioning_strategy, depends_on,
                         per_market, per_currency, per_counterparty, is_active)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, TRUE)
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
                .execute(pool)
                .await?;

                result.inserted += 1;
                debug!("Inserted SRDEF: {}", srdef_id);
            }

            // Sync attribute requirements
            self.sync_attribute_requirements(pool, srdef_id, &srdef.attributes)
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
    ) -> Result<()> {
        // Get resource_id for this SRDEF
        let resource_id: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE srdef_id = $1"#,
        )
        .bind(srdef_id)
        .fetch_optional(pool)
        .await?;

        let Some((resource_id,)) = resource_id else {
            warn!("Cannot sync attributes: SRDEF {} not found in DB", srdef_id);
            return Ok(());
        };

        for (idx, attr) in attributes.iter().enumerate() {
            // Look up attribute UUID by name/id
            let attr_uuid: Option<(Uuid,)> =
                sqlx::query_as(r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1"#)
                    .bind(&attr.attr_id)
                    .fetch_optional(pool)
                    .await?;

            let Some((attr_uuid,)) = attr_uuid else {
                debug!(
                    "Attribute {} not found in registry, skipping for {}",
                    attr.attr_id, srdef_id
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
                     is_mandatory, display_order)
                VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (resource_id, attribute_id) DO UPDATE SET
                    requirement_type = EXCLUDED.requirement_type,
                    source_policy = EXCLUDED.source_policy,
                    constraints = EXCLUDED.constraints,
                    default_value = EXCLUDED.default_value,
                    condition_expression = EXCLUDED.condition_expression,
                    is_mandatory = EXCLUDED.is_mandatory,
                    display_order = EXCLUDED.display_order
                "#,
            )
            .bind(resource_id)
            .bind(attr_uuid)
            .bind(&attr.requirement)
            .bind(&source_policy)
            .bind(&attr.constraints)
            .bind(attr.default_value.as_ref().map(|v| v.to_string()))
            .bind(&attr.condition)
            .bind(attr.requirement == "required")
            .bind(idx as i32)
            .execute(pool)
            .await?;
        }

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
        };

        let loaded = loader.config_to_loaded("CUSTODY", &config);
        assert_eq!(
            loaded.srdef_id,
            "SRDEF::CUSTODY::Account::custody_securities"
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
