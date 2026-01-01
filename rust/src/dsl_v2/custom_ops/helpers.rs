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
        let row: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1"#)
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
                           LIMIT 1"#,
                    )
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
                }
                _ => {
                    // Generic entity lookup by name
                    sqlx::query_scalar(
                        r#"SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE $1 LIMIT 1"#,
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
