//! Discovery domain CustomOps for the SemTaxonomy replacement path.
//!
//! These verbs expose a read-only discovery surface over existing entity search,
//! Sem OS registry/schema tooling, and lightweight operational context queries.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_core::config::loader::ConfigLoader;
use dsl_core::config::types::{ArgConfig, DomainConfig, VerbConfig, VerbMetadata};
use ob_poc_macros::register_custom_op;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use super::sem_os_helpers::{build_actor_from_ctx, get_bool_arg, get_string_arg};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};
use crate::dsl_v2::gateway_resolver::gateway_addr;
use crate::entity_kind::matches as entity_kind_matches;
use crate::sem_reg::agent::mcp_tools::{dispatch_tool, SemRegToolContext, SemRegToolResult};
use crate::stategraph::{load_state_graphs, validate_graphs, walk_graph};

#[cfg(feature = "database")]
use {
    entity_gateway::proto::ob::gateway::v1::{
        entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
    },
    sqlx::{PgPool, Row},
};

#[cfg(feature = "database")]
use crate::database::{GovernedDocumentRequirementsService, GovernedRequirementMatrix};

fn get_int_arg_or_default(verb_call: &VerbCall, name: &str, default: i64) -> i64 {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_integer())
        .unwrap_or(default)
}

fn get_list_arg(verb_call: &VerbCall, name: &str) -> Vec<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_list())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_string().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn get_uuid_arg(ctx: &ExecutionContext, verb_call: &VerbCall, name: &str) -> Option<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|arg| {
            if let Some(symbol) = arg.value.as_symbol() {
                ctx.resolve(symbol)
            } else {
                arg.value.as_uuid()
            }
        })
}

fn normalize_entity_type(entity_type: &str) -> String {
    match entity_type.to_lowercase().as_str() {
        "cbu" | "client" => "CBU".to_string(),
        "entity" | "entities" => "ENTITY".to_string(),
        "person" | "proper_person" | "individual" => "PERSON".to_string(),
        "company" | "limited_company" | "legal_entity" => "LEGAL_ENTITY".to_string(),
        "product" | "products" => "PRODUCT".to_string(),
        "service" | "services" => "SERVICE".to_string(),
        "document" | "documents" => "DOCUMENT".to_string(),
        "fund" | "funds" => "FUND".to_string(),
        other => other.to_uppercase(),
    }
}

fn extract_jurisdiction(display: &str) -> Option<String> {
    if let Some(idx) = display.find(" | ") {
        return Some(display[idx + 3..].trim().to_string());
    }
    if let Some(start) = display.rfind('(') {
        if let Some(end) = display.rfind(')') {
            if end > start {
                return Some(display[start + 1..end].trim().to_string());
            }
        }
    }
    None
}

fn infer_polarity(metadata: Option<&VerbMetadata>) -> &'static str {
    match metadata.and_then(|m| m.side_effects.as_deref()) {
        Some("facts_only") => "read",
        _ => "write",
    }
}

fn matches_subject_kind(metadata: Option<&VerbMetadata>, entity_type: &str) -> bool {
    let Some(metadata) = metadata else {
        return true;
    };
    metadata.subject_kinds.is_empty()
        || metadata
            .subject_kinds
            .iter()
            .any(|kind| entity_kind_matches(kind, entity_type))
}

fn matches_aspect(metadata: Option<&VerbMetadata>, aspect: Option<&str>) -> bool {
    let Some(aspect) = aspect else {
        return true;
    };
    let Some(metadata) = metadata else {
        return true;
    };
    metadata.phase_tags.is_empty()
        || metadata
            .phase_tags
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case(aspect))
}

fn governance_status(config: &VerbConfig) -> &'static str {
    match config
        .metadata
        .as_ref()
        .and_then(|m| m.replaced_by.as_ref())
    {
        Some(_) => "pending",
        None => "active",
    }
}

fn lane_for_verb(domain_name: &str, config: &VerbConfig) -> String {
    config
        .metadata
        .as_ref()
        .and_then(|m| m.phase_tags.first().cloned())
        .unwrap_or_else(|| domain_name.to_string())
}

fn signal_flag(entity_state: &Value, key: &str) -> bool {
    entity_state["signals"][key].as_bool().unwrap_or(false)
}

fn active_lanes(entity_state: &Value) -> Vec<String> {
    let mut lanes = Vec::new();
    if signal_flag(entity_state, "has_active_onboarding") {
        lanes.push("onboarding".to_string());
        lanes.push("cbu".to_string());
    }
    if signal_flag(entity_state, "has_active_deal") {
        lanes.push("deal".to_string());
    }
    if signal_flag(entity_state, "has_active_kyc") {
        lanes.push("kyc".to_string());
        lanes.push("case".to_string());
    }
    if signal_flag(entity_state, "has_pending_documentation") {
        lanes.push("document".to_string());
    }
    if signal_flag(entity_state, "has_incomplete_ubo") {
        lanes.push("ubo".to_string());
        lanes.push("ownership".to_string());
    }
    lanes.sort();
    lanes.dedup();
    lanes
}

fn current_phase_for_lane(entity_state: &Value, lane: &str) -> String {
    entity_state["activities"]
        .as_array()
        .and_then(|activities| {
            activities.iter().find_map(|activity| {
                let domain = activity["domain"].as_str().unwrap_or_default();
                let activity_type = activity["activity_type"].as_str().unwrap_or_default();
                if domain.eq_ignore_ascii_case(lane) || activity_type.contains(lane) {
                    activity["phase"].as_str().map(str::to_string)
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "current".to_string())
}

fn matches_lane_filter(config: &VerbConfig, domain_name: &str, lanes: &[String]) -> bool {
    if lanes.is_empty() {
        return true;
    }
    let lane = lane_for_verb(domain_name, config);
    lanes.iter().any(|candidate| {
        candidate.eq_ignore_ascii_case(domain_name)
            || candidate.eq_ignore_ascii_case(&lane)
            || config
                .metadata
                .as_ref()
                .map(|m| {
                    m.phase_tags
                        .iter()
                        .any(|tag| tag.eq_ignore_ascii_case(candidate))
                })
                .unwrap_or(false)
    })
}

fn evaluate_preconditions(config: &VerbConfig, entity_state: &Value) -> Vec<String> {
    let Some(lifecycle) = config.lifecycle.as_ref() else {
        return Vec::new();
    };

    let mut unmet = Vec::new();
    for check in &lifecycle.precondition_checks {
        let ok = match check.as_str() {
            "check_cbu_evidence_completeness" => {
                !signal_flag(entity_state, "has_pending_documentation")
            }
            "check_required_parties_present" => {
                !entity_state["signals"]["stale"].as_bool().unwrap_or(false)
            }
            "check_ubo_completeness" => !signal_flag(entity_state, "has_incomplete_ubo"),
            "check_kyc_case_approved" => !signal_flag(entity_state, "has_active_kyc"),
            "check_service_delivery_ready" => !signal_flag(entity_state, "has_active_onboarding"),
            "check_resources_provisioned" => !signal_flag(entity_state, "has_active_onboarding"),
            other => {
                // Unknown checks remain advisory, not blocking, until wired.
                tracing::debug!(precondition_check = %other, "valid-transitions skipping unknown precondition check");
                true
            }
        };
        if !ok {
            unmet.push(check.clone());
        }
    }
    unmet
}

fn derive_unblocking_actions(unmet: &[String]) -> Vec<String> {
    let mut actions = Vec::new();
    for precondition in unmet {
        match precondition.as_str() {
            "check_cbu_evidence_completeness" => {
                actions.push("document.missing-for-entity".to_string())
            }
            "check_required_parties_present" => actions.push("cbu.parties".to_string()),
            "check_ubo_completeness" => actions.push("ubo.list-ubos".to_string()),
            "check_kyc_case_approved" => actions.push("case.list".to_string()),
            "check_service_delivery_ready" => {
                actions.push("service-resource.check-lifecycle-readiness".to_string())
            }
            "check_resources_provisioned" => {
                actions.push("service-resource.list-lifecycle-instances".to_string())
            }
            _ => {}
        }
    }
    actions.sort();
    actions.dedup();
    actions
}

fn compute_relevance(domain_name: &str, config: &VerbConfig, entity_state: &Value) -> f32 {
    let lane = lane_for_verb(domain_name, config);
    let active_lanes = active_lanes(entity_state);
    let mut relevance = 0.2f32;
    if active_lanes.iter().any(|active| {
        active.eq_ignore_ascii_case(domain_name) || active.eq_ignore_ascii_case(&lane)
    }) {
        relevance += 0.5;
    }
    if infer_polarity(config.metadata.as_ref()) == "write" {
        relevance += 0.1;
    }
    if !config.invocation_phrases.is_empty() {
        relevance += 0.1;
    }
    relevance.min(1.0)
}

fn find_verb_config<'a>(
    verbs: &'a dsl_core::config::types::VerbsConfig,
    verb_id: &'a str,
) -> Option<(&'a str, &'a VerbConfig)> {
    let (domain_name, verb_name) = verb_id.split_once('.')?;
    let domain = verbs.domains.get(domain_name)?;
    let verb = domain.verbs.get(verb_name)?;
    Some((domain_name, verb))
}

fn param_summary(arg: &ArgConfig) -> Value {
    json!({
        "name": arg.name,
        "type": format!("{:?}", arg.arg_type).to_lowercase(),
        "required": arg.required,
        "description": arg.description,
    })
}

fn tool_error(result: SemRegToolResult) -> Result<Value> {
    if result.success {
        Ok(result.data)
    } else {
        Err(anyhow!(
            "{}",
            result
                .error
                .unwrap_or_else(|| "Unknown Sem OS tool error".to_string())
        ))
    }
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Default)]
struct GovernedDocumentSignalSummary {
    matched_profile_count: usize,
    pending_component_count: usize,
    rejected_component_count: usize,
    expired_component_count: usize,
    mandatory_obligations: usize,
    mandatory_satisfied_obligations: usize,
    total_obligations: usize,
    satisfied_obligations: usize,
    partially_satisfied_obligations: usize,
    unsatisfied_obligations: usize,
}

#[cfg(feature = "database")]
impl GovernedDocumentSignalSummary {
    fn mandatory_coverage(&self) -> f64 {
        if self.mandatory_obligations == 0 {
            1.0
        } else {
            self.mandatory_satisfied_obligations as f64 / self.mandatory_obligations as f64
        }
    }

    fn overall_coverage(&self) -> f64 {
        if self.total_obligations == 0 {
            1.0
        } else {
            self.satisfied_obligations as f64 / self.total_obligations as f64
        }
    }

    fn has_matches(&self) -> bool {
        self.matched_profile_count > 0
    }
}

#[cfg(feature = "database")]
fn pending_components_for_matrix(matrix: &GovernedRequirementMatrix) -> (usize, usize, usize) {
    let mut pending = 0usize;
    let mut rejected = 0usize;
    let mut expired = 0usize;

    for category in &matrix.categories {
        for obligation in &category.obligations {
            let Some(strategy) = obligation.active_strategy.as_ref() else {
                continue;
            };
            for component in &strategy.components {
                if component.status != "satisfied" {
                    pending += 1;
                    match component.status.as_str() {
                        "rejected" => rejected += 1,
                        "expired" => expired += 1,
                        _ => {}
                    }
                }
            }
        }
    }

    (pending, rejected, expired)
}

#[cfg(feature = "database")]
async fn governed_document_signal_summary_for_entities(
    pool: &PgPool,
    entity_ids: &[Uuid],
) -> Result<Option<GovernedDocumentSignalSummary>> {
    if entity_ids.is_empty() {
        return Ok(None);
    }

    let service = GovernedDocumentRequirementsService::new(pool.clone());
    let mut summary = GovernedDocumentSignalSummary::default();

    for entity_id in entity_ids {
        let Some(matrix) = service.compute_matrix_for_entity(*entity_id).await? else {
            continue;
        };

        let (pending, rejected, expired) = pending_components_for_matrix(&matrix);
        summary.matched_profile_count += 1;
        summary.pending_component_count += pending;
        summary.rejected_component_count += rejected;
        summary.expired_component_count += expired;
        summary.mandatory_obligations += matrix.mandatory_obligations;
        summary.mandatory_satisfied_obligations += matrix.mandatory_satisfied_obligations;
        summary.total_obligations += matrix.total_obligations;
        summary.satisfied_obligations += matrix.satisfied_obligations;
        summary.partially_satisfied_obligations += matrix.partially_satisfied;
        summary.unsatisfied_obligations += matrix.unsatisfied_obligations;
    }

    if summary.has_matches() {
        Ok(Some(summary))
    } else {
        Ok(None)
    }
}

#[cfg(feature = "database")]
async fn search_entities_via_db(
    pool: &PgPool,
    query: &str,
    entity_types: &[String],
    limit: i32,
) -> Result<Vec<Value>> {
    async fn lifecycle_stats_for(
        pool: &PgPool,
        entity_id: Uuid,
        entity_type: &str,
        linked_cbu_ids: &[Uuid],
    ) -> (bool, usize, bool) {
        let linked_entity_count = match entity_type {
            "client-group" => sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM "ob-poc".client_group_entity
                WHERE group_id = $1
                "#,
            )
            .bind(entity_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0_i64) as usize,
            "cbu" => sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM "ob-poc".client_group_entity
                WHERE cbu_id = $1
                "#,
            )
            .bind(entity_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0_i64) as usize,
            _ => 0,
        };

        let has_active_workflow = match entity_type {
            "client-group" => {
                let deal_count: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*) FROM "ob-poc".deals
                    WHERE primary_client_group_id = $1
                      AND deal_status NOT IN ('OFFBOARDED', 'CANCELLED')
                    "#,
                )
                .bind(entity_id)
                .fetch_one(pool)
                .await
                .unwrap_or(0);
                let onboarding_count: i64 = if linked_cbu_ids.is_empty() {
                    0
                } else {
                    sqlx::query_scalar(
                        r#"
                        SELECT COUNT(*) FROM "ob-poc".onboarding_requests
                        WHERE cbu_id = ANY($1) AND request_state <> 'complete'
                        "#,
                    )
                    .bind(linked_cbu_ids)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0)
                };
                deal_count > 0 || onboarding_count > 0
            }
            "cbu" => {
                let onboarding_count: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*) FROM "ob-poc".onboarding_requests
                    WHERE cbu_id = $1 AND request_state <> 'complete'
                    "#,
                )
                .bind(entity_id)
                .fetch_one(pool)
                .await
                .unwrap_or(0);
                let case_count: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*) FROM "ob-poc".cases
                    WHERE cbu_id = $1
                      AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'DO_NOT_ONBOARD')
                    "#,
                )
                .bind(entity_id)
                .fetch_one(pool)
                .await
                .unwrap_or(0);
                onboarding_count > 0 || case_count > 0
            }
            "deal" => {
                let status: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1
                    "#,
                )
                .bind(entity_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);
                status.is_some_and(|value| value != "OFFBOARDED" && value != "CANCELLED")
            }
            _ => !linked_cbu_ids.is_empty(),
        };

        let lifecycle_populated =
            has_active_workflow || linked_entity_count > 0 || !linked_cbu_ids.is_empty();
        (
            lifecycle_populated,
            linked_entity_count,
            has_active_workflow,
        )
    }

    async fn linked_cbu_ids_for(pool: &PgPool, entity_id: Uuid, entity_type: &str) -> Vec<Uuid> {
        match entity_type {
            "client-group" => sqlx::query_scalar(
                r#"
                    SELECT DISTINCT cbu_id
                    FROM "ob-poc".client_group_entity
                    WHERE group_id = $1
                      AND cbu_id IS NOT NULL
                    "#,
            )
            .bind(entity_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default(),
            "cbu" => vec![entity_id],
            _ => sqlx::query_scalar(
                r#"
                    SELECT DISTINCT cbu_id
                    FROM "ob-poc".client_group_entity
                    WHERE entity_id = $1
                      AND cbu_id IS NOT NULL
                    "#,
            )
            .bind(entity_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default(),
        }
    }

    let normalized_types = entity_types
        .iter()
        .map(|entity_type| entity_type.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let wants_client_groups =
        normalized_types.is_empty() || normalized_types.iter().any(|kind| kind == "client-group");
    let wants_cbus =
        normalized_types.is_empty() || normalized_types.iter().any(|kind| kind == "cbu");
    let wants_deals =
        normalized_types.is_empty() || normalized_types.iter().any(|kind| kind == "deal");
    let wants_documents =
        normalized_types.is_empty() || normalized_types.iter().any(|kind| kind == "document");
    let entity_type_filter = if normalized_types.is_empty() {
        None
    } else {
        Some(normalized_types)
    };
    let rows = sqlx::query(
        r#"
        WITH entity_hits AS (
            SELECT
                e.entity_id,
                e.name,
                COALESCE(NULLIF(et.type_code, ''), e.bods_entity_type, 'entity') AS entity_type,
                0.5::float8 AS match_score
            FROM "ob-poc".entities e
            LEFT JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            WHERE (
                e.name ILIKE $1
                OR e.name_norm ILIKE LOWER($1)
                OR e.entity_id::text ILIKE $1
            )
              AND e.deleted_at IS NULL
              AND (
                  $2::text[] IS NULL
                  OR LOWER(COALESCE(NULLIF(et.type_code, ''), e.bods_entity_type, 'entity')) = ANY($2)
              )
        ),
        client_group_hits AS (
            SELECT
                cg.id AS entity_id,
                cg.canonical_name AS name,
                'client-group'::text AS entity_type,
                CASE
                    WHEN cg.canonical_name ILIKE $1 THEN 0.95::float8
                    WHEN cga.alias_norm IS NOT NULL THEN 0.9::float8
                    ELSE 0.8::float8
                END AS match_score
            FROM "ob-poc".client_group cg
            LEFT JOIN "ob-poc".client_group_alias cga ON cga.group_id = cg.id
            WHERE $3::boolean = true
              AND (
                  cg.canonical_name ILIKE $1
                  OR cga.alias_norm ILIKE LOWER($1)
                  OR cga.alias_norm ILIKE '%' || LOWER($4) || '%'
              )
        ),
        cbu_hits AS (
            SELECT
                c.cbu_id AS entity_id,
                c.name,
                'cbu'::text AS entity_type,
                CASE
                    WHEN c.name ILIKE $1 THEN 0.92::float8
                    ELSE 0.78::float8
                END AS match_score
            FROM "ob-poc".cbus c
            WHERE $5::boolean = true
              AND c.deleted_at IS NULL
              AND (
                  c.name ILIKE $1
                  OR c.description ILIKE $1
                  OR c.cbu_id::text ILIKE $1
              )
        ),
        deal_hits AS (
            SELECT
                d.deal_id AS entity_id,
                d.deal_name AS name,
                'deal'::text AS entity_type,
                CASE
                    WHEN d.deal_name ILIKE $1 THEN 0.9::float8
                    WHEN d.deal_reference ILIKE $1 THEN 0.88::float8
                    ELSE 0.76::float8
                END AS match_score
            FROM "ob-poc".deals d
            WHERE $6::boolean = true
              AND (
                  d.deal_name ILIKE $1
                  OR COALESCE(d.deal_reference, '') ILIKE $1
                  OR d.deal_id::text ILIKE $1
              )
        ),
        document_hits AS (
            SELECT
                dc.doc_id AS entity_id,
                COALESCE(NULLIF(dc.document_name, ''), NULLIF(dc.document_type_code, ''), 'document') AS name,
                'document'::text AS entity_type,
                CASE
                    WHEN COALESCE(dc.document_name, '') ILIKE $1 THEN 0.87::float8
                    WHEN COALESCE(dc.document_type_code, '') ILIKE $1 THEN 0.8::float8
                    ELSE 0.7::float8
                END AS match_score
            FROM "ob-poc".document_catalog dc
            WHERE $7::boolean = true
              AND (
                  COALESCE(dc.document_name, '') ILIKE $1
                  OR COALESCE(dc.document_type_code, '') ILIKE $1
                  OR dc.doc_id::text ILIKE $1
              )
        )
        SELECT entity_id, name, entity_type, match_score
        FROM (
            SELECT * FROM entity_hits
            UNION ALL
            SELECT * FROM client_group_hits
            UNION ALL
            SELECT * FROM cbu_hits
            UNION ALL
            SELECT * FROM deal_hits
            UNION ALL
            SELECT * FROM document_hits
        ) hits
        ORDER BY match_score DESC, name
        LIMIT $8
        "#,
    )
    .bind(format!("%{}%", query))
    .bind(entity_type_filter)
    .bind(wants_client_groups)
    .bind(query.to_ascii_lowercase())
    .bind(wants_cbus)
    .bind(wants_deals)
    .bind(wants_documents)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let entity_id = row.get::<Uuid, _>("entity_id");
        let entity_type = row.get::<String, _>("entity_type");
        let linked_cbu_ids = linked_cbu_ids_for(pool, entity_id, entity_type.as_str()).await;
        let is_onboarding_member =
            !linked_cbu_ids.is_empty() || entity_type.eq_ignore_ascii_case("cbu");
        let (lifecycle_populated, linked_entity_count, has_active_workflow) =
            lifecycle_stats_for(pool, entity_id, entity_type.as_str(), &linked_cbu_ids).await;
        results.push(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "name": row.get::<String, _>("name"),
            "aliases": [],
            "match_score": row.get::<f64, _>("match_score"),
            "match_field": "name",
            "source_kind": "db_search",
            "linked_cbu_ids": linked_cbu_ids,
            "is_onboarding_member": is_onboarding_member,
            "candidate_for_cbu": !is_onboarding_member,
            "lifecycle_populated": lifecycle_populated,
            "linked_entity_count": linked_entity_count,
            "has_active_workflow": has_active_workflow,
            "summary": {
                "jurisdiction": Value::Null,
                "status": "active",
                "has_active_engagement": true,
                "primary_domain": Value::Null,
                "last_activity": Value::Null,
            }
        }));
    }
    Ok(results)
}

#[cfg(feature = "database")]
async fn search_entities_internal(
    pool: &PgPool,
    query: &str,
    entity_types: &[String],
    limit: i32,
) -> Result<Vec<Value>> {
    let addr = gateway_addr();
    let mut client = match EntityGatewayClient::connect(addr.clone()).await {
        Ok(client) => client,
        Err(_) => return search_entities_via_db(pool, query, entity_types, limit).await,
    };

    let effective_types = if entity_types.is_empty() {
        vec!["entity".to_string()]
    } else {
        entity_types.to_vec()
    };

    let mut hits = Vec::new();
    for entity_type in effective_types {
        let request = SearchRequest {
            nickname: normalize_entity_type(&entity_type),
            values: vec![query.to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(limit + 1),
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };
        let response = match client.search(request).await {
            Ok(response) => response,
            Err(_) => return search_entities_via_db(pool, query, entity_types, limit).await,
        };
        for hit in response
            .into_inner()
            .matches
            .into_iter()
            .take(limit as usize)
        {
            hits.push(json!({
                "entity_id": hit.token,
                "entity_type": entity_type,
                "name": hit.display,
                "aliases": [],
                "match_score": hit.score,
                "match_field": "name",
                "source_kind": "entity_gateway",
                "linked_cbu_ids": [],
                "is_onboarding_member": false,
                "candidate_for_cbu": true,
                "lifecycle_populated": false,
                "linked_entity_count": 0,
                "has_active_workflow": false,
                "summary": {
                    "jurisdiction": extract_jurisdiction(&hit.display),
                    "status": "active",
                    "has_active_engagement": true,
                    "primary_domain": Value::Null,
                    "last_activity": Value::Null,
                }
            }));
        }
    }

    hits.sort_by(|a, b| {
        b["match_score"]
            .as_f64()
            .partial_cmp(&a["match_score"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit as usize);
    Ok(hits)
}

#[cfg(feature = "database")]
async fn load_entity_record(pool: &PgPool, entity_id: Uuid) -> Result<(String, String)> {
    if let Some(row) = sqlx::query(
        r#"
        SELECT canonical_name
        FROM "ob-poc".client_group
        WHERE id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok((
            row.get::<String, _>("canonical_name"),
            "client-group".to_string(),
        ));
    }

    if let Some(row) = sqlx::query(
        r#"
        SELECT name
        FROM "ob-poc".cbus
        WHERE cbu_id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok((row.get::<String, _>("name"), "cbu".to_string()));
    }

    if let Some(row) = sqlx::query(
        r#"
        SELECT deal_name
        FROM "ob-poc".deals
        WHERE deal_id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok((row.get::<String, _>("deal_name"), "deal".to_string()));
    }

    if let Some(row) = sqlx::query(
        r#"
        SELECT COALESCE(NULLIF(document_name, ''), NULLIF(document_type_code, ''), 'document') AS name
        FROM "ob-poc".document_catalog
        WHERE doc_id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok((row.get::<String, _>("name"), "document".to_string()));
    }

    let row = sqlx::query(
        r#"
        SELECT
            e.name,
            COALESCE(NULLIF(et.type_code, ''), e.bods_entity_type, 'entity') AS entity_type
        FROM "ob-poc".entities e
        LEFT JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        WHERE e.entity_id = $1
          AND e.deleted_at IS NULL
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;

    Ok((
        row.get::<String, _>("name"),
        row.get::<String, _>("entity_type"),
    ))
}

#[cfg(feature = "database")]
fn activity_record(
    domain: &str,
    activity_type: &str,
    phase: &str,
    status: &str,
    completion_pct: Option<f64>,
    blockers: Vec<String>,
) -> Value {
    json!({
        "domain": domain,
        "activity_type": activity_type,
        "phase": phase,
        "status": status,
        "completion_pct": completion_pct,
        "blockers": blockers,
        "last_activity": Value::Null,
        "last_actor": Value::Null,
        "linked_entities": [],
    })
}

#[cfg(feature = "database")]
async fn screening_counts_for_client_group(pool: &PgPool, group_id: Uuid) -> (i64, i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (
                WHERE s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS active_total,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'SANCTIONS'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS sanctions_count,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'PEP'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS pep_count,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'ADVERSE_MEDIA'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS adverse_media_count
        FROM "ob-poc".screenings s
        JOIN "ob-poc".entity_workstreams ew ON ew.workstream_id = s.workstream_id
        JOIN "ob-poc".cases c ON c.case_id = ew.case_id
        WHERE c.client_group_id = $1
        "#,
    )
    .bind(group_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("active_total").unwrap_or(0),
            row.get::<Option<i64>, _>("sanctions_count").unwrap_or(0),
            row.get::<Option<i64>, _>("pep_count").unwrap_or(0),
            row.get::<Option<i64>, _>("adverse_media_count")
                .unwrap_or(0),
        ),
        Err(_) => (0, 0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn screening_counts_for_cbu(pool: &PgPool, cbu_id: Uuid) -> (i64, i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (
                WHERE s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS active_total,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'SANCTIONS'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS sanctions_count,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'PEP'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS pep_count,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'ADVERSE_MEDIA'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS adverse_media_count
        FROM "ob-poc".screenings s
        JOIN "ob-poc".entity_workstreams ew ON ew.workstream_id = s.workstream_id
        JOIN "ob-poc".cases c ON c.case_id = ew.case_id
        WHERE c.cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("active_total").unwrap_or(0),
            row.get::<Option<i64>, _>("sanctions_count").unwrap_or(0),
            row.get::<Option<i64>, _>("pep_count").unwrap_or(0),
            row.get::<Option<i64>, _>("adverse_media_count")
                .unwrap_or(0),
        ),
        Err(_) => (0, 0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn case_status_counts_for_client_group(pool: &PgPool, group_id: Uuid) -> (i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'DO_NOT_ONBOARD')) AS active_count,
            COUNT(*) FILTER (WHERE status = 'BLOCKED') AS blocked_count,
            COUNT(*) FILTER (WHERE status = 'REVIEW') AS review_count
        FROM "ob-poc".cases
        WHERE client_group_id = $1
        "#,
    )
    .bind(group_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("active_count").unwrap_or(0),
            row.get::<Option<i64>, _>("blocked_count").unwrap_or(0),
            row.get::<Option<i64>, _>("review_count").unwrap_or(0),
        ),
        Err(_) => (0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn case_status_counts_for_cbu(pool: &PgPool, cbu_id: Uuid) -> (i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'DO_NOT_ONBOARD')) AS active_count,
            COUNT(*) FILTER (WHERE status = 'BLOCKED') AS blocked_count,
            COUNT(*) FILTER (WHERE status = 'REVIEW') AS review_count
        FROM "ob-poc".cases
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("active_count").unwrap_or(0),
            row.get::<Option<i64>, _>("blocked_count").unwrap_or(0),
            row.get::<Option<i64>, _>("review_count").unwrap_or(0),
        ),
        Err(_) => (0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn case_status_counts_for_deal(pool: &PgPool, deal_id: Uuid) -> (i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'DO_NOT_ONBOARD')) AS active_count,
            COUNT(*) FILTER (WHERE status = 'BLOCKED') AS blocked_count,
            COUNT(*) FILTER (WHERE status = 'REVIEW') AS review_count
        FROM "ob-poc".cases
        WHERE deal_id = $1
        "#,
    )
    .bind(deal_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("active_count").unwrap_or(0),
            row.get::<Option<i64>, _>("blocked_count").unwrap_or(0),
            row.get::<Option<i64>, _>("review_count").unwrap_or(0),
        ),
        Err(_) => (0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn document_status_counts_for_client_group(
    pool: &PgPool,
    group_id: Uuid,
) -> (i64, i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE dc.status NOT IN ('archived', 'verified', 'approved')) AS pending_count,
            COUNT(*) FILTER (WHERE dc.status IN ('verified', 'approved')) AS verified_count,
            COUNT(*) FILTER (WHERE dc.status = 'rejected') AS rejected_count,
            COUNT(*) AS catalogued_count
        FROM "ob-poc".document_catalog dc
        JOIN "ob-poc".client_group_entity cge ON cge.cbu_id = dc.cbu_id
        WHERE cge.group_id = $1
        "#,
    )
    .bind(group_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("pending_count").unwrap_or(0),
            row.get::<Option<i64>, _>("verified_count").unwrap_or(0),
            row.get::<Option<i64>, _>("rejected_count").unwrap_or(0),
            row.get::<Option<i64>, _>("catalogued_count").unwrap_or(0),
        ),
        Err(_) => (0, 0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn document_status_counts_for_cbu(pool: &PgPool, cbu_id: Uuid) -> (i64, i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status NOT IN ('archived', 'verified', 'approved')) AS pending_count,
            COUNT(*) FILTER (WHERE status IN ('verified', 'approved')) AS verified_count,
            COUNT(*) FILTER (WHERE status = 'rejected') AS rejected_count,
            COUNT(*) AS catalogued_count
        FROM "ob-poc".document_catalog
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("pending_count").unwrap_or(0),
            row.get::<Option<i64>, _>("verified_count").unwrap_or(0),
            row.get::<Option<i64>, _>("rejected_count").unwrap_or(0),
            row.get::<Option<i64>, _>("catalogued_count").unwrap_or(0),
        ),
        Err(_) => (0, 0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn document_status_counts_for_deal(pool: &PgPool, deal_id: Uuid) -> (i64, i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE document_status NOT IN ('SIGNED', 'EXECUTED', 'ARCHIVED')) AS pending_count,
            COUNT(*) FILTER (WHERE document_status IN ('SIGNED', 'EXECUTED')) AS complete_count,
            COUNT(*) FILTER (WHERE document_status = 'REJECTED') AS rejected_count,
            COUNT(*) AS total_count
        FROM "ob-poc".deal_documents
        WHERE deal_id = $1
        "#,
    )
    .bind(deal_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("pending_count").unwrap_or(0),
            row.get::<Option<i64>, _>("complete_count").unwrap_or(0),
            row.get::<Option<i64>, _>("rejected_count").unwrap_or(0),
            row.get::<Option<i64>, _>("total_count").unwrap_or(0),
        ),
        Err(_) => (0, 0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn fund_counts_for_entities(pool: &PgPool, entity_ids: &[Uuid]) -> (i64, i64, i64) {
    if entity_ids.is_empty() {
        return (0, 0, 0);
    }

    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS fund_entity_count,
            COUNT(*) FILTER (WHERE parent_fund_id IS NOT NULL) AS nested_fund_count,
            COUNT(*) FILTER (WHERE master_fund_id IS NOT NULL) AS feeder_or_master_link_count
        FROM "ob-poc".entity_funds
        WHERE entity_id = ANY($1)
        "#,
    )
    .bind(entity_ids)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("fund_entity_count").unwrap_or(0),
            row.get::<Option<i64>, _>("nested_fund_count").unwrap_or(0),
            row.get::<Option<i64>, _>("feeder_or_master_link_count")
                .unwrap_or(0),
        ),
        Err(_) => (0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn fund_signals_for_entity(pool: &PgPool, entity_id: Uuid) -> (bool, bool, bool) {
    let row = sqlx::query(
        r#"
        SELECT
            EXISTS(
                SELECT 1
                FROM "ob-poc".entity_funds
                WHERE entity_id = $1
            ) AS is_fund_entity,
            EXISTS(
                SELECT 1
                FROM "ob-poc".entity_funds
                WHERE entity_id = $1
                  AND parent_fund_id IS NOT NULL
            ) AS is_nested_fund,
            EXISTS(
                SELECT 1
                FROM "ob-poc".entity_funds
                WHERE entity_id = $1
                  AND master_fund_id IS NOT NULL
            ) AS has_master_feeder_link
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<bool>, _>("is_fund_entity")
                .unwrap_or(false),
            row.get::<Option<bool>, _>("is_nested_fund")
                .unwrap_or(false),
            row.get::<Option<bool>, _>("has_master_feeder_link")
                .unwrap_or(false),
        ),
        Err(_) => (false, false, false),
    }
}

#[cfg(feature = "database")]
async fn deal_onboarding_request_counts(pool: &PgPool, deal_id: Uuid) -> (i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE request_status NOT IN ('COMPLETED', 'CANCELLED')) AS active_count,
            COUNT(*) FILTER (WHERE request_status = 'COMPLETED') AS completed_count,
            COUNT(*) AS total_count
        FROM "ob-poc".deal_onboarding_requests
        WHERE deal_id = $1
        "#,
    )
    .bind(deal_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("active_count").unwrap_or(0),
            row.get::<Option<i64>, _>("completed_count").unwrap_or(0),
            row.get::<Option<i64>, _>("total_count").unwrap_or(0),
        ),
        Err(_) => (0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn screening_counts_for_deal(pool: &PgPool, deal_id: Uuid) -> (i64, i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (
                WHERE s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS active_total,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'SANCTIONS'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS sanctions_count,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'PEP'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS pep_count,
            COUNT(*) FILTER (
                WHERE s.screening_type = 'ADVERSE_MEDIA'
                  AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            ) AS adverse_media_count
        FROM "ob-poc".screenings s
        JOIN "ob-poc".entity_workstreams ew ON ew.workstream_id = s.workstream_id
        JOIN "ob-poc".cases c ON c.case_id = ew.case_id
        WHERE c.deal_id = $1
        "#,
    )
    .bind(deal_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => (
            row.get::<Option<i64>, _>("active_total").unwrap_or(0),
            row.get::<Option<i64>, _>("sanctions_count").unwrap_or(0),
            row.get::<Option<i64>, _>("pep_count").unwrap_or(0),
            row.get::<Option<i64>, _>("adverse_media_count")
                .unwrap_or(0),
        ),
        Err(_) => (0, 0, 0, 0),
    }
}

#[cfg(feature = "database")]
async fn build_entity_context_record(
    pool: &PgPool,
    entity_id: Uuid,
    include_completed: bool,
) -> Result<Value> {
    let (name, entity_type) = load_entity_record(pool, entity_id).await?;

    if entity_type.eq_ignore_ascii_case("client-group") {
        let linked_entity_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT entity_id
            FROM "ob-poc".client_group_entity
            WHERE group_id = $1
              AND entity_id IS NOT NULL
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        let active_deal_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".deals
            WHERE primary_client_group_id = $1
              AND deal_status NOT IN ('OFFBOARDED', 'CANCELLED')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let linked_cbu_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT cbu_id
            FROM "ob-poc".client_group_entity
            WHERE group_id = $1
              AND cbu_id IS NOT NULL
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        let onboarding_active_count: i64 = if linked_cbu_ids.is_empty() {
            0
        } else {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM "ob-poc".onboarding_requests
                WHERE cbu_id = ANY($1)
                  AND request_state <> 'complete'
                "#,
            )
            .bind(&linked_cbu_ids)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        };
        let (active_kyc_count, blocked_kyc_count, review_kyc_count) =
            case_status_counts_for_client_group(pool, entity_id).await;
        let (pending_docs_count, verified_docs_count, rejected_docs_count, catalogued_docs_count) =
            document_status_counts_for_client_group(pool, entity_id).await;
        let governed_doc_summary =
            governed_document_signal_summary_for_entities(pool, &linked_entity_ids).await?;
        let effective_pending_docs_count = governed_doc_summary
            .as_ref()
            .map(|summary| summary.pending_component_count as i64)
            .unwrap_or(pending_docs_count);
        let effective_rejected_docs_count = governed_doc_summary
            .as_ref()
            .map(|summary| summary.rejected_component_count as i64)
            .unwrap_or(rejected_docs_count);
        let (
            screening_review_count,
            sanctions_screening_count,
            pep_screening_count,
            adverse_media_screening_count,
        ) = screening_counts_for_client_group(pool, entity_id).await;
        let entity_ubo_count: i64 = if linked_entity_ids.is_empty() {
            0
        } else {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM "ob-poc".entity_ubos
                WHERE entity_id = ANY($1)
                "#,
            )
            .bind(&linked_entity_ids)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        };
        let active_registry_ubo_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".kyc_ubo_registry ur
            JOIN "ob-poc".cases c ON c.case_id = ur.case_id
            WHERE c.client_group_id = $1
              AND ur.status NOT IN ('APPROVED', 'WAIVED')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let has_incomplete_ubo = active_registry_ubo_count > 0
            || (!linked_entity_ids.is_empty() && entity_ubo_count == 0);
        let (fund_entity_count, nested_fund_count, feeder_or_master_link_count) =
            fund_counts_for_entities(pool, &linked_entity_ids).await;

        let mut activities = Vec::new();
        if include_completed || active_deal_count > 0 {
            activities.push(activity_record(
                "deal",
                "deal",
                "active",
                if active_deal_count > 0 {
                    "in_progress"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }
        if include_completed || onboarding_active_count > 0 {
            activities.push(activity_record(
                "onboarding",
                "onboarding",
                "active",
                if onboarding_active_count > 0 {
                    "in_progress"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }
        if include_completed || active_kyc_count > 0 {
            activities.push(activity_record(
                "kyc",
                "kyc_review",
                "active",
                if active_kyc_count > 0 {
                    "in_progress"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }
        if include_completed || screening_review_count > 0 {
            activities.push(activity_record(
                "screening",
                "screening",
                "review",
                if screening_review_count > 0 {
                    "pending_review"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }
        if include_completed || effective_pending_docs_count > 0 {
            activities.push(activity_record(
                "document",
                "documentation",
                "collection",
                if effective_pending_docs_count > 0 {
                    "blocked"
                } else {
                    "not_started"
                },
                None,
                if effective_pending_docs_count > 0 {
                    vec!["pending documentation".to_string()]
                } else {
                    Vec::new()
                },
            ));
        }

        let signals = json!({
            "anchor": "client-group",
            "onboarding_present": onboarding_active_count > 0,
            "linked_cbu_count": linked_cbu_ids.len(),
            "linked_entity_count": linked_entity_ids.len(),
            "has_active_onboarding": onboarding_active_count > 0,
            "has_active_deal": active_deal_count > 0,
            "has_active_kyc": active_kyc_count > 0,
            "active_kyc_case_count": active_kyc_count,
            "blocked_kyc_case_count": blocked_kyc_count,
            "review_kyc_case_count": review_kyc_count,
            "has_incomplete_ubo": has_incomplete_ubo,
            "ubo_registry_pending_count": active_registry_ubo_count,
            "ubo_record_count": entity_ubo_count,
            "has_governed_document_profile": governed_doc_summary.is_some(),
            "governed_document_profile_count": governed_doc_summary.as_ref().map(|summary| summary.matched_profile_count).unwrap_or(0),
            "has_pending_documentation": effective_pending_docs_count > 0,
            "pending_document_count": effective_pending_docs_count,
            "legacy_pending_document_count": pending_docs_count,
            "verified_document_count": verified_docs_count,
            "rejected_document_count": effective_rejected_docs_count,
            "legacy_rejected_document_count": rejected_docs_count,
            "catalogued_document_count": catalogued_docs_count,
            "doc_mandatory_coverage_pct": governed_doc_summary.as_ref().map(|summary| (summary.mandatory_coverage() * 100.0).round() as i64),
            "doc_overall_coverage_pct": governed_doc_summary.as_ref().map(|summary| (summary.overall_coverage() * 100.0).round() as i64),
            "doc_requirements_satisfied": governed_doc_summary.as_ref().map(|summary| summary.pending_component_count == 0 && summary.partially_satisfied_obligations == 0 && summary.unsatisfied_obligations == 0),
            "doc_in_progress_obligation_count": governed_doc_summary.as_ref().map(|summary| summary.partially_satisfied_obligations).unwrap_or(0),
            "doc_unsatisfied_obligation_count": governed_doc_summary.as_ref().map(|summary| summary.unsatisfied_obligations).unwrap_or(0),
            "doc_expired_component_count": governed_doc_summary.as_ref().map(|summary| summary.expired_component_count).unwrap_or(0),
            "fund_entity_count": fund_entity_count,
            "nested_fund_count": nested_fund_count,
            "feeder_or_master_link_count": feeder_or_master_link_count,
            "screening_active_count": screening_review_count,
            "screening_sanctions_count": sanctions_screening_count,
            "screening_pep_count": pep_screening_count,
            "screening_adverse_media_count": adverse_media_screening_count,
            "days_since_last_activity": Value::Null,
            "stale": active_deal_count == 0
                && onboarding_active_count == 0
                && active_kyc_count == 0
                && screening_review_count == 0,
        });

        return Ok(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "name": name,
            "activities": activities,
            "signals": signals
        }));
    }

    if entity_type.eq_ignore_ascii_case("cbu") {
        let linked_entity_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT entity_id
            FROM "ob-poc".client_group_entity
            WHERE cbu_id = $1
              AND entity_id IS NOT NULL
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        let onboarding_active_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".onboarding_requests
            WHERE cbu_id = $1
              AND request_state <> 'complete'
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let (active_kyc_count, blocked_kyc_count, review_kyc_count) =
            case_status_counts_for_cbu(pool, entity_id).await;
        let (pending_docs_count, verified_docs_count, rejected_docs_count, catalogued_docs_count) =
            document_status_counts_for_cbu(pool, entity_id).await;
        let governed_doc_summary =
            governed_document_signal_summary_for_entities(pool, &linked_entity_ids).await?;
        let effective_pending_docs_count = governed_doc_summary
            .as_ref()
            .map(|summary| summary.pending_component_count as i64)
            .unwrap_or(pending_docs_count);
        let effective_rejected_docs_count = governed_doc_summary
            .as_ref()
            .map(|summary| summary.rejected_component_count as i64)
            .unwrap_or(rejected_docs_count);
        let (
            screening_review_count,
            sanctions_screening_count,
            pep_screening_count,
            adverse_media_screening_count,
        ) = screening_counts_for_cbu(pool, entity_id).await;
        let entity_ubo_count: i64 = if linked_entity_ids.is_empty() {
            0
        } else {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM "ob-poc".entity_ubos
                WHERE entity_id = ANY($1)
                "#,
            )
            .bind(&linked_entity_ids)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        };
        let active_registry_ubo_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".kyc_ubo_registry ur
            JOIN "ob-poc".cases c ON c.case_id = ur.case_id
            WHERE c.cbu_id = $1
              AND ur.status NOT IN ('APPROVED', 'WAIVED')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let has_incomplete_ubo = active_registry_ubo_count > 0
            || (!linked_entity_ids.is_empty() && entity_ubo_count == 0);
        let (fund_entity_count, nested_fund_count, feeder_or_master_link_count) =
            fund_counts_for_entities(pool, &linked_entity_ids).await;

        let mut activities = Vec::new();
        if include_completed || onboarding_active_count > 0 {
            activities.push(activity_record(
                "onboarding",
                "onboarding",
                "active",
                if onboarding_active_count > 0 {
                    "in_progress"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }
        if include_completed || active_kyc_count > 0 {
            activities.push(activity_record(
                "kyc",
                "kyc_review",
                "active",
                if active_kyc_count > 0 {
                    "in_progress"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }
        if include_completed || screening_review_count > 0 {
            activities.push(activity_record(
                "screening",
                "screening",
                "review",
                if screening_review_count > 0 {
                    "pending_review"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }
        if include_completed || effective_pending_docs_count > 0 {
            activities.push(activity_record(
                "document",
                "documentation",
                "collection",
                if effective_pending_docs_count > 0 {
                    "in_progress"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }

        let signals = json!({
            "anchor": "cbu",
            "onboarding_present": onboarding_active_count > 0,
            "linked_entity_count": linked_entity_ids.len(),
            "has_active_onboarding": onboarding_active_count > 0,
            "has_active_deal": false,
            "has_active_kyc": active_kyc_count > 0,
            "active_kyc_case_count": active_kyc_count,
            "blocked_kyc_case_count": blocked_kyc_count,
            "review_kyc_case_count": review_kyc_count,
            "has_incomplete_ubo": has_incomplete_ubo,
            "ubo_registry_pending_count": active_registry_ubo_count,
            "ubo_record_count": entity_ubo_count,
            "has_governed_document_profile": governed_doc_summary.is_some(),
            "governed_document_profile_count": governed_doc_summary.as_ref().map(|summary| summary.matched_profile_count).unwrap_or(0),
            "has_pending_documentation": effective_pending_docs_count > 0,
            "pending_document_count": effective_pending_docs_count,
            "legacy_pending_document_count": pending_docs_count,
            "verified_document_count": verified_docs_count,
            "rejected_document_count": effective_rejected_docs_count,
            "legacy_rejected_document_count": rejected_docs_count,
            "catalogued_document_count": catalogued_docs_count,
            "doc_mandatory_coverage_pct": governed_doc_summary.as_ref().map(|summary| (summary.mandatory_coverage() * 100.0).round() as i64),
            "doc_overall_coverage_pct": governed_doc_summary.as_ref().map(|summary| (summary.overall_coverage() * 100.0).round() as i64),
            "doc_requirements_satisfied": governed_doc_summary.as_ref().map(|summary| summary.pending_component_count == 0 && summary.partially_satisfied_obligations == 0 && summary.unsatisfied_obligations == 0),
            "doc_in_progress_obligation_count": governed_doc_summary.as_ref().map(|summary| summary.partially_satisfied_obligations).unwrap_or(0),
            "doc_unsatisfied_obligation_count": governed_doc_summary.as_ref().map(|summary| summary.unsatisfied_obligations).unwrap_or(0),
            "doc_expired_component_count": governed_doc_summary.as_ref().map(|summary| summary.expired_component_count).unwrap_or(0),
            "fund_entity_count": fund_entity_count,
            "nested_fund_count": nested_fund_count,
            "feeder_or_master_link_count": feeder_or_master_link_count,
            "screening_active_count": screening_review_count,
            "screening_sanctions_count": sanctions_screening_count,
            "screening_pep_count": pep_screening_count,
            "screening_adverse_media_count": adverse_media_screening_count,
            "days_since_last_activity": Value::Null,
            "stale": onboarding_active_count == 0 && active_kyc_count == 0 && screening_review_count == 0,
        });

        return Ok(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "name": name,
            "activities": activities,
            "signals": signals
        }));
    }

    if entity_type.eq_ignore_ascii_case("deal") {
        let deal_status: Option<String> = sqlx::query_scalar(
            r#"
            SELECT deal_status
            FROM "ob-poc".deals
            WHERE deal_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        let (active_kyc_count, blocked_kyc_count, review_kyc_count) =
            case_status_counts_for_deal(pool, entity_id).await;
        let (pending_docs_count, complete_docs_count, rejected_docs_count, total_docs_count) =
            document_status_counts_for_deal(pool, entity_id).await;
        let (
            active_onboarding_request_count,
            completed_onboarding_request_count,
            total_onboarding_request_count,
        ) = deal_onboarding_request_counts(pool, entity_id).await;
        let (
            screening_review_count,
            sanctions_screening_count,
            pep_screening_count,
            adverse_media_screening_count,
        ) = screening_counts_for_deal(pool, entity_id).await;

        return Ok(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "name": name,
            "activities": [
                activity_record(
                    "deal",
                    "deal",
                    deal_status.as_deref().unwrap_or("active"),
                    if deal_status.as_deref() == Some("ACTIVE") { "active" } else { "in_progress" },
                    None,
                    if pending_docs_count > 0 {
                        vec!["pending deal documents".to_string()]
                    } else {
                        Vec::new()
                    },
                )
            ],
            "signals": {
                "anchor": "deal",
                "deal_phase": deal_status.as_deref().unwrap_or("active"),
                "onboarding_present": deal_status.as_deref() == Some("ONBOARDING"),
                "has_active_onboarding": deal_status.as_deref() == Some("ONBOARDING"),
                "has_active_deal": true,
                "has_active_kyc": active_kyc_count > 0,
                "active_kyc_case_count": active_kyc_count,
                "blocked_kyc_case_count": blocked_kyc_count,
                "review_kyc_case_count": review_kyc_count,
                "has_incomplete_ubo": false,
                "has_pending_documentation": pending_docs_count > 0,
                "pending_document_count": pending_docs_count,
                "complete_document_count": complete_docs_count,
                "rejected_document_count": rejected_docs_count,
                "catalogued_document_count": total_docs_count,
                "active_onboarding_request_count": active_onboarding_request_count,
                "completed_onboarding_request_count": completed_onboarding_request_count,
                "total_onboarding_request_count": total_onboarding_request_count,
                "screening_active_count": screening_review_count,
                "screening_sanctions_count": sanctions_screening_count,
                "screening_pep_count": pep_screening_count,
                "screening_adverse_media_count": adverse_media_screening_count,
                "days_since_last_activity": Value::Null,
                "stale": false,
            }
        }));
    }

    let relationship_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".entity_relationships_current
        WHERE from_entity_id = $1 OR to_entity_id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let latest_relationship_activity: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        r#"
        SELECT MAX(created_at)
        FROM "ob-poc".entity_relationships_current
        WHERE from_entity_id = $1 OR to_entity_id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let entity_ubo_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".entity_ubos
        WHERE entity_id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let active_registry_ubo_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".kyc_ubo_registry
        WHERE subject_entity_id = $1
          AND status NOT IN ('APPROVED', 'WAIVED')
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let has_incomplete_ubo =
        active_registry_ubo_count > 0 || (relationship_count > 0 && entity_ubo_count == 0);
    let (is_fund_entity, is_nested_fund, has_master_feeder_link) =
        fund_signals_for_entity(pool, entity_id).await;
    let governed_doc_summary =
        governed_document_signal_summary_for_entities(pool, &[entity_id]).await?;
    let effective_pending_docs_count = governed_doc_summary
        .as_ref()
        .map(|summary| summary.pending_component_count as i64)
        .unwrap_or(0);

    let mut activities = if include_completed {
        vec![json!({
            "domain": "discovery",
            "activity_type": "relationship_review",
            "phase": "current",
            "status": if relationship_count > 0 { "in_progress" } else { "not_started" },
            "completion_pct": if relationship_count > 0 { json!(0.25) } else { Value::Null },
            "blockers": [],
            "last_activity": latest_relationship_activity,
            "last_actor": Value::Null,
            "linked_entities": [],
        })]
    } else {
        Vec::new()
    };
    if include_completed || is_fund_entity {
        activities.push(activity_record(
            "fund",
            "fund",
            if is_nested_fund {
                "nested"
            } else {
                "standalone"
            },
            if is_fund_entity {
                "in_progress"
            } else {
                "not_started"
            },
            None,
            Vec::new(),
        ));
    }
    if include_completed || effective_pending_docs_count > 0 {
        activities.push(activity_record(
            "document",
            "documentation",
            "collection",
            if effective_pending_docs_count > 0 {
                "in_progress"
            } else {
                "not_started"
            },
            None,
            if effective_pending_docs_count > 0 {
                vec!["pending documentation".to_string()]
            } else {
                Vec::new()
            },
        ));
    }

    let signals = json!({
        "anchor": "entity",
        "onboarding_present": false,
        "has_active_onboarding": false,
        "has_active_deal": false,
        "has_active_kyc": false,
        "has_incomplete_ubo": has_incomplete_ubo,
        "ubo_registry_pending_count": active_registry_ubo_count,
        "ubo_record_count": entity_ubo_count,
        "has_governed_document_profile": governed_doc_summary.is_some(),
        "governed_document_profile_count": governed_doc_summary.as_ref().map(|summary| summary.matched_profile_count).unwrap_or(0),
        "has_pending_documentation": effective_pending_docs_count > 0,
        "pending_document_count": effective_pending_docs_count,
        "doc_mandatory_coverage_pct": governed_doc_summary.as_ref().map(|summary| (summary.mandatory_coverage() * 100.0).round() as i64),
        "doc_overall_coverage_pct": governed_doc_summary.as_ref().map(|summary| (summary.overall_coverage() * 100.0).round() as i64),
        "doc_requirements_satisfied": governed_doc_summary.as_ref().map(|summary| summary.pending_component_count == 0 && summary.partially_satisfied_obligations == 0 && summary.unsatisfied_obligations == 0),
        "doc_in_progress_obligation_count": governed_doc_summary.as_ref().map(|summary| summary.partially_satisfied_obligations).unwrap_or(0),
        "doc_unsatisfied_obligation_count": governed_doc_summary.as_ref().map(|summary| summary.unsatisfied_obligations).unwrap_or(0),
        "doc_rejected_component_count": governed_doc_summary.as_ref().map(|summary| summary.rejected_component_count).unwrap_or(0),
        "doc_expired_component_count": governed_doc_summary.as_ref().map(|summary| summary.expired_component_count).unwrap_or(0),
        "linked_entity_count": relationship_count,
        "is_fund_entity": is_fund_entity,
        "is_nested_fund": is_nested_fund,
        "has_master_feeder_link": has_master_feeder_link,
        "days_since_last_activity": Value::Null,
        "stale": latest_relationship_activity.is_none(),
    });

    Ok(json!({
        "entity_id": entity_id,
        "entity_type": entity_type,
        "name": name,
        "activities": activities,
        "signals": signals
    }))
}

#[cfg(feature = "database")]
async fn build_entity_relationships_record(
    pool: &PgPool,
    entity_id: Uuid,
    relationship_types: &[String],
    max_depth: i32,
) -> Result<Value> {
    let (name, entity_type) = load_entity_record(pool, entity_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT
            relationship_type,
            from_entity_id,
            to_entity_id,
            percentage,
            ownership_type,
            control_type,
            from_name,
            to_name
        FROM "ob-poc".entity_relationships_current
        WHERE (from_entity_id = $1 OR to_entity_id = $1)
          AND ($2::text[] IS NULL OR relationship_type = ANY($2))
        ORDER BY relationship_type, from_name, to_name
        LIMIT 250
        "#,
    )
    .bind(entity_id)
    .bind(if relationship_types.is_empty() {
        None::<Vec<String>>
    } else {
        Some(relationship_types.to_vec())
    })
    .fetch_all(pool)
    .await?;

    let relationships: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            let from_id: Uuid = row.get("from_entity_id");
            let to_id: Uuid = row.get("to_entity_id");
            let direction = if from_id == entity_id { "outbound" } else { "inbound" };
            let (target_id, target_name) = if from_id == entity_id {
                (to_id, row.get::<String, _>("to_name"))
            } else {
                (from_id, row.get::<String, _>("from_name"))
            };
            json!({
                "relationship_type": row.get::<String, _>("relationship_type"),
                "direction": direction,
                "target": {
                    "entity_id": target_id,
                    "entity_type": "entity",
                    "name": target_name,
                },
                "depth": 1.min(max_depth),
                "metadata": {
                    "percentage": row.try_get::<Option<rust_decimal::Decimal>, _>("percentage").ok().flatten().map(|d| d.to_string()),
                    "ownership_type": row.try_get::<Option<String>, _>("ownership_type").ok().flatten(),
                    "control_type": row.try_get::<Option<String>, _>("control_type").ok().flatten(),
                }
            })
        })
        .collect();

    Ok(json!({
        "entity_id": entity_id,
        "entity_type": entity_type,
        "name": name,
        "relationships": relationships,
        "summary": {
            "total_relationships": relationships.len(),
            "ownership_chain_depth": if relationships.is_empty() { Value::Null } else { json!(1.min(max_depth)) },
            "ubo_count": Value::Null,
            "ubo_verified_count": Value::Null,
            "sub_fund_count": Value::Null,
            "active_deal_count": 0,
            "active_onboarding_count": 0,
            "client_groups": [],
        }
    }))
}

#[cfg(feature = "database")]
async fn sem_reg_tool(
    pool: &PgPool,
    ctx: &ExecutionContext,
    tool_name: &str,
    args: Value,
) -> Result<Value> {
    let actor = build_actor_from_ctx(ctx);
    let tool_ctx = SemRegToolContext {
        pool,
        actor: &actor,
        sem_os_service: None,
    };
    let result = dispatch_tool(&tool_ctx, tool_name, &args).await;
    tool_error(result)
}

#[cfg(feature = "database")]
async fn sem_reg_tool_json(
    pool: &PgPool,
    ctx: &sem_os_core::execution::VerbExecutionContext,
    tool_name: &str,
    args: Value,
) -> Result<Value> {
    let actor = crate::sem_reg::abac::ActorContext {
        actor_id: ctx.principal.actor_id.clone(),
        roles: if ctx.principal.is_admin() {
            vec!["admin".to_string(), "operator".to_string()]
        } else {
            vec!["operator".to_string()]
        },
        department: None,
        clearance: Some(crate::sem_reg::types::Classification::Internal),
        jurisdictions: vec![],
    };
    let tool_ctx = SemRegToolContext {
        pool,
        actor: &actor,
        sem_os_service: None,
    };
    let result = dispatch_tool(&tool_ctx, tool_name, &args).await;
    tool_error(result)
}

#[register_custom_op]
pub struct DiscoverySearchEntitiesOp;

#[async_trait]
impl CustomOperation for DiscoverySearchEntitiesOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "search-entities"
    }

    fn rationale(&self) -> &'static str {
        "Wraps the existing EntityGateway-backed entity search surface with a stable discovery contract"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let query = get_string_arg(verb_call, "query")
            .ok_or_else(|| anyhow!("discovery.search-entities requires :query"))?;
        let entity_types = get_list_arg(verb_call, "entity-types");
        let max_results = get_int_arg_or_default(verb_call, "max-results", 10) as i32;
        let include_inactive = get_bool_arg(verb_call, "include-inactive").unwrap_or(false);
        let results = search_entities_internal(pool, &query, &entity_types, max_results).await?;
        Ok(ExecutionResult::Record(json!({
            "query": query,
            "entity_types": entity_types,
            "include_inactive": include_inactive,
            "total_matches": results.len(),
            "results": results,
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_bool_opt, json_extract_int_opt, json_extract_string,
            json_extract_string_list_opt,
        };
        let query = json_extract_string(args, "query")?;
        let entity_types = json_extract_string_list_opt(args, "entity-types").unwrap_or_default();
        let max_results = json_extract_int_opt(args, "max-results").unwrap_or(10) as i32;
        let include_inactive = json_extract_bool_opt(args, "include-inactive").unwrap_or(false);
        let results = search_entities_internal(pool, &query, &entity_types, max_results).await?;
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            json!({
                "query": query, "entity_types": entity_types,
                "include_inactive": include_inactive,
                "total_matches": results.len(), "results": results,
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.search-entities requires database"))
    }
}

#[register_custom_op]
pub struct DiscoveryEntityContextOp;

#[async_trait]
impl CustomOperation for DiscoveryEntityContextOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "entity-context"
    }

    fn rationale(&self) -> &'static str {
        "Builds an entity context envelope from operational entity state and relationship signals"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_uuid_arg(ctx, verb_call, "entity-id")
            .ok_or_else(|| anyhow!("discovery.entity-context requires :entity-id"))?;
        let include_completed = get_bool_arg(verb_call, "include-completed").unwrap_or(false);
        Ok(ExecutionResult::Record(
            build_entity_context_record(pool, entity_id, include_completed).await?,
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_uuid};
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let include_completed = json_extract_bool_opt(args, "include-completed").unwrap_or(false);
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            build_entity_context_record(pool, entity_id, include_completed).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.entity-context requires database"))
    }
}

#[register_custom_op]
pub struct DiscoveryEntityRelationshipsOp;

#[async_trait]
impl CustomOperation for DiscoveryEntityRelationshipsOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "entity-relationships"
    }

    fn rationale(&self) -> &'static str {
        "Reads the normalized entity relationship graph and returns a stable relationship summary"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_uuid_arg(ctx, verb_call, "entity-id")
            .ok_or_else(|| anyhow!("discovery.entity-relationships requires :entity-id"))?;
        let relationship_types = get_list_arg(verb_call, "relationship-types");
        let max_depth = get_int_arg_or_default(verb_call, "max-depth", 2) as i32;
        Ok(ExecutionResult::Record(
            build_entity_relationships_record(pool, entity_id, &relationship_types, max_depth)
                .await?,
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_int_opt, json_extract_string_list_opt, json_extract_uuid,
        };
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let relationship_types =
            json_extract_string_list_opt(args, "relationship-types").unwrap_or_default();
        let max_depth = json_extract_int_opt(args, "max-depth").unwrap_or(2) as i32;
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            build_entity_relationships_record(pool, entity_id, &relationship_types, max_depth)
                .await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.entity-relationships requires database"))
    }
}

#[register_custom_op]
pub struct DiscoveryCascadeResearchOp;

#[async_trait]
impl CustomOperation for DiscoveryCascadeResearchOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "cascade-research"
    }

    fn rationale(&self) -> &'static str {
        "Composes entity search, entity context, and relationship discovery into one read-only research result"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let query = get_string_arg(verb_call, "query")
            .ok_or_else(|| anyhow!("discovery.cascade-research requires :query"))?;
        let top_n = get_int_arg_or_default(verb_call, "top-n", 3) as i32;
        let include_relationships =
            get_bool_arg(verb_call, "include-relationships").unwrap_or(true);
        let results = search_entities_internal(pool, &query, &[], top_n).await?;
        let mut entities = Vec::new();

        for hit in results {
            let entity_id = hit
                .get("entity_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let context = match entity_id {
                Some(id) => Some(build_entity_context_record(pool, id, false).await?),
                None => None,
            };
            let relationships = if include_relationships {
                match entity_id {
                    Some(id) => Some(build_entity_relationships_record(pool, id, &[], 1).await?),
                    None => None,
                }
            } else {
                None
            };
            entities.push(json!({
                "entity_id": hit["entity_id"].clone(),
                "entity_type": hit["entity_type"].clone(),
                "name": hit["name"].clone(),
                "aliases": hit["aliases"].clone(),
                "match_score": hit["match_score"].clone(),
                "match_field": hit["match_field"].clone(),
                "linked_cbu_ids": hit["linked_cbu_ids"].clone(),
                "is_onboarding_member": hit["is_onboarding_member"].clone(),
                "candidate_for_cbu": hit["candidate_for_cbu"].clone(),
                "lifecycle_populated": hit["lifecycle_populated"].clone(),
                "linked_entity_count": hit["linked_entity_count"].clone(),
                "has_active_workflow": hit["has_active_workflow"].clone(),
                "context": context,
                "relationships": relationships,
                "signals": context.as_ref().map(|c| c["signals"].clone()).unwrap_or(Value::Null),
                "likely_intents": [json!({
                    "intent": "inspect-current-state",
                    "confidence": "medium",
                    "reason": "Discovery returned grounded entity context and available relationship state"
                })],
            }));
        }

        Ok(ExecutionResult::Record(json!({
            "query": query,
            "total_entity_matches": entities.len(),
            "entities": entities,
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_int_opt, json_extract_string};
        let query = json_extract_string(args, "query")?;
        let top_n = json_extract_int_opt(args, "top-n").unwrap_or(3) as i32;
        let include_relationships =
            json_extract_bool_opt(args, "include-relationships").unwrap_or(true);
        let results = search_entities_internal(pool, &query, &[], top_n).await?;
        let mut entities = Vec::new();
        for hit in results {
            let entity_id = hit
                .get("entity_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let context = match entity_id {
                Some(id) => Some(build_entity_context_record(pool, id, false).await?),
                None => None,
            };
            let relationships = if include_relationships {
                match entity_id {
                    Some(id) => Some(build_entity_relationships_record(pool, id, &[], 1).await?),
                    None => None,
                }
            } else {
                None
            };
            entities.push(json!({
                "entity_id": hit["entity_id"].clone(),
                "entity_type": hit["entity_type"].clone(),
                "name": hit["name"].clone(),
                "aliases": hit["aliases"].clone(),
                "match_score": hit["match_score"].clone(),
                "match_field": hit["match_field"].clone(),
                "linked_cbu_ids": hit["linked_cbu_ids"].clone(),
                "is_onboarding_member": hit["is_onboarding_member"].clone(),
                "candidate_for_cbu": hit["candidate_for_cbu"].clone(),
                "lifecycle_populated": hit["lifecycle_populated"].clone(),
                "linked_entity_count": hit["linked_entity_count"].clone(),
                "has_active_workflow": hit["has_active_workflow"].clone(),
                "context": context,
                "relationships": relationships,
                "signals": context.as_ref().map(|c| c["signals"].clone()).unwrap_or(Value::Null),
                "likely_intents": [json!({"intent": "inspect-current-state", "confidence": "medium", "reason": "Discovery returned grounded entity context and available relationship state"})],
            }));
        }
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            json!({
                "query": query, "total_entity_matches": entities.len(), "entities": entities,
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.cascade-research requires database"))
    }
}

#[register_custom_op]
pub struct DiscoveryAvailableActionsOp;

#[async_trait]
impl CustomOperation for DiscoveryAvailableActionsOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "available-actions"
    }

    fn rationale(&self) -> &'static str {
        "Builds a grouped action surface from verb YAML metadata for the target domain and entity type"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let domain = get_string_arg(verb_call, "domain")
            .ok_or_else(|| anyhow!("discovery.available-actions requires :domain"))?;
        let entity_type = get_string_arg(verb_call, "entity-type")
            .ok_or_else(|| anyhow!("discovery.available-actions requires :entity-type"))?;
        let aspect = get_string_arg(verb_call, "aspect");
        let polarity = get_string_arg(verb_call, "polarity").unwrap_or_else(|| "all".to_string());

        let verbs = ConfigLoader::from_env().load_verbs()?;
        let domain_config: &DomainConfig = verbs
            .domains
            .get(&domain)
            .ok_or_else(|| anyhow!("Unknown domain: {}", domain))?;

        let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for (verb_name, config) in &domain_config.verbs {
            let verb_polarity = infer_polarity(config.metadata.as_ref());
            if polarity != "all" && polarity != verb_polarity {
                continue;
            }
            if !matches_subject_kind(config.metadata.as_ref(), &entity_type) {
                continue;
            }
            if !matches_aspect(config.metadata.as_ref(), aspect.as_deref()) {
                continue;
            }

            let group_key = config
                .metadata
                .as_ref()
                .and_then(|m| m.phase_tags.first().cloned())
                .or_else(|| aspect.clone())
                .unwrap_or_else(|| "general".to_string());
            groups.entry(group_key).or_default().push(json!({
                "verb_id": format!("{}.{}", domain, verb_name),
                "name": verb_name,
                "description": config.description,
                "polarity": verb_polarity,
                "parameters": config.args.iter().map(param_summary).collect::<Vec<_>>(),
                "preconditions": Value::Null,
                "governance_status": governance_status(config),
            }));
        }

        let groups_json: Vec<Value> = groups
            .into_iter()
            .map(|(aspect_name, verbs)| {
                json!({
                    "aspect": aspect_name,
                    "verbs": verbs,
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "domain": domain,
            "entity_type": entity_type,
            "total_verbs": groups_json.iter().map(|g| g["verbs"].as_array().map(|a| a.len()).unwrap_or(0)).sum::<usize>(),
            "groups": groups_json,
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string, json_extract_string_opt};
        let domain = json_extract_string(args, "domain")?;
        let entity_type = json_extract_string(args, "entity-type")?;
        let aspect = json_extract_string_opt(args, "aspect");
        let polarity =
            json_extract_string_opt(args, "polarity").unwrap_or_else(|| "all".to_string());

        let verbs = ConfigLoader::from_env().load_verbs()?;
        let domain_config: &DomainConfig = verbs
            .domains
            .get(&domain)
            .ok_or_else(|| anyhow!("Unknown domain: {}", domain))?;
        let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for (verb_name, config) in &domain_config.verbs {
            let verb_polarity = infer_polarity(config.metadata.as_ref());
            if polarity != "all" && polarity != verb_polarity {
                continue;
            }
            if !matches_subject_kind(config.metadata.as_ref(), &entity_type) {
                continue;
            }
            if !matches_aspect(config.metadata.as_ref(), aspect.as_deref()) {
                continue;
            }
            let group_key = config
                .metadata
                .as_ref()
                .and_then(|m| m.phase_tags.first().cloned())
                .or_else(|| aspect.clone())
                .unwrap_or_else(|| "general".to_string());
            groups.entry(group_key).or_default().push(json!({
                "verb_id": format!("{}.{}", domain, verb_name),
                "name": verb_name, "description": config.description,
                "polarity": verb_polarity,
                "parameters": config.args.iter().map(param_summary).collect::<Vec<_>>(),
                "preconditions": Value::Null, "governance_status": governance_status(config),
            }));
        }
        let groups_json: Vec<Value> = groups
            .into_iter()
            .map(|(aspect_name, verbs)| json!({"aspect": aspect_name, "verbs": verbs}))
            .collect();
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            json!({
                "domain": domain, "entity_type": entity_type,
                "total_verbs": groups_json.iter().map(|g| g["verbs"].as_array().map(|a| a.len()).unwrap_or(0)).sum::<usize>(),
                "groups": groups_json,
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.available-actions requires database"))
    }
}

#[register_custom_op]
pub struct DiscoveryVerbDetailOp;

#[async_trait]
impl CustomOperation for DiscoveryVerbDetailOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "verb-detail"
    }

    fn rationale(&self) -> &'static str {
        "Returns a normalized verb contract from YAML config and Sem OS governance detail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let verb_id = get_string_arg(verb_call, "verb-id")
            .ok_or_else(|| anyhow!("discovery.verb-detail requires :verb-id"))?;
        let verbs = ConfigLoader::from_env().load_verbs()?;
        let (domain_name, config) = find_verb_config(&verbs, &verb_id)
            .ok_or_else(|| anyhow!("Unknown verb id: {}", verb_id))?;
        let sem_reg_surface = sem_reg_tool(
            pool,
            ctx,
            "sem_reg_verb_surface",
            json!({
                "verb_fqn": verb_id,
            }),
        )
        .await
        .unwrap_or(Value::Null);

        Ok(ExecutionResult::Record(json!({
            "verb_id": verb_id,
            "domain": domain_name,
            "name": verb_id.split('.').nth(1).unwrap_or(""),
            "description": config.description,
            "polarity": infer_polarity(config.metadata.as_ref()),
            "subject_kinds": config.metadata.as_ref().map(|m| m.subject_kinds.clone()).unwrap_or_default(),
            "phase_tags": config.metadata.as_ref().map(|m| m.phase_tags.clone()).unwrap_or_default(),
            "parameters": config.args.iter().map(|arg| json!({
                "name": arg.name,
                "type": format!("{:?}", arg.arg_type).to_lowercase(),
                "required": arg.required,
                "default": arg.default,
                "description": arg.description,
                "validation": Value::Null,
            })).collect::<Vec<_>>(),
            "preconditions": sem_reg_surface.get("preconditions").cloned().unwrap_or(Value::Null),
            "postconditions": Value::Null,
            "governance": {
                "status": governance_status(config),
                "required_roles": [],
                "abac_policy": Value::Null,
                "sem_reg_surface": sem_reg_surface,
            }
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::json_extract_string;
        let verb_id = json_extract_string(args, "verb-id")?;
        let verbs = ConfigLoader::from_env().load_verbs()?;
        let (domain_name, config) = find_verb_config(&verbs, &verb_id)
            .ok_or_else(|| anyhow!("Unknown verb id: {}", verb_id))?;
        let sem_reg_surface = sem_reg_tool_json(
            pool,
            ctx,
            "sem_reg_verb_surface",
            json!({
                "verb_fqn": verb_id,
            }),
        )
        .await
        .unwrap_or(Value::Null);

        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            json!({
                "verb_id": verb_id,
                "domain": domain_name,
                "name": verb_id.split('.').nth(1).unwrap_or(""),
                "description": config.description,
                "polarity": infer_polarity(config.metadata.as_ref()),
                "subject_kinds": config.metadata.as_ref().map(|m| m.subject_kinds.clone()).unwrap_or_default(),
                "phase_tags": config.metadata.as_ref().map(|m| m.phase_tags.clone()).unwrap_or_default(),
                "parameters": config.args.iter().map(|arg| json!({
                    "name": arg.name,
                    "type": format!("{:?}", arg.arg_type).to_lowercase(),
                    "required": arg.required,
                    "default": arg.default,
                    "description": arg.description,
                    "validation": Value::Null,
                })).collect::<Vec<_>>(),
                "preconditions": sem_reg_surface.get("preconditions").cloned().unwrap_or(Value::Null),
                "postconditions": Value::Null,
                "governance": {
                    "status": governance_status(config),
                    "required_roles": [],
                    "abac_policy": Value::Null,
                    "sem_reg_surface": sem_reg_surface,
                }
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.verb-detail requires database"))
    }
}

#[register_custom_op]
pub struct DiscoveryValidTransitionsOp;

#[async_trait]
impl CustomOperation for DiscoveryValidTransitionsOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "valid-transitions"
    }

    fn rationale(&self) -> &'static str {
        "Computes the currently valid and blocked transitions for a grounded entity using existing verb contracts, lifecycle metadata, and entity-context signals"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_uuid_arg(_ctx, verb_call, "entity-id")
            .ok_or_else(|| anyhow!("discovery.valid-transitions requires :entity-id"))?;
        let include_blocked = get_bool_arg(verb_call, "include-blocked").unwrap_or(true);
        let lanes = get_list_arg(verb_call, "lanes");
        let context = build_entity_context_record(pool, entity_id, true).await?;
        let entity_type = context["entity_type"]
            .as_str()
            .unwrap_or("entity")
            .to_string();
        let entity_name = context["name"].as_str().unwrap_or("unknown").to_string();
        let verbs = ConfigLoader::from_env().load_verbs()?;

        let mut grouped_valid: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let mut blocked = Vec::new();

        for (domain_name, domain) in &verbs.domains {
            if domain_name == "discovery" {
                continue;
            }

            for (verb_name, config) in &domain.verbs {
                if !matches_subject_kind(config.metadata.as_ref(), &entity_type) {
                    continue;
                }
                if !matches_lane_filter(config, domain_name, &lanes) {
                    continue;
                }

                let unmet = evaluate_preconditions(config, &context);
                let lane = lane_for_verb(domain_name, config);
                let verb_id = format!("{}.{}", domain_name, verb_name);
                if unmet.is_empty() {
                    grouped_valid.entry(lane.clone()).or_default().push(json!({
                        "verb_id": verb_id,
                        "description": config.description,
                        "polarity": infer_polarity(config.metadata.as_ref()),
                        "invocation_phrases": config.invocation_phrases,
                        "enables": Value::Array(vec![]),
                        "parameters": config.args.iter().map(param_summary).collect::<Vec<_>>(),
                        "phase_tags": config.metadata.as_ref().map(|m| m.phase_tags.clone()).unwrap_or_default(),
                        "relevance": compute_relevance(domain_name, config, &context),
                    }));
                } else if include_blocked {
                    blocked.push(json!({
                        "verb_id": verb_id,
                        "description": config.description,
                        "unmet_preconditions": unmet,
                        "unblocking_actions": derive_unblocking_actions(&evaluate_preconditions(config, &context)),
                    }));
                }
            }
        }

        let mut lanes_json = grouped_valid
            .into_iter()
            .map(|(lane, mut valid)| {
                valid.sort_by(|left, right| {
                    right["relevance"]
                        .as_f64()
                        .partial_cmp(&left["relevance"].as_f64())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                json!({
                    "lane": lane.clone(),
                    "current_phase": current_phase_for_lane(&context, &lane),
                    "valid": valid,
                })
            })
            .collect::<Vec<_>>();
        lanes_json.sort_by(|left, right| left["lane"].as_str().cmp(&right["lane"].as_str()));
        blocked.sort_by(|left, right| left["verb_id"].as_str().cmp(&right["verb_id"].as_str()));

        let suggested_next = lanes_json
            .iter()
            .flat_map(|lane| lane["valid"].as_array().into_iter().flatten())
            .max_by(|left, right| {
                left["relevance"]
                    .as_f64()
                    .partial_cmp(&right["relevance"].as_f64())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|best| best["verb_id"].as_str().map(str::to_string));

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "entity_name": entity_name,
            "lanes": lanes_json,
            "blocked": blocked,
            "summary": {
                "total_valid": lanes_json.iter().map(|lane| lane["valid"].as_array().map(|items| items.len()).unwrap_or(0)).sum::<usize>(),
                "total_blocked": blocked.len(),
                "suggested_next": suggested_next,
            }
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_bool_opt, json_extract_string_list_opt, json_extract_uuid,
        };
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let include_blocked = json_extract_bool_opt(args, "include-blocked").unwrap_or(true);
        let lanes = json_extract_string_list_opt(args, "lanes").unwrap_or_default();
        let context = build_entity_context_record(pool, entity_id, true).await?;
        let entity_type = context["entity_type"]
            .as_str()
            .unwrap_or("entity")
            .to_string();
        let entity_name = context["name"].as_str().unwrap_or("unknown").to_string();
        let verbs = ConfigLoader::from_env().load_verbs()?;

        let mut grouped_valid: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let mut blocked = Vec::new();
        for (domain_name, domain) in &verbs.domains {
            if domain_name == "discovery" {
                continue;
            }
            for (verb_name, config) in &domain.verbs {
                if !matches_subject_kind(config.metadata.as_ref(), &entity_type) {
                    continue;
                }
                if !matches_lane_filter(config, domain_name, &lanes) {
                    continue;
                }
                let unmet = evaluate_preconditions(config, &context);
                let lane = lane_for_verb(domain_name, config);
                let verb_id = format!("{}.{}", domain_name, verb_name);
                if unmet.is_empty() {
                    grouped_valid.entry(lane.clone()).or_default().push(json!({
                        "verb_id": verb_id, "description": config.description,
                        "polarity": infer_polarity(config.metadata.as_ref()),
                        "invocation_phrases": config.invocation_phrases,
                        "enables": Value::Array(vec![]),
                        "parameters": config.args.iter().map(param_summary).collect::<Vec<_>>(),
                        "phase_tags": config.metadata.as_ref().map(|m| m.phase_tags.clone()).unwrap_or_default(),
                        "relevance": compute_relevance(domain_name, config, &context),
                    }));
                } else if include_blocked {
                    blocked.push(json!({
                        "verb_id": verb_id, "description": config.description,
                        "unmet_preconditions": unmet,
                        "unblocking_actions": derive_unblocking_actions(&evaluate_preconditions(config, &context)),
                    }));
                }
            }
        }
        let mut lanes_json = grouped_valid.into_iter().map(|(lane, mut valid)| {
            valid.sort_by(|left, right| right["relevance"].as_f64().partial_cmp(&left["relevance"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
            json!({"lane": lane.clone(), "current_phase": current_phase_for_lane(&context, &lane), "valid": valid})
        }).collect::<Vec<_>>();
        lanes_json.sort_by(|left, right| left["lane"].as_str().cmp(&right["lane"].as_str()));
        blocked.sort_by(|left, right| left["verb_id"].as_str().cmp(&right["verb_id"].as_str()));
        let suggested_next = lanes_json
            .iter()
            .flat_map(|lane| lane["valid"].as_array().into_iter().flatten())
            .max_by(|left, right| {
                left["relevance"]
                    .as_f64()
                    .partial_cmp(&right["relevance"].as_f64())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|best| best["verb_id"].as_str().map(str::to_string));

        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            json!({
                "entity_id": entity_id, "entity_type": entity_type, "entity_name": entity_name,
                "lanes": lanes_json, "blocked": blocked,
                "summary": {
                    "total_valid": lanes_json.iter().map(|lane| lane["valid"].as_array().map(|items| items.len()).unwrap_or(0)).sum::<usize>(),
                    "total_blocked": blocked.len(),
                    "suggested_next": suggested_next,
                }
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.valid-transitions requires database"))
    }
}

#[register_custom_op]
pub struct DiscoveryGraphWalkOp;

#[async_trait]
impl CustomOperation for DiscoveryGraphWalkOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "graph-walk"
    }

    fn rationale(&self) -> &'static str {
        "Walks applicable authored StateGraphs for a grounded entity and returns frontier and blocked actions"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_uuid_arg(ctx, verb_call, "entity-id")
            .ok_or_else(|| anyhow!("discovery.graph-walk requires :entity-id"))?;
        let include_blocked = get_bool_arg(verb_call, "include-blocked").unwrap_or(true);
        let context = build_entity_context_record(pool, entity_id, true).await?;
        let entity_type = context["entity_type"].as_str().unwrap_or("entity");

        let graphs = load_state_graphs()?;
        validate_graphs(&graphs)?;
        let applicable = graphs
            .into_iter()
            .filter(|graph| {
                graph.entity_types.is_empty()
                    || graph
                        .entity_types
                        .iter()
                        .any(|candidate| entity_kind_matches(candidate, entity_type))
            })
            .collect::<Vec<_>>();

        let mut valid = BTreeMap::<String, Value>::new();
        let mut blocked = BTreeMap::<String, Value>::new();
        let mut satisfied_nodes = BTreeSet::new();
        let mut frontier_nodes = BTreeSet::new();
        let mut gate_status = Vec::new();
        let mut graph_ids = Vec::new();

        for graph in applicable {
            let result = walk_graph(&graph, &context)?;
            graph_ids.push(result.graph_id.clone());
            satisfied_nodes.extend(result.satisfied_nodes);
            frontier_nodes.extend(result.frontier_nodes);
            gate_status.extend(
                result
                    .gate_status
                    .into_iter()
                    .map(|status| serde_json::to_value(status).unwrap_or(Value::Null)),
            );
            for verb in result.valid_verbs {
                valid
                    .entry(verb.verb_id.clone())
                    .or_insert_with(|| serde_json::to_value(verb).unwrap_or(Value::Null));
            }
            if include_blocked {
                for verb in result.blocked_verbs {
                    blocked
                        .entry(verb.verb_id.clone())
                        .or_insert_with(|| serde_json::to_value(verb).unwrap_or(Value::Null));
                }
            }
        }

        let valid_verbs = valid.into_values().collect::<Vec<_>>();
        let blocked_verbs = blocked.into_values().collect::<Vec<_>>();

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "entity_name": context["name"].as_str().unwrap_or("unknown"),
            "graph_ids": graph_ids,
            "satisfied_nodes": satisfied_nodes.into_iter().collect::<Vec<_>>(),
            "frontier_nodes": frontier_nodes.into_iter().collect::<Vec<_>>(),
            "valid_verbs": valid_verbs,
            "blocked_verbs": blocked_verbs,
            "gate_status": gate_status,
            "summary": {
                "total_valid": valid_verbs.len(),
                "total_blocked": blocked_verbs.len(),
            }
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_uuid};
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let include_blocked = json_extract_bool_opt(args, "include-blocked").unwrap_or(true);
        let context = build_entity_context_record(pool, entity_id, true).await?;
        let entity_type = context["entity_type"].as_str().unwrap_or("entity");

        let graphs = load_state_graphs()?;
        validate_graphs(&graphs)?;
        let applicable = graphs
            .into_iter()
            .filter(|graph| {
                graph.entity_types.is_empty()
                    || graph
                        .entity_types
                        .iter()
                        .any(|candidate| entity_kind_matches(candidate, entity_type))
            })
            .collect::<Vec<_>>();

        let mut valid = BTreeMap::<String, Value>::new();
        let mut blocked_map = BTreeMap::<String, Value>::new();
        let mut satisfied_nodes = BTreeSet::new();
        let mut frontier_nodes = BTreeSet::new();
        let mut gate_status = Vec::new();
        let mut graph_ids = Vec::new();
        for graph in applicable {
            let result = walk_graph(&graph, &context)?;
            graph_ids.push(result.graph_id.clone());
            satisfied_nodes.extend(result.satisfied_nodes);
            frontier_nodes.extend(result.frontier_nodes);
            gate_status.extend(
                result
                    .gate_status
                    .into_iter()
                    .map(|status| serde_json::to_value(status).unwrap_or(Value::Null)),
            );
            for verb in result.valid_verbs {
                valid
                    .entry(verb.verb_id.clone())
                    .or_insert_with(|| serde_json::to_value(verb).unwrap_or(Value::Null));
            }
            if include_blocked {
                for verb in result.blocked_verbs {
                    blocked_map
                        .entry(verb.verb_id.clone())
                        .or_insert_with(|| serde_json::to_value(verb).unwrap_or(Value::Null));
                }
            }
        }
        let valid_verbs = valid.into_values().collect::<Vec<_>>();
        let blocked_verbs = blocked_map.into_values().collect::<Vec<_>>();
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            json!({
                "entity_id": entity_id, "entity_type": entity_type,
                "entity_name": context["name"].as_str().unwrap_or("unknown"),
                "graph_ids": graph_ids,
                "satisfied_nodes": satisfied_nodes.into_iter().collect::<Vec<_>>(),
                "frontier_nodes": frontier_nodes.into_iter().collect::<Vec<_>>(),
                "valid_verbs": valid_verbs, "blocked_verbs": blocked_verbs,
                "gate_status": gate_status,
                "summary": {"total_valid": valid_verbs.len(), "total_blocked": blocked_verbs.len()}
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.graph-walk requires database"))
    }
}

#[register_custom_op]
pub struct DiscoveryInspectDataOp;

#[async_trait]
impl CustomOperation for DiscoveryInspectDataOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "inspect-data"
    }

    fn rationale(&self) -> &'static str {
        "Builds a scoped discovery snapshot by combining entity context and available actions"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_uuid_arg(ctx, verb_call, "entity-id")
            .ok_or_else(|| anyhow!("discovery.inspect-data requires :entity-id"))?;
        let domain = get_string_arg(verb_call, "domain")
            .ok_or_else(|| anyhow!("discovery.inspect-data requires :domain"))?;
        let aspect = get_string_arg(verb_call, "aspect");
        let depth = get_string_arg(verb_call, "depth").unwrap_or_else(|| "summary".to_string());
        let context = build_entity_context_record(pool, entity_id, false).await?;
        let entity_type = context["entity_type"]
            .as_str()
            .unwrap_or("entity")
            .to_string();

        let verbs = ConfigLoader::from_env().load_verbs()?;
        let action_count = verbs
            .domains
            .get(&domain)
            .map(|d| {
                d.verbs
                    .values()
                    .filter(|cfg| matches_subject_kind(cfg.metadata.as_ref(), &entity_type))
                    .filter(|cfg| matches_aspect(cfg.metadata.as_ref(), aspect.as_deref()))
                    .count()
            })
            .unwrap_or(0);

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "domain": domain,
            "aspect": aspect,
            "snapshot_at": chrono::Utc::now(),
            "data": {
                "entity_context": context,
                "available_action_count": action_count,
                "depth": depth,
            },
            "summary": {
                "record_count": 1,
                "complete_count": if depth == "detail" { 1 } else { 0 },
                "incomplete_count": 0,
                "blocked_count": 0,
                "last_modified": Value::Null,
                "notable_gaps": [],
            }
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string, json_extract_string_opt, json_extract_uuid};
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let domain = json_extract_string(args, "domain")?;
        let aspect = json_extract_string_opt(args, "aspect");
        let depth = json_extract_string_opt(args, "depth").unwrap_or_else(|| "summary".to_string());
        let context = build_entity_context_record(pool, entity_id, false).await?;
        let entity_type = context["entity_type"]
            .as_str()
            .unwrap_or("entity")
            .to_string();

        let verbs = ConfigLoader::from_env().load_verbs()?;
        let action_count = verbs
            .domains
            .get(&domain)
            .map(|d| {
                d.verbs
                    .values()
                    .filter(|cfg| matches_subject_kind(cfg.metadata.as_ref(), &entity_type))
                    .filter(|cfg| matches_aspect(cfg.metadata.as_ref(), aspect.as_deref()))
                    .count()
            })
            .unwrap_or(0);

        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            json!({
                "entity_id": entity_id, "entity_type": entity_type,
                "domain": domain, "aspect": aspect,
                "snapshot_at": chrono::Utc::now(),
                "data": {"entity_context": context, "available_action_count": action_count, "depth": depth},
                "summary": {"record_count": 1, "complete_count": if depth == "detail" { 1 } else { 0 },
                    "incomplete_count": 0, "blocked_count": 0, "last_modified": Value::Null, "notable_gaps": []}
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.inspect-data requires database"))
    }
}

#[register_custom_op]
pub struct DiscoverySearchDataOp;

#[async_trait]
impl CustomOperation for DiscoverySearchDataOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }

    fn verb(&self) -> &'static str {
        "search-data"
    }

    fn rationale(&self) -> &'static str {
        "Wraps Sem OS search with entity/domain scope and a normalized data-search result envelope"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_uuid_arg(ctx, verb_call, "entity-id")
            .ok_or_else(|| anyhow!("discovery.search-data requires :entity-id"))?;
        let domain = get_string_arg(verb_call, "domain")
            .ok_or_else(|| anyhow!("discovery.search-data requires :domain"))?;
        let query = get_string_arg(verb_call, "query")
            .ok_or_else(|| anyhow!("discovery.search-data requires :query"))?;
        let aspect = get_string_arg(verb_call, "aspect");
        let max_results = get_int_arg_or_default(verb_call, "max-results", 20) as usize;

        let result = sem_reg_tool(
            pool,
            ctx,
            "sem_reg_search",
            json!({
                "query": query,
                "limit": max_results,
            }),
        )
        .await?;
        let filtered_results: Vec<Value> = result
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|item| {
                item.get("fqn")
                    .and_then(|v| v.as_str())
                    .map(|fqn| fqn.starts_with(&format!("{}.", domain)))
                    .unwrap_or(true)
            })
            .take(max_results)
            .map(|item| {
                json!({
                    "record_type": item.get("object_type").cloned().unwrap_or(json!("registry_object")),
                    "record_id": item.get("object_id").cloned().unwrap_or(Value::Null),
                    "match_field": "definition",
                    "match_value": item.get("fqn").cloned().unwrap_or(Value::Null),
                    "match_score": item.get("score").cloned().unwrap_or(Value::Null),
                    "context": {
                        "domain": domain,
                        "aspect": aspect,
                        "entity_id": entity_id,
                    }
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id,
            "domain": domain,
            "query": query,
            "aspect": aspect,
            "total_matches": filtered_results.len(),
            "results": filtered_results,
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_int_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
        };
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let domain = json_extract_string(args, "domain")?;
        let query = json_extract_string(args, "query")?;
        let aspect = json_extract_string_opt(args, "aspect");
        let max_results = json_extract_int_opt(args, "max-results").unwrap_or(20) as usize;

        let result = sem_reg_tool_json(
            pool,
            ctx,
            "sem_reg_search",
            json!({
                "query": query,
                "limit": max_results,
            }),
        )
        .await?;
        let filtered_results: Vec<Value> = result
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|item| {
                item.get("fqn")
                    .and_then(|v| v.as_str())
                    .map(|fqn| fqn.starts_with(&format!("{}.", domain)))
                    .unwrap_or(true)
            })
            .take(max_results)
            .map(|item| {
                json!({
                    "record_type": item.get("object_type").cloned().unwrap_or(json!("registry_object")),
                    "record_id": item.get("object_id").cloned().unwrap_or(Value::Null),
                    "match_field": "definition",
                    "match_value": item.get("fqn").cloned().unwrap_or(Value::Null),
                    "match_score": item.get("score").cloned().unwrap_or(Value::Null),
                    "context": {
                        "domain": domain,
                        "aspect": aspect,
                        "entity_id": entity_id,
                    }
                })
            })
            .collect();

        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            json!({
                "entity_id": entity_id,
                "domain": domain,
                "query": query,
                "aspect": aspect,
                "total_matches": filtered_results.len(),
                "results": filtered_results,
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.search-data requires database"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_ops::CustomOperationRegistry;
    use dsl_core::config::types::VerbStatus;

    #[test]
    fn discovery_domain_is_loaded_from_yaml() {
        let verbs = ConfigLoader::from_env()
            .load_verbs()
            .expect("verbs config should load");
        let domain = verbs
            .domains
            .get("discovery")
            .expect("discovery domain should exist");
        assert!(domain.verbs.contains_key("search-entities"));
        assert!(domain.verbs.contains_key("available-actions"));
        assert!(domain.verbs.contains_key("verb-detail"));
        assert!(domain.verbs.contains_key("valid-transitions"));
        assert!(domain.verbs.contains_key("graph-walk"));
    }

    #[test]
    fn discovery_custom_ops_are_registered() {
        let registry = CustomOperationRegistry::new();
        assert!(registry.has("discovery", "search-entities"));
        assert!(registry.has("discovery", "entity-context"));
        assert!(registry.has("discovery", "entity-relationships"));
        assert!(registry.has("discovery", "cascade-research"));
        assert!(registry.has("discovery", "available-actions"));
        assert!(registry.has("discovery", "verb-detail"));
        assert!(registry.has("discovery", "valid-transitions"));
        assert!(registry.has("discovery", "graph-walk"));
        assert!(registry.has("discovery", "inspect-data"));
        assert!(registry.has("discovery", "search-data"));
    }

    #[test]
    fn matches_subject_kind_uses_canonical_entity_kind_aliases() {
        let metadata = VerbMetadata {
            tier: None,
            source_of_truth: None,
            scope: None,
            writes_operational: false,
            side_effects: None,
            harm_class: None,
            action_class: None,
            noun: None,
            internal: false,
            tags: Vec::new(),
            replaces: None,
            status: VerbStatus::Active,
            replaced_by: None,
            since_version: None,
            removal_version: None,
            dangerous: false,
            subject_kinds: vec!["client-group".to_string(), "kyc-case".to_string()],
            phase_tags: Vec::new(),
            requires_subject: true,
            produces_focus: false,
        };

        assert!(matches_subject_kind(Some(&metadata), "client_group"));
        assert!(matches_subject_kind(Some(&metadata), "kyc_case"));
        assert!(!matches_subject_kind(Some(&metadata), "deal"));
    }
}
