//! Service Resource Pipeline Service
//!
//! Database operations for the service resource pipeline.

use anyhow::{Context, Result};
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use super::types::*;

/// Service for the CBU resource pipeline operations.
#[derive(Clone, Debug)]
pub struct ServiceResourcePipelineService {
    pool: PgPool,
}

impl ServiceResourcePipelineService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // =========================================================================
    // SERVICE INTENTS
    // =========================================================================

    /// Create a new service intent
    pub async fn create_service_intent(&self, input: &NewServiceIntent) -> Result<Uuid> {
        let intent_id = Uuid::now_v7();
        let options = input.options.clone().unwrap_or(json!({}));

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".service_intents
                (intent_id, cbu_id, product_id, service_id, options, status, created_by)
            VALUES ($1, $2, $3, $4, $5, 'active', $6)
            ON CONFLICT (cbu_id, product_id, service_id)
            DO UPDATE SET
                options = EXCLUDED.options,
                status = 'active',
                updated_at = NOW()
            "#,
        )
        .bind(intent_id)
        .bind(input.cbu_id)
        .bind(input.product_id)
        .bind(input.service_id)
        .bind(&options)
        .bind(&input.created_by)
        .execute(&self.pool)
        .await
        .context("Failed to create service intent")?;

        info!(
            "Created service intent {} for CBU {}",
            intent_id, input.cbu_id
        );
        Ok(intent_id)
    }

    /// Get service intents for a CBU
    pub async fn get_service_intents(&self, cbu_id: Uuid) -> Result<Vec<ServiceIntent>> {
        sqlx::query_as::<_, ServiceIntent>(
            r#"
            SELECT intent_id, cbu_id, product_id, service_id, options, status,
                   created_at, updated_at, created_by
            FROM "ob-poc".service_intents
            WHERE cbu_id = $1 AND status = 'active'
            ORDER BY created_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get service intents")
    }

    /// Get a specific service intent
    pub async fn get_service_intent(&self, intent_id: Uuid) -> Result<Option<ServiceIntent>> {
        sqlx::query_as::<_, ServiceIntent>(
            r#"
            SELECT intent_id, cbu_id, product_id, service_id, options, status,
                   created_at, updated_at, created_by
            FROM "ob-poc".service_intents
            WHERE intent_id = $1
            "#,
        )
        .bind(intent_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get service intent")
    }

    /// Cancel a service intent
    pub async fn cancel_service_intent(&self, intent_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".service_intents
            SET status = 'cancelled', updated_at = NOW()
            WHERE intent_id = $1 AND status = 'active'
            "#,
        )
        .bind(intent_id)
        .execute(&self.pool)
        .await
        .context("Failed to cancel service intent")?;

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // SRDEF DISCOVERY
    // =========================================================================

    /// Record a discovery reason
    pub async fn record_discovery(&self, input: &NewSrdefDiscovery) -> Result<Uuid> {
        let discovery_id = Uuid::now_v7();
        let triggered_by = json!(input.triggered_by_intents);
        let parameters = input.parameters.clone().unwrap_or(json!({}));

        // First, supersede any existing active discovery for this CBU/SRDEF/params
        sqlx::query(
            r#"
            UPDATE "ob-poc".srdef_discovery_reasons
            SET superseded_at = NOW()
            WHERE cbu_id = $1 AND srdef_id = $2 AND parameters = $3 AND superseded_at IS NULL
            "#,
        )
        .bind(input.cbu_id)
        .bind(&input.srdef_id)
        .bind(&parameters)
        .execute(&self.pool)
        .await?;

        // Insert new discovery
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".srdef_discovery_reasons
                (discovery_id, cbu_id, srdef_id, resource_type_id, triggered_by_intents,
                 discovery_rule, discovery_reason, parameters)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(discovery_id)
        .bind(input.cbu_id)
        .bind(&input.srdef_id)
        .bind(input.resource_type_id)
        .bind(&triggered_by)
        .bind(&input.discovery_rule)
        .bind(&input.discovery_reason)
        .bind(&parameters)
        .execute(&self.pool)
        .await
        .context("Failed to record discovery")?;

        info!(
            "Recorded discovery {} for CBU {} SRDEF {}",
            discovery_id, input.cbu_id, input.srdef_id
        );
        Ok(discovery_id)
    }

    /// Get active discoveries for a CBU
    pub async fn get_active_discoveries(&self, cbu_id: Uuid) -> Result<Vec<SrdefDiscoveryReason>> {
        sqlx::query_as::<_, SrdefDiscoveryReason>(
            r#"
            SELECT discovery_id, cbu_id, srdef_id, resource_type_id, triggered_by_intents,
                   discovery_rule, discovery_reason, parameters, discovered_at, superseded_at
            FROM "ob-poc".srdef_discovery_reasons
            WHERE cbu_id = $1 AND superseded_at IS NULL
            ORDER BY discovered_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get discoveries")
    }

    // =========================================================================
    // CBU UNIFIED ATTRIBUTES
    // =========================================================================

    /// Upsert a unified attribute requirement
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_unified_attr_requirement(
        &self,
        cbu_id: Uuid,
        attr_id: Uuid,
        requirement_strength: &str,
        merged_constraints: &JsonValue,
        preferred_source: Option<&str>,
        required_by_srdefs: &[String],
        conflict: Option<&JsonValue>,
    ) -> Result<()> {
        let srdefs_json = json!(required_by_srdefs);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_unified_attr_requirements
                (cbu_id, attr_id, requirement_strength, merged_constraints,
                 preferred_source, required_by_srdefs, conflict)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (cbu_id, attr_id) DO UPDATE SET
                requirement_strength = EXCLUDED.requirement_strength,
                merged_constraints = EXCLUDED.merged_constraints,
                preferred_source = EXCLUDED.preferred_source,
                required_by_srdefs = EXCLUDED.required_by_srdefs,
                conflict = EXCLUDED.conflict,
                updated_at = NOW()
            "#,
        )
        .bind(cbu_id)
        .bind(attr_id)
        .bind(requirement_strength)
        .bind(merged_constraints)
        .bind(preferred_source)
        .bind(&srdefs_json)
        .bind(conflict)
        .execute(&self.pool)
        .await
        .context("Failed to upsert unified attr requirement")?;

        Ok(())
    }

    /// Get unified attribute requirements for a CBU
    pub async fn get_unified_attr_requirements(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<CbuUnifiedAttrRequirement>> {
        sqlx::query_as::<_, CbuUnifiedAttrRequirement>(
            r#"
            SELECT cbu_id, attr_id, requirement_strength, merged_constraints,
                   preferred_source, required_by_srdefs, conflict, created_at, updated_at
            FROM "ob-poc".cbu_unified_attr_requirements
            WHERE cbu_id = $1
            ORDER BY requirement_strength, attr_id
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get unified attr requirements")
    }

    /// Clear unified attr requirements for a CBU (before rebuild)
    pub async fn clear_unified_attr_requirements(&self, cbu_id: Uuid) -> Result<u64> {
        let result =
            sqlx::query(r#"DELETE FROM "ob-poc".cbu_unified_attr_requirements WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(&self.pool)
                .await
                .context("Failed to clear unified attr requirements")?;

        Ok(result.rows_affected())
    }

    // =========================================================================
    // CBU ATTRIBUTE VALUES
    // =========================================================================

    /// Set a CBU attribute value
    pub async fn set_cbu_attr_value(&self, input: &SetCbuAttrValue) -> Result<()> {
        let evidence = json!(input.evidence_refs);
        let explain = json!(input.explain_refs);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_attr_values
                (cbu_id, attr_id, value, source, evidence_refs, explain_refs, as_of)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            ON CONFLICT (cbu_id, attr_id) DO UPDATE SET
                value = EXCLUDED.value,
                source = EXCLUDED.source,
                evidence_refs = EXCLUDED.evidence_refs,
                explain_refs = EXCLUDED.explain_refs,
                as_of = NOW(),
                updated_at = NOW()
            "#,
        )
        .bind(input.cbu_id)
        .bind(input.attr_id)
        .bind(&input.value)
        .bind(input.source.to_string())
        .bind(&evidence)
        .bind(&explain)
        .execute(&self.pool)
        .await
        .context("Failed to set CBU attr value")?;

        info!(
            "Set CBU {} attr {} from source {}",
            input.cbu_id, input.attr_id, input.source
        );
        Ok(())
    }

    /// Get CBU attribute values
    pub async fn get_cbu_attr_values(&self, cbu_id: Uuid) -> Result<Vec<CbuAttrValue>> {
        sqlx::query_as::<_, CbuAttrValue>(
            r#"
            SELECT cbu_id, attr_id, value, source, evidence_refs, explain_refs,
                   as_of, created_at, updated_at
            FROM "ob-poc".cbu_attr_values
            WHERE cbu_id = $1
            ORDER BY attr_id
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CBU attr values")
    }

    /// Get a specific CBU attribute value
    pub async fn get_cbu_attr_value(
        &self,
        cbu_id: Uuid,
        attr_id: Uuid,
    ) -> Result<Option<CbuAttrValue>> {
        sqlx::query_as::<_, CbuAttrValue>(
            r#"
            SELECT cbu_id, attr_id, value, source, evidence_refs, explain_refs,
                   as_of, created_at, updated_at
            FROM "ob-poc".cbu_attr_values
            WHERE cbu_id = $1 AND attr_id = $2
            "#,
        )
        .bind(cbu_id)
        .bind(attr_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CBU attr value")
    }

    // =========================================================================
    // PROVISIONING REQUESTS
    // =========================================================================

    /// Create a provisioning request
    pub async fn create_provisioning_request(
        &self,
        input: &NewProvisioningRequest,
    ) -> Result<Uuid> {
        let request_id = Uuid::now_v7();
        let payload = serde_json::to_value(&input.request_payload)?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".provisioning_requests
                (request_id, cbu_id, srdef_id, instance_id, requested_by,
                 request_payload, owner_system, parameters)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(request_id)
        .bind(input.cbu_id)
        .bind(&input.srdef_id)
        .bind(input.instance_id)
        .bind(input.requested_by.to_string())
        .bind(&payload)
        .bind(&input.owner_system)
        .bind(&input.parameters)
        .execute(&self.pool)
        .await
        .context("Failed to create provisioning request")?;

        info!(
            "Created provisioning request {} for CBU {} SRDEF {}",
            request_id, input.cbu_id, input.srdef_id
        );
        Ok(request_id)
    }

    /// Get provisioning requests for a CBU
    pub async fn get_provisioning_requests(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<ProvisioningRequest>> {
        sqlx::query_as::<_, ProvisioningRequest>(
            r#"
            SELECT request_id, cbu_id, srdef_id, instance_id, requested_by, requested_at,
                   request_payload, status, owner_system, owner_ticket_id, parameters,
                   status_changed_at
            FROM "ob-poc".provisioning_requests
            WHERE cbu_id = $1
            ORDER BY requested_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get provisioning requests")
    }

    /// Get pending provisioning requests
    pub async fn get_pending_requests(&self) -> Result<Vec<ProvisioningRequest>> {
        sqlx::query_as::<_, ProvisioningRequest>(
            r#"
            SELECT request_id, cbu_id, srdef_id, instance_id, requested_by, requested_at,
                   request_payload, status, owner_system, owner_ticket_id, parameters,
                   status_changed_at
            FROM "ob-poc".provisioning_requests
            WHERE status IN ('queued', 'sent', 'ack')
            ORDER BY requested_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get pending requests")
    }

    /// Update provisioning request status
    pub async fn update_request_status(
        &self,
        request_id: Uuid,
        status: ProvisioningStatus,
        owner_ticket_id: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".provisioning_requests
            SET status = $1, owner_ticket_id = COALESCE($2, owner_ticket_id)
            WHERE request_id = $3
            "#,
        )
        .bind(status.to_string())
        .bind(owner_ticket_id)
        .bind(request_id)
        .execute(&self.pool)
        .await
        .context("Failed to update request status")?;

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // PROVISIONING EVENTS
    // =========================================================================

    /// Record a provisioning event
    pub async fn record_provisioning_event(
        &self,
        request_id: Uuid,
        direction: EventDirection,
        kind: EventKind,
        payload: &JsonValue,
        content_hash: Option<&str>,
    ) -> Result<Uuid> {
        let event_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".provisioning_events
                (event_id, request_id, direction, kind, payload, content_hash)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(event_id)
        .bind(request_id)
        .bind(direction.to_string())
        .bind(kind.to_string())
        .bind(payload)
        .bind(content_hash)
        .execute(&self.pool)
        .await
        .context("Failed to record provisioning event")?;

        info!(
            "Recorded {} event {} for request {}",
            kind, event_id, request_id
        );
        Ok(event_id)
    }

    /// Get events for a request
    pub async fn get_request_events(&self, request_id: Uuid) -> Result<Vec<ProvisioningEvent>> {
        sqlx::query_as::<_, ProvisioningEvent>(
            r#"
            SELECT event_id, request_id, occurred_at, direction, kind, payload, content_hash
            FROM "ob-poc".provisioning_events
            WHERE request_id = $1
            ORDER BY occurred_at ASC
            "#,
        )
        .bind(request_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get request events")
    }

    /// Check if event hash exists (for deduplication)
    pub async fn event_hash_exists(&self, content_hash: &str) -> Result<bool> {
        let result = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM "ob-poc".provisioning_events WHERE content_hash = $1
            "#,
        )
        .bind(content_hash)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check event hash")?;

        Ok(result > 0)
    }

    // =========================================================================
    // SERVICE READINESS
    // =========================================================================

    /// Upsert service readiness
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_service_readiness(
        &self,
        cbu_id: Uuid,
        product_id: Uuid,
        service_id: Uuid,
        status: ReadinessStatus,
        blocking_reasons: &[BlockingReason],
        required_srdefs: &[String],
        active_srids: &[String],
        trigger: Option<&str>,
    ) -> Result<()> {
        let reasons = json!(blocking_reasons);
        let srdefs = json!(required_srdefs);
        let srids = json!(active_srids);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_service_readiness
                (cbu_id, product_id, service_id, status, blocking_reasons,
                 required_srdefs, active_srids, recomputation_trigger, is_stale)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, FALSE)
            ON CONFLICT (cbu_id, product_id, service_id) DO UPDATE SET
                status = EXCLUDED.status,
                blocking_reasons = EXCLUDED.blocking_reasons,
                required_srdefs = EXCLUDED.required_srdefs,
                active_srids = EXCLUDED.active_srids,
                as_of = NOW(),
                last_recomputed_at = NOW(),
                recomputation_trigger = EXCLUDED.recomputation_trigger,
                is_stale = FALSE
            "#,
        )
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(status.to_string())
        .bind(&reasons)
        .bind(&srdefs)
        .bind(&srids)
        .bind(trigger)
        .execute(&self.pool)
        .await
        .context("Failed to upsert service readiness")?;

        Ok(())
    }

    /// Get service readiness for a CBU
    pub async fn get_service_readiness(&self, cbu_id: Uuid) -> Result<Vec<CbuServiceReadiness>> {
        sqlx::query_as::<_, CbuServiceReadiness>(
            r#"
            SELECT cbu_id, product_id, service_id, status, blocking_reasons,
                   required_srdefs, active_srids, as_of, last_recomputed_at,
                   recomputation_trigger, is_stale
            FROM "ob-poc".cbu_service_readiness
            WHERE cbu_id = $1
            ORDER BY status DESC, product_id, service_id
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get service readiness")
    }

    /// Get stale readiness records (need recomputation)
    pub async fn get_stale_readiness(&self) -> Result<Vec<(Uuid, Uuid, Uuid)>> {
        sqlx::query_as::<_, (Uuid, Uuid, Uuid)>(
            r#"
            SELECT cbu_id, product_id, service_id
            FROM "ob-poc".cbu_service_readiness
            WHERE is_stale = TRUE
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get stale readiness records")
    }

    /// Clear readiness for a CBU (before rebuild)
    pub async fn clear_service_readiness(&self, cbu_id: Uuid) -> Result<u64> {
        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_service_readiness WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&self.pool)
            .await
            .context("Failed to clear service readiness")?;

        Ok(result.rows_affected())
    }

    // =========================================================================
    // LOOKUP HELPERS
    // =========================================================================

    /// Lookup SRDEF by ID
    pub async fn get_srdef_by_id(&self, srdef_id: &str) -> Result<Option<Srdef>> {
        sqlx::query_as::<_, Srdef>(
            r#"
            SELECT resource_id, name, description, owner, resource_code, resource_type,
                   resource_purpose, srdef_id, provisioning_strategy, depends_on, is_active
            FROM "ob-poc".service_resource_types
            WHERE srdef_id = $1
            "#,
        )
        .bind(srdef_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get SRDEF")
    }

    /// Get all active SRDEFs
    pub async fn get_all_srdefs(&self) -> Result<Vec<Srdef>> {
        sqlx::query_as::<_, Srdef>(
            r#"
            SELECT resource_id, name, description, owner, resource_code, resource_type,
                   resource_purpose, srdef_id, provisioning_strategy, depends_on, is_active
            FROM "ob-poc".service_resource_types
            WHERE is_active = TRUE OR is_active IS NULL
            ORDER BY owner, resource_type, resource_code
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get SRDEFs")
    }
}
