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
pub fn json_extract_cbu_id(args: &serde_json::Value, ctx: &VerbExecutionContext) -> Result<Uuid> {
    json_extract_uuid_opt(args, ctx, "cbu-id")
        .or_else(|| json_extract_uuid_opt(args, ctx, "cbu"))
        .ok_or_else(|| anyhow!("Missing cbu or cbu-id argument"))
}

// ---------------------------------------------------------------------------
// Phase C.3 helpers (F7 follow-on, 2026-04-22)
// ---------------------------------------------------------------------------

/// Emit a `PendingStateAdvance`-shaped JSON payload via the
/// `ctx.extensions["_pending_state_advance"]` side channel. Read by the
/// dispatcher post-execute and shadow-logged; Phase C.2-main applies the
/// advance via SemOS inside the Sequencer's outer scope.
///
/// Use from plugin ops that mutate state, AFTER the DB write has committed
/// (or is guaranteed by the ambient txn). Only emit on a genuine state
/// transition — idempotent returns that found an existing row should NOT
/// emit, because no state advance occurred.
///
/// # Shape contract (important — deliberately DIVERGES from `PendingStateAdvance`)
///
/// The JSON this helper writes is **pre-resolution**. Specifically:
/// - `to_node` is a taxonomy **string token** (e.g. `"cbu:onboarded"`)
///   while [`ob_poc_types::PendingStateAdvance`] types `to_node` as
///   [`ob_poc_types::DagNodeId(Uuid)`]. Direct `serde_json::from_value`
///   into `PendingStateAdvance` will therefore fail at the first state
///   transition.
/// - `slot_path` is a logical path string with no structural typing yet.
/// - `constellation_marks[].entity_id` is a UUID string, fine as-is.
///
/// Phase C.2-main is responsible for:
/// 1. Resolving each `to_node` token against the state-machine catalogue
///    to produce a `DagNodeId`.
/// 2. Constructing a real `PendingStateAdvance` from the resolved parts.
/// 3. Applying it via SemOS inside the outer transaction.
///
/// Until Phase C.2-main lands, the payload is observed-and-logged only
/// (see `dispatch_plugin_via_sem_os_op_in_scope` for the peek site). The
/// shape-contract test `emit_shape_pins_pre_resolution_payload` pins the
/// current JSON so drift is caught immediately.
///
/// # Arguments
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

/// Phase C.2 accessor (2026-04-22): typed read of the
/// `_pending_state_advance` side channel after a verb executes. Returns
/// the raw JSON the emitter placed — callers typically deserialise to
/// `ob_poc_types::PendingStateAdvance`. When the verb did not emit
/// (idempotent, validation failure, or simply doesn't participate in
/// state-advance yet), returns `None`.
///
/// This is the read half of the contract that `emit_pending_state_advance`
/// writes. Phase B.2b's Sequencer reads this after each step in the
/// dispatch loop, aggregates across steps, then applies the union via
/// SemOS inside the outer transaction before stage-9a commit.
///
/// The accessor also **removes** the key from `ctx.extensions` so
/// subsequent verbs in the same plan don't see stale advance data from
/// an earlier step. This matches the single-write, single-read
/// contract the C.1/C.3 emitters assume.
pub fn take_pending_state_advance(ctx: &mut VerbExecutionContext) -> Option<serde_json::Value> {
    let obj = ctx.extensions.as_object_mut()?;
    obj.remove("_pending_state_advance")
}

/// Non-destructive peek — for logging / observability paths that want
/// to see the emitted advance without consuming it. Phase C.2 apply
/// path uses `take_pending_state_advance` so the advance is applied
/// exactly once; shadow logging uses `peek_pending_state_advance` so
/// dispatch-level observability + stage-9a apply can coexist.
pub fn peek_pending_state_advance(ctx: &VerbExecutionContext) -> Option<&serde_json::Value> {
    ctx.extensions.as_object()?.get("_pending_state_advance")
}

/// A single declarative state transition — the input shape for
/// [`emit_pending_state_advance_batch`]. Collected by verbs that mutate
/// multiple entities in one call (e.g. `cbu.create-from-client-group`
/// creates N CBUs; `capital.split` touches the share class + every
/// holder; remediation sweeps mark K entities at once).
#[derive(Debug, Clone)]
pub struct StateTransitionInput<'a> {
    pub entity_id: Uuid,
    pub to_node: &'a str,
    pub slot_path: &'a str,
    pub reason: &'a str,
}

/// Emit a single `PendingStateAdvance` carrying N state transitions +
/// N constellation marks, one per input. `writes_since_push_delta` is
/// the batch size (each entity is one logical write from the session's
/// perspective).
///
/// Use this when a single verb causes multiple entity-level state
/// advances that must apply atomically. Single-transition emitters
/// should continue to use [`emit_pending_state_advance`] — this
/// variant is strictly for the fan-out case.
///
/// Like the single-transition emitter, last-writer-wins within a plan
/// step: a second call in the same step OVERWRITES the previous
/// advance (enforced by the shared `_pending_state_advance` key).
pub fn emit_pending_state_advance_batch(
    ctx: &mut VerbExecutionContext,
    transitions: &[StateTransitionInput<'_>],
) {
    if transitions.is_empty() {
        return;
    }

    if !ctx.extensions.is_object() {
        ctx.extensions = serde_json::Value::Object(serde_json::Map::new());
    }

    let state_transitions: Vec<serde_json::Value> = transitions
        .iter()
        .map(|t| {
            serde_json::json!({
                "entity_id": t.entity_id.to_string(),
                "from_node": null,
                "to_node": t.to_node,
                "reason": t.reason,
            })
        })
        .collect();

    let constellation_marks: Vec<serde_json::Value> = transitions
        .iter()
        .map(|t| {
            serde_json::json!({
                "slot_path": t.slot_path,
                "entity_id": t.entity_id.to_string(),
            })
        })
        .collect();

    if let Some(ext_obj) = ctx.extensions.as_object_mut() {
        ext_obj.insert(
            "_pending_state_advance".to_string(),
            serde_json::json!({
                "state_transitions": state_transitions,
                "constellation_marks": constellation_marks,
                "writes_since_push_delta": transitions.len() as u64,
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

    #[test]
    fn emit_and_peek_pending_state_advance_roundtrip() {
        let mut ctx = VerbExecutionContext::default();
        let entity_id = Uuid::new_v4();

        // Nothing emitted yet.
        assert!(peek_pending_state_advance(&ctx).is_none());

        // Emit once.
        emit_pending_state_advance(
            &mut ctx,
            entity_id,
            "cbu:onboarded",
            "cbu/trading-profile",
            "test",
        );

        // Peek must return the shape the emitter wrote.
        let peeked = peek_pending_state_advance(&ctx).expect("must be present");
        assert_eq!(peeked["state_transitions"][0]["to_node"], "cbu:onboarded");
        assert_eq!(
            peeked["constellation_marks"][0]["slot_path"],
            "cbu/trading-profile"
        );
        assert_eq!(peeked["writes_since_push_delta"], 1);

        // Peek does NOT consume.
        assert!(peek_pending_state_advance(&ctx).is_some());
    }

    #[test]
    fn take_pending_state_advance_consumes_once() {
        let mut ctx = VerbExecutionContext::default();
        let entity_id = Uuid::new_v4();

        emit_pending_state_advance(
            &mut ctx,
            entity_id,
            "entity:ghost",
            "entity/identity",
            "test-consume",
        );

        let taken = take_pending_state_advance(&mut ctx).expect("must be present");
        assert_eq!(taken["state_transitions"][0]["to_node"], "entity:ghost");

        // Second take yields None — the key was removed.
        assert!(take_pending_state_advance(&mut ctx).is_none());
        assert!(peek_pending_state_advance(&ctx).is_none());
    }

    #[test]
    fn take_pending_state_advance_returns_none_when_never_emitted() {
        let mut ctx = VerbExecutionContext::default();
        assert!(take_pending_state_advance(&mut ctx).is_none());
    }

    #[test]
    fn emit_pending_state_advance_batch_fan_out() {
        let mut ctx = VerbExecutionContext::default();
        let e1 = Uuid::new_v4();
        let e2 = Uuid::new_v4();
        let e3 = Uuid::new_v4();

        emit_pending_state_advance_batch(
            &mut ctx,
            &[
                StateTransitionInput {
                    entity_id: e1,
                    to_node: "cbu:onboarded",
                    slot_path: "cbu/trading-profile",
                    reason: "batch-1",
                },
                StateTransitionInput {
                    entity_id: e2,
                    to_node: "cbu:onboarded",
                    slot_path: "cbu/trading-profile",
                    reason: "batch-2",
                },
                StateTransitionInput {
                    entity_id: e3,
                    to_node: "cbu:onboarded",
                    slot_path: "cbu/trading-profile",
                    reason: "batch-3",
                },
            ],
        );

        let peeked = peek_pending_state_advance(&ctx).expect("must be present");
        assert_eq!(peeked["state_transitions"].as_array().unwrap().len(), 3);
        assert_eq!(peeked["constellation_marks"].as_array().unwrap().len(), 3);
        assert_eq!(peeked["writes_since_push_delta"], 3);
        assert_eq!(peeked["state_transitions"][0]["entity_id"], e1.to_string());
        assert_eq!(peeked["state_transitions"][2]["entity_id"], e3.to_string());
    }

    #[test]
    fn emit_pending_state_advance_batch_empty_is_noop() {
        let mut ctx = VerbExecutionContext::default();
        emit_pending_state_advance_batch(&mut ctx, &[]);
        assert!(peek_pending_state_advance(&ctx).is_none());
    }

    #[test]
    fn emit_shape_pins_pre_resolution_payload() {
        // Shape-contract test: pins the exact JSON the emitter writes so
        // any future drift (e.g. accidentally switching `to_node` to an
        // already-resolved UUID, or renaming `slot_path`) is caught
        // before it lands unseen in 70+ plugin verb call sites.
        //
        // This test DELIBERATELY does NOT deserialize into
        // `ob_poc_types::PendingStateAdvance` — see the doc-comment on
        // `emit_pending_state_advance` for the pre-resolution rationale.
        let mut ctx = VerbExecutionContext::default();
        let entity_id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();

        emit_pending_state_advance(
            &mut ctx,
            entity_id,
            "cbu:onboarded",
            "cbu/trading-profile",
            "shape-contract-test",
        );

        let peeked = peek_pending_state_advance(&ctx).expect("must be present");

        // state_transitions: exactly one, with a STRING to_node (not DagNodeId).
        assert_eq!(peeked["state_transitions"].as_array().unwrap().len(), 1);
        let t0 = &peeked["state_transitions"][0];
        assert_eq!(t0["entity_id"], "11111111-2222-3333-4444-555555555555");
        assert!(t0["from_node"].is_null());
        assert_eq!(t0["to_node"], "cbu:onboarded"); // String, not UUID.
        assert_eq!(t0["reason"], "shape-contract-test");

        // constellation_marks: exactly one, slot path is a plain string.
        assert_eq!(peeked["constellation_marks"].as_array().unwrap().len(), 1);
        let m0 = &peeked["constellation_marks"][0];
        assert_eq!(m0["slot_path"], "cbu/trading-profile");
        assert_eq!(m0["entity_id"], "11111111-2222-3333-4444-555555555555");

        assert_eq!(peeked["writes_since_push_delta"], 1);
        assert_eq!(peeked["catalogue_effects"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn emit_shape_is_not_directly_deserializable_into_typed_advance() {
        // Documents the type GAP between the emitter's pre-resolution
        // payload and `ob_poc_types::PendingStateAdvance`. Phase C.2-main
        // is responsible for the token -> DagNodeId resolution step; if
        // someone deletes that step, this test breaks and surfaces the
        // gap again.
        //
        // The typed struct expects `to_node: DagNodeId(Uuid)`. The
        // emitter writes `to_node: <taxonomy_token_string>`. Direct
        // from_value MUST fail.
        use ob_poc_types::PendingStateAdvance;

        let mut ctx = VerbExecutionContext::default();
        emit_pending_state_advance(
            &mut ctx,
            Uuid::new_v4(),
            "cbu:onboarded",
            "cbu/trading-profile",
            "test",
        );

        let raw = peek_pending_state_advance(&ctx).unwrap().clone();
        let result: Result<PendingStateAdvance, _> = serde_json::from_value(raw);
        assert!(
            result.is_err(),
            "emitter payload MUST NOT round-trip into typed PendingStateAdvance \
             directly — Phase C.2-main resolves tokens to UUIDs first. If this \
             test starts passing, the resolver step was removed."
        );
    }

    #[test]
    fn emit_pending_state_advance_batch_overwrites_single() {
        let mut ctx = VerbExecutionContext::default();
        let e1 = Uuid::new_v4();
        let e2 = Uuid::new_v4();

        // Single emit first.
        emit_pending_state_advance(
            &mut ctx,
            e1,
            "cbu:onboarded",
            "cbu/trading-profile",
            "single",
        );
        assert_eq!(
            peek_pending_state_advance(&ctx).unwrap()["writes_since_push_delta"],
            1
        );

        // Batch emit overwrites (last-writer-wins).
        emit_pending_state_advance_batch(
            &mut ctx,
            &[
                StateTransitionInput {
                    entity_id: e1,
                    to_node: "cbu:active",
                    slot_path: "cbu/lifecycle",
                    reason: "batch-overwrite-1",
                },
                StateTransitionInput {
                    entity_id: e2,
                    to_node: "cbu:active",
                    slot_path: "cbu/lifecycle",
                    reason: "batch-overwrite-2",
                },
            ],
        );

        let peeked = peek_pending_state_advance(&ctx).unwrap();
        assert_eq!(peeked["writes_since_push_delta"], 2);
        assert_eq!(peeked["state_transitions"][0]["to_node"], "cbu:active");
    }
}
