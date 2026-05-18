use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use ob_poc_types::{InstanceState, InstanceStatus, InstanceSummary};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::BpmnControllerError;

// ── Private helpers ───────────────────────────────────────────────────────────

fn parse_instance_state(v: &serde_json::Value) -> InstanceState {
    match v {
        serde_json::Value::String(s) if s == "Running" => InstanceState::Running,
        serde_json::Value::Object(map) => {
            if map.contains_key("Completed") {
                InstanceState::Completed
            } else if map.contains_key("Failed") {
                InstanceState::Failed
            } else if map.contains_key("Cancelled") {
                InstanceState::Cancelled
            } else if map.contains_key("Terminated") {
                InstanceState::Terminated
            } else {
                InstanceState::Running
            }
        }
        _ => InstanceState::Running,
    }
}

fn extract_completed_at(state_json: &serde_json::Value) -> Option<DateTime<Utc>> {
    let ms = state_json
        .get("Completed")
        .and_then(|c| c.get("at"))
        .and_then(|v| v.as_i64())?;
    DateTime::from_timestamp_millis(ms)
}

/// Decode a 64-char hex string to a fixed 32-byte array.
fn hex_decode_32(hex: &str) -> Result<[u8; 32]> {
    if hex.len() != 64 {
        return Err(anyhow!(
            "bytecode_version must be 64 hex chars, got {}",
            hex.len()
        ));
    }
    let mut out = [0u8; 32];
    for (i, pair) in hex.as_bytes().chunks(2).enumerate() {
        out[i] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(out)
}

fn nibble(c: u8) -> Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(anyhow!("invalid hex character: {}", c as char)),
    }
}

/// Compute the A19 integrity hash for a new process instance.
///
/// Matches `bpmn_lite_types::integrity::compute_instance_integrity_hash`.
///
/// Input order (BLAKE3, `|`-separated):
/// `instance_id (16B) | tenant_id | bytecode_version (32B) |
///  created_at_ms (8B LE) | process_key | entry_id (16B) | runbook_id (16B) |
///  b"" (created_by_identity placeholder — v0.2)`
fn compute_instance_integrity_hash(
    instance_id: Uuid,
    tenant_id: &str,
    bytecode_version: &[u8; 32],
    created_at_ms: i64,
    process_key: &str,
    entry_id: Uuid,
    runbook_id: Uuid,
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(instance_id.as_bytes());
    h.update(b"|");
    h.update(tenant_id.as_bytes());
    h.update(b"|");
    h.update(bytecode_version.as_ref());
    h.update(b"|");
    h.update(&created_at_ms.to_le_bytes());
    h.update(b"|");
    h.update(process_key.as_bytes());
    h.update(b"|");
    h.update(entry_id.as_bytes());
    h.update(b"|");
    h.update(runbook_id.as_bytes());
    h.update(b""); // created_by_identity — v0.2 field, absent in v0.1
    h.finalize().into()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Start a new process instance for a tenant.
///
/// Validates that the tenant and process definition (latest published template
/// for `process_definition_id`) exist, then writes a row to
/// `process_instances` with `state = Running`. Returns the new `instance_id`
/// immediately — execution is handled asynchronously by the pool's workers.
///
/// Pass `idempotency_key` to make the call retry-safe: if an instance already
/// exists for `(tenant_id, key)`, its `instance_id` is returned without
/// creating a duplicate. The key is stored as the instance's `correlation_id`.
///
/// `entry_id` and `runbook_id` default to `Uuid::nil()` in v0.1. The L6 DSL
/// verb will populate these from the caller's session context.
///
/// # Errors
/// - `TenantNotFound` — tenant does not exist.
/// - `ProcessDefinitionNotFound` — no published template for the given process key.
pub async fn start_instance(
    pg: &PgPool,
    tenant_id: &str,
    process_definition_id: &str,
    initial_payload: serde_json::Value,
    idempotency_key: Option<&str>,
) -> Result<Uuid> {
    // 1. Idempotency short-circuit — before any writes.
    if let Some(key) = idempotency_key {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT instance_id FROM process_instances \
             WHERE tenant_id = $1 AND correlation_id = $2 \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(tenant_id)
        .bind(key)
        .fetch_optional(pg)
        .await?;
        if let Some(id) = existing {
            return Ok(id);
        }
    }

    // 2. Validate tenant.
    let tenant_ok: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenants WHERE tenant_id = $1)")
            .bind(tenant_id)
            .fetch_one(pg)
            .await?;
    if !tenant_ok {
        return Err(BpmnControllerError::TenantNotFound(tenant_id.to_string()).into());
    }

    // 3. Resolve latest published template → bytecode_version hex.
    let bytecode_hex: Option<String> = sqlx::query_scalar(
        "SELECT bytecode_version FROM workflow_templates \
         WHERE process_key = $1 AND state = 'published' \
         ORDER BY template_version DESC LIMIT 1",
    )
    .bind(process_definition_id)
    .fetch_optional(pg)
    .await?;

    let bytecode_hex = bytecode_hex.ok_or_else(|| {
        BpmnControllerError::ProcessDefinitionNotFound(process_definition_id.to_string())
    })?;
    let bytecode_version = hex_decode_32(&bytecode_hex)?;

    // 4. Prepare fields.
    let instance_id = Uuid::now_v7();
    let created_at: DateTime<Utc> = Utc::now();
    let created_at_ms = created_at.timestamp_millis();
    // Lineage fields default to nil; L6's DSL verb will supply real values.
    let entry_id = Uuid::nil();
    let runbook_id = Uuid::nil();

    let payload_str = serde_json::to_string(&initial_payload)?;
    let domain_payload_hash: [u8; 32] = blake3::hash(payload_str.as_bytes()).into();
    let integrity_hash = compute_instance_integrity_hash(
        instance_id,
        tenant_id,
        &bytecode_version,
        created_at_ms,
        process_definition_id,
        entry_id,
        runbook_id,
    );

    let correlation_id = idempotency_key
        .map(str::to_string)
        .unwrap_or_else(|| instance_id.to_string());

    // 5. Insert.
    //
    // session_stack, flags, counters, join_expected use their NOT NULL DEFAULT '{}'.
    // created_at is set explicitly so the integrity_hash computation matches.
    sqlx::query(
        "INSERT INTO process_instances \
         (instance_id, tenant_id, process_key, bytecode_version, \
          domain_payload, domain_payload_hash, state, correlation_id, \
          entry_id, runbook_id, integrity_hash, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)",
    )
    .bind(instance_id)
    .bind(tenant_id)
    .bind(process_definition_id)
    .bind(&bytecode_version[..])
    .bind(&payload_str)
    .bind(&domain_payload_hash[..])
    .bind(serde_json::json!("Running")) // ProcessState::Running serialised form
    .bind(&correlation_id)
    .bind(entry_id)
    .bind(runbook_id)
    .bind(&integrity_hash[..])
    .bind(created_at)
    .execute(pg)
    .await?;

    Ok(instance_id)
}

/// Return the current status of a process instance.
pub async fn instance_status(pg: &PgPool, instance_id: Uuid) -> Result<InstanceStatus> {
    use sqlx::Row;

    let row = sqlx::query(
        "SELECT instance_id, tenant_id, process_key, state, created_at, quarantine_state \
         FROM process_instances \
         WHERE instance_id = $1",
    )
    .bind(instance_id)
    .fetch_optional(pg)
    .await?;

    let row = row.ok_or_else(|| BpmnControllerError::InstanceNotFound(instance_id.to_string()))?;

    let state_json: serde_json::Value = row.get("state");
    let state = parse_instance_state(&state_json);
    let completed_at = extract_completed_at(&state_json);

    Ok(InstanceStatus {
        instance_id: row.get("instance_id"),
        tenant_id: row.get("tenant_id"),
        process_key: row.get("process_key"),
        state,
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        completed_at,
        quarantine_state: row.get("quarantine_state"),
    })
}

/// List instances for a tenant, ordered by creation time descending.
pub async fn list_tenant_instances(pg: &PgPool, tenant_id: &str) -> Result<Vec<InstanceSummary>> {
    use sqlx::Row;

    let rows = sqlx::query(
        "SELECT instance_id, process_key, state, created_at \
         FROM process_instances \
         WHERE tenant_id = $1 \
         ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pg)
    .await?;

    let summaries = rows
        .iter()
        .map(|r| {
            let state_json: serde_json::Value = r.get("state");
            InstanceSummary {
                instance_id: r.get("instance_id"),
                process_key: r.get("process_key"),
                state: parse_instance_state(&state_json),
                created_at: r.get::<DateTime<Utc>, _>("created_at"),
            }
        })
        .collect();

    Ok(summaries)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── State parsing ─────────────────────────────────────────────────────────

    #[test]
    fn parse_running_state() {
        assert_eq!(
            parse_instance_state(&json!("Running")),
            InstanceState::Running
        );
    }

    #[test]
    fn parse_completed_state() {
        assert_eq!(
            parse_instance_state(&json!({"Completed": {"at": 1700000000000_i64}})),
            InstanceState::Completed
        );
    }

    #[test]
    fn parse_failed_state() {
        assert_eq!(
            parse_instance_state(&json!({"Failed": {"incident_id": "abc"}})),
            InstanceState::Failed
        );
    }

    #[test]
    fn parse_cancelled_state() {
        assert_eq!(
            parse_instance_state(
                &json!({"Cancelled": {"reason": "user", "at": 1700000000000_i64}})
            ),
            InstanceState::Cancelled
        );
    }

    #[test]
    fn parse_terminated_state() {
        assert_eq!(
            parse_instance_state(&json!({"Terminated": {"at": 1700000000000_i64}})),
            InstanceState::Terminated
        );
    }

    #[test]
    fn extract_completed_at_present() {
        let ms = 1700000000000_i64;
        let dt = extract_completed_at(&json!({"Completed": {"at": ms}})).unwrap();
        assert_eq!(dt.timestamp_millis(), ms);
    }

    #[test]
    fn extract_completed_at_absent_for_running() {
        assert!(extract_completed_at(&json!("Running")).is_none());
    }

    // ── hex_decode_32 ─────────────────────────────────────────────────────────

    #[test]
    fn hex_decode_32_all_zeros() {
        let result = hex_decode_32(&"00".repeat(32)).unwrap();
        assert_eq!(result, [0u8; 32]);
    }

    #[test]
    fn hex_decode_32_all_ff() {
        let result = hex_decode_32(&"ff".repeat(32)).unwrap();
        assert_eq!(result, [0xffu8; 32]);
    }

    #[test]
    fn hex_decode_32_known_pattern() {
        let hex = format!("{}{}", "0a".repeat(16), "f5".repeat(16));
        let result = hex_decode_32(&hex).unwrap();
        for b in &result[..16] {
            assert_eq!(*b, 0x0a);
        }
        for b in &result[16..] {
            assert_eq!(*b, 0xf5);
        }
    }

    #[test]
    fn hex_decode_32_wrong_length_errors() {
        assert!(hex_decode_32("aabb").is_err());
        assert!(hex_decode_32(&"aa".repeat(33)).is_err());
    }

    #[test]
    fn hex_decode_32_invalid_char_errors() {
        let bad = format!("{}zz{}", "aa".repeat(15), "aa".repeat(16));
        assert!(hex_decode_32(&bad).is_err());
    }

    // ── integrity hash ────────────────────────────────────────────────────────

    #[test]
    fn integrity_hash_is_deterministic() {
        let id = Uuid::nil();
        let bv = [0xabu8; 32];
        let h1 = compute_instance_integrity_hash(id, "tenant", &bv, 12345, "proc", id, id);
        let h2 = compute_instance_integrity_hash(id, "tenant", &bv, 12345, "proc", id, id);
        assert_eq!(h1, h2);
    }

    #[test]
    fn integrity_hash_sensitive_to_tenant_id() {
        let id = Uuid::nil();
        let bv = [0u8; 32];
        let h1 = compute_instance_integrity_hash(id, "tenant-a", &bv, 0, "proc", id, id);
        let h2 = compute_instance_integrity_hash(id, "tenant-b", &bv, 0, "proc", id, id);
        assert_ne!(h1, h2);
    }

    #[test]
    fn integrity_hash_sensitive_to_created_at_ms() {
        let id = Uuid::nil();
        let bv = [0u8; 32];
        let h1 = compute_instance_integrity_hash(id, "t", &bv, 1000, "proc", id, id);
        let h2 = compute_instance_integrity_hash(id, "t", &bv, 1001, "proc", id, id);
        assert_ne!(h1, h2);
    }

    #[test]
    fn integrity_hash_sensitive_to_instance_id() {
        let id1 = Uuid::nil();
        let id2 = Uuid::max();
        let bv = [0u8; 32];
        let h1 = compute_instance_integrity_hash(id1, "t", &bv, 0, "p", id1, id1);
        let h2 = compute_instance_integrity_hash(id2, "t", &bv, 0, "p", id1, id1);
        assert_ne!(h1, h2);
    }
}
