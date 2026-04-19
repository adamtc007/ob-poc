//! Common helper functions for custom operations
//!
//! This module provides shared utilities for argument extraction, UUID resolution,
//! and entity lookups used across all custom operation handlers.

use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::ExecutionContext;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// UUID Extraction (sync, no DB)
// ============================================================================

/// Extract a required UUID argument from a verb call.
/// Handles @symbol references and literal UUIDs.
pub fn extract_uuid(verb_call: &VerbCall, ctx: &ExecutionContext, arg_name: &str) -> Result<Uuid> {
    extract_uuid_opt(verb_call, ctx, arg_name)
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Simple UUID extraction without context - for ops that don't use symbol resolution.
/// Handles literal UUIDs and string UUIDs directly from verb arguments.
pub fn get_required_uuid(verb_call: &VerbCall, arg_name: &str) -> Result<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            // Try literal UUID
            if let Some(uuid) = a.value.as_uuid() {
                return Some(uuid);
            }
            // Try parsing string as UUID
            if let Some(s) = a.value.as_string() {
                return Uuid::parse_str(s).ok();
            }
            None
        })
        .ok_or_else(|| anyhow!("Missing or invalid {} argument", arg_name))
}

/// Extract an optional UUID argument from a verb call.
/// Handles @symbol references and literal UUIDs.
pub fn extract_uuid_opt(
    verb_call: &VerbCall,
    ctx: &ExecutionContext,
    arg_name: &str,
) -> Option<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            // Try symbol reference first
            if let Some(sym) = a.value.as_symbol() {
                return ctx.resolve(sym);
            }
            // Try literal UUID
            if let Some(uuid) = a.value.as_uuid() {
                return Some(uuid);
            }
            // Try parsing string as UUID
            if let Some(s) = a.value.as_string() {
                return Uuid::parse_str(s).ok();
            }
            None
        })
}

// ============================================================================
// String Extraction (sync, no DB)
// ============================================================================

/// Extract a required string argument from a verb call.
pub fn extract_string(verb_call: &VerbCall, arg_name: &str) -> Result<String> {
    extract_string_opt(verb_call, arg_name).ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract an optional string argument from a verb call.
pub fn extract_string_opt(verb_call: &VerbCall, arg_name: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

/// Extract a list of strings from a verb argument.
pub fn extract_string_list(verb_call: &VerbCall, arg_name: &str) -> Result<Vec<String>> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            a.value.as_list().map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_string().map(|s| s.to_string()))
                    .collect()
            })
        })
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract an optional list of strings from a verb argument.
pub fn extract_string_list_opt(verb_call: &VerbCall, arg_name: &str) -> Option<Vec<String>> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            a.value.as_list().map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_string().map(|s| s.to_string()))
                    .collect()
            })
        })
}

// ============================================================================
// Boolean Extraction (sync, no DB)
// ============================================================================

/// Extract an optional boolean argument from a verb call.
pub fn extract_bool_opt(verb_call: &VerbCall, arg_name: &str) -> Option<bool> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| a.value.as_boolean())
}

/// Extract a required boolean argument from a verb call.
pub fn extract_bool(verb_call: &VerbCall, arg_name: &str) -> Result<bool> {
    extract_bool_opt(verb_call, arg_name).ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

// ============================================================================
// Integer Extraction (sync, no DB)
// ============================================================================

/// Extract an optional integer argument from a verb call.
pub fn extract_int_opt(verb_call: &VerbCall, arg_name: &str) -> Option<i64> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| a.value.as_integer())
}

/// Extract a required integer argument from a verb call.
pub fn extract_int(verb_call: &VerbCall, arg_name: &str) -> Result<i64> {
    extract_int_opt(verb_call, arg_name).ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

// ============================================================================
// CBU ID Extraction (sync, no DB) - handles "cbu" or "cbu-id" aliases
// ============================================================================

/// Extract CBU ID from a verb call, accepting either "cbu" or "cbu-id" as the argument name.
/// This is the common pattern used ~12 times in ubo_graph_ops.rs.
pub fn extract_cbu_id(verb_call: &VerbCall, ctx: &ExecutionContext) -> Result<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == "cbu" || a.key == "cbu-id")
        .and_then(|a| {
            // Try symbol reference first
            if let Some(sym) = a.value.as_symbol() {
                return ctx.resolve(sym);
            }
            // Try literal UUID
            if let Some(uuid) = a.value.as_uuid() {
                return Some(uuid);
            }
            // Try parsing string as UUID
            if let Some(s) = a.value.as_string() {
                return Uuid::parse_str(s).ok();
            }
            None
        })
        .ok_or_else(|| anyhow!("Missing cbu or cbu-id argument"))
}

// ============================================================================
// CBU Resolution (async, requires DB)
// ============================================================================

/// Resolve a CBU by either cbu-id (UUID/@symbol) or cbu-name (string lookup).
#[cfg(feature = "database")]
pub async fn resolve_cbu_id(
    verb_call: &VerbCall,
    ctx: &ExecutionContext,
    pool: &PgPool,
) -> Result<Uuid> {
    // First try cbu-id (direct UUID or @symbol reference)
    if let Some(cbu_id) = extract_uuid_opt(verb_call, ctx, "cbu-id") {
        return Ok(cbu_id);
    }

    // Fall back to cbu-name lookup
    if let Some(cbu_name) = extract_string_opt(verb_call, "cbu-name") {
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1 AND deleted_at IS NULL"#,
        )
        .bind(&cbu_name)
        .fetch_optional(pool)
        .await?;

        return row
            .map(|(id,)| id)
            .ok_or_else(|| anyhow!("CBU not found: {}", cbu_name));
    }

    Err(anyhow!("Missing cbu-id or cbu-name argument"))
}

// ============================================================================
// Entity Resolution (async, requires DB)
// ============================================================================

// ============================================================================
// JSON-based extraction (for SemOS VerbExecutionContext path — Phase 2)
// ============================================================================
// These mirror the VerbCall-based helpers above but work with
// serde_json::Value args and sem_os_core::VerbExecutionContext.

/// Extract a required string from JSON args.
pub fn json_extract_string(args: &serde_json::Value, arg_name: &str) -> Result<String> {
    json_extract_string_opt(args, arg_name).ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract an optional string from JSON args.
pub fn json_extract_string_opt(args: &serde_json::Value, arg_name: &str) -> Option<String> {
    args.get(arg_name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract a required UUID from JSON args + context symbols.
pub fn json_extract_uuid(
    args: &serde_json::Value,
    ctx: &dsl_runtime::VerbExecutionContext,
    arg_name: &str,
) -> Result<Uuid> {
    json_extract_uuid_opt(args, ctx, arg_name)
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract an optional UUID from JSON args + context symbols.
pub fn json_extract_uuid_opt(
    args: &serde_json::Value,
    ctx: &dsl_runtime::VerbExecutionContext,
    arg_name: &str,
) -> Option<Uuid> {
    args.get(arg_name).and_then(|v| {
        // Try as string → parse as UUID
        if let Some(s) = v.as_str() {
            // Check if it's a @symbol reference
            if let Some(sym) = s.strip_prefix('@') {
                return ctx.resolve(sym);
            }
            return Uuid::parse_str(s).ok();
        }
        None
    })
}

/// Simple UUID extraction from JSON args without context.
pub fn json_get_required_uuid(args: &serde_json::Value, arg_name: &str) -> Result<Uuid> {
    args.get(arg_name)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| anyhow!("Missing or invalid {} argument", arg_name))
}

/// Extract an optional boolean from JSON args.
pub fn json_extract_bool_opt(args: &serde_json::Value, arg_name: &str) -> Option<bool> {
    args.get(arg_name).and_then(|v| v.as_bool())
}

/// Extract a required boolean from JSON args.
pub fn json_extract_bool(args: &serde_json::Value, arg_name: &str) -> Result<bool> {
    json_extract_bool_opt(args, arg_name).ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract an optional integer from JSON args.
pub fn json_extract_int_opt(args: &serde_json::Value, arg_name: &str) -> Option<i64> {
    args.get(arg_name).and_then(|v| v.as_i64())
}

/// Extract a required integer from JSON args.
pub fn json_extract_int(args: &serde_json::Value, arg_name: &str) -> Result<i64> {
    json_extract_int_opt(args, arg_name).ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract an optional string list from JSON args.
pub fn json_extract_string_list_opt(
    args: &serde_json::Value,
    arg_name: &str,
) -> Option<Vec<String>> {
    args.get(arg_name).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    })
}

/// Extract a required string list from JSON args.
pub fn json_extract_string_list(args: &serde_json::Value, arg_name: &str) -> Result<Vec<String>> {
    json_extract_string_list_opt(args, arg_name)
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract CBU ID from JSON args, accepting "cbu" or "cbu-id".
pub fn json_extract_cbu_id(
    args: &serde_json::Value,
    ctx: &dsl_runtime::VerbExecutionContext,
) -> Result<Uuid> {
    json_extract_uuid_opt(args, ctx, "cbu-id")
        .or_else(|| json_extract_uuid_opt(args, ctx, "cbu"))
        .ok_or_else(|| anyhow!("Missing cbu or cbu-id argument"))
}

// ============================================================================
// Entity Resolution (async, requires DB)
// ============================================================================

/// Extract and resolve an entity reference from a verb argument.
///
/// Handles multiple formats:
/// - @symbol reference (looked up in context)
/// - Direct UUID literal
/// - Entity ref tuple: ("entity_type" "name" "uuid_or_nil")
///
/// For tuples, if UUID is nil, looks up entity by type and name in the database.
#[cfg(feature = "database")]
pub async fn extract_entity_ref(
    verb_call: &VerbCall,
    arg_name: &str,
    ctx: &ExecutionContext,
    pool: &PgPool,
) -> Result<Uuid> {
    let arg = verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))?;

    // Try symbol reference first
    if let Some(symbol) = arg.value.as_symbol() {
        return ctx
            .resolve(symbol)
            .ok_or_else(|| anyhow!("Unresolved symbol: @{}", symbol));
    }

    // Try direct UUID
    if let Some(uuid) = arg.value.as_uuid() {
        return Ok(uuid);
    }

    // Try entity ref tuple: ("entity_type" "name" "uuid_or_nil")
    if let Some(items) = arg.value.as_list() {
        if items.len() >= 2 {
            // If third item is a UUID, use it
            if items.len() >= 3 {
                if let Some(uuid) = items[2].as_uuid() {
                    return Ok(uuid);
                }
            }

            // Otherwise, look up by entity_type and name
            let entity_type = items[0]
                .as_string()
                .ok_or_else(|| anyhow!("Invalid entity ref: expected entity_type string"))?;
            let name = items[1]
                .as_string()
                .ok_or_else(|| anyhow!("Invalid entity ref: expected name string"))?;

            // Look up entity by type and name
            let entity_id: Option<Uuid> = match entity_type {
                "proper_person" | "person" => {
                    sqlx::query_scalar(
                        r#"SELECT e.entity_id FROM "ob-poc".entities e
                           JOIN "ob-poc".entity_proper_persons p ON p.entity_id = e.entity_id
                           WHERE CONCAT(p.first_name, ' ', p.last_name) ILIKE $1
                           AND e.deleted_at IS NULL
                           LIMIT 1"#,
                    )
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
                }
                "fund" => {
                    sqlx::query_scalar(
                        r#"SELECT e.entity_id FROM "ob-poc".entities e
                           JOIN "ob-poc".entity_funds f ON f.entity_id = e.entity_id
                           WHERE e.name ILIKE $1
                           AND e.deleted_at IS NULL
                           LIMIT 1"#,
                    )
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
                }
                "manco" => {
                    sqlx::query_scalar(
                        r#"SELECT e.entity_id FROM "ob-poc".entities e
                           JOIN "ob-poc".entity_manco m ON m.entity_id = e.entity_id
                           WHERE e.name ILIKE $1
                           AND e.deleted_at IS NULL
                           LIMIT 1"#,
                    )
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
                }
                "company" | "limited_company" => {
                    sqlx::query_scalar(
                        r#"SELECT e.entity_id FROM "ob-poc".entities e
                           JOIN "ob-poc".entity_limited_companies c ON c.entity_id = e.entity_id
                           WHERE c.company_name ILIKE $1
                           AND e.deleted_at IS NULL
                           LIMIT 1"#,
                    )
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
                }
                _ => {
                    // Generic entity lookup by name
                    sqlx::query_scalar(
                        r#"SELECT entity_id
                           FROM "ob-poc".entities
                           WHERE name ILIKE $1
                             AND deleted_at IS NULL
                           LIMIT 1"#,
                    )
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
                }
            };

            return entity_id
                .ok_or_else(|| anyhow!("Entity not found: {} '{}'", entity_type, name));
        }
    }

    Err(anyhow!(
        "Invalid {} argument: expected @symbol, UUID, or entity ref tuple",
        arg_name
    ))
}

/// Extract an optional entity reference. Returns None if argument is missing.
#[cfg(feature = "database")]
pub async fn extract_entity_ref_opt(
    verb_call: &VerbCall,
    arg_name: &str,
    ctx: &ExecutionContext,
    pool: &PgPool,
) -> Result<Option<Uuid>> {
    if verb_call.arguments.iter().any(|a| a.key == arg_name) {
        Ok(Some(
            extract_entity_ref(verb_call, arg_name, ctx, pool).await?,
        ))
    } else {
        Ok(None)
    }
}

// ============================================================================
// VerbExecutionContext extensions transport (Phase 2.5 Slice B+)
// ============================================================================
// Native `execute_json` bodies read/write session-scoped side-channel state
// through `VerbExecutionContext.extensions` under stable JSON keys. These
// helpers replace the legacy `ExecutionContext.pending_*` field access
// pattern used by session/view/agent ops.

use crate::session::{UnifiedSession, ViewState};

/// JSON key used to carry the pending `UnifiedSession` across the
/// dispatch boundary.
pub const EXT_KEY_PENDING_SESSION: &str = "pending_session";

/// JSON key used to carry the pending `ViewState` across the dispatch
/// boundary.
pub const EXT_KEY_PENDING_VIEW_STATE: &str = "pending_view_state";

fn ext_obj_mut(ctx: &mut dsl_runtime::VerbExecutionContext) -> &mut serde_json::Map<String, serde_json::Value> {
    if !ctx.extensions.is_object() {
        ctx.extensions = serde_json::Value::Object(serde_json::Map::new());
    }
    ctx.extensions.as_object_mut().unwrap()
}

/// Consume the pending `UnifiedSession` from `sem_ctx.extensions` if any.
pub fn ext_take_pending_session(ctx: &mut dsl_runtime::VerbExecutionContext) -> Option<UnifiedSession> {
    let obj = ctx.extensions.as_object_mut()?;
    let v = obj.remove(EXT_KEY_PENDING_SESSION)?;
    serde_json::from_value(v).ok()
}

/// Write a `UnifiedSession` to `sem_ctx.extensions` under the pending-session key.
pub fn ext_set_pending_session(ctx: &mut dsl_runtime::VerbExecutionContext, session: UnifiedSession) {
    if let Ok(v) = serde_json::to_value(&session) {
        ext_obj_mut(ctx).insert(EXT_KEY_PENDING_SESSION.to_string(), v);
    }
}

/// Get-or-create the pending `UnifiedSession` in `sem_ctx.extensions`.
/// Mirrors `ExecutionContext::get_or_create_session_mut` but over JSON
/// transport — caller mutates the owned value, then writes back via
/// `ext_set_pending_session`.
pub fn ext_take_or_create_pending_session(
    ctx: &mut dsl_runtime::VerbExecutionContext,
) -> UnifiedSession {
    ext_take_pending_session(ctx).unwrap_or_else(UnifiedSession::new)
}

/// Write a `ViewState` to `sem_ctx.extensions` under the pending-view-state key.
pub fn ext_set_pending_view_state(ctx: &mut dsl_runtime::VerbExecutionContext, view: ViewState) {
    if let Ok(v) = serde_json::to_value(&view) {
        ext_obj_mut(ctx).insert(EXT_KEY_PENDING_VIEW_STATE.to_string(), v);
    }
}

/// Read a string-valued key from `sem_ctx.extensions`.
pub fn ext_get_string(ctx: &dsl_runtime::VerbExecutionContext, key: &str) -> Option<String> {
    ctx.extensions
        .as_object()?
        .get(key)?
        .as_str()
        .map(|s| s.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_core::principal::Principal;

    #[test]
    fn json_extract_string_present() {
        let args = serde_json::json!({"name": "Acme Fund"});
        assert_eq!(json_extract_string(&args, "name").unwrap(), "Acme Fund");
    }

    #[test]
    fn json_extract_string_missing() {
        let args = serde_json::json!({});
        assert!(json_extract_string(&args, "name").is_err());
    }

    #[test]
    fn json_extract_string_opt_present() {
        let args = serde_json::json!({"name": "Test"});
        assert_eq!(
            json_extract_string_opt(&args, "name"),
            Some("Test".to_string())
        );
    }

    #[test]
    fn json_extract_string_opt_missing() {
        let args = serde_json::json!({});
        assert_eq!(json_extract_string_opt(&args, "name"), None);
    }

    #[test]
    fn json_extract_uuid_from_string() {
        let id = uuid::Uuid::new_v4();
        let args = serde_json::json!({"entity-id": id.to_string()});
        let ctx = dsl_runtime::VerbExecutionContext::new(Principal::system());
        assert_eq!(json_extract_uuid(&args, &ctx, "entity-id").unwrap(), id);
    }

    #[test]
    fn json_extract_uuid_from_symbol() {
        let id = uuid::Uuid::new_v4();
        let args = serde_json::json!({"cbu-id": "@cbu"});
        let mut ctx = dsl_runtime::VerbExecutionContext::new(Principal::system());
        ctx.bind("cbu", id);
        assert_eq!(json_extract_uuid(&args, &ctx, "cbu-id").unwrap(), id);
    }

    #[test]
    fn json_extract_uuid_missing() {
        let args = serde_json::json!({});
        let ctx = dsl_runtime::VerbExecutionContext::new(Principal::system());
        assert!(json_extract_uuid(&args, &ctx, "id").is_err());
    }

    #[test]
    fn json_get_required_uuid_valid() {
        let id = uuid::Uuid::new_v4();
        let args = serde_json::json!({"id": id.to_string()});
        assert_eq!(json_get_required_uuid(&args, "id").unwrap(), id);
    }

    #[test]
    fn json_get_required_uuid_invalid() {
        let args = serde_json::json!({"id": "not-a-uuid"});
        assert!(json_get_required_uuid(&args, "id").is_err());
    }

    #[test]
    fn json_extract_bool_opt_present() {
        let args = serde_json::json!({"active": true});
        assert_eq!(json_extract_bool_opt(&args, "active"), Some(true));
    }

    #[test]
    fn json_extract_int_present() {
        let args = serde_json::json!({"count": 42});
        assert_eq!(json_extract_int(&args, "count").unwrap(), 42);
    }

    #[test]
    fn json_extract_string_list_present() {
        let args = serde_json::json!({"tags": ["a", "b", "c"]});
        assert_eq!(
            json_extract_string_list(&args, "tags").unwrap(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn json_extract_cbu_id_from_cbu_id_key() {
        let id = uuid::Uuid::new_v4();
        let args = serde_json::json!({"cbu-id": id.to_string()});
        let ctx = dsl_runtime::VerbExecutionContext::new(Principal::system());
        assert_eq!(json_extract_cbu_id(&args, &ctx).unwrap(), id);
    }

    #[test]
    fn json_extract_cbu_id_from_cbu_key() {
        let id = uuid::Uuid::new_v4();
        let args = serde_json::json!({"cbu": id.to_string()});
        let ctx = dsl_runtime::VerbExecutionContext::new(Principal::system());
        assert_eq!(json_extract_cbu_id(&args, &ctx).unwrap(), id);
    }

    #[test]
    fn json_extract_cbu_id_missing() {
        let args = serde_json::json!({});
        let ctx = dsl_runtime::VerbExecutionContext::new(Principal::system());
        assert!(json_extract_cbu_id(&args, &ctx).is_err());
    }
}
