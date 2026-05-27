# BPMN Engine Findings & Deferred Spec Questions
Date: 2026-05-27

During the implementation of parallel state isolation and gateway multiplicity policies, two architectural items were parked for future resolution. They are recorded here so they are not lost.

## 1. Merge Protocol: `union` Semantics
Currently, the `union` operator in `apply_merge_operator` acts as a raw array concatenation (e.g., merging `"us"` and `"us"` from two branches yields `["us", "us"]`). It does not perform set-union deduplication.
**Action Required:** Confirm with the spec owners whether `union` is intended to mean concatenation or strict set-union in a KYC context. If deduplication is required, the operator implementation must be updated.

## 2. Gateway Evaluation Context (C1 Invariant)
In `handle_decision_gateway` and `handle_inclusive_fork`, the engine currently passes a hardcoded empty object to the switch adaptor: `context_data: serde_json::json!({})`.
Because gateway evaluation is completely blind to `instance_data`, it was safe to implement "Variant A" for parallel state isolation (deferring branch writes to `write_log` exclusively, instead of global state).
**Action Required / Invariant Warning:** If a future feature wires real `instance_data` into gateway `context_data`, Variant A will silently break. In-branch gateways will read stale global state instead of seeing prior in-branch writes. If this occurs, the C1 fix MUST be upgraded to "Variant B", which involves overlaying the token's `write_log` on top of global state before passing it to the evaluator.