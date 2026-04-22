//! JSON-based arg-extraction helpers for plugin ops that live in the data plane.
//!
//! Ported from `ob-poc::domain_ops::helpers` as part of Phase 4 Slice A of the
//! three-plane architecture refactor. Only the `json_*` family — which takes
//! `&serde_json::Value` + `&VerbExecutionContext` — is mirrored here because
//! those are the only helpers the live `execute_json` path needs. The legacy
//! `VerbCall`-based `extract_*` helpers remain in `ob-poc::domain_ops::helpers`
//! and stay there alongside the legacy inherent `execute` methods they serve.

use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::execution::VerbExecutionContext;

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
    ctx: &VerbExecutionContext,
    arg_name: &str,
) -> Result<Uuid> {
    json_extract_uuid_opt(args, ctx, arg_name)
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract an optional UUID from JSON args + context symbols.
pub fn json_extract_uuid_opt(
    args: &serde_json::Value,
    ctx: &VerbExecutionContext,
    arg_name: &str,
) -> Option<Uuid> {
    args.get(arg_name).and_then(|v| {
        if let Some(s) = v.as_str() {
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
    ctx: &VerbExecutionContext,
) -> Result<Uuid> {
    json_extract_uuid_opt(args, ctx, "cbu-id")
        .or_else(|| json_extract_uuid_opt(args, ctx, "cbu"))
        .ok_or_else(|| anyhow!("Missing cbu or cbu-id argument"))
}

// ---------------------------------------------------------------------------
// Phase C.3 helpers (F7 follow-on, 2026-04-22)
// ---------------------------------------------------------------------------

/// Emit a `PendingStateAdvance` via the `ctx.extensions["_pending_state_advance"]`
/// side channel. Read by the dispatcher post-execute and shadow-logged; Phase C.2
/// applies the advance via SemOS inside the Sequencer's outer scope.
///
/// Use from plugin ops that mutate state, AFTER the DB write has committed
/// (or is guaranteed by the ambient txn). Only emit on a genuine state
/// transition — idempotent returns that found an existing row should NOT
/// emit, because no state advance occurred.
///
/// Arguments:
/// - `ctx`: verb execution context — `ctx.extensions` will be promoted to
///   an object if it isn't one already
/// - `entity_id`: the entity whose DAG node transitioned
/// - `to_node`: spec-defined state-machine node, e.g. `"cbu:onboarded"`,
///   `"entity:ghost"`, `"kyc-case:open"`
/// - `slot_path`: constellation slot that needs rehydration, e.g.
///   `"cbu/trading-profile"`, `"entity/identity"`
/// - `reason`: human-readable audit string
pub fn emit_pending_state_advance(
    ctx: &mut VerbExecutionContext,
    entity_id: Uuid,
    to_node: &str,
    slot_path: &str,
    reason: &str,
) {
    if !ctx.extensions.is_object() {
        ctx.extensions = serde_json::Value::Object(serde_json::Map::new());
    }
    if let Some(ext_obj) = ctx.extensions.as_object_mut() {
        ext_obj.insert(
            "_pending_state_advance".to_string(),
            serde_json::json!({
                "state_transitions": [{
                    "entity_id": entity_id.to_string(),
                    "from_node": null,
                    "to_node": to_node,
                    "reason": reason,
                }],
                "constellation_marks": [{
                    "slot_path": slot_path,
                    "entity_id": entity_id.to_string(),
                }],
                "writes_since_push_delta": 1,
                "catalogue_effects": [],
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn json_extract_string_list_ok() {
        let args = serde_json::json!({"xs": ["a", "b", "c"]});
        assert_eq!(
            json_extract_string_list(&args, "xs").unwrap(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn json_extract_uuid_opt_symbol_resolves() {
        let id = Uuid::new_v4();
        let mut ctx = VerbExecutionContext::default();
        ctx.symbols.insert("entity1".to_string(), id);
        let args = serde_json::json!({"entity": "@entity1"});
        assert_eq!(json_extract_uuid_opt(&args, &ctx, "entity").unwrap(), id);
    }
}
