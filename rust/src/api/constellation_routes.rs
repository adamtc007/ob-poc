//! Constellation hydration API routes.
//!
//! Endpoints for producing the server-side constellation graph payload
//! that is returned to the UI.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[cfg(feature = "vnext-repl")]
use crate::repl::types_v2::{
    ActionHint, AgentMode, ConstellationContextRef, ProgressSummary, ResolvedConstellationContext,
    SessionScope, SubjectKind, SubjectRef, VerbRef, WorkspaceKind, WorkspaceStateView,
};
use crate::sem_os_runtime::constellation_runtime::{
    handle_constellation_hydrate, handle_constellation_summary, ConstellationSummary,
    HydratedConstellation,
};

/// Application state for constellation routes.
#[derive(Clone)]
pub struct ConstellationAppState {
    pub pool: PgPool,
}

#[derive(Debug, Deserialize)]
struct ConstellationQuery {
    #[serde(rename = "case_id")]
    case_id: Option<Uuid>,
    #[serde(rename = "map_name")]
    map_name: Option<String>,
}

#[cfg(feature = "vnext-repl")]
#[derive(Debug, Clone, Deserialize)]
pub struct HydrateContextQuery {
    pub session_id: Uuid,
    pub client_group_id: Uuid,
    pub workspace: WorkspaceKind,
    #[serde(default)]
    pub constellation_family: Option<String>,
    #[serde(default)]
    pub constellation_map: Option<String>,
    #[serde(default)]
    pub subject_kind: Option<SubjectKind>,
    #[serde(default)]
    pub subject_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct ConstellationNameQuery {
    name: String,
    #[serde(rename = "case_id")]
    case_id: Option<Uuid>,
    #[serde(rename = "map_name")]
    map_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchCbusQuery {
    name: String,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ResolvedCbu {
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
}

#[derive(Debug, Serialize)]
struct HydrateByNameResponse {
    resolved_cbu: ResolvedCbu,
    hydrated: HydratedConstellation,
}

#[derive(Debug, Serialize)]
struct CbuCaseSummary {
    case_id: Uuid,
    status: Option<String>,
    case_type: Option<String>,
    opened_at: Option<DateTime<Utc>>,
}

/// Create API routes for constellation hydration and summary lookup.
///
/// # Examples
/// ```rust,no_run
/// # #[cfg(feature = "server")]
/// # {
/// use ob_poc::api::create_constellation_router;
/// # let pool = sqlx::PgPool::connect_lazy("postgresql:///data_designer").unwrap();
/// let _router = create_constellation_router(pool);
/// # }
/// ```
pub fn create_constellation_router(pool: PgPool) -> Router {
    let state = ConstellationAppState { pool };
    let router = Router::new()
        .route("/api/cbu/:cbu_id/constellation", get(get_constellation))
        .route("/api/cbu/:cbu_id/cases", get(get_constellation_cases))
        .route(
            "/api/cbu/:cbu_id/constellation/summary",
            get(get_constellation_summary),
        )
        .route("/api/constellation/by-name", get(get_constellation_by_name))
        .route(
            "/api/constellation/search-cbus",
            get(search_constellation_cbus),
        );
    #[cfg(feature = "vnext-repl")]
    let router = router
        .route(
            "/api/constellation/resolve",
            axum::routing::post(resolve_constellation),
        )
        .route("/api/constellation/hydrate", get(get_workspace_hydrate))
        .route("/api/constellation/summary", get(get_workspace_summary));
    router.with_state(state)
}

/// Resolve a session-scoped constellation context into concrete defaults.
#[cfg(feature = "vnext-repl")]
async fn resolve_constellation(
    State(state): State<ConstellationAppState>,
    Json(context): Json<ConstellationContextRef>,
) -> Result<Json<ResolvedConstellationContext>, (StatusCode, String)> {
    let resolved = resolve_context(&state.pool, &context).await?;
    Ok(Json(resolved))
}

/// Hydrate the current workspace state view from a resolved context query.
#[cfg(feature = "vnext-repl")]
async fn get_workspace_hydrate(
    State(state): State<ConstellationAppState>,
    Query(query): Query<HydrateContextQuery>,
) -> Result<Json<WorkspaceStateView>, (StatusCode, String)> {
    let resolved = resolve_context(&state.pool, &query.into()).await?;
    let hydrated = hydrate_workspace_state(&state.pool, &resolved).await?;
    Ok(Json(hydrated))
}

/// Return the progress summary for a resolved context query.
#[cfg(feature = "vnext-repl")]
async fn get_workspace_summary(
    State(state): State<ConstellationAppState>,
    Query(query): Query<HydrateContextQuery>,
) -> Result<Json<ProgressSummary>, (StatusCode, String)> {
    let resolved = resolve_context(&state.pool, &query.into()).await?;
    let hydrated = hydrate_workspace_state(&state.pool, &resolved).await?;
    Ok(Json(hydrated.progress_summary.unwrap_or(ProgressSummary {
        total_slots: 0,
        completion_pct: 0,
        blocking_slots: 0,
    })))
}

async fn get_constellation(
    State(state): State<ConstellationAppState>,
    Path(cbu_id): Path<Uuid>,
    Query(query): Query<ConstellationQuery>,
) -> Result<Json<HydratedConstellation>, (StatusCode, String)> {
    let map_name = query
        .map_name
        .unwrap_or_else(|| String::from("struct.lux.ucits.sicav"));
    let hydrated = handle_constellation_hydrate(&state.pool, cbu_id, query.case_id, &map_name)
        .await
        .map_err(internal_error)?;
    Ok(Json(hydrated))
}

async fn get_constellation_summary(
    State(state): State<ConstellationAppState>,
    Path(cbu_id): Path<Uuid>,
    Query(query): Query<ConstellationQuery>,
) -> Result<Json<ConstellationSummary>, (StatusCode, String)> {
    let map_name = query
        .map_name
        .unwrap_or_else(|| String::from("struct.lux.ucits.sicav"));
    let summary = handle_constellation_summary(&state.pool, cbu_id, query.case_id, &map_name)
        .await
        .map_err(internal_error)?;
    Ok(Json(summary))
}

async fn get_constellation_cases(
    State(state): State<ConstellationAppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<Vec<CbuCaseSummary>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, Option<DateTime<Utc>>)>(
        r#"
            SELECT case_id, status, case_type, opened_at
            FROM "ob-poc".cases
            WHERE cbu_id = $1
            ORDER BY opened_at DESC NULLS LAST
            "#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(
        rows.into_iter()
            .map(|(case_id, status, case_type, opened_at)| CbuCaseSummary {
                case_id,
                status,
                case_type,
                opened_at,
            })
            .collect(),
    ))
}

async fn get_constellation_by_name(
    State(state): State<ConstellationAppState>,
    Query(query): Query<ConstellationNameQuery>,
) -> Result<Json<HydrateByNameResponse>, (StatusCode, String)> {
    let map_name = query
        .map_name
        .unwrap_or_else(|| String::from("struct.lux.ucits.sicav"));
    let resolved = resolve_cbu_by_name(&state.pool, &query.name)
        .await?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("No CBU found matching '{}'", query.name),
            )
        })?;
    let hydrated =
        handle_constellation_hydrate(&state.pool, resolved.cbu_id, query.case_id, &map_name)
            .await
            .map_err(internal_error)?;
    Ok(Json(HydrateByNameResponse {
        resolved_cbu: resolved,
        hydrated,
    }))
}

async fn search_constellation_cbus(
    State(state): State<ConstellationAppState>,
    Query(query): Query<SearchCbusQuery>,
) -> Result<Json<Vec<ResolvedCbu>>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        r#"
        SELECT cbu_id, name, jurisdiction
        FROM "ob-poc".cbus
        WHERE name ILIKE '%' || $1 || '%'
        ORDER BY
            CASE
                WHEN LOWER(name) = LOWER($1) THEN 0
                WHEN LOWER(name) LIKE LOWER($1) || '%' THEN 1
                ELSE 2
            END,
            name ASC
        LIMIT $2
        "#,
    )
    .bind(&query.name)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(
        rows.into_iter()
            .map(|(cbu_id, name, jurisdiction)| ResolvedCbu {
                cbu_id,
                name,
                jurisdiction,
            })
            .collect(),
    ))
}

async fn resolve_cbu_by_name(
    pool: &PgPool,
    name: &str,
) -> Result<Option<ResolvedCbu>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        r#"
        SELECT cbu_id, name, jurisdiction
        FROM "ob-poc".cbus
        WHERE name ILIKE '%' || $1 || '%'
        ORDER BY
            CASE
                WHEN LOWER(name) = LOWER($1) THEN 0
                WHEN LOWER(name) LIKE LOWER($1) || '%' THEN 1
                ELSE 2
            END,
            name ASC
        LIMIT 1
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?;

    Ok(row.map(|(cbu_id, name, jurisdiction)| ResolvedCbu {
        cbu_id,
        name,
        jurisdiction,
    }))
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Constellation API error: {error}"),
    )
}

#[cfg(feature = "vnext-repl")]
impl From<HydrateContextQuery> for ConstellationContextRef {
    fn from(value: HydrateContextQuery) -> Self {
        Self {
            session_id: value.session_id,
            client_group_id: value.client_group_id,
            workspace: value.workspace,
            constellation_family: value.constellation_family,
            constellation_map: value.constellation_map,
            subject_kind: value.subject_kind,
            subject_id: value.subject_id,
            handoff_context: None,
        }
    }
}

#[cfg(feature = "vnext-repl")]
pub(crate) async fn resolve_context(
    pool: &PgPool,
    context: &ConstellationContextRef,
) -> Result<ResolvedConstellationContext, (StatusCode, String)> {
    let registry = context.workspace.registry_entry();
    let session_scope = resolve_session_scope(pool, context.client_group_id).await?;
    let subject_kind = context
        .subject_kind
        .clone()
        .or_else(|| registry.subject_kinds.first().cloned());
    let subject_id = match context.subject_id {
        Some(subject_id) => Some(subject_id),
        None => {
            resolve_subject_id(
                pool,
                context.client_group_id,
                &context.workspace,
                subject_kind.as_ref(),
            )
            .await?
        }
    };

    Ok(ResolvedConstellationContext {
        session_id: context.session_id,
        client_group_id: context.client_group_id,
        workspace: context.workspace.clone(),
        constellation_family: context
            .constellation_family
            .clone()
            .unwrap_or_else(|| registry.default_constellation_family.to_string()),
        constellation_map: context
            .constellation_map
            .clone()
            .unwrap_or_else(|| registry.default_constellation_map.to_string()),
        subject_kind,
        subject_id,
        handoff_context: context.handoff_context.clone(),
        session_scope,
        agent_mode: AgentMode::Sage,
    })
}

#[cfg(feature = "vnext-repl")]
pub(crate) async fn hydrate_workspace_state(
    pool: &PgPool,
    resolved: &ResolvedConstellationContext,
) -> Result<WorkspaceStateView, (StatusCode, String)> {
    let cbu_id = resolve_constellation_cbu_id(pool, resolved).await?;
    let hydrated_constellation = if let Some(cbu_id) = cbu_id {
        Some(
            handle_constellation_hydrate(
                pool,
                cbu_id,
                extract_case_id(resolved),
                &resolved.constellation_map,
            )
            .await
            .map_err(internal_error)?,
        )
    } else {
        None
    };
    let progress_summary = if let Some(cbu_id) = cbu_id {
        let summary = handle_constellation_summary(
            pool,
            cbu_id,
            extract_case_id(resolved),
            &resolved.constellation_map,
        )
        .await
        .map_err(internal_error)?;
        Some(progress_summary_from(&summary))
    } else {
        None
    };
    let scoped_verb_surface = hydrated_constellation
        .as_ref()
        .map(flatten_scoped_verbs)
        .unwrap_or_default();
    let subject_ref = resolved
        .subject_id
        .zip(resolved.subject_kind.clone())
        .map(|(id, kind)| SubjectRef { kind, id });
    let available_actions = build_action_hints(&resolved.workspace, &scoped_verb_surface);

    Ok(WorkspaceStateView {
        workspace: resolved.workspace.clone(),
        constellation_family: resolved.constellation_family.clone(),
        constellation_map: resolved.constellation_map.clone(),
        subject_ref,
        hydrated_constellation,
        scoped_verb_surface,
        progress_summary,
        available_actions,
    })
}

#[cfg(feature = "vnext-repl")]
async fn resolve_session_scope(
    pool: &PgPool,
    client_group_id: Uuid,
) -> Result<SessionScope, (StatusCode, String)> {
    let group_name = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT canonical_name
        FROM "ob-poc".client_group
        WHERE id = $1
        "#,
    )
    .bind(client_group_id)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?
    .flatten();

    Ok(SessionScope {
        client_group_id,
        client_group_name: group_name,
    })
}

#[cfg(feature = "vnext-repl")]
async fn resolve_subject_id(
    pool: &PgPool,
    client_group_id: Uuid,
    workspace: &WorkspaceKind,
    subject_kind: Option<&SubjectKind>,
) -> Result<Option<Uuid>, (StatusCode, String)> {
    match subject_kind {
        Some(SubjectKind::ClientGroup) => Ok(Some(client_group_id)),
        Some(SubjectKind::Cbu)
        | Some(SubjectKind::Case)
        | Some(SubjectKind::Deal)
        | Some(SubjectKind::Matrix)
        | Some(SubjectKind::Handoff)
        | None => resolve_first_cbu_for_group(pool, client_group_id).await,
        Some(SubjectKind::Product)
        | Some(SubjectKind::Service)
        | Some(SubjectKind::Resource)
        | Some(SubjectKind::Attribute) => {
            if matches!(workspace, WorkspaceKind::ProductMaintenance) {
                Ok(None)
            } else {
                resolve_first_cbu_for_group(pool, client_group_id).await
            }
        }
    }
}

#[cfg(feature = "vnext-repl")]
async fn resolve_first_cbu_for_group(
    pool: &PgPool,
    client_group_id: Uuid,
) -> Result<Option<Uuid>, (StatusCode, String)> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT cge.cbu_id
        FROM "ob-poc".client_group_entity cge
        JOIN "ob-poc".cbus c ON c.cbu_id = cge.cbu_id
        WHERE cge.group_id = $1
          AND c.deleted_at IS NULL
        ORDER BY c.name ASC
        LIMIT 1
        "#,
    )
    .bind(client_group_id)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)
}

#[cfg(feature = "vnext-repl")]
async fn resolve_constellation_cbu_id(
    pool: &PgPool,
    resolved: &ResolvedConstellationContext,
) -> Result<Option<Uuid>, (StatusCode, String)> {
    match resolved.subject_kind {
        Some(SubjectKind::Cbu) => Ok(resolved.subject_id),
        Some(SubjectKind::Case) => {
            if let Some(case_id) = resolved.subject_id {
                sqlx::query_scalar::<_, Uuid>(
                    r#"
                    SELECT cbu_id
                    FROM "ob-poc".cases
                    WHERE case_id = $1
                    "#,
                )
                .bind(case_id)
                .fetch_optional(pool)
                .await
                .map_err(internal_error)
            } else {
                resolve_first_cbu_for_group(pool, resolved.client_group_id).await
            }
        }
        _ => {
            if let Some(target_cbu_id) = resolved
                .handoff_context
                .as_ref()
                .and_then(|handoff| handoff.target_cbu_id)
            {
                Ok(Some(target_cbu_id))
            } else {
                resolve_first_cbu_for_group(pool, resolved.client_group_id).await
            }
        }
    }
}

#[cfg(feature = "vnext-repl")]
fn extract_case_id(resolved: &ResolvedConstellationContext) -> Option<Uuid> {
    match resolved.subject_kind {
        Some(SubjectKind::Case) => resolved.subject_id,
        _ => None,
    }
}

#[cfg(feature = "vnext-repl")]
fn progress_summary_from(summary: &ConstellationSummary) -> ProgressSummary {
    ProgressSummary {
        total_slots: summary.total_slots,
        completion_pct: summary.completion_pct,
        blocking_slots: summary.blocking_slots,
    }
}

#[cfg(feature = "vnext-repl")]
fn flatten_scoped_verbs(hydrated: &HydratedConstellation) -> Vec<VerbRef> {
    fn walk(
        slot: &crate::sem_os_runtime::constellation_runtime::HydratedSlot,
        acc: &mut Vec<String>,
    ) {
        for verb in &slot.available_verbs {
            if !acc.contains(verb) {
                acc.push(verb.clone());
            }
        }
        for child in &slot.children {
            walk(child, acc);
        }
    }

    let mut verbs = Vec::new();
    for slot in &hydrated.slots {
        walk(slot, &mut verbs);
    }
    verbs
        .into_iter()
        .map(|verb_fqn| VerbRef {
            display_name: verb_fqn.clone(),
            verb_fqn,
        })
        .collect()
}

#[cfg(feature = "vnext-repl")]
fn build_action_hints(workspace: &WorkspaceKind, verbs: &[VerbRef]) -> Vec<ActionHint> {
    let mut hints: Vec<ActionHint> = verbs
        .iter()
        .take(3)
        .map(|verb| ActionHint {
            label: format!("Run {}", verb.display_name),
            verb_fqn: Some(verb.verb_fqn.clone()),
            action_type: "verb".to_string(),
        })
        .collect();

    if hints.is_empty() {
        let label = match workspace {
            WorkspaceKind::Deal => "Review deal lifecycle",
            WorkspaceKind::Cbu => "Review operating footprint",
            WorkspaceKind::Kyc => "Review clearance state",
            WorkspaceKind::InstrumentMatrix => "Review matrix readiness",
            WorkspaceKind::ProductMaintenance => "Review product taxonomy",
            WorkspaceKind::OnBoarding => "Review onboarding handoff",
        };
        hints.push(ActionHint {
            label: label.to_string(),
            verb_fqn: None,
            action_type: "inspect".to_string(),
        });
    }

    hints
}
