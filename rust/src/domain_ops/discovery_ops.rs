//! Discovery domain CustomOps for the SemTaxonomy replacement path.
//!
//! These verbs expose a read-only discovery surface over existing entity search,
//! SemReg registry/schema tooling, and lightweight operational context queries.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_core::config::loader::ConfigLoader;
use dsl_core::config::types::{ArgConfig, DomainConfig, VerbConfig, VerbMetadata};
use ob_poc_macros::register_custom_op;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

use super::sem_reg_helpers::{build_actor_from_ctx, get_bool_arg, get_string_arg};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};
use crate::dsl_v2::gateway_resolver::gateway_addr;
use crate::sem_reg::agent::mcp_tools::{dispatch_tool, SemRegToolContext, SemRegToolResult};

#[cfg(feature = "database")]
use {
    entity_gateway::proto::ob::gateway::v1::{
        entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
    },
    sqlx::{PgPool, Row},
};

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
    verb_call.arguments.iter().find(|a| a.key == name).and_then(|arg| {
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
            .any(|kind| kind.eq_ignore_ascii_case(entity_type))
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
    match config.metadata.as_ref().and_then(|m| m.replaced_by.as_ref()) {
        Some(_) => "pending",
        None => "active",
    }
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
            result.error.unwrap_or_else(|| "Unknown SemReg tool error".to_string())
        ))
    }
}

#[cfg(feature = "database")]
async fn search_entities_via_db(
    pool: &PgPool,
    query: &str,
    entity_types: &[String],
    limit: i32,
) -> Result<Vec<Value>> {
    async fn linked_cbu_ids_for(pool: &PgPool, entity_id: Uuid, entity_type: &str) -> Vec<Uuid> {
        match entity_type {
            "client-group" => {
                sqlx::query_scalar(
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
                .unwrap_or_default()
            }
            "cbu" => vec![entity_id],
            _ => {
                sqlx::query_scalar(
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
                .unwrap_or_default()
            }
        }
    }

    let normalized_types = entity_types
        .iter()
        .map(|entity_type| entity_type.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let wants_client_groups =
        normalized_types.is_empty() || normalized_types.iter().any(|kind| kind == "client-group");
    let wants_cbus = normalized_types.is_empty() || normalized_types.iter().any(|kind| kind == "cbu");
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
        for hit in response.into_inner().matches.into_iter().take(limit as usize) {
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
        return Ok((row.get::<String, _>("canonical_name"), "client-group".to_string()));
    }

    if let Some(row) = sqlx::query(
        r#"
        SELECT name
        FROM "ob-poc".cbus
        WHERE cbu_id = $1
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
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;

    Ok((row.get::<String, _>("name"), row.get::<String, _>("entity_type")))
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
async fn build_entity_context_record(
    pool: &PgPool,
    entity_id: Uuid,
    include_completed: bool,
) -> Result<Value> {
    let (name, entity_type) = load_entity_record(pool, entity_id).await?;

    if entity_type.eq_ignore_ascii_case("client-group") {
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
        let active_kyc_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".cases
            WHERE client_group_id = $1
              AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'DO_NOT_ONBOARD')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let pending_docs_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".deal_documents dd
            JOIN "ob-poc".deals d ON d.deal_id = dd.deal_id
            WHERE d.primary_client_group_id = $1
              AND dd.document_status NOT IN ('SIGNED', 'EXECUTED', 'ARCHIVED')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let screening_review_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".screenings s
            JOIN "ob-poc".entity_workstreams ew ON ew.workstream_id = s.workstream_id
            JOIN "ob-poc".cases c ON c.case_id = ew.case_id
            WHERE c.client_group_id = $1
              AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

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
        if include_completed || pending_docs_count > 0 {
            activities.push(activity_record(
                "document",
                "documentation",
                "collection",
                if pending_docs_count > 0 {
                    "blocked"
                } else {
                    "not_started"
                },
                None,
                if pending_docs_count > 0 {
                    vec!["pending documentation".to_string()]
                } else {
                    Vec::new()
                },
            ));
        }

        return Ok(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "name": name,
            "activities": activities,
            "signals": {
                "anchor": "client-group",
                "onboarding_present": onboarding_active_count > 0,
                "has_active_onboarding": onboarding_active_count > 0,
                "has_active_deal": active_deal_count > 0,
                "has_active_kyc": active_kyc_count > 0,
                "has_incomplete_ubo": false,
                "has_pending_documentation": pending_docs_count > 0,
                "days_since_last_activity": Value::Null,
                "stale": active_deal_count == 0
                    && onboarding_active_count == 0
                    && active_kyc_count == 0
                    && screening_review_count == 0,
            }
        }));
    }

    if entity_type.eq_ignore_ascii_case("cbu") {
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
        let active_kyc_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".cases
            WHERE cbu_id = $1
              AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'DO_NOT_ONBOARD')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let pending_docs_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".document_catalog
            WHERE cbu_id = $1
              AND status <> 'archived'
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let screening_review_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".screenings s
            JOIN "ob-poc".entity_workstreams ew ON ew.workstream_id = s.workstream_id
            JOIN "ob-poc".cases c ON c.case_id = ew.case_id
            WHERE c.cbu_id = $1
              AND s.status IN ('PENDING', 'RUNNING', 'HIT_PENDING_REVIEW', 'HIT_CONFIRMED')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

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
        if include_completed || pending_docs_count > 0 {
            activities.push(activity_record(
                "document",
                "documentation",
                "collection",
                if pending_docs_count > 0 {
                    "in_progress"
                } else {
                    "not_started"
                },
                None,
                Vec::new(),
            ));
        }

        return Ok(json!({
            "entity_id": entity_id,
            "entity_type": entity_type,
            "name": name,
            "activities": activities,
            "signals": {
                "anchor": "cbu",
                "onboarding_present": onboarding_active_count > 0,
                "has_active_onboarding": onboarding_active_count > 0,
                "has_active_deal": false,
                "has_active_kyc": active_kyc_count > 0,
                "has_incomplete_ubo": false,
                "has_pending_documentation": pending_docs_count > 0,
                "days_since_last_activity": Value::Null,
                "stale": onboarding_active_count == 0 && active_kyc_count == 0 && screening_review_count == 0,
            }
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
        let linked_case_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".cases
            WHERE deal_id = $1
              AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'DO_NOT_ONBOARD')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let pending_docs_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".deal_documents
            WHERE deal_id = $1
              AND document_status NOT IN ('SIGNED', 'EXECUTED', 'ARCHIVED')
            "#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

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
                "onboarding_present": deal_status.as_deref() == Some("ONBOARDING"),
                "has_active_onboarding": deal_status.as_deref() == Some("ONBOARDING"),
                "has_active_deal": true,
                "has_active_kyc": linked_case_count > 0,
                "has_incomplete_ubo": false,
                "has_pending_documentation": pending_docs_count > 0,
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

    let activities = if include_completed {
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

    Ok(json!({
        "entity_id": entity_id,
        "entity_type": entity_type,
        "name": name,
        "activities": activities,
        "signals": {
            "anchor": "entity",
            "onboarding_present": false,
            "has_active_onboarding": false,
            "has_active_deal": false,
            "has_active_kyc": false,
            "has_incomplete_ubo": relationship_count > 0,
            "has_pending_documentation": false,
            "days_since_last_activity": Value::Null,
            "stale": latest_relationship_activity.is_none(),
        }
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

    #[cfg(not(feature = "database"))]
    async fn execute(&self, _verb_call: &VerbCall, _ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
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

    #[cfg(not(feature = "database"))]
    async fn execute(&self, _verb_call: &VerbCall, _ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
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

    #[cfg(not(feature = "database"))]
    async fn execute(&self, _verb_call: &VerbCall, _ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
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
        let include_relationships = get_bool_arg(verb_call, "include-relationships").unwrap_or(true);
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

    #[cfg(not(feature = "database"))]
    async fn execute(&self, _verb_call: &VerbCall, _ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
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

    #[cfg(not(feature = "database"))]
    async fn execute(&self, _verb_call: &VerbCall, _ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
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
        "Returns a normalized verb contract from YAML config and SemReg governance detail"
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
        let sem_reg_surface = sem_reg_tool(pool, ctx, "sem_reg_verb_surface", json!({
            "verb_fqn": verb_id,
        }))
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

    #[cfg(not(feature = "database"))]
    async fn execute(&self, _verb_call: &VerbCall, _ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.verb-detail requires database"))
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
        let entity_type = context["entity_type"].as_str().unwrap_or("entity").to_string();

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

    #[cfg(not(feature = "database"))]
    async fn execute(&self, _verb_call: &VerbCall, _ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
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
        "Wraps SemReg search with entity/domain scope and a normalized data-search result envelope"
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

        let result = sem_reg_tool(pool, ctx, "sem_reg_search", json!({
            "query": query,
            "limit": max_results,
        }))
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

    #[cfg(not(feature = "database"))]
    async fn execute(&self, _verb_call: &VerbCall, _ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
        Err(anyhow!("discovery.search-data requires database"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_ops::CustomOperationRegistry;

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
        assert!(registry.has("discovery", "inspect-data"));
        assert!(registry.has("discovery", "search-data"));
    }
}
