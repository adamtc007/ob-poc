//! Resource Discovery Engine
//!
//! Derives required SRDEFs from ServiceIntents.
//! The discovery engine applies rules to determine which resources are needed
//! based on what services a CBU has subscribed to.

use anyhow::{Context, Result};
use serde_json::{json, Value as JsonValue};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::service::ServiceResourcePipelineService;
use super::srdef_loader::SrdefRegistry;
use super::types::*;
use crate::derived_attributes::repository::{
    acquire_derivation_lock, compute_content_hash, get_current, get_current_tx,
    get_recompute_queue, insert_dependencies_tx, insert_derived_value_tx, supersede_current_tx,
    BatchRecomputeResult, ContentHashInput, DependencyRowInput, DerivedValueRowInput,
};
use crate::entity_kind;
use crate::sem_reg::{DerivationFunctionRegistry, DerivationSpecBody};
use crate::services::attribute_identity_service::AttributeIdentityService;
use crate::services::attribute_registry_enrichment::ensure_semos_registry_bridge;

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
    identity: AttributeIdentityService,
}

#[derive(Debug, Clone)]
struct ActiveDerivationSpec {
    snapshot_id: Uuid,
    body: DerivationSpecBody,
}

#[derive(Debug)]
struct DerivationInputCollection {
    inputs: JsonValue,
    input_snapshot_ids: Vec<Uuid>,
    dependencies: Vec<DependencyRowInput>,
    content_hash_inputs: Vec<ContentHashInput>,
    dependency_depth: i32,
}

#[derive(Debug, Clone)]
struct EffectiveAttrValue {
    value: JsonValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecomputeOutcome {
    Recomputed,
    StillStale,
    UnsupportedEntityType,
}

impl<'a> PopulationEngine<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            service: ServiceResourcePipelineService::new(pool.clone()),
            identity: AttributeIdentityService::new(pool.clone()),
        }
    }

    /// Attempt to populate missing attribute values for a CBU
    pub async fn populate_for_cbu(&self, cbu_id: Uuid) -> Result<PopulationResult> {
        info!("Starting attribute population for CBU {}", cbu_id);
        ensure_semos_registry_bridge(self.pool).await?;

        // Get unified requirements
        let requirements = self.service.get_unified_attr_requirements(cbu_id).await?;
        let derivation_specs = self.load_derivation_specs_by_attr().await?;

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
                match self
                    .try_populate(cbu_id, req.attr_id, source, &derivation_specs)
                    .await?
                {
                    Some(value) => {
                        // Derived values are persisted canonically by
                        // try_derive_for_entity — never dual-write to
                        // the legacy cbu_attr_values table.
                        if source != "derived" {
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
                        }
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

    /// Recompute a single derived attribute target.
    pub async fn recompute_derived(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        attr_id: Uuid,
    ) -> Result<RecomputeOutcome> {
        let canonical_entity_type = entity_kind::canonicalize(entity_type);
        let mut tx = self.pool.begin().await?;
        let canonical_entity_type = self
            .validate_entity_type(&mut tx, &canonical_entity_type)
            .await?;
        tx.rollback().await?;

        let derivation_specs = self.load_derivation_specs_by_attr().await?;
        let specs = derivation_specs
            .get(&attr_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if specs.is_empty() {
            return Ok(RecomputeOutcome::StillStale);
        }

        self.try_derive_for_entity(&canonical_entity_type, entity_id, attr_id, specs)
            .await?;
        match get_current(self.pool, &canonical_entity_type, entity_id, attr_id).await? {
            Some(_) => Ok(RecomputeOutcome::Recomputed),
            None => Ok(RecomputeOutcome::StillStale),
        }
    }

    /// Recompute stale rows from the canonical queue in deterministic order.
    pub async fn recompute_stale_batch(&self, limit: i64) -> Result<BatchRecomputeResult> {
        let queue = get_recompute_queue(self.pool, limit).await?;
        let queue_size = queue.len();
        let mut result = BatchRecomputeResult::default();
        let mut seen = HashSet::new();

        for row in queue {
            let key = (row.entity_type.clone(), row.entity_id, row.attr_id);
            if !seen.insert(key) {
                continue;
            }

            result.picked += 1;
            match self
                .recompute_derived(&row.entity_type, row.entity_id, row.attr_id)
                .await
            {
                Ok(RecomputeOutcome::Recomputed) => result.recomputed += 1,
                Ok(RecomputeOutcome::StillStale | RecomputeOutcome::UnsupportedEntityType) => {
                    result.still_stale += 1
                }
                Err(error) => {
                    result.failed += 1;
                    warn!(
                        entity_type = %row.entity_type,
                        entity_id = %row.entity_id,
                        attr_id = %row.attr_id,
                        dependency_depth = row.dependency_depth,
                        error = %error,
                        "Failed stale derived recompute"
                    );
                }
            }
        }

        info!(
            queue_size,
            picked = result.picked,
            recomputed = result.recomputed,
            skipped_already_current = result.skipped_already_current,
            still_stale = result.still_stale,
            failed = result.failed,
            "Batch derived recompute complete"
        );

        Ok(result)
    }

    /// Try to populate a single attribute from a source
    async fn try_populate(
        &self,
        cbu_id: Uuid,
        attr_id: Uuid,
        source: &str,
        derivation_specs: &HashMap<Uuid, Vec<ActiveDerivationSpec>>,
    ) -> Result<Option<JsonValue>> {
        match source {
            "derived" => {
                let specs = derivation_specs
                    .get(&attr_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                self.try_derive_for_entity("cbu", cbu_id, attr_id, specs)
                    .await
            }
            "entity" => self.try_from_entity(cbu_id, attr_id).await,
            "cbu" => self.try_from_cbu(cbu_id, attr_id).await,
            "document" => Ok(None), // TODO: Document extraction
            "external" => Ok(None), // TODO: External API
            _ => Ok(None),
        }
    }

    /// Try to derive a value from other data
    async fn try_derive_for_entity(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        attr_id: Uuid,
        specs: &[ActiveDerivationSpec],
    ) -> Result<Option<JsonValue>> {
        if specs.is_empty() {
            return Ok(None);
        }

        for spec in specs {
            let mut tx = self.pool.begin().await?;
            let entity_type = self.validate_entity_type(&mut tx, entity_type).await?;
            let (attr_code, value_type) = self
                .load_derived_attr_metadata(&mut tx, attr_id)
                .await
                .with_context(|| {
                format!("Failed to load derived attr metadata for {attr_id}")
            })?;

            acquire_derivation_lock(&mut tx, &entity_type, entity_id, attr_id).await?;

            let collected_inputs = self
                .collect_derivation_inputs(&mut tx, &entity_type, entity_id, &spec.body)
                .await?;
            let content_hash =
                compute_content_hash(spec.snapshot_id, &collected_inputs.content_hash_inputs);

            if let Some(current) = get_current_tx(&mut tx, &entity_type, entity_id, attr_id).await?
            {
                if current.content_hash == content_hash && !current.stale {
                    info!(
                        entity_type = %entity_type,
                        entity_id = %entity_id,
                        attr_id = %attr_id,
                        attr_code = %attr_code,
                        derivation_spec_fqn = %spec.body.fqn,
                        spec_snapshot_id = %spec.snapshot_id,
                        old_row_id = %current.id,
                        dependency_count = collected_inputs.dependencies.len(),
                        dependency_depth = collected_inputs.dependency_depth,
                        skip_reason = "content_hash_match",
                        "Skipped canonical derived write"
                    );
                    tx.commit().await?;
                    return Ok(Some(current.value));
                }
            }

            let result = match DERIVATION_REGISTRY.evaluate(
                &spec.body,
                &collected_inputs.inputs,
                &[],
                spec.snapshot_id,
                collected_inputs.input_snapshot_ids.clone(),
            ) {
                Ok(result) => result,
                Err(err) => {
                    warn!(
                        entity_type = %entity_type,
                        entity_id = %entity_id,
                        attr_id = %attr_id,
                        attr_code = %attr_code,
                        derivation_spec_fqn = %spec.body.fqn,
                        spec_snapshot_id = %spec.snapshot_id,
                        error = %err,
                        "Derivation evaluation failed"
                    );
                    tx.rollback().await?;
                    continue;
                }
            };

            if result.value.is_null() {
                tx.rollback().await?;
                continue;
            }

            validate_value_shape(&result.value, &value_type).with_context(|| {
                format!(
                    "Derived value for attr {} did not match declared type {}",
                    attr_code, value_type
                )
            })?;
            let new_id = Uuid::new_v4();
            let superseded_row_id =
                supersede_current_tx(&mut tx, &entity_type, entity_id, attr_id, new_id).await?;
            let derived_row = DerivedValueRowInput {
                id: new_id,
                attr_id,
                entity_id,
                entity_type: entity_type.clone(),
                value: result.value.clone(),
                derivation_spec_fqn: spec.body.fqn.clone(),
                spec_snapshot_id: spec.snapshot_id,
                content_hash,
                input_values: collected_inputs.inputs.clone(),
                inherited_security_label: Some(serde_json::to_value(&result.inherited_label)?),
                dependency_depth: collected_inputs.dependency_depth,
                evaluated_at: result.evaluated_at,
                stale: false,
            };
            let inserted = insert_derived_value_tx(&mut tx, &derived_row).await?;
            insert_dependencies_tx(&mut tx, inserted.id, &collected_inputs.dependencies).await?;
            let stale_propagation_count: i64 = sqlx::query_scalar(
                r#"SELECT COALESCE("ob-poc".propagate_derived_chain_staleness($1, $2), 0)"#,
            )
            .bind(attr_id)
            .bind(entity_id)
            .fetch_one(&mut *tx)
            .await?;
            info!(
                entity_type = %entity_type,
                entity_id = %entity_id,
                attr_id = %attr_id,
                attr_code = %attr_code,
                derivation_spec_fqn = %spec.body.fqn,
                spec_snapshot_id = %spec.snapshot_id,
                old_row_id = ?superseded_row_id,
                new_row_id = %inserted.id,
                dependency_count = collected_inputs.dependencies.len(),
                dependency_depth = collected_inputs.dependency_depth,
                stale_propagation_count,
                "Persisted canonical derived value"
            );
            tx.commit().await?;
            return Ok(Some(result.value));
        }

        Ok(None)
    }

    async fn load_derivation_specs_by_attr(
        &self,
    ) -> Result<HashMap<Uuid, Vec<ActiveDerivationSpec>>> {
        let rows: Vec<(Uuid, serde_json::Value)> = sqlx::query_as(
            r#"
            SELECT snapshot_id, definition
            FROM sem_reg.snapshots
            WHERE object_type = 'derivation_spec'
              AND status = 'active'
              AND effective_until IS NULL
            "#,
        )
        .fetch_all(self.pool)
        .await?;

        let mut output_attr_cache: HashMap<String, Option<Uuid>> = HashMap::new();
        let mut out: HashMap<Uuid, Vec<ActiveDerivationSpec>> = HashMap::new();
        for (snapshot_id, definition) in rows {
            let body: DerivationSpecBody = match serde_json::from_value(definition) {
                Ok(body) => body,
                Err(_) => continue,
            };

            let mapped_attr_id =
                if let Some(cached) = output_attr_cache.get(&body.output_attribute_fqn) {
                    *cached
                } else {
                    let resolved = self
                        .resolve_operational_attr_for_semos_fqn(&body.output_attribute_fqn)
                        .await?;
                    output_attr_cache.insert(body.output_attribute_fqn.clone(), resolved);
                    resolved
                };

            let Some(mapped_attr_id) = mapped_attr_id else {
                continue;
            };

            out.entry(mapped_attr_id)
                .or_default()
                .push(ActiveDerivationSpec { snapshot_id, body });
        }

        Ok(out)
    }

    async fn collect_derivation_inputs(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_type: &str,
        entity_id: Uuid,
        spec: &DerivationSpecBody,
    ) -> Result<DerivationInputCollection> {
        let mut inputs = serde_json::Map::new();
        let mut dependencies = Vec::new();
        let mut content_hash_inputs = Vec::new();
        let mut max_upstream_depth = 0;

        for input in &spec.inputs {
            let Some(input_attr_id) = self
                .resolve_operational_attr_for_semos_fqn(&input.attribute_fqn)
                .await?
            else {
                continue;
            };

            let Some(existing) = self
                .lookup_effective_attr_value(tx, entity_type, entity_id, input_attr_id)
                .await?
            else {
                continue;
            };
            let current_derived = get_current_tx(tx, entity_type, entity_id, input_attr_id).await?;

            inputs.insert(input.attribute_fqn.clone(), existing.value.clone());
            inputs.insert(input.role.clone(), existing.value.clone());
            if let Some(row) = current_derived.as_ref() {
                max_upstream_depth = max_upstream_depth.max(row.dependency_depth + 1);
            }
            let dependency = DependencyRowInput {
                input_kind: current_derived
                    .as_ref()
                    .map(|_| "derived_value")
                    .unwrap_or("observation")
                    .to_string(),
                input_attr_id,
                input_entity_id: entity_id,
                input_source_row_id: current_derived.as_ref().map(|row| row.id),
                dependency_role: Some(input.role.clone()),
                resolved_value: Some(existing.value.clone()),
            };
            content_hash_inputs.push(ContentHashInput {
                input_kind: dependency.input_kind.clone(),
                input_attr_id,
                input_entity_id: entity_id,
                input_source_row_id: dependency.input_source_row_id,
                dependency_role: dependency.dependency_role.clone(),
                resolved_value: existing.value.clone(),
            });
            dependencies.push(dependency);
        }

        Ok(DerivationInputCollection {
            inputs: JsonValue::Object(inputs),
            input_snapshot_ids: Vec::new(),
            dependencies,
            content_hash_inputs,
            dependency_depth: max_upstream_depth,
        })
    }

    async fn resolve_operational_attr_for_semos_fqn(&self, fqn: &str) -> Result<Option<Uuid>> {
        self.identity.resolve_runtime_uuid(fqn).await
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

        // For now, non-CBU entity attributes are sourced from attribute_values_typed
        // through the commercial client entity linked to the CBU.
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

async fn get_cbu_attr_value_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    attr_id: Uuid,
) -> Result<Option<CbuAttrValue>> {
    sqlx::query_as::<_, CbuAttrValue>(
        r#"
        WITH effective_value AS (
            SELECT cbu_id, attr_id, value, source, evidence_refs, explain_refs,
                   as_of, created_at, updated_at
            FROM "ob-poc".cbu_attr_values
            WHERE cbu_id = $1
              AND attr_id = $2
              AND source <> 'derived'

            UNION ALL

            SELECT cbu_id, attr_id, value, source, evidence_refs, explain_refs,
                   as_of, created_at, updated_at
            FROM "ob-poc".v_cbu_derived_values
            WHERE cbu_id = $1
              AND attr_id = $2
        )
        SELECT cbu_id, attr_id, value, source, evidence_refs, explain_refs,
               as_of, created_at, updated_at
        FROM effective_value
        LIMIT 1
        "#,
    )
    .bind(cbu_id)
    .bind(attr_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

fn validate_value_shape(value: &JsonValue, value_type: &str) -> Result<()> {
    let valid = match value_type {
        "string" | "email" | "phone" | "tax_id" | "currency" | "address" | "enum" => {
            value.is_string()
        }
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" | "decimal" | "percentage" => value.is_number(),
        "boolean" => value.is_boolean(),
        "date" | "datetime" | "timestamp" | "uuid" => value.is_string(),
        "json" => true,
        other => return Err(anyhow::anyhow!("Unsupported value_type: {other}")),
    };

    if valid {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Value shape did not match declared type {}",
            value_type
        ))
    }
}

impl<'a> PopulationEngine<'a> {
    async fn lookup_effective_attr_value(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_type: &str,
        entity_id: Uuid,
        attr_id: Uuid,
    ) -> Result<Option<EffectiveAttrValue>> {
        if let Some(current) = get_current_tx(tx, entity_type, entity_id, attr_id).await? {
            return Ok(Some(EffectiveAttrValue {
                value: current.value,
            }));
        }

        if entity_type == "cbu" {
            return Ok(get_cbu_attr_value_tx(tx, entity_id, attr_id)
                .await?
                .map(|value| EffectiveAttrValue { value: value.value }));
        }

        let attr_code: Option<(String,)> =
            sqlx::query_as(r#"SELECT id FROM "ob-poc".attribute_registry WHERE uuid = $1"#)
                .bind(attr_id)
                .fetch_optional(&mut **tx)
                .await?;
        let Some((attr_code,)) = attr_code else {
            return Ok(None);
        };

        #[allow(clippy::type_complexity)]
        let row: Option<(
            Option<String>,
            Option<rust_decimal::Decimal>,
            Option<i64>,
            Option<bool>,
            Option<chrono::NaiveDate>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<JsonValue>,
            Option<JsonValue>,
            Option<chrono::DateTime<chrono::Utc>>,
        )> = sqlx::query_as(
            r#"
            SELECT
                value_text,
                value_number,
                value_integer,
                value_boolean,
                value_date,
                value_datetime,
                value_json,
                source,
                created_at
            FROM "ob-poc".attribute_values_typed
            WHERE entity_id = $1
              AND attribute_id = $2
              AND effective_to IS NULL
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(entity_id)
        .bind(attr_code)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };

        let value = if let Some(text) = row.0 {
            json!(text)
        } else if let Some(number) = row.1 {
            json!(number)
        } else if let Some(integer) = row.2 {
            json!(integer)
        } else if let Some(boolean) = row.3 {
            json!(boolean)
        } else if let Some(date) = row.4 {
            json!(date.to_string())
        } else if let Some(datetime) = row.5 {
            json!(datetime)
        } else if let Some(json_value) = row.6 {
            json_value
        } else {
            return Ok(None);
        };

        Ok(Some(EffectiveAttrValue { value }))
    }

    async fn load_derived_attr_metadata(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        attr_id: Uuid,
    ) -> Result<(String, String)> {
        let row: Option<(String, String)> = sqlx::query_as(
            r#"
            SELECT id, value_type
            FROM "ob-poc".attribute_registry
            WHERE uuid = $1
              AND is_derived = true
            "#,
        )
        .bind(attr_id)
        .fetch_optional(&mut **tx)
        .await?;

        row.ok_or_else(|| anyhow::anyhow!("Attribute {} is not registered as derived", attr_id))
    }

    async fn validate_entity_type(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_type: &str,
    ) -> Result<String> {
        let canonical = entity_kind::canonicalize(entity_type);
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM "ob-poc".entity_types
                WHERE lower(COALESCE(type_code, '')) = $1
                   OR lower(name) = $1
            )
            "#,
        )
        .bind(&canonical)
        .fetch_one(&mut **tx)
        .await?;

        if exists || canonical == "cbu" {
            Ok(canonical)
        } else {
            Err(anyhow::anyhow!("Unknown entity_type: {}", entity_type))
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

fn build_derivation_registry() -> DerivationFunctionRegistry {
    let mut registry = DerivationFunctionRegistry::new();
    registry.register("sum_ownership_chain", Arc::new(sum_ownership_chain));
    registry.register("weighted_average", Arc::new(weighted_average));
    registry.register("percentage_ratio", Arc::new(percentage_ratio));
    registry.register("threshold_flag", Arc::new(threshold_flag));
    registry.register("sum_aggregate", Arc::new(sum_aggregate));
    registry
}

static DERIVATION_REGISTRY: LazyLock<DerivationFunctionRegistry> =
    LazyLock::new(build_derivation_registry);

fn numeric_value(input: &JsonValue, key: &str) -> Option<f64> {
    input.get(key).and_then(|value| match value {
        JsonValue::Number(num) => num.as_f64(),
        JsonValue::String(text) => text.parse::<f64>().ok(),
        _ => None,
    })
}

fn sum_ownership_chain(
    inputs: &JsonValue,
) -> std::result::Result<JsonValue, crate::sem_reg::derivation::DerivationError> {
    let direct = numeric_value(inputs, "ubo.direct_holding_pct")
        .or_else(|| numeric_value(inputs, "primary"))
        .unwrap_or(0.0);
    let indirect = numeric_value(inputs, "ubo.indirect_holding_pct")
        .or_else(|| numeric_value(inputs, "secondary"))
        .unwrap_or(0.0);
    Ok(json!(direct + indirect))
}

fn weighted_average(
    inputs: &JsonValue,
) -> std::result::Result<JsonValue, crate::sem_reg::derivation::DerivationError> {
    // Seeded v1 risk model: fixed weights and coarse credit normalization are intentional until
    // derivation functions accept authored parameters from DerivationSpecBody.
    let credit = numeric_value(inputs, "risk.credit_score")
        .or_else(|| numeric_value(inputs, "primary"))
        .unwrap_or(0.0);
    let credit = if credit > 1.0 {
        credit / 1000.0
    } else {
        credit
    };
    let market = numeric_value(inputs, "risk.market_volatility")
        .or_else(|| numeric_value(inputs, "secondary"))
        .unwrap_or(0.0);
    let operational = numeric_value(inputs, "risk.operational_score")
        .or_else(|| numeric_value(inputs, "weight"))
        .unwrap_or(0.0);
    Ok(json!(
        (credit * 0.5) + ((1.0 - market) * 0.2) + (operational * 0.3)
    ))
}

fn percentage_ratio(
    inputs: &JsonValue,
) -> std::result::Result<JsonValue, crate::sem_reg::derivation::DerivationError> {
    let denominator = numeric_value(inputs, "kyc.required_document_count")
        .or_else(|| numeric_value(inputs, "denominator"))
        .unwrap_or(0.0);
    let numerator = numeric_value(inputs, "kyc.verified_document_count")
        .or_else(|| numeric_value(inputs, "numerator"))
        .unwrap_or(0.0);
    if denominator <= f64::EPSILON {
        return Ok(JsonValue::Null);
    }
    Ok(json!((numerator / denominator) * 100.0))
}

fn threshold_flag(
    inputs: &JsonValue,
) -> std::result::Result<JsonValue, crate::sem_reg::derivation::DerivationError> {
    let value = numeric_value(inputs, "ubo.total_ownership_pct_value")
        .or_else(|| numeric_value(inputs, "primary"))
        .unwrap_or(0.0);
    Ok(json!(value >= 25.0))
}

fn sum_aggregate(
    inputs: &JsonValue,
) -> std::result::Result<JsonValue, crate::sem_reg::derivation::DerivationError> {
    if let Some(JsonValue::Array(values)) = inputs
        .get("trading.cbu_aum")
        .or_else(|| inputs.get("addend"))
    {
        let sum: f64 = values
            .iter()
            .filter_map(|value| match value {
                JsonValue::Number(num) => num.as_f64(),
                JsonValue::String(text) => text.parse::<f64>().ok(),
                _ => None,
            })
            .sum();
        return Ok(json!(sum));
    }

    let single = numeric_value(inputs, "trading.cbu_aum")
        .or_else(|| numeric_value(inputs, "addend"))
        .unwrap_or(0.0);
    Ok(json!(single))
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
