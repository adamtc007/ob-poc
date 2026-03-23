//! Sage session context — tracks which client group, constellation, and entity
//! are in focus for the SemOS-scoped verb resolution pipeline.

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
/// Queries CBUs linked via `client_group_entity` and returns their current
/// status as FSM states. Also queries KYC cases linked through those CBUs.
#[cfg(feature = "database")]
pub async fn load_entity_states_for_group(
    pool: &PgPool,
    client_group_id: Uuid,
) -> Result<Vec<EntityState>> {
    let mut states = Vec::new();

    // CBUs linked to this client group
    let cbu_rows = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT c.cbu_id, COALESCE(c.status, 'DISCOVERED')
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
              AND deleted_at IS NULL
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

    Ok(states)
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
