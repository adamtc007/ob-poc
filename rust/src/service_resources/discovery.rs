//! Resource Discovery Engine
//!
//! Derives required SRDEFs from ServiceIntents.
//! The discovery engine applies rules to determine which resources are needed
//! based on what services a CBU has subscribed to.

use anyhow::{Context, Result};
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};
use uuid::Uuid;

use super::service::ServiceResourcePipelineService;
use super::srdef_loader::SrdefRegistry;
use super::types::*;

// =============================================================================
// DISCOVERY ENGINE
// =============================================================================

/// Engine for discovering required SRDEFs from service intents
pub struct ResourceDiscoveryEngine<'a> {
    pool: &'a PgPool,
    service: ServiceResourcePipelineService,
    registry: &'a SrdefRegistry,
}

impl<'a> ResourceDiscoveryEngine<'a> {
    pub fn new(pool: &'a PgPool, registry: &'a SrdefRegistry) -> Self {
        Self {
            pool,
            service: ServiceResourcePipelineService::new(pool.clone()),
            registry,
        }
    }

    /// Discover required SRDEFs for a CBU based on their service intents
    pub async fn discover_for_cbu(&self, cbu_id: Uuid) -> Result<DiscoveryResult> {
        info!("Starting resource discovery for CBU {}", cbu_id);

        // Get active service intents
        let intents = self.service.get_service_intents(cbu_id).await?;
        if intents.is_empty() {
            info!("No active service intents for CBU {}", cbu_id);
            return Ok(DiscoveryResult::default());
        }

        info!("Found {} active service intents", intents.len());

        // Collect all discovered SRDEFs
        let mut discovered: HashMap<String, DiscoveredSrdefInfo> = HashMap::new();

        for intent in &intents {
            // Get service code
            let service_code = self.get_service_code(intent.service_id).await?;

            // Find SRDEFs triggered by this service
            let triggered_srdefs = self.registry.get_by_service(&service_code);

            debug!(
                "Service {} triggers {} SRDEFs",
                service_code,
                triggered_srdefs.len()
            );

            for srdef in triggered_srdefs {
                // Check if this SRDEF is parameterized (per-market, per-currency, per-counterparty)
                let params = self.extract_parameters(&srdef.srdef_id, srdef, &intent.options)?;

                for param_set in params {
                    let key = format!("{}:{}", srdef.srdef_id, serde_json::to_string(&param_set)?);

                    discovered
                        .entry(key)
                        .and_modify(|info| {
                            info.triggered_by.push(intent.intent_id);
                        })
                        .or_insert(DiscoveredSrdefInfo {
                            srdef_id: srdef.srdef_id.clone(),
                            parameters: param_set,
                            triggered_by: vec![intent.intent_id],
                            discovery_rule: format!("service_trigger:{}", service_code),
                        });
                }
            }
        }

        // Add transitive dependencies
        let srdef_ids: Vec<String> = discovered.values().map(|d| d.srdef_id.clone()).collect();
        let sorted = self.registry.topo_sort(&srdef_ids)?;

        for srdef_id in &sorted {
            if !discovered.values().any(|d| &d.srdef_id == srdef_id) {
                // This is a dependency that wasn't directly discovered
                discovered.insert(
                    srdef_id.clone(),
                    DiscoveredSrdefInfo {
                        srdef_id: srdef_id.clone(),
                        parameters: json!({}),
                        triggered_by: vec![],
                        discovery_rule: "dependency".to_string(),
                    },
                );
            }
        }

        // Persist discoveries
        let mut result = DiscoveryResult::default();
        for info in discovered.values() {
            let discovery = NewSrdefDiscovery {
                cbu_id,
                srdef_id: info.srdef_id.clone(),
                resource_type_id: self.get_resource_type_id(&info.srdef_id).await?,
                triggered_by_intents: info.triggered_by.clone(),
                discovery_rule: info.discovery_rule.clone(),
                discovery_reason: json!({
                    "triggered_by_count": info.triggered_by.len(),
                    "rule": info.discovery_rule,
                }),
                parameters: Some(info.parameters.clone()),
            };

            self.service.record_discovery(&discovery).await?;
            result.discovered.push(info.clone());
        }

        result.total_discovered = result.discovered.len();
        info!(
            "Discovery complete for CBU {}: {} SRDEFs",
            cbu_id, result.total_discovered
        );

        Ok(result)
    }

    /// Get service code by ID
    async fn get_service_code(&self, service_id: Uuid) -> Result<String> {
        let code: Option<(String,)> = sqlx::query_as(
            r#"SELECT COALESCE(service_code, name) FROM "ob-poc".services WHERE service_id = $1"#,
        )
        .bind(service_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(code.map(|(c,)| c).unwrap_or_else(|| service_id.to_string()))
    }

    /// Get resource type ID by SRDEF ID
    async fn get_resource_type_id(&self, srdef_id: &str) -> Result<Option<Uuid>> {
        let id: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE srdef_id = $1"#,
        )
        .bind(srdef_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(id.map(|(id,)| id))
    }

    /// Extract parameters for parameterized SRDEFs
    fn extract_parameters(
        &self,
        _srdef_id: &str,
        srdef: &super::srdef_loader::LoadedSrdef,
        options: &JsonValue,
    ) -> Result<Vec<JsonValue>> {
        let mut params = Vec::new();

        if srdef.per_market {
            // Extract markets from options
            if let Some(markets) = options.get("markets").and_then(|m| m.as_array()) {
                for market in markets {
                    params.push(json!({ "market": market }));
                }
            } else {
                // Default: single instance without market parameter
                params.push(json!({}));
            }
        } else if srdef.per_currency {
            // Extract currencies from options
            if let Some(currencies) = options.get("currencies").and_then(|c| c.as_array()) {
                for currency in currencies {
                    params.push(json!({ "currency": currency }));
                }
            } else if let Some(currency) = options.get("settlement_currency") {
                params.push(json!({ "currency": currency }));
            } else {
                params.push(json!({}));
            }
        } else if srdef.per_counterparty {
            // Extract counterparties from options
            if let Some(counterparties) = options.get("counterparties").and_then(|c| c.as_array()) {
                for cp in counterparties {
                    params.push(json!({ "counterparty": cp }));
                }
            } else {
                params.push(json!({}));
            }
        } else {
            // Non-parameterized: single instance
            params.push(json!({}));
        }

        Ok(params)
    }
}

/// Information about a discovered SRDEF
#[derive(Debug, Clone)]
pub struct DiscoveredSrdefInfo {
    pub srdef_id: String,
    pub parameters: JsonValue,
    pub triggered_by: Vec<Uuid>,
    pub discovery_rule: String,
}

/// Result of discovery operation
#[derive(Debug, Default)]
pub struct DiscoveryResult {
    pub discovered: Vec<DiscoveredSrdefInfo>,
    pub total_discovered: usize,
}

// =============================================================================
// ATTRIBUTE ROLLUP ENGINE
// =============================================================================

/// Engine for rolling up attribute requirements across discovered SRDEFs
pub struct AttributeRollupEngine<'a> {
    pool: &'a PgPool,
    service: ServiceResourcePipelineService,
}

impl<'a> AttributeRollupEngine<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            service: ServiceResourcePipelineService::new(pool.clone()),
        }
    }

    /// Build unified attribute requirements for a CBU
    pub async fn rollup_for_cbu(&self, cbu_id: Uuid) -> Result<RollupResult> {
        info!("Starting attribute rollup for CBU {}", cbu_id);

        // Get active discoveries
        let discoveries = self.service.get_active_discoveries(cbu_id).await?;
        if discoveries.is_empty() {
            info!("No active discoveries for CBU {}", cbu_id);
            return Ok(RollupResult::default());
        }

        info!(
            "Rolling up attributes from {} discovered SRDEFs",
            discoveries.len()
        );

        // Collect all attribute requirements
        let mut attr_map: HashMap<Uuid, AttributeRollupInfo> = HashMap::new();

        for discovery in &discoveries {
            // Get attribute requirements for this SRDEF
            let requirements = self
                .get_srdef_attribute_requirements(&discovery.srdef_id)
                .await?;

            for req in requirements {
                attr_map
                    .entry(req.attr_id)
                    .and_modify(|info| {
                        // Merge: required dominates optional
                        if req.requirement == "required" {
                            info.requirement_strength = "required".to_string();
                        }
                        info.required_by_srdefs.push(discovery.srdef_id.clone());

                        // Merge constraints (simplified - just combine)
                        if let Some(constraints) = &req.constraints {
                            if info.merged_constraints.is_null() {
                                info.merged_constraints = constraints.clone();
                            }
                            // TODO: Proper constraint merging with conflict detection
                        }

                        // Merge source policy (union)
                        for source in &req.source_policy {
                            if !info.source_policy.contains(source) {
                                info.source_policy.push(source.clone());
                            }
                        }
                    })
                    .or_insert(AttributeRollupInfo {
                        attr_id: req.attr_id,
                        requirement_strength: req.requirement.clone(),
                        merged_constraints: req.constraints.clone().unwrap_or(json!({})),
                        source_policy: req.source_policy.clone(),
                        required_by_srdefs: vec![discovery.srdef_id.clone()],
                        conflict: None,
                    });
            }
        }

        // Clear existing and write new
        self.service.clear_unified_attr_requirements(cbu_id).await?;

        let mut result = RollupResult::default();
        for info in attr_map.values() {
            // Determine preferred source
            let preferred = info.source_policy.first().cloned();

            self.service
                .upsert_unified_attr_requirement(
                    cbu_id,
                    info.attr_id,
                    &info.requirement_strength,
                    &info.merged_constraints,
                    preferred.as_deref(),
                    &info.required_by_srdefs,
                    info.conflict.as_ref(),
                )
                .await?;

            if info.requirement_strength == "required" {
                result.required_count += 1;
            } else {
                result.optional_count += 1;
            }
        }

        result.total_attributes = attr_map.len();
        result.conflict_count = attr_map.values().filter(|i| i.conflict.is_some()).count();

        info!(
            "Rollup complete for CBU {}: {} attrs ({} required, {} optional, {} conflicts)",
            cbu_id,
            result.total_attributes,
            result.required_count,
            result.optional_count,
            result.conflict_count
        );

        Ok(result)
    }

    /// Get attribute requirements for an SRDEF
    async fn get_srdef_attribute_requirements(
        &self,
        srdef_id: &str,
    ) -> Result<Vec<SrdefAttrRequirement>> {
        let rows = sqlx::query_as::<_, SrdefAttrRequirementRow>(
            r#"
            SELECT
                rar.attribute_id,
                rar.requirement_type,
                rar.source_policy,
                rar.constraints,
                rar.condition_expression
            FROM "ob-poc".resource_attribute_requirements rar
            JOIN "ob-poc".service_resource_types srt ON srt.resource_id = rar.resource_id
            WHERE srt.srdef_id = $1
            ORDER BY rar.display_order
            "#,
        )
        .bind(srdef_id)
        .fetch_all(self.pool)
        .await
        .context("Failed to get SRDEF attribute requirements")?;

        Ok(rows
            .into_iter()
            .map(|r| SrdefAttrRequirement {
                attr_id: r.attribute_id,
                requirement: r.requirement_type.unwrap_or_else(|| "required".to_string()),
                source_policy: r
                    .source_policy
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_else(|| vec!["manual".to_string()]),
                constraints: r.constraints,
            })
            .collect())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SrdefAttrRequirementRow {
    attribute_id: Uuid,
    requirement_type: Option<String>,
    source_policy: Option<JsonValue>,
    constraints: Option<JsonValue>,
    #[allow(dead_code)] // Populated by sqlx FromRow
    condition_expression: Option<String>,
}

#[derive(Debug)]
struct SrdefAttrRequirement {
    attr_id: Uuid,
    requirement: String,
    source_policy: Vec<String>,
    constraints: Option<JsonValue>,
}

/// Information about a rolled-up attribute
#[derive(Debug)]
struct AttributeRollupInfo {
    attr_id: Uuid,
    requirement_strength: String,
    merged_constraints: JsonValue,
    source_policy: Vec<String>,
    required_by_srdefs: Vec<String>,
    conflict: Option<JsonValue>,
}

/// Result of rollup operation
#[derive(Debug, Default)]
pub struct RollupResult {
    pub total_attributes: usize,
    pub required_count: usize,
    pub optional_count: usize,
    pub conflict_count: usize,
}

// =============================================================================
// POPULATION ENGINE
// =============================================================================

/// Engine for populating attribute values from various sources
pub struct PopulationEngine<'a> {
    pool: &'a PgPool,
    service: ServiceResourcePipelineService,
}

impl<'a> PopulationEngine<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            service: ServiceResourcePipelineService::new(pool.clone()),
        }
    }

    /// Attempt to populate missing attribute values for a CBU
    pub async fn populate_for_cbu(&self, cbu_id: Uuid) -> Result<PopulationResult> {
        info!("Starting attribute population for CBU {}", cbu_id);

        // Get unified requirements
        let requirements = self.service.get_unified_attr_requirements(cbu_id).await?;

        // Get existing values
        let existing_values = self.service.get_cbu_attr_values(cbu_id).await?;
        let existing_ids: HashSet<Uuid> = existing_values.iter().map(|v| v.attr_id).collect();

        let mut result = PopulationResult::default();

        for req in &requirements {
            if existing_ids.contains(&req.attr_id) {
                result.already_populated += 1;
                continue;
            }

            // Try to populate from sources in order
            let source_policy: Vec<String> = serde_json::from_value(req.required_by_srdefs.clone())
                .ok()
                .and_then(|_: Vec<String>| req.preferred_source.as_ref().map(|s| vec![s.clone()]))
                .unwrap_or_else(|| {
                    vec![
                        "derived".to_string(),
                        "entity".to_string(),
                        "cbu".to_string(),
                    ]
                });

            for source in &source_policy {
                match self.try_populate(cbu_id, req.attr_id, source).await? {
                    Some(value) => {
                        let input = SetCbuAttrValue {
                            cbu_id,
                            attr_id: req.attr_id,
                            value,
                            source: parse_source(source),
                            evidence_refs: None,
                            explain_refs: Some(vec![ExplainRef {
                                rule: format!("auto_populate:{}", source),
                                input: None,
                                output: None,
                            }]),
                        };
                        self.service.set_cbu_attr_value(&input).await?;
                        result.populated += 1;
                        break;
                    }
                    None => continue,
                }
            }

            if !existing_ids.contains(&req.attr_id) {
                result.still_missing += 1;
            }
        }

        info!(
            "Population complete for CBU {}: {} populated, {} already had values, {} still missing",
            cbu_id, result.populated, result.already_populated, result.still_missing
        );

        Ok(result)
    }

    /// Try to populate a single attribute from a source
    async fn try_populate(
        &self,
        cbu_id: Uuid,
        attr_id: Uuid,
        source: &str,
    ) -> Result<Option<JsonValue>> {
        match source {
            "derived" => self.try_derive(cbu_id, attr_id).await,
            "entity" => self.try_from_entity(cbu_id, attr_id).await,
            "cbu" => self.try_from_cbu(cbu_id, attr_id).await,
            "document" => Ok(None), // TODO: Document extraction
            "external" => Ok(None), // TODO: External API
            _ => Ok(None),
        }
    }

    /// Try to derive a value from other data
    async fn try_derive(&self, _cbu_id: Uuid, _attr_id: Uuid) -> Result<Option<JsonValue>> {
        // TODO: Implement derivation rules
        Ok(None)
    }

    /// Try to get value from entity data
    async fn try_from_entity(&self, cbu_id: Uuid, attr_id: Uuid) -> Result<Option<JsonValue>> {
        // Get attribute code
        let attr_code: Option<(String,)> =
            sqlx::query_as(r#"SELECT id FROM "ob-poc".attribute_registry WHERE uuid = $1"#)
                .bind(attr_id)
                .fetch_optional(self.pool)
                .await?;

        let Some((attr_code,)) = attr_code else {
            return Ok(None);
        };

        // Try to find in entity attributes (simplified - would need proper mapping)
        // For now, check if there's a matching attribute in attribute_values_typed
        let value: Option<(JsonValue,)> = sqlx::query_as(
            r#"
            SELECT value_json
            FROM "ob-poc".attribute_values_typed avt
            JOIN "ob-poc".cbus c ON c.commercial_client_entity_id = avt.entity_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = avt.attribute_id
            WHERE c.cbu_id = $1 AND ar.id = $2
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .bind(&attr_code)
        .fetch_optional(self.pool)
        .await?;

        Ok(value.map(|(v,)| v))
    }

    /// Try to get value from CBU data
    async fn try_from_cbu(&self, cbu_id: Uuid, attr_id: Uuid) -> Result<Option<JsonValue>> {
        // Get attribute code
        let attr_code: Option<(String,)> =
            sqlx::query_as(r#"SELECT id FROM "ob-poc".attribute_registry WHERE uuid = $1"#)
                .bind(attr_id)
                .fetch_optional(self.pool)
                .await?;

        let Some((attr_code,)) = attr_code else {
            return Ok(None);
        };

        // Map common attribute codes to CBU fields
        match attr_code.as_str() {
            "tax_jurisdiction" | "jurisdiction" => {
                let value: Option<(String,)> =
                    sqlx::query_as(r#"SELECT jurisdiction FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                        .bind(cbu_id)
                        .fetch_optional(self.pool)
                        .await?;

                Ok(value.map(|(v,)| json!(v)))
            }
            _ => Ok(None),
        }
    }
}

fn parse_source(s: &str) -> AttributeSource {
    match s {
        "derived" => AttributeSource::Derived,
        "entity" => AttributeSource::Entity,
        "cbu" => AttributeSource::Cbu,
        "document" => AttributeSource::Document,
        "manual" => AttributeSource::Manual,
        "external" => AttributeSource::External,
        _ => AttributeSource::Manual,
    }
}

/// Result of population operation
#[derive(Debug, Default)]
pub struct PopulationResult {
    pub populated: usize,
    pub already_populated: usize,
    pub still_missing: usize,
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Run the full discovery + rollup + populate pipeline for a CBU
pub async fn run_discovery_pipeline(
    pool: &PgPool,
    registry: &SrdefRegistry,
    cbu_id: Uuid,
) -> Result<PipelineResult> {
    // Discovery
    let discovery_engine = ResourceDiscoveryEngine::new(pool, registry);
    let discovery = discovery_engine.discover_for_cbu(cbu_id).await?;

    // Rollup
    let rollup_engine = AttributeRollupEngine::new(pool);
    let rollup = rollup_engine.rollup_for_cbu(cbu_id).await?;

    // Population
    let population_engine = PopulationEngine::new(pool);
    let population = population_engine.populate_for_cbu(cbu_id).await?;

    Ok(PipelineResult {
        cbu_id,
        srdefs_discovered: discovery.total_discovered,
        attrs_rolled_up: rollup.total_attributes,
        attrs_populated: population.populated,
        attrs_missing: population.still_missing,
    })
}

/// Result of running the full pipeline
#[derive(Debug)]
pub struct PipelineResult {
    pub cbu_id: Uuid,
    pub srdefs_discovered: usize,
    pub attrs_rolled_up: usize,
    pub attrs_populated: usize,
    pub attrs_missing: usize,
}
