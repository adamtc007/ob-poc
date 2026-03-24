//! Sage session context — tracks which client group, constellation, and entity
//! are in focus for the SemOS-scoped verb resolution pipeline.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// SageSession
// ---------------------------------------------------------------------------

/// Session context for the Sage utterance pipeline.
///
/// Tracks which client group the agent is working with, which constellation
/// template applies, and which entity is currently in focus. This context
/// drives the valid verb set computation for SemOS-scoped resolution.
#[derive(Debug, Clone)]
pub struct SageSession {
    pub session_id: Uuid,
    pub client_group_id: Option<Uuid>,
    pub constellation_id: Option<String>,
    pub active_entity_id: Option<Uuid>,
    pub active_domain: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl SageSession {
    /// Create a blank session with a new UUID.
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            client_group_id: None,
            constellation_id: None,
            active_entity_id: None,
            active_domain: None,
            updated_at: Utc::now(),
        }
    }

    /// Whether a client group has been resolved for this session.
    pub fn has_client_group(&self) -> bool {
        self.client_group_id.is_some()
    }

    /// Set the client group and associated constellation template.
    pub fn set_client_group(&mut self, group_id: Uuid, constellation_id: String) {
        self.client_group_id = Some(group_id);
        self.constellation_id = Some(constellation_id);
        self.updated_at = Utc::now();
    }

    /// Set the currently focused entity and domain.
    pub fn set_active_entity(&mut self, entity_id: Uuid, domain: String) {
        self.active_entity_id = Some(entity_id);
        self.active_domain = Some(domain);
        self.updated_at = Utc::now();
    }

    /// Initialize session from UI navigation context (e.g., user clicked into a client group).
    pub fn from_ui_context(
        client_group_id: Uuid,
        constellation_id: String,
        active_entity_id: Option<Uuid>,
    ) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            client_group_id: Some(client_group_id),
            constellation_id: Some(constellation_id),
            active_entity_id,
            active_domain: None,
            updated_at: Utc::now(),
        }
    }
}

impl Default for SageSession {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EntityState — entity state snapshot for valid verb set computation
// ---------------------------------------------------------------------------

/// Snapshot of an entity's current FSM state within a client group.
#[derive(Debug, Clone)]
pub struct EntityState {
    pub entity_id: Uuid,
    /// Entity type key: "cbu", "kyc_case", "entity", etc.
    pub entity_type: String,
    /// Current FSM state: "DRAFT", "ACTIVE", "DISCOVERED", etc.
    pub current_state: String,
    /// Constellation slot this entity maps to (if known).
    pub slot_name: Option<String>,
}

#[cfg(feature = "database")]
type WorkstreamRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<String>,
    bool,
    bool,
    bool,
    bool,
    bool,
);

// ---------------------------------------------------------------------------
// Database operations
// ---------------------------------------------------------------------------

/// Persist a new sage session.
#[cfg(feature = "database")]
pub async fn create_session(pool: &PgPool, session: &SageSession) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".sage_sessions
            (session_id, client_group_id, constellation_id, active_entity_id, active_domain, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(session.session_id)
    .bind(session.client_group_id)
    .bind(&session.constellation_id)
    .bind(session.active_entity_id)
    .bind(&session.active_domain)
    .bind(session.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update an existing sage session.
#[cfg(feature = "database")]
pub async fn update_session(pool: &PgPool, session: &SageSession) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".sage_sessions
        SET client_group_id = $2,
            constellation_id = $3,
            active_entity_id = $4,
            active_domain = $5,
            updated_at = $6
        WHERE session_id = $1
        "#,
    )
    .bind(session.session_id)
    .bind(session.client_group_id)
    .bind(&session.constellation_id)
    .bind(session.active_entity_id)
    .bind(&session.active_domain)
    .bind(session.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Load a sage session by ID.
#[cfg(feature = "database")]
pub async fn load_session(pool: &PgPool, session_id: Uuid) -> Result<Option<SageSession>> {
    let row = sqlx::query_as::<_, (Uuid, Option<Uuid>, Option<String>, Option<Uuid>, Option<String>, DateTime<Utc>)>(
        r#"
        SELECT session_id, client_group_id, constellation_id, active_entity_id, active_domain, updated_at
        FROM "ob-poc".sage_sessions
        WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(sid, cgid, cid, aeid, ad, ua)| SageSession {
        session_id: sid,
        client_group_id: cgid,
        constellation_id: cid,
        active_entity_id: aeid,
        active_domain: ad,
        updated_at: ua,
    }))
}

/// List active client groups with their canonical names.
/// Returns (group_id, canonical_name, discovery_status).
#[cfg(feature = "database")]
pub async fn list_active_client_groups(pool: &PgPool) -> Result<Vec<(Uuid, String, String)>> {
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"
        SELECT id, canonical_name, COALESCE(discovery_status, 'unknown')
        FROM "ob-poc".client_group
        ORDER BY canonical_name
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Load entity states for all entities belonging to a client group.
///
/// Queries the current client-group scope and projects it into the SemOS
/// constellation slots used by constrained matching. This includes CBUs,
/// KYC cases, entity workstreams, screening, requests, identifiers,
/// service agreements, and tollgate evaluations.
#[cfg(feature = "database")]
pub async fn load_entity_states_for_group(
    pool: &PgPool,
    client_group_id: Uuid,
) -> Result<Vec<EntityState>> {
    let mut states = Vec::new();
    let mut entity_slot_states: HashMap<Uuid, String> = HashMap::new();

    if let Some(discovery_status) = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT discovery_status
        FROM "ob-poc".client_group
        WHERE id = $1
        "#,
    )
    .bind(client_group_id)
    .fetch_optional(pool)
    .await?
    .flatten()
    {
        states.push(EntityState {
            entity_id: client_group_id,
            entity_type: "client_group".to_string(),
            current_state: client_group_state(&discovery_status),
            slot_name: Some("client_group".to_string()),
        });
    }

    // CBUs linked to this client group
    let cbu_rows = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT DISTINCT c.cbu_id, COALESCE(c.status, 'DISCOVERED')
        FROM "ob-poc".cbus c
        JOIN "ob-poc".client_group_entity cge ON cge.cbu_id = c.cbu_id
        WHERE cge.group_id = $1
          AND c.deleted_at IS NULL
        "#,
    )
    .bind(client_group_id)
    .fetch_all(pool)
    .await?;

    for (cbu_id, status) in &cbu_rows {
        states.push(EntityState {
            entity_id: *cbu_id,
            entity_type: "cbu".to_string(),
            current_state: status.clone(),
            slot_name: Some("cbu".to_string()),
        });
    }

    // KYC cases linked through CBUs
    if !cbu_rows.is_empty() {
        let cbu_ids: Vec<Uuid> = cbu_rows.iter().map(|(id, _)| *id).collect();
        let case_rows = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT case_id, COALESCE(status, 'INTAKE')
            FROM "ob-poc".cases
            WHERE cbu_id = ANY($1)
            "#,
        )
        .bind(&cbu_ids)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (case_id, status) in case_rows {
            states.push(EntityState {
                entity_id: case_id,
                entity_type: "kyc_case".to_string(),
                current_state: status,
                slot_name: Some("kyc_case".to_string()),
            });
        }
    }

    let case_ids: Vec<Uuid> = states
        .iter()
        .filter(|state| state.slot_name.as_deref() == Some("kyc_case"))
        .map(|state| state.entity_id)
        .collect();

    if !case_ids.is_empty() {
        let workstream_rows = sqlx::query_as::<_, WorkstreamRow>(
            r#"
            SELECT
                w.workstream_id,
                w.entity_id,
                COALESCE(et.type_code, 'entity'),
                COALESCE(w.status, 'PENDING'),
                w.blocker_type,
                COALESCE(w.identity_verified, false),
                COALESCE(w.ownership_proved, false),
                COALESCE(w.screening_cleared, false),
                COALESCE(w.evidence_complete, false),
                w.risk_rating IS NOT NULL
            FROM "ob-poc".entity_workstreams w
            JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
            JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            WHERE w.case_id = ANY($1)
            "#,
        )
        .bind(&case_ids)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if let Some((workstream_id, entity_type, current_state)) =
            aggregate_workstream_state(&workstream_rows)
        {
            for row in &workstream_rows {
                entity_slot_states.insert(row.1, entity_role_state_from_workstream(row));
            }
            states.push(EntityState {
                entity_id: workstream_id,
                entity_type,
                current_state,
                slot_name: Some("entity_workstream".to_string()),
            });
        }

        let workstream_ids: Vec<Uuid> = workstream_rows.iter().map(|row| row.0).collect();
        let entity_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT entity_id
            FROM "ob-poc".entity_workstreams
            WHERE case_id = ANY($1)
            "#,
        )
        .bind(&case_ids)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if !workstream_ids.is_empty() {
            let screening_rows = sqlx::query_as::<_, (Uuid, String, String)>(
                r#"
                SELECT screening_id, screening_type, COALESCE(status, 'PENDING')
                FROM "ob-poc".screenings
                WHERE workstream_id = ANY($1)
                "#,
            )
            .bind(&workstream_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            if let Some((screening_id, screening_state)) =
                aggregate_screening_state(&workstream_ids, &screening_rows)
            {
                states.push(EntityState {
                    entity_id: screening_id,
                    entity_type: "entity".to_string(),
                    current_state: screening_state,
                    slot_name: Some("screening".to_string()),
                });
            }
        }

        if !entity_ids.is_empty() {
            if let Some((identifier_id, is_validated, identifier_type)) =
                sqlx::query_as::<_, (Uuid, bool, String)>(
                    r#"
                SELECT identifier_id, COALESCE(is_validated, false), COALESCE(identifier_type, '')
                FROM "ob-poc".entity_identifiers
                WHERE entity_id = ANY($1)
                ORDER BY created_at DESC
                LIMIT 1
                "#,
                )
                .bind(&entity_ids)
                .fetch_optional(pool)
                .await
                .unwrap_or(None)
            {
                states.push(EntityState {
                    entity_id: identifier_id,
                    entity_type: "entity".to_string(),
                    current_state: identifier_state(is_validated, &identifier_type),
                    slot_name: Some("identifier".to_string()),
                });
            }
        }

        if let Some((request_id, request_status)) = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT request_id, COALESCE(status, 'PENDING')
            FROM "ob-poc".outstanding_requests
            WHERE case_id = ANY($1)
            ORDER BY requested_at DESC NULLS LAST, created_at DESC
            LIMIT 1
            "#,
        )
        .bind(&case_ids)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        {
            states.push(EntityState {
                entity_id: request_id,
                entity_type: "entity".to_string(),
                current_state: request_state(&request_status),
                slot_name: Some("request".to_string()),
            });
        }

        let agreement_rows = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT a.agreement_id, COALESCE(a.status, 'ACTIVE')
            FROM "ob-poc".kyc_service_agreements a
            JOIN "ob-poc".cases c ON c.service_agreement_id = a.agreement_id
            WHERE c.case_id = ANY($1)
            "#,
        )
        .bind(&case_ids)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if let Some((agreement_id, agreement_status)) = agreement_rows.into_iter().next() {
            states.push(EntityState {
                entity_id: agreement_id,
                entity_type: "company".to_string(),
                current_state: agreement_state(&agreement_status),
                slot_name: Some("kyc_agreement".to_string()),
            });
        }

        if let Some(tollgate_id) = sqlx::query_scalar(
            r#"
            SELECT evaluation_id
            FROM "ob-poc".tollgate_evaluations
            WHERE case_id = ANY($1)
            ORDER BY evaluated_at DESC
            LIMIT 1
            "#,
        )
        .bind(&case_ids)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        {
            states.push(EntityState {
                entity_id: tollgate_id,
                entity_type: "tollgate".to_string(),
                current_state: "filled".to_string(),
                slot_name: Some("tollgate".to_string()),
            });
        }
    }

    let group_entity_rows = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT cge.entity_id, COALESCE(et.type_code, 'entity')
        FROM "ob-poc".client_group_entity cge
        JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
        JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        WHERE cge.group_id = $1
        ORDER BY cge.added_at ASC NULLS LAST, cge.entity_id ASC
        "#,
    )
    .bind(client_group_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if let Some((entity_id, entity_type)) = group_entity_rows.first() {
        states.push(EntityState {
            entity_id: *entity_id,
            entity_type: normalize_entity_type(entity_type),
            current_state: "filled".to_string(),
            slot_name: Some("gleif_import".to_string()),
        });
    }

    if !cbu_rows.is_empty() {
        let cbu_ids: Vec<Uuid> = cbu_rows.iter().map(|(id, _)| *id).collect();
        let role_rows = sqlx::query_as::<_, (Uuid, String, String)>(
            r#"
            SELECT cer.entity_id, COALESCE(r.name, ''), COALESCE(et.type_code, 'entity')
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
            JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            LEFT JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            WHERE cer.cbu_id = ANY($1)
            "#,
        )
        .bind(&cbu_ids)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (entity_id, role_name, entity_type) in role_rows {
            if let Some(slot_name) = struct_role_slot_name(&role_name) {
                states.push(EntityState {
                    entity_id,
                    entity_type: normalize_entity_type(&entity_type),
                    current_state: entity_slot_states
                        .get(&entity_id)
                        .cloned()
                        .unwrap_or_else(|| "filled".to_string()),
                    slot_name: Some(slot_name.to_string()),
                });
            }
        }
    }

    Ok(states)
}

#[cfg(feature = "database")]
fn aggregate_workstream_state(rows: &[WorkstreamRow]) -> Option<(Uuid, String, String)> {
    rows.iter()
        .map(
            |(
                workstream_id,
                _entity_id,
                entity_type,
                status,
                blocker_type,
                identity_verified,
                ownership_proved,
                screening_cleared,
                evidence_complete,
                has_risk_rating,
            )| {
                let normalized_entity_type = normalize_entity_type(entity_type);
                let normalized_state = if status.eq_ignore_ascii_case("COMPLETE")
                    && (*has_risk_rating || *identity_verified || *ownership_proved)
                {
                    "verified"
                } else if blocker_type
                    .as_deref()
                    .is_some_and(|value: &str| value.eq_ignore_ascii_case("AWAITING_DOCUMENT"))
                    || status.eq_ignore_ascii_case("COLLECT")
                {
                    "documents_requested"
                } else if status.eq_ignore_ascii_case("SCREEN") {
                    "screening_initiated"
                } else if *evidence_complete {
                    "evidence_collected"
                } else if *screening_cleared {
                    "screening_complete"
                } else {
                    "open"
                };
                (
                    *workstream_id,
                    normalized_entity_type,
                    normalized_state.to_string(),
                )
            },
        )
        .min_by_key(|(_, _, state)| entity_workstream_state_rank(state))
}

#[cfg(feature = "database")]
fn entity_role_state_from_workstream(row: &WorkstreamRow) -> String {
    let (
        _workstream_id,
        _entity_id,
        _entity_type,
        status,
        _blocker_type,
        identity_verified,
        ownership_proved,
        screening_cleared,
        evidence_complete,
        has_risk_rating,
    ) = row;

    if status.eq_ignore_ascii_case("COMPLETE")
        && (*has_risk_rating || *identity_verified || *ownership_proved)
    {
        "verified".to_string()
    } else if *screening_cleared {
        "screening_complete".to_string()
    } else if *evidence_complete {
        "evidence_collected".to_string()
    } else {
        "workstream_open".to_string()
    }
}

#[cfg(feature = "database")]
fn aggregate_screening_state(
    workstream_ids: &[Uuid],
    rows: &[(Uuid, String, String)],
) -> Option<(Uuid, String)> {
    if rows.is_empty() {
        return workstream_ids
            .first()
            .copied()
            .map(|workstream_id| (workstream_id, "not_started".to_string()));
    }

    let first_id = rows[0].0;
    let mut per_type: HashMap<&str, &str> = HashMap::new();
    for (_, screening_type, status) in rows {
        per_type.insert(screening_type.as_str(), status.as_str());
    }

    if screening_all_clear(&per_type) {
        return Some((first_id, "all_clear".to_string()));
    }

    if let Some(state) = screening_type_state("SANCTIONS", &per_type, "sanctions") {
        return Some((first_id, state));
    }
    if let Some(state) = screening_type_state("PEP", &per_type, "pep") {
        return Some((first_id, state));
    }
    if let Some(state) = screening_type_state("ADVERSE_MEDIA", &per_type, "media") {
        return Some((first_id, state));
    }

    Some((first_id, "not_started".to_string()))
}

#[cfg(feature = "database")]
fn screening_all_clear(per_type: &HashMap<&str, &str>) -> bool {
    ["SANCTIONS", "PEP", "ADVERSE_MEDIA"].iter().all(|key| {
        per_type
            .get(key)
            .is_some_and(|status| status.eq_ignore_ascii_case("CLEAR"))
    })
}

#[cfg(feature = "database")]
fn screening_type_state(
    screening_type: &'static str,
    per_type: &HashMap<&str, &str>,
    prefix: &str,
) -> Option<String> {
    let status = per_type.get(screening_type)?;
    let suffix = if status.eq_ignore_ascii_case("PENDING") || status.eq_ignore_ascii_case("RUNNING")
    {
        "pending"
    } else if status.eq_ignore_ascii_case("CLEAR") {
        "clear"
    } else if status.eq_ignore_ascii_case("HIT_PENDING_REVIEW")
        || status.eq_ignore_ascii_case("HIT_CONFIRMED")
    {
        "hit"
    } else if status.eq_ignore_ascii_case("HIT_DISMISSED") {
        "clear"
    } else {
        return None;
    };
    Some(format!("{prefix}_{suffix}"))
}

#[cfg(feature = "database")]
fn entity_workstream_state_rank(state: &str) -> usize {
    match state {
        "open" => 0,
        "documents_requested" => 1,
        "screening_initiated" => 2,
        "screening_complete" => 3,
        "evidence_collected" => 4,
        "verified" => 5,
        "closed" => 6,
        _ => usize::MAX,
    }
}

#[cfg(feature = "database")]
fn normalize_entity_type(entity_type: &str) -> String {
    let upper = entity_type.to_ascii_uppercase();
    if upper.contains("PERSON") || upper.contains("INDIVIDUAL") || upper.contains("NATURAL") {
        "person".to_string()
    } else if upper.contains("COMPANY")
        || upper.contains("LEGAL")
        || upper.contains("CORPORATE")
        || upper.contains("FUND")
    {
        "company".to_string()
    } else {
        "entity".to_string()
    }
}

#[cfg(feature = "database")]
fn identifier_state(is_validated: bool, identifier_type: &str) -> String {
    if is_validated {
        "verified".to_string()
    } else if identifier_type.is_empty() {
        "empty".to_string()
    } else {
        "captured".to_string()
    }
}

#[cfg(feature = "database")]
fn request_state(status: &str) -> String {
    if status.eq_ignore_ascii_case("PENDING") {
        "requested".to_string()
    } else if status.eq_ignore_ascii_case("FULFILLED") {
        "verified".to_string()
    } else if status.eq_ignore_ascii_case("CANCELLED") || status.eq_ignore_ascii_case("EXPIRED") {
        "rejected".to_string()
    } else if status.eq_ignore_ascii_case("WAIVED") {
        "waived".to_string()
    } else if status.eq_ignore_ascii_case("PARTIAL") {
        "received".to_string()
    } else {
        "pending".to_string()
    }
}

#[cfg(feature = "database")]
fn agreement_state(status: &str) -> String {
    if status.eq_ignore_ascii_case("DRAFT") {
        "draft".to_string()
    } else if status.eq_ignore_ascii_case("SENT") {
        "sent".to_string()
    } else if status.eq_ignore_ascii_case("SIGNED") {
        "signed".to_string()
    } else if status.eq_ignore_ascii_case("ACTIVE") {
        "active".to_string()
    } else if status.eq_ignore_ascii_case("TERMINATED") {
        "terminated".to_string()
    } else {
        "pending".to_string()
    }
}

#[cfg(feature = "database")]
fn client_group_state(discovery_status: &str) -> String {
    if discovery_status.eq_ignore_ascii_case("complete") {
        "onboarding".to_string()
    } else if discovery_status.eq_ignore_ascii_case("in_progress") {
        "researching".to_string()
    } else {
        "prospect".to_string()
    }
}

#[cfg(feature = "database")]
fn struct_role_slot_name(role_name: &str) -> Option<&'static str> {
    let normalized = role_name.to_ascii_uppercase().replace('-', "_");
    match normalized.as_str() {
        "MANAGEMENT_COMPANY" => Some("management_company"),
        "DEPOSITARY" => Some("depositary"),
        "INVESTMENT_MANAGER" => Some("investment_manager"),
        _ => None,
    }
}

/// Try to extract a client group from the utterance by name matching.
///
/// Loads all client groups and checks if the utterance contains any group name
/// (case-insensitive substring match). Returns (group_id, constellation_id) if found.
#[cfg(feature = "database")]
pub async fn try_extract_client_group(
    utterance: &str,
    pool: &PgPool,
) -> Result<Option<(Uuid, String)>> {
    let groups = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, canonical_name
        FROM "ob-poc".client_group
        "#,
    )
    .fetch_all(pool)
    .await?;

    let utterance_lower = utterance.to_lowercase();
    for (id, name) in &groups {
        if utterance_lower.contains(&name.to_lowercase()) {
            // Default constellation — in a full implementation this would look up
            // the client group's jurisdiction + structure type to select the template.
            let constellation_id = "group.ownership".to_string();
            return Ok(Some((*id, constellation_id)));
        }
    }

    Ok(None)
}

/// Update session context after a verb is executed.
#[cfg(feature = "database")]
pub async fn post_verb_update(
    session: &mut SageSession,
    executed_verb: &str,
    entity_id: Option<Uuid>,
    pool: &PgPool,
) -> Result<()> {
    if let Some(eid) = entity_id {
        let domain = executed_verb.split('.').next().unwrap_or("").to_string();
        session.set_active_entity(eid, domain);
    }
    update_session(pool, session).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle() {
        let mut session = SageSession::new();
        assert!(!session.has_client_group());
        assert!(session.client_group_id.is_none());

        let group_id = Uuid::new_v4();
        session.set_client_group(group_id, "lu_ucits_sicav".to_string());
        assert!(session.has_client_group());
        assert_eq!(session.client_group_id, Some(group_id));
        assert_eq!(session.constellation_id.as_deref(), Some("lu_ucits_sicav"));
    }

    #[test]
    fn test_from_ui_context() {
        let group_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        let session =
            SageSession::from_ui_context(group_id, "group.ownership".to_string(), Some(entity_id));

        assert!(session.has_client_group());
        assert_eq!(session.client_group_id, Some(group_id));
        assert_eq!(session.active_entity_id, Some(entity_id));
    }

    #[test]
    fn test_has_client_group() {
        let session = SageSession::new();
        assert!(!session.has_client_group());

        let session = SageSession::from_ui_context(Uuid::new_v4(), "test".to_string(), None);
        assert!(session.has_client_group());
    }

    #[test]
    fn test_set_active_entity() {
        let mut session = SageSession::new();
        let eid = Uuid::new_v4();
        session.set_active_entity(eid, "document".to_string());
        assert_eq!(session.active_entity_id, Some(eid));
        assert_eq!(session.active_domain.as_deref(), Some("document"));
    }
}
