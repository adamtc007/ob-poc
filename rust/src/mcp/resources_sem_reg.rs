//! Semantic Registry MCP Resource handlers.
//!
//! Implements 9 `sem_reg://` resource URIs for the MCP `resources/list` and
//! `resources/read` surface. Each handler reads from `RegistryService`,
//! applies ABAC via `enforce_read()`, and returns JSON content.
//!
//! When a `SemOsClient` is available, `read_resource_via_client()` routes
//! resource reads through the DI boundary instead of direct DB calls.

use serde_json::json;
use sqlx::PgPool;

use super::protocol::{Resource, ResourceReadResult, ResourceTemplate};
use crate::sem_reg::{
    abac::ActorContext,
    context_resolution::{
        resolve_context, ContextResolutionRequest, EvidenceMode, ResolutionConstraints, SubjectRef,
    },
    enforce::{enforce_read, EnforceResult},
    projections::metrics::MetricsStore,
    store::SnapshotStore,
    types::{ObjectType, SnapshotRow},
};

// ── Resource listing ──────────────────────────────────────────

/// Static resources (no URI parameter).
pub fn static_resources() -> Vec<Resource> {
    vec![Resource {
        uri: "sem_reg://coverage".into(),
        name: "Registry Coverage Report".into(),
        description: Some(
            "Coverage metrics: classification, stewardship, policy, evidence, security labels"
                .into(),
        ),
        mime_type: Some("application/json".into()),
    }]
}

/// Parameterised resource templates.
pub fn resource_templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate {
            uri_template: "sem_reg://attributes/{fqn}".into(),
            name: "Attribute Definition".into(),
            description: Some("Resolve an attribute definition by FQN".into()),
            mime_type: Some("application/json".into()),
        },
        ResourceTemplate {
            uri_template: "sem_reg://entity-types/{fqn}".into(),
            name: "Entity Type Definition".into(),
            description: Some("Resolve an entity type definition by FQN".into()),
            mime_type: Some("application/json".into()),
        },
        ResourceTemplate {
            uri_template: "sem_reg://verbs/{fqn}".into(),
            name: "Verb Contract".into(),
            description: Some("Resolve a verb contract by FQN".into()),
            mime_type: Some("application/json".into()),
        },
        ResourceTemplate {
            uri_template: "sem_reg://taxonomies/{fqn}".into(),
            name: "Taxonomy Definition".into(),
            description: Some("Resolve a taxonomy definition by FQN".into()),
            mime_type: Some("application/json".into()),
        },
        ResourceTemplate {
            uri_template: "sem_reg://views/{fqn}".into(),
            name: "View Definition".into(),
            description: Some("Resolve a view definition by FQN".into()),
            mime_type: Some("application/json".into()),
        },
        ResourceTemplate {
            uri_template: "sem_reg://policies/{fqn}".into(),
            name: "Policy Rule".into(),
            description: Some("Resolve a policy rule by FQN".into()),
            mime_type: Some("application/json".into()),
        },
        ResourceTemplate {
            uri_template: "sem_reg://evidence/{fqn}".into(),
            name: "Evidence Requirement".into(),
            description: Some("Resolve an evidence requirement by FQN".into()),
            mime_type: Some("application/json".into()),
        },
        ResourceTemplate {
            uri_template: "sem_reg://context/{subject_id}".into(),
            name: "Context Resolution".into(),
            description: Some(
                "Run context resolution for a subject entity, returning ranked verbs, attributes, and governance signals".into(),
            ),
            mime_type: Some("application/json".into()),
        },
    ]
}

// ── URI dispatch ──────────────────────────────────────────────

/// Route a `sem_reg://` URI to the appropriate handler.
/// Returns `ResourceReadResult` with JSON content or a not-found stub.
pub async fn read_resource(uri: &str, pool: &PgPool, actor: &ActorContext) -> ResourceReadResult {
    match parse_uri(uri) {
        Some(("attributes", fqn)) => {
            read_by_fqn(uri, pool, actor, ObjectType::AttributeDef, fqn).await
        }
        Some(("entity-types", fqn)) => {
            read_by_fqn(uri, pool, actor, ObjectType::EntityTypeDef, fqn).await
        }
        Some(("verbs", fqn)) => read_by_fqn(uri, pool, actor, ObjectType::VerbContract, fqn).await,
        Some(("taxonomies", fqn)) => {
            read_by_fqn(uri, pool, actor, ObjectType::TaxonomyDef, fqn).await
        }
        Some(("views", fqn)) => read_by_fqn(uri, pool, actor, ObjectType::ViewDef, fqn).await,
        Some(("policies", fqn)) => read_by_fqn(uri, pool, actor, ObjectType::PolicyRule, fqn).await,
        Some(("evidence", fqn)) => {
            read_by_fqn(uri, pool, actor, ObjectType::EvidenceRequirement, fqn).await
        }
        Some(("context", subject_id)) => read_context(uri, pool, actor, subject_id).await,
        Some(("coverage", _)) => read_coverage(uri, pool).await,
        _ => ResourceReadResult::not_found(uri),
    }
}

/// Route a `sem_reg://` URI through `SemOsClient` instead of direct DB calls.
///
/// Maps resource URIs to the corresponding `dispatch_tool()` calls:
/// - `sem_reg://attributes/{fqn}` → `sem_reg_describe_attribute`
/// - `sem_reg://verbs/{fqn}` → `sem_reg_describe_verb`
/// - `sem_reg://entity-types/{fqn}` → `sem_reg_describe_entity_type`
/// - `sem_reg://context/{id}` → `sem_reg_resolve_context`
/// - `sem_reg://coverage` → `sem_reg_coverage`
/// - Others → `sem_reg_describe_attribute` (generic describe)
///
/// Falls back to direct `read_resource()` on client errors.
pub async fn read_resource_via_client(
    uri: &str,
    client: &dyn sem_os_client::SemOsClient,
    pool: &PgPool,
    actor: &ActorContext,
) -> ResourceReadResult {
    let (segment, value) = match parse_uri(uri) {
        Some(sv) => sv,
        None => return ResourceReadResult::not_found(uri),
    };

    let (tool_name, arguments) = match segment {
        "attributes" => ("sem_reg_describe_attribute", json!({"fqn": value})),
        "entity-types" => ("sem_reg_describe_entity_type", json!({"fqn": value})),
        "verbs" => ("sem_reg_describe_verb", json!({"fqn": value})),
        "taxonomies" => ("sem_reg_taxonomy_tree", json!({"fqn": value})),
        "views" => ("sem_reg_describe_view", json!({"fqn": value})),
        "policies" => ("sem_reg_describe_policy", json!({"fqn": value})),
        "evidence" => ("sem_reg_check_evidence_freshness", json!({"fqn": value})),
        "context" => (
            "sem_reg_resolve_context",
            json!({"subject_id": value, "subject_type": "entity"}),
        ),
        "coverage" => ("sem_reg_coverage", json!({})),
        _ => return ResourceReadResult::not_found(uri),
    };

    // Build a principal from the actor context for ABAC.
    let principal =
        sem_os_core::principal::Principal::in_process(&actor.actor_id, actor.roles.clone());

    let req = sem_os_core::proto::ToolCallRequest {
        tool_name: tool_name.to_string(),
        arguments,
    };

    match client.dispatch_tool(&principal, req).await {
        Ok(resp) if resp.success => ResourceReadResult::json_content(uri, &resp.data),
        Ok(resp) => {
            let msg = resp.error.unwrap_or_else(|| "tool returned failure".into());
            tracing::warn!(uri, tool_name, error = %msg, "SemOsClient resource dispatch failed");
            // Fallback to direct
            read_resource(uri, pool, actor).await
        }
        Err(e) => {
            tracing::warn!(uri, tool_name, error = %e, "SemOsClient resource dispatch error, falling back to direct");
            read_resource(uri, pool, actor).await
        }
    }
}

/// Parse `sem_reg://{segment}/{value}` into `(segment, value)`.
/// Also handles `sem_reg://coverage` (no value segment).
fn parse_uri(uri: &str) -> Option<(&str, &str)> {
    let path = uri.strip_prefix("sem_reg://")?;
    if path == "coverage" {
        return Some(("coverage", ""));
    }
    let (segment, value) = path.split_once('/')?;
    if value.is_empty() {
        return None;
    }
    Some((segment, value))
}

// ── Handlers ──────────────────────────────────────────────────

/// Generic handler: resolve by FQN, apply ABAC, return JSON.
async fn read_by_fqn(
    uri: &str,
    pool: &PgPool,
    actor: &ActorContext,
    object_type: ObjectType,
    fqn: &str,
) -> ResourceReadResult {
    let row =
        match SnapshotStore::find_active_by_definition_field(pool, object_type, "fqn", fqn).await {
            Ok(Some(r)) => r,
            Ok(None) => return ResourceReadResult::not_found(uri),
            Err(e) => {
                tracing::warn!(uri, error = %e, "Resource read failed");
                return ResourceReadResult::not_found(uri);
            }
        };

    match enforce_read(actor, &row) {
        EnforceResult::Allow | EnforceResult::AllowWithMasking { .. } => {
            let value = snapshot_to_json(&row);
            ResourceReadResult::json_content(uri, &value)
        }
        EnforceResult::Deny { reason } => {
            let stub = crate::sem_reg::enforce::redacted_stub(&row, &reason);
            ResourceReadResult::json_content(uri, &stub)
        }
    }
}

/// Context resolution handler.
async fn read_context(
    uri: &str,
    pool: &PgPool,
    actor: &ActorContext,
    subject_id: &str,
) -> ResourceReadResult {
    let subject_uuid = match uuid::Uuid::parse_str(subject_id) {
        Ok(u) => u,
        Err(_) => return ResourceReadResult::not_found(uri),
    };

    let req = ContextResolutionRequest {
        subject: SubjectRef::EntityId(subject_uuid),
        intent: None,
        actor: actor.clone(),
        goals: vec![],
        constraints: ResolutionConstraints::default(),
        evidence_mode: EvidenceMode::Normal,
        point_in_time: None,
        entity_kind: None,
    };

    match resolve_context(pool, &req).await {
        Ok(resp) => match serde_json::to_value(&resp) {
            Ok(v) => ResourceReadResult::json_content(uri, &v),
            Err(e) => {
                tracing::warn!(uri, error = %e, "Context resolution serialization failed");
                ResourceReadResult::not_found(uri)
            }
        },
        Err(e) => {
            tracing::warn!(uri, error = %e, "Context resolution failed");
            ResourceReadResult::not_found(uri)
        }
    }
}

/// Coverage report handler.
async fn read_coverage(uri: &str, pool: &PgPool) -> ResourceReadResult {
    match MetricsStore::coverage_report(pool, None).await {
        Ok(report) => match serde_json::to_value(&report) {
            Ok(v) => ResourceReadResult::json_content(uri, &v),
            Err(e) => {
                tracing::warn!(uri, error = %e, "Coverage report serialization failed");
                ResourceReadResult::not_found(uri)
            }
        },
        Err(e) => {
            tracing::warn!(uri, error = %e, "Coverage report failed");
            ResourceReadResult::not_found(uri)
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

/// Convert a `SnapshotRow` to a JSON value suitable for resource responses.
fn snapshot_to_json(row: &SnapshotRow) -> serde_json::Value {
    json!({
        "snapshot_id": row.snapshot_id,
        "object_type": row.object_type.to_string(),
        "object_id": row.object_id,
        "version": format!("{}.{}", row.version_major, row.version_minor),
        "status": row.status,
        "governance_tier": row.governance_tier,
        "trust_class": row.trust_class,
        "effective_from": row.effective_from.to_rfc3339(),
        "created_by": &row.created_by,
        "definition": &row.definition,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uri_attributes() {
        assert_eq!(
            parse_uri("sem_reg://attributes/cbu.jurisdiction_code"),
            Some(("attributes", "cbu.jurisdiction_code"))
        );
    }

    #[test]
    fn test_parse_uri_entity_types() {
        assert_eq!(
            parse_uri("sem_reg://entity-types/cbu"),
            Some(("entity-types", "cbu"))
        );
    }

    #[test]
    fn test_parse_uri_context_with_uuid() {
        assert_eq!(
            parse_uri("sem_reg://context/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            Some(("context", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"))
        );
    }

    #[test]
    fn test_parse_uri_coverage() {
        assert_eq!(parse_uri("sem_reg://coverage"), Some(("coverage", "")));
    }

    #[test]
    fn test_parse_uri_invalid_no_scheme() {
        assert_eq!(parse_uri("http://attributes/foo"), None);
    }

    #[test]
    fn test_parse_uri_invalid_empty_value() {
        assert_eq!(parse_uri("sem_reg://attributes/"), None);
    }

    #[test]
    fn test_parse_uri_invalid_no_segment() {
        assert_eq!(parse_uri("sem_reg://"), None);
    }

    #[test]
    fn test_static_resources_coverage() {
        let resources = static_resources();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].uri, "sem_reg://coverage");
    }

    #[test]
    fn test_resource_templates_count() {
        let templates = resource_templates();
        assert_eq!(templates.len(), 8, "Expected 8 resource templates");
        let uris: Vec<&str> = templates.iter().map(|t| t.uri_template.as_str()).collect();
        assert!(uris.contains(&"sem_reg://attributes/{fqn}"));
        assert!(uris.contains(&"sem_reg://entity-types/{fqn}"));
        assert!(uris.contains(&"sem_reg://verbs/{fqn}"));
        assert!(uris.contains(&"sem_reg://taxonomies/{fqn}"));
        assert!(uris.contains(&"sem_reg://views/{fqn}"));
        assert!(uris.contains(&"sem_reg://policies/{fqn}"));
        assert!(uris.contains(&"sem_reg://evidence/{fqn}"));
        assert!(uris.contains(&"sem_reg://context/{subject_id}"));
    }

    #[test]
    fn test_resource_read_result_json_content() {
        let result = ResourceReadResult::json_content("sem_reg://coverage", &json!({"total": 42}));
        assert_eq!(result.contents.len(), 1);
        assert_eq!(result.contents[0].uri, "sem_reg://coverage");
        assert_eq!(
            result.contents[0].mime_type.as_deref(),
            Some("application/json")
        );
        assert!(result.contents[0].text.as_ref().unwrap().contains("42"));
    }

    #[test]
    fn test_resource_read_result_not_found() {
        let result = ResourceReadResult::not_found("sem_reg://attributes/nonexistent");
        assert_eq!(result.contents.len(), 1);
        assert!(result.contents[0]
            .text
            .as_ref()
            .unwrap()
            .contains("not found"));
    }
}
