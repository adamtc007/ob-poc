//! ob-poc impl of [`dsl_runtime::service_traits::SessionService`].
//!
//! Single-method dispatcher for the 19 `session.*` verbs from
//! `config/verbs/session.yaml`. The bridge holds the full ob-poc
//! session lifecycle logic — `crate::session::UnifiedSession` +
//! `crate::session::unified::StructureType` + the SQL queries
//! that hydrate / load CBUs and contextual frames.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::PgPool;
use uuid::Uuid;

use dsl_runtime::service_traits::SessionService;

use crate::session::unified::StructureType;
use crate::session::UnifiedSession;

const EXT_KEY_PENDING: &str = "_pending_session";

pub struct ObPocSessionService;

impl ObPocSessionService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ObPocSessionService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionService for ObPocSessionService {
    async fn dispatch_session_verb(
        &self,
        pool: &PgPool,
        verb_name: &str,
        args: &Value,
        extensions: &mut Value,
    ) -> Result<Value> {
        match verb_name {
            "start" => session_start(args, extensions),
            "load-universe" => session_load_universe(pool, args, extensions).await,
            "load-galaxy" => session_load_galaxy(pool, args, extensions).await,
            "load-cluster" => session_load_cluster(pool, args, extensions).await,
            "load-system" => session_load_system(pool, args, extensions).await,
            "unload-system" => session_unload_system(pool, args, extensions).await,
            "filter-jurisdiction" => session_filter_jurisdiction(pool, args, extensions).await,
            "clear" => session_clear(extensions),
            "undo" => session_undo(extensions),
            "redo" => session_redo(extensions),
            "info" => session_info(pool, extensions).await,
            "list" => session_list(pool, args, extensions).await,
            "set-client" => session_set_client(pool, args, extensions).await,
            "set-persona" => session_set_persona(args, extensions),
            "set-structure" => session_set_structure(pool, args, extensions).await,
            "set-case" => session_set_case(pool, args, extensions).await,
            "set-mandate" => session_set_mandate(pool, args, extensions).await,
            "load-deal" => session_load_deal(pool, args, extensions).await,
            "unload-deal" => session_unload_deal(extensions),
            other => Err(anyhow!("unknown session verb: {other}")),
        }
    }
}

// ── Result types (mirrors of the legacy session_ops result structs) ───────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
struct CbuSummary {
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LoadUniverseResult {
    count_added: usize,
    total_loaded: usize,
}

#[derive(Debug, Clone, Serialize)]
struct LoadGalaxyResult {
    jurisdiction: String,
    count_added: usize,
    total_loaded: usize,
}

#[derive(Debug, Clone, Serialize)]
struct LoadClusterResult {
    manco_name: String,
    manco_entity_id: Uuid,
    jurisdiction: Option<String>,
    count_added: usize,
    total_loaded: usize,
}

#[derive(Debug, Clone, Serialize)]
struct LoadSystemResult {
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    total_loaded: usize,
    was_new: bool,
}

#[derive(Debug, Clone, Serialize)]
struct UnloadSystemResult {
    cbu_id: Uuid,
    name: String,
    total_loaded: usize,
    was_present: bool,
}

#[derive(Debug, Clone, Serialize)]
struct FilterJurisdictionResult {
    jurisdiction: String,
    count_kept: usize,
    count_removed: usize,
    total_loaded: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ClearResult {
    cleared: bool,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct HistoryResult {
    success: bool,
    scope_size: usize,
    history_depth: usize,
    future_depth: usize,
}

#[derive(Debug, Clone, Serialize)]
struct JurisdictionCount {
    jurisdiction: String,
    count: i64,
}

#[derive(Debug, Clone, Serialize)]
struct SessionInfo {
    session_id: Uuid,
    name: Option<String>,
    total_cbus: usize,
    jurisdictions: Vec<JurisdictionCount>,
    history_depth: usize,
    future_depth: usize,
}

// ── Extension helpers ─────────────────────────────────────────────────────────

fn ext_obj_mut(extensions: &mut Value) -> &mut Map<String, Value> {
    if !extensions.is_object() {
        *extensions = Value::Object(Map::new());
    }
    extensions.as_object_mut().unwrap()
}

fn take_or_create_session(extensions: &mut Value) -> UnifiedSession {
    extensions
        .as_object_mut()
        .and_then(|obj| obj.remove(EXT_KEY_PENDING))
        .and_then(|v| serde_json::from_value::<UnifiedSession>(v).ok())
        .unwrap_or_else(UnifiedSession::new)
}

fn set_session(extensions: &mut Value, session: UnifiedSession) {
    if let Ok(v) = serde_json::to_value(&session) {
        ext_obj_mut(extensions).insert(EXT_KEY_PENDING.to_string(), v);
    }
}

// ── JSON arg helpers ──────────────────────────────────────────────────────────

fn arg_string(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("missing required string arg :{name}"))
}

fn arg_string_opt(args: &Value, name: &str) -> Option<String> {
    args.get(name).and_then(|v| v.as_str()).map(String::from)
}

fn arg_uuid(args: &Value, name: &str) -> Result<Uuid> {
    args.get(name)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| anyhow!("missing or invalid UUID arg :{name}"))
}

fn arg_uuid_opt(args: &Value, name: &str) -> Option<Uuid> {
    args.get(name)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

fn arg_int_opt(args: &Value, name: &str) -> Option<i64> {
    args.get(name).and_then(|v| v.as_i64())
}

// ── session.start ─────────────────────────────────────────────────────────────

fn session_start(args: &Value, extensions: &mut Value) -> Result<Value> {
    let mode = arg_string(args, "mode")?;
    let from = arg_string_opt(args, "from");
    let session = UnifiedSession::new();
    let session_id = session.id;
    set_session(extensions, session);
    Ok(json!({
        "session_id": session_id,
        "mode": mode,
        "client_group_name": Value::Null,
        "workspace": from,
    }))
}

// ── session.load-universe ─────────────────────────────────────────────────────

async fn session_load_universe(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let client_id = arg_uuid_opt(args, "client-id");
    let cbu_ids: Vec<Uuid> = if let Some(client_id) = client_id {
        sqlx::query_scalar!(
            r#"
            SELECT DISTINCT c.cbu_id as "cbu_id!"
            FROM "ob-poc".cbus c
            LEFT JOIN "ob-poc".cbu_groups g ON g.manco_entity_id = $1
            LEFT JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id AND gm.cbu_id = c.cbu_id
            WHERE c.deleted_at IS NULL
              AND (
                   c.commercial_client_entity_id = $1
               OR gm.cbu_id IS NOT NULL
              )
            "#,
            client_id
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_scalar!(
            r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus WHERE deleted_at IS NULL"#
        )
        .fetch_all(pool)
        .await?
    };

    let mut session = take_or_create_session(extensions);
    let count_added = session.load_cbus(cbu_ids);
    let total_loaded = session.cbu_count();
    set_session(extensions, session);
    Ok(json!(LoadUniverseResult {
        count_added,
        total_loaded,
    }))
}

// ── session.load-galaxy ───────────────────────────────────────────────────────

async fn session_load_galaxy(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let jurisdiction = arg_string(args, "jurisdiction")?;
    let cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
        r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus WHERE jurisdiction = $1 AND deleted_at IS NULL"#,
        jurisdiction
    )
    .fetch_all(pool)
    .await?;
    let mut session = take_or_create_session(extensions);
    let count_added = session.load_cbus(cbu_ids);
    let total_loaded = session.cbu_count();
    set_session(extensions, session);
    Ok(json!(LoadGalaxyResult {
        jurisdiction,
        count_added,
        total_loaded,
    }))
}

// ── session.load-cluster ──────────────────────────────────────────────────────

async fn session_load_cluster(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let jurisdiction = arg_string_opt(args, "jurisdiction");
    let client_group_id = arg_uuid_opt(args, "client");

    let apex_entity_id: Uuid = if let Some(group_id) = client_group_id {
        let anchor: Option<Uuid> = sqlx::query_scalar!(
            r#"
            SELECT anchor_entity_id as "anchor_entity_id!"
            FROM "ob-poc".resolve_client_group_anchor($1, 'governance_controller', COALESCE($2, ''))
            "#,
            group_id,
            jurisdiction.as_deref()
        )
        .fetch_optional(pool)
        .await?;
        anchor.ok_or_else(|| {
            anyhow!(
                "No anchor entity found for client group {group_id} (jurisdiction: {jurisdiction:?})"
            )
        })?
    } else {
        arg_uuid(args, "apex-entity-id")?
    };

    let apex_name: String = sqlx::query_scalar!(
        r#"SELECT name as "name!" FROM "ob-poc".entities WHERE entity_id = $1 AND deleted_at IS NULL"#,
        apex_entity_id
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or_else(|| "Unknown".to_string());

    let group_id: Uuid = if let Some(gid) = client_group_id {
        gid
    } else {
        sqlx::query_scalar(
            r#"SELECT group_id FROM "ob-poc".client_group_anchor WHERE anchor_entity_id = $1 LIMIT 1"#,
        )
        .bind(apex_entity_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("No client group found for anchor entity {apex_entity_id}"))?
    };

    let cbu_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT cge.cbu_id
        FROM "ob-poc".client_group_entity cge
        JOIN "ob-poc".cbus c ON c.cbu_id = cge.cbu_id
        WHERE cge.group_id = $1
          AND cge.cbu_id IS NOT NULL
          AND cge.membership_type NOT IN ('historical', 'rejected')
          AND c.deleted_at IS NULL
          AND ($2::text IS NULL OR c.jurisdiction = $2)
        "#,
    )
    .bind(group_id)
    .bind(jurisdiction.as_deref())
    .fetch_all(pool)
    .await?;

    if cbu_ids.is_empty() {
        return Err(anyhow!("No CBUs found under '{apex_name}' ({apex_entity_id})"));
    }

    let mut session = take_or_create_session(extensions);
    session.name = Some(format!("{apex_name} Book"));
    let count_added = session.load_cbus(cbu_ids);
    let total_loaded = session.cbu_count();
    set_session(extensions, session);

    Ok(json!(LoadClusterResult {
        manco_name: apex_name,
        manco_entity_id: apex_entity_id,
        jurisdiction,
        count_added,
        total_loaded,
    }))
}

// ── session.load-system ───────────────────────────────────────────────────────

async fn session_load_system(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let cbu = sqlx::query!(
        r#"SELECT cbu_id, name, jurisdiction FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        cbu_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("CBU not found: {cbu_id}"))?;

    let mut session = take_or_create_session(extensions);
    let was_new = session.load_cbu(cbu_id);
    let total_loaded = session.cbu_count();
    set_session(extensions, session);

    Ok(json!(LoadSystemResult {
        cbu_id,
        name: cbu.name,
        jurisdiction: cbu.jurisdiction,
        total_loaded,
        was_new,
    }))
}

// ── session.unload-system ─────────────────────────────────────────────────────

async fn session_unload_system(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let name: String = sqlx::query_scalar!(
        r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        cbu_id
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();

    let mut session = take_or_create_session(extensions);
    let was_present = session.unload_cbu(cbu_id);
    let total_loaded = session.cbu_count();
    set_session(extensions, session);

    Ok(json!(UnloadSystemResult {
        cbu_id,
        name,
        total_loaded,
        was_present,
    }))
}

// ── session.filter-jurisdiction ───────────────────────────────────────────────

async fn session_filter_jurisdiction(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let jurisdiction = arg_string(args, "jurisdiction")?;
    let mut session = take_or_create_session(extensions);
    let before_count = session.cbu_count();
    let current_cbu_ids = session.cbu_ids_vec();

    if current_cbu_ids.is_empty() {
        set_session(extensions, session);
        return Ok(json!(FilterJurisdictionResult {
            jurisdiction,
            count_kept: 0,
            count_removed: 0,
            total_loaded: 0,
        }));
    }

    let matching: Vec<Uuid> = sqlx::query_scalar!(
        r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus
           WHERE cbu_id = ANY($1) AND jurisdiction = $2 AND deleted_at IS NULL"#,
        &current_cbu_ids,
        &jurisdiction
    )
    .fetch_all(pool)
    .await?;

    session.clear_cbus_with_history();
    let count_kept = session.load_cbus(matching);
    let count_removed = before_count - count_kept;
    let total_loaded = session.cbu_count();
    set_session(extensions, session);

    Ok(json!(FilterJurisdictionResult {
        jurisdiction,
        count_kept,
        count_removed,
        total_loaded,
    }))
}

// ── session.clear ─────────────────────────────────────────────────────────────

fn session_clear(extensions: &mut Value) -> Result<Value> {
    let mut session = take_or_create_session(extensions);
    let count = session.clear_cbus_with_history();
    set_session(extensions, session);
    Ok(json!(ClearResult { cleared: true, count }))
}

// ── session.undo / session.redo ───────────────────────────────────────────────

fn session_undo(extensions: &mut Value) -> Result<Value> {
    let mut session = take_or_create_session(extensions);
    let success = session.undo_cbu();
    let result = HistoryResult {
        success,
        scope_size: session.cbu_count(),
        history_depth: session.cbu_history_depth(),
        future_depth: session.cbu_future_depth(),
    };
    set_session(extensions, session);
    Ok(json!(result))
}

fn session_redo(extensions: &mut Value) -> Result<Value> {
    let mut session = take_or_create_session(extensions);
    let success = session.redo_cbu();
    let result = HistoryResult {
        success,
        scope_size: session.cbu_count(),
        history_depth: session.cbu_history_depth(),
        future_depth: session.cbu_future_depth(),
    };
    set_session(extensions, session);
    Ok(json!(result))
}

// ── session.info ──────────────────────────────────────────────────────────────

async fn session_info(pool: &PgPool, extensions: &mut Value) -> Result<Value> {
    let session = take_or_create_session(extensions);
    let cbu_ids = session.cbu_ids_vec();
    let jurisdictions: Vec<JurisdictionCount> = if cbu_ids.is_empty() {
        vec![]
    } else {
        sqlx::query!(
            r#"
            SELECT jurisdiction, COUNT(*) as count
            FROM "ob-poc".cbus
            WHERE cbu_id = ANY($1) AND deleted_at IS NULL
            GROUP BY jurisdiction
            ORDER BY count DESC
            "#,
            &cbu_ids
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| JurisdictionCount {
            jurisdiction: r.jurisdiction.unwrap_or_default(),
            count: r.count.unwrap_or(0),
        })
        .collect()
    };

    let result = SessionInfo {
        session_id: session.id,
        name: session.name.clone(),
        total_cbus: session.cbu_count(),
        jurisdictions,
        history_depth: session.cbu_history_depth(),
        future_depth: session.cbu_future_depth(),
    };
    set_session(extensions, session);
    Ok(json!(result))
}

// ── session.list ──────────────────────────────────────────────────────────────

async fn session_list(pool: &PgPool, args: &Value, extensions: &mut Value) -> Result<Value> {
    let limit = arg_int_opt(args, "limit").unwrap_or(100);
    let jurisdiction_filter = arg_string_opt(args, "jurisdiction");
    let session = take_or_create_session(extensions);
    let cbu_ids = session.cbu_ids_vec();
    let total_in_session = session.cbu_count();

    let cbus: Vec<CbuSummary> = if cbu_ids.is_empty() {
        vec![]
    } else {
        sqlx::query_as!(
            CbuSummary,
            r#"
            SELECT cbu_id, name, jurisdiction
            FROM "ob-poc".cbus
            WHERE cbu_id = ANY($1) AND deleted_at IS NULL
              AND ($2::text IS NULL OR jurisdiction = $2)
            ORDER BY name
            LIMIT $3
            "#,
            &cbu_ids,
            jurisdiction_filter.as_deref(),
            limit
        )
        .fetch_all(pool)
        .await?
    };

    set_session(extensions, session);
    let count = cbus.len();
    Ok(json!({
        "cbus": cbus,
        "count": count,
        "total_in_session": total_in_session,
    }))
}

// ── session.set-client ────────────────────────────────────────────────────────

async fn session_set_client(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let client = arg_string(args, "client")?;
    let client_norm = client.to_lowercase().trim().to_string();

    let matches = sqlx::query!(
        r#"
        SELECT
            cg.id as group_id,
            cg.canonical_name as "group_name!",
            cga.confidence as "confidence!",
            (cga.alias_norm = $1) as "exact_match!"
        FROM "ob-poc".client_group_alias cga
        JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
        WHERE cga.alias_norm = $1
           OR cga.alias_norm ILIKE '%' || $1 || '%'
           OR similarity(cga.alias_norm, $1) > 0.3
        ORDER BY
            (cga.alias_norm = $1) DESC,
            cga.confidence DESC,
            similarity(cga.alias_norm, $1) DESC
        LIMIT 5
        "#,
        client_norm
    )
    .fetch_all(pool)
    .await?;

    if matches.is_empty() {
        return Ok(json!({
            "group_id": Value::Null,
            "group_name": Value::Null,
            "entity_count": 0,
            "candidates": [],
            "resolved": false,
        }));
    }

    let top = &matches[0];
    let has_clear_winner = top.exact_match
        || matches.len() == 1
        || (matches.len() > 1 && (top.confidence - matches[1].confidence) > 0.10);

    if has_clear_winner {
        let group_id = top.group_id;
        let group_name = top.group_name.clone();
        let entity_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM "ob-poc".client_group_entity
               WHERE group_id = $1 AND membership_type != 'historical'"#,
            group_id
        )
        .fetch_one(pool)
        .await?;

        let ext = ext_obj_mut(extensions);
        ext.insert(
            "client_group_id".to_string(),
            Value::String(group_id.to_string()),
        );
        ext.insert(
            "client_group_name".to_string(),
            Value::String(group_name.clone()),
        );

        return Ok(json!({
            "group_id": group_id,
            "group_name": group_name,
            "entity_count": entity_count,
            "candidates": [],
            "resolved": true,
        }));
    }

    let candidates: Vec<Value> = matches
        .into_iter()
        .take(3)
        .map(|m| {
            json!({
                "group_id": m.group_id,
                "group_name": m.group_name,
                "confidence": m.confidence,
            })
        })
        .collect();

    Ok(json!({
        "group_id": Value::Null,
        "group_name": Value::Null,
        "entity_count": 0,
        "candidates": candidates,
        "resolved": false,
    }))
}

// ── session.set-persona ───────────────────────────────────────────────────────

fn session_set_persona(args: &Value, extensions: &mut Value) -> Result<Value> {
    let persona = arg_string(args, "persona")?;
    let valid = ["kyc", "trading", "ops", "onboarding"];
    let persona_lower = persona.to_lowercase();
    if !valid.contains(&persona_lower.as_str()) {
        return Err(anyhow!(
            "Invalid persona '{persona}'. Valid options: {valid:?}"
        ));
    }
    ext_obj_mut(extensions).insert("persona".to_string(), Value::String(persona_lower.clone()));
    Ok(json!({ "persona": persona_lower, "set": true }))
}

// ── session.set-structure ─────────────────────────────────────────────────────

async fn session_set_structure(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let structure_id = arg_uuid(args, "structure-id")?;
    let structure_type_str = arg_string_opt(args, "structure-type");

    let cbu = sqlx::query!(
        r#"SELECT cbu_id, name, jurisdiction FROM "ob-poc".cbus
           WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        structure_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Structure (CBU) not found: {structure_id}"))?;

    let structure_type = structure_type_str
        .as_deref()
        .and_then(StructureType::from_internal);

    let mut session = take_or_create_session(extensions);
    if let Some(st) = structure_type {
        session.set_current_structure(structure_id, cbu.name.clone(), st);
    } else {
        session.set_current_structure(structure_id, cbu.name.clone(), StructureType::Pe);
    }
    session.set_dag_flag("structure.selected", true);
    session.set_dag_flag("structure.exists", true);
    set_session(extensions, session);

    Ok(json!({
        "structure_id": structure_id,
        "structure_name": cbu.name,
        "structure_type": structure_type_str,
    }))
}

// ── session.set-case ──────────────────────────────────────────────────────────

async fn session_set_case(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let case_id = arg_uuid(args, "case-id")?;
    let case = sqlx::query!(
        r#"SELECT case_id, status, case_type FROM "ob-poc".cases WHERE case_id = $1"#,
        case_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("KYC case not found: {case_id}"))?;

    let display_name = format!(
        "Case {} ({})",
        &case_id.to_string()[..8],
        case.case_type.as_deref().unwrap_or("NEW_CLIENT")
    );

    let mut session = take_or_create_session(extensions);
    session.set_current_case(case_id, display_name.clone());
    session.set_dag_flag("case.selected", true);
    session.set_dag_flag("case.exists", true);
    set_session(extensions, session);

    Ok(json!({
        "case_id": case_id,
        "case_reference": display_name,
        "status": case.status,
    }))
}

// ── session.set-mandate ───────────────────────────────────────────────────────

async fn session_set_mandate(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let mandate_id = arg_uuid(args, "mandate-id")?;
    let profile = sqlx::query!(
        r#"SELECT profile_id, cbu_id, status, version FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        mandate_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Mandate (trading profile) not found: {mandate_id}"))?;

    let display_name = format!(
        "Profile {} v{}",
        &profile.profile_id.to_string()[..8],
        profile.version
    );

    let mut session = take_or_create_session(extensions);
    session.set_current_mandate(mandate_id, display_name.clone());
    session.set_dag_flag("mandate.selected", true);
    set_session(extensions, session);

    Ok(json!({
        "mandate_id": mandate_id,
        "mandate_name": display_name,
        "structure_id": profile.cbu_id,
    }))
}

// ── session.load-deal ─────────────────────────────────────────────────────────

async fn session_load_deal(
    pool: &PgPool,
    args: &Value,
    extensions: &mut Value,
) -> Result<Value> {
    let deal_id = arg_uuid_opt(args, "deal-id");
    let deal_name = arg_string_opt(args, "deal-name");

    let resolved_id = match (deal_id, deal_name) {
        (Some(id), _) => id,
        (None, Some(name)) => {
            let row = sqlx::query!(
                r#"
                SELECT deal_id FROM "ob-poc".deals
                WHERE deal_name ILIKE '%' || $1 || '%'
                ORDER BY
                    CASE WHEN deal_name ILIKE $1 THEN 0
                         WHEN deal_name ILIKE $1 || '%' THEN 1
                         ELSE 2 END,
                    deal_name
                LIMIT 1
                "#,
                name
            )
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow!("No deal found matching: {name}"))?;
            row.deal_id
        }
        (None, None) => return Err(anyhow!("Either :deal-id or :deal-name must be provided")),
    };

    let deal = sqlx::query!(
        r#"
        SELECT
            d.deal_id,
            d.deal_name,
            d.deal_status,
            cg.canonical_name as "client_group_name?",
            COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_products WHERE deal_id = d.deal_id), 0)::int as "product_count!",
            COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = d.deal_id), 0)::int as "rate_card_count!"
        FROM "ob-poc".deals d
        LEFT JOIN "ob-poc".client_group cg ON cg.id = d.primary_client_group_id
        WHERE d.deal_id = $1
        "#,
        resolved_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Deal not found: {resolved_id}"))?;

    let mut session = take_or_create_session(extensions);
    session.context.deal_id = Some(deal.deal_id);
    session.context.deal_name = Some(deal.deal_name.clone());
    set_session(extensions, session);

    Ok(json!({
        "deal_id": deal.deal_id,
        "deal_name": deal.deal_name,
        "deal_status": deal.deal_status,
        "client_group_name": deal.client_group_name,
        "product_count": deal.product_count,
        "rate_card_count": deal.rate_card_count,
    }))
}

// ── session.unload-deal ───────────────────────────────────────────────────────

fn session_unload_deal(extensions: &mut Value) -> Result<Value> {
    let mut session = take_or_create_session(extensions);
    let previous_deal_id = session.context.deal_id.take();
    let previous_deal_name = session.context.deal_name.take();
    set_session(extensions, session);
    Ok(json!({
        "previous_deal_id": previous_deal_id,
        "previous_deal_name": previous_deal_name,
    }))
}
