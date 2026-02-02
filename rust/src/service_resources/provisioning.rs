//! Provisioning Orchestrator and Readiness Engine
//!
//! Handles:
//! 1. SRDEF readiness checks (are all required attrs satisfied?)
//! 2. Provisioning orchestration (topo-sort, create requests)
//! 3. Service readiness computation ("good to transact" status)

use anyhow::Result;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::service::ServiceResourcePipelineService;
use super::srdef_loader::SrdefRegistry;
use super::types::*;

// =============================================================================
// PROVISIONING ORCHESTRATOR
// =============================================================================

/// Orchestrates provisioning of resources for a CBU
pub struct ProvisioningOrchestrator<'a> {
    pool: &'a PgPool,
    service: ServiceResourcePipelineService,
    registry: &'a SrdefRegistry,
}

impl<'a> ProvisioningOrchestrator<'a> {
    pub fn new(pool: &'a PgPool, registry: &'a SrdefRegistry) -> Self {
        Self {
            pool,
            service: ServiceResourcePipelineService::new(pool.clone()),
            registry,
        }
    }

    /// Provision all ready SRDEFs for a CBU
    pub async fn provision_for_cbu(&self, cbu_id: Uuid) -> Result<ProvisioningOrchestratorResult> {
        info!("Starting provisioning orchestration for CBU {}", cbu_id);

        // Get active discoveries
        let discoveries = self.service.get_active_discoveries(cbu_id).await?;
        if discoveries.is_empty() {
            info!("No discoveries to provision for CBU {}", cbu_id);
            return Ok(ProvisioningOrchestratorResult::default());
        }

        // Get existing instances
        let existing_instances = self.get_existing_instances(cbu_id).await?;

        // Topo-sort discoveries by dependencies
        let srdef_ids: Vec<String> = discoveries.iter().map(|d| d.srdef_id.clone()).collect();
        let sorted = self.registry.topo_sort(&srdef_ids)?;

        let mut result = ProvisioningOrchestratorResult::default();

        for srdef_id in sorted {
            // Skip if already provisioned and active
            if existing_instances.contains_key(&srdef_id) {
                let status = existing_instances.get(&srdef_id).unwrap();
                if status == "ACTIVE" {
                    result.already_active += 1;
                    continue;
                }
            }

            // Check readiness
            match self.check_srdef_readiness(cbu_id, &srdef_id).await? {
                SrdefReadinessResult::Ready { attrs } => {
                    // Create provisioning request
                    match self
                        .create_provisioning_request(cbu_id, &srdef_id, attrs)
                        .await
                    {
                        Ok(request_id) => {
                            result.requests_created += 1;
                            result.created_request_ids.push(request_id);
                        }
                        Err(e) => {
                            warn!("Failed to create request for {}: {}", srdef_id, e);
                            result.errors.push(format!("{}: {}", srdef_id, e));
                        }
                    }
                }
                SrdefReadinessResult::NotReady { reason } => {
                    debug!("SRDEF {} not ready: {}", srdef_id, reason);
                    result.not_ready += 1;
                    result.not_ready_reasons.push((srdef_id.clone(), reason));
                }
            }
        }

        info!(
            "Provisioning orchestration complete for CBU {}: {} requests created, {} already active, {} not ready",
            cbu_id, result.requests_created, result.already_active, result.not_ready
        );

        Ok(result)
    }

    /// Get existing resource instances for a CBU
    async fn get_existing_instances(&self, cbu_id: Uuid) -> Result<HashMap<String, String>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT COALESCE(srdef_id, '') as srdef_id, status
            FROM "ob-poc".cbu_resource_instances
            WHERE cbu_id = $1 AND srdef_id IS NOT NULL
            "#,
        )
        .bind(cbu_id)
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().collect())
    }

    /// Check if an SRDEF is ready to provision
    async fn check_srdef_readiness(
        &self,
        cbu_id: Uuid,
        srdef_id: &str,
    ) -> Result<SrdefReadinessResult> {
        // Get required attributes for this SRDEF
        let required_attrs = self.get_required_attrs(srdef_id).await?;

        // Get populated values for this CBU
        let cbu_values = self.service.get_cbu_attr_values(cbu_id).await?;
        let populated_ids: HashMap<Uuid, &CbuAttrValue> =
            cbu_values.iter().map(|v| (v.attr_id, v)).collect();

        // Check if all required attrs are satisfied
        let mut missing: Vec<Uuid> = Vec::new();
        let mut attrs: HashMap<String, JsonValue> = HashMap::new();

        for (attr_id, attr_code) in &required_attrs {
            if let Some(value) = populated_ids.get(attr_id) {
                attrs.insert(attr_code.clone(), value.value.clone());
            } else {
                missing.push(*attr_id);
            }
        }

        if !missing.is_empty() {
            return Ok(SrdefReadinessResult::NotReady {
                reason: format!("Missing {} required attributes", missing.len()),
            });
        }

        // Check dependencies are satisfied
        if let Some(srdef) = self.registry.get(srdef_id) {
            for dep_id in &srdef.depends_on {
                // Check if dependency has an active instance
                let dep_status = self.get_instance_status(cbu_id, dep_id).await?;
                if dep_status != Some("ACTIVE".to_string()) {
                    return Ok(SrdefReadinessResult::NotReady {
                        reason: format!("Dependency {} not active", dep_id),
                    });
                }
            }
        }

        Ok(SrdefReadinessResult::Ready { attrs })
    }

    /// Get required attributes for an SRDEF
    async fn get_required_attrs(&self, srdef_id: &str) -> Result<Vec<(Uuid, String)>> {
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT ar.uuid, ar.id
            FROM "ob-poc".resource_attribute_requirements rar
            JOIN "ob-poc".service_resource_types srt ON srt.resource_id = rar.resource_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = rar.attribute_id
            WHERE srt.srdef_id = $1 AND rar.is_mandatory = TRUE
            "#,
        )
        .bind(srdef_id)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    /// Get instance status for an SRDEF
    async fn get_instance_status(&self, cbu_id: Uuid, srdef_id: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT status
            FROM "ob-poc".cbu_resource_instances
            WHERE cbu_id = $1 AND srdef_id = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .bind(srdef_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(|(s,)| s))
    }

    /// Create a provisioning request
    async fn create_provisioning_request(
        &self,
        cbu_id: Uuid,
        srdef_id: &str,
        attrs: HashMap<String, JsonValue>,
    ) -> Result<Uuid> {
        // Get SRDEF details
        let srdef = self.service.get_srdef_by_id(srdef_id).await?;
        let owner_system = srdef
            .as_ref()
            .map(|s| s.owner.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        // Create the instance first (in PENDING state)
        let instance_id = Uuid::now_v7();
        let instance_url = format!("urn:ob-poc:instance:{}", instance_id);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_resource_instances
                (instance_id, cbu_id, srdef_id, instance_url, status)
            VALUES ($1, $2, $3, $4, 'PENDING')
            "#,
        )
        .bind(instance_id)
        .bind(cbu_id)
        .bind(srdef_id)
        .bind(&instance_url)
        .execute(self.pool)
        .await?;

        // Create provisioning request
        let request = NewProvisioningRequest {
            cbu_id,
            srdef_id: srdef_id.to_string(),
            instance_id: Some(instance_id),
            requested_by: RequestedBy::System,
            request_payload: ProvisioningPayload {
                attrs: json!(attrs),
                bind_to: None,
                evidence_refs: None,
                idempotency_key: Some(format!("{}:{}:{}", cbu_id, srdef_id, instance_id)),
            },
            owner_system,
            parameters: None,
        };

        let request_id = self.service.create_provisioning_request(&request).await?;

        // Record the REQUEST_SENT event
        self.service
            .record_provisioning_event(
                request_id,
                EventDirection::Out,
                EventKind::RequestSent,
                &json!({
                    "srdef_id": srdef_id,
                    "attrs": attrs,
                    "instance_id": instance_id,
                }),
                None,
            )
            .await?;

        // Update instance with request reference
        sqlx::query(
            r#"
            UPDATE "ob-poc".cbu_resource_instances
            SET last_request_id = $1, status = 'PROVISIONING'
            WHERE instance_id = $2
            "#,
        )
        .bind(request_id)
        .bind(instance_id)
        .execute(self.pool)
        .await?;

        Ok(request_id)
    }
}

enum SrdefReadinessResult {
    Ready { attrs: HashMap<String, JsonValue> },
    NotReady { reason: String },
}

/// Result of provisioning orchestration
#[derive(Debug, Default)]
pub struct ProvisioningOrchestratorResult {
    pub requests_created: usize,
    pub created_request_ids: Vec<Uuid>,
    pub already_active: usize,
    pub not_ready: usize,
    pub not_ready_reasons: Vec<(String, String)>,
    pub errors: Vec<String>,
}

// =============================================================================
// READINESS ENGINE
// =============================================================================

/// Computes service readiness ("good to transact") for a CBU
pub struct ReadinessEngine<'a> {
    pool: &'a PgPool,
    service: ServiceResourcePipelineService,
    registry: &'a SrdefRegistry,
}

impl<'a> ReadinessEngine<'a> {
    pub fn new(pool: &'a PgPool, registry: &'a SrdefRegistry) -> Self {
        Self {
            pool,
            service: ServiceResourcePipelineService::new(pool.clone()),
            registry,
        }
    }

    /// Compute readiness for all services of a CBU
    pub async fn compute_for_cbu(&self, cbu_id: Uuid) -> Result<ReadinessComputeResult> {
        info!("Computing service readiness for CBU {}", cbu_id);

        // Get active service intents
        let intents = self.service.get_service_intents(cbu_id).await?;
        if intents.is_empty() {
            info!("No active service intents for CBU {}", cbu_id);
            return Ok(ReadinessComputeResult::default());
        }

        // Get active discoveries
        let discoveries = self.service.get_active_discoveries(cbu_id).await?;
        let discovered_srdefs: HashSet<String> =
            discoveries.iter().map(|d| d.srdef_id.clone()).collect();

        // Get active instances
        let instances = self.get_active_instances(cbu_id).await?;

        // Get attribute values
        let attr_values = self.service.get_cbu_attr_values(cbu_id).await?;
        let attr_ids: HashSet<Uuid> = attr_values.iter().map(|v| v.attr_id).collect();

        // Get unified requirements
        let requirements = self.service.get_unified_attr_requirements(cbu_id).await?;

        let mut result = ReadinessComputeResult::default();

        for intent in &intents {
            let service_code = self.get_service_code(intent.service_id).await?;

            // Get required SRDEFs for this service
            let required_srdefs: Vec<String> = self
                .registry
                .get_by_service(&service_code)
                .iter()
                .map(|s| s.srdef_id.clone())
                .collect();

            // Check status of each required SRDEF
            let mut blocking_reasons: Vec<BlockingReason> = Vec::new();
            let mut active_srids: Vec<String> = Vec::new();

            for srdef_id in &required_srdefs {
                // Check if discovered
                if !discovered_srdefs.contains(srdef_id) {
                    blocking_reasons.push(BlockingReason {
                        reason_type: BlockingReasonType::MissingSrdef,
                        srdef_id: Some(srdef_id.clone()),
                        details: json!({}),
                        explain: format!("SRDEF {} not discovered", srdef_id),
                    });
                    continue;
                }

                // Check instance status
                match instances.get(srdef_id) {
                    Some((srid, status)) if status == "ACTIVE" => {
                        active_srids.push(srid.clone());
                    }
                    Some((_, status)) if status == "PROVISIONING" || status == "PENDING" => {
                        blocking_reasons.push(BlockingReason {
                            reason_type: BlockingReasonType::PendingProvisioning,
                            srdef_id: Some(srdef_id.clone()),
                            details: json!({ "status": status }),
                            explain: format!("SRDEF {} provisioning in progress", srdef_id),
                        });
                    }
                    Some((_, status)) if status == "FAILED" => {
                        blocking_reasons.push(BlockingReason {
                            reason_type: BlockingReasonType::FailedProvisioning,
                            srdef_id: Some(srdef_id.clone()),
                            details: json!({ "status": status }),
                            explain: format!("SRDEF {} provisioning failed", srdef_id),
                        });
                    }
                    _ => {
                        blocking_reasons.push(BlockingReason {
                            reason_type: BlockingReasonType::MissingSrdef,
                            srdef_id: Some(srdef_id.clone()),
                            details: json!({}),
                            explain: format!("No instance for SRDEF {}", srdef_id),
                        });
                    }
                }

                // Check if required attrs for this SRDEF are satisfied
                let missing_attrs = self
                    .get_missing_attrs_for_srdef(srdef_id, &attr_ids)
                    .await?;
                if !missing_attrs.is_empty() {
                    blocking_reasons.push(BlockingReason {
                        reason_type: BlockingReasonType::MissingAttrs,
                        srdef_id: Some(srdef_id.clone()),
                        details: json!({ "missing_count": missing_attrs.len() }),
                        explain: format!(
                            "{} required attributes missing for SRDEF {}",
                            missing_attrs.len(),
                            srdef_id
                        ),
                    });
                }
            }

            // Check for attribute conflicts
            let conflicts: Vec<_> = requirements
                .iter()
                .filter(|r| r.conflict.is_some())
                .collect();

            for conflict in conflicts {
                blocking_reasons.push(BlockingReason {
                    reason_type: BlockingReasonType::AttrConflict,
                    srdef_id: None,
                    details: conflict.conflict.clone().unwrap_or(json!({})),
                    explain: "Attribute constraint conflict detected".to_string(),
                });
            }

            // Determine status
            let status = if blocking_reasons.is_empty() {
                ReadinessStatus::Ready
            } else if !active_srids.is_empty() {
                ReadinessStatus::Partial
            } else {
                ReadinessStatus::Blocked
            };

            // Persist readiness
            self.service
                .upsert_service_readiness(
                    cbu_id,
                    intent.product_id,
                    intent.service_id,
                    status,
                    &blocking_reasons,
                    &required_srdefs,
                    &active_srids,
                    Some("compute_for_cbu"),
                )
                .await?;

            match status {
                ReadinessStatus::Ready => result.ready += 1,
                ReadinessStatus::Partial => result.partial += 1,
                ReadinessStatus::Blocked => result.blocked += 1,
            }
        }

        result.total_services = intents.len();
        info!(
            "Readiness computation complete for CBU {}: {} ready, {} partial, {} blocked",
            cbu_id, result.ready, result.partial, result.blocked
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

    /// Get active instances for a CBU (srdef_id -> (srid, status))
    async fn get_active_instances(
        &self,
        cbu_id: Uuid,
    ) -> Result<HashMap<String, (String, String)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT COALESCE(srdef_id, ''), instance_url, status
            FROM "ob-poc".cbu_resource_instances
            WHERE cbu_id = $1 AND srdef_id IS NOT NULL
            "#,
        )
        .bind(cbu_id)
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(|(s, u, st)| (s, (u, st))).collect())
    }

    /// Get missing required attributes for an SRDEF
    async fn get_missing_attrs_for_srdef(
        &self,
        srdef_id: &str,
        populated_attr_ids: &HashSet<Uuid>,
    ) -> Result<Vec<Uuid>> {
        let required: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT rar.attribute_id
            FROM "ob-poc".resource_attribute_requirements rar
            JOIN "ob-poc".service_resource_types srt ON srt.resource_id = rar.resource_id
            WHERE srt.srdef_id = $1 AND rar.is_mandatory = TRUE
            "#,
        )
        .bind(srdef_id)
        .fetch_all(self.pool)
        .await?;

        Ok(required
            .into_iter()
            .map(|(id,)| id)
            .filter(|id| !populated_attr_ids.contains(id))
            .collect())
    }
}

/// Result of readiness computation
#[derive(Debug, Default)]
pub struct ReadinessComputeResult {
    pub total_services: usize,
    pub ready: usize,
    pub partial: usize,
    pub blocked: usize,
}

// =============================================================================
// STUB PROVISIONER
// =============================================================================

/// Trait for resource provisioners
#[async_trait::async_trait]
pub trait ResourceProvisioner: Send + Sync {
    /// Provision a resource
    async fn provision(
        &self,
        cbu_id: Uuid,
        srdef_id: &str,
        attrs: &HashMap<String, JsonValue>,
    ) -> Result<ProvisionResult>;
}

/// Result of provisioning
pub struct ProvisionResult {
    pub srid: String,
    pub native_key: String,
    pub resource_url: Option<String>,
}

/// Stub provisioner that synthesizes fake resources
pub struct StubProvisioner;

#[async_trait::async_trait]
impl ResourceProvisioner for StubProvisioner {
    async fn provision(
        &self,
        _cbu_id: Uuid,
        srdef_id: &str,
        _attrs: &HashMap<String, JsonValue>,
    ) -> Result<ProvisionResult> {
        let fake_key = format!("FAKE-{}", Uuid::now_v7().to_string()[..8].to_uppercase());
        let parts: Vec<&str> = srdef_id.split("::").collect();
        let (app, kind) = if parts.len() >= 3 {
            (parts[1], parts[2])
        } else {
            ("UNKNOWN", "Resource")
        };

        Ok(ProvisionResult {
            srid: format!("SR::{}::{}::{}", app, kind, fake_key),
            native_key: fake_key,
            resource_url: None,
        })
    }
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Run the full provisioning + readiness pipeline for a CBU
pub async fn run_provisioning_pipeline(
    pool: &PgPool,
    registry: &SrdefRegistry,
    cbu_id: Uuid,
) -> Result<FullPipelineResult> {
    // Provision
    let orchestrator = ProvisioningOrchestrator::new(pool, registry);
    let provisioning = orchestrator.provision_for_cbu(cbu_id).await?;

    // Compute readiness
    let readiness_engine = ReadinessEngine::new(pool, registry);
    let readiness = readiness_engine.compute_for_cbu(cbu_id).await?;

    Ok(FullPipelineResult {
        cbu_id,
        requests_created: provisioning.requests_created,
        already_active: provisioning.already_active,
        not_ready: provisioning.not_ready,
        services_ready: readiness.ready,
        services_partial: readiness.partial,
        services_blocked: readiness.blocked,
    })
}

/// Result of running the full provisioning pipeline
#[derive(Debug)]
pub struct FullPipelineResult {
    pub cbu_id: Uuid,
    pub requests_created: usize,
    pub already_active: usize,
    pub not_ready: usize,
    pub services_ready: usize,
    pub services_partial: usize,
    pub services_blocked: usize,
}
