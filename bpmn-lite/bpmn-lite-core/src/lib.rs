//! Standalone BPMN-lite runtime and persistence layer.
//!
//! The BPMN-lite database schema is owned entirely by this crate and must be
//! deployable independently of ob-poc. It uses a shared Rust DTO crate
//! (`ob-poc-types`) for `SessionStackState`, but it does not read or write
//! ob-poc database tables and it does not alias ob-poc session records.
//!
//! Session stack behavior at the integration boundary is copy-by-value:
//! ob-poc passes a `SessionStackState` value into BPMN-lite at process start,
//! BPMN-lite persists its own copy on `ProcessInstance` / `JobActivation`, and
//! later BPMN-side mutations must not reach back into ob-poc persistence unless
//! an explicit synchronization path is implemented.

// Pub-discipline (cleanup Phase 0.2):
//   `authoring` and `compiler` have no consumers outside this crate
//   (verified by grep on bpmn-lite-server/ and xtask/ at the time of
//   the audit). Demoted to `pub(crate)` so the unreachable_pub lint
//   can start working on their internals. The remaining modules stay
//   `pub` because the server or its integration tests reach into
//   them; tightening those is a later slice.
// Cleanup Phase 2.x compat re-exports.
//
// Phase 2.1 — `types` + `events` moved to `bpmn-lite-types`.
// Phase 2.2 — `compiler` (ir + parser + lowering + verifier) moved
//             to `bpmn-lite-compiler`.
// Phase 2.3 — `store` + `store_memory` moved to `bpmn-lite-store`.
// Phase 2.4 — `store_postgres` + `migrations/` moved to
//             `bpmn-lite-store-postgres`.
// Phase 2.5 — `vm` moved to `bpmn-lite-vm`.
// Phase 2.6 — `authoring/*` (11 sub-modules) moved to
//             `bpmn-lite-authoring`.
// The remaining in-crate consumer (engine + its tests) reaches the
// moved modules through `crate::*` — these re-exports preserve
// those paths until `bpmn-lite-core` itself goes away at the end of
// Phase 2. `store_postgres` is feature-gated because the
// downstream `bpmn-lite-core` feature `postgres` was the
// activation switch.
pub use bpmn_lite_authoring as authoring;
pub use bpmn_lite_compiler as compiler;
pub use bpmn_lite_engine as engine;
pub use bpmn_lite_store::{store, store_memory};
#[cfg(feature = "postgres")]
pub use bpmn_lite_store_postgres as store_postgres;
pub use bpmn_lite_types::{events, types};
pub use bpmn_lite_vm as vm;

#[cfg(test)]
mod tests {
    const MASTER_SCHEMA: &str = include_str!("../schema.sql");

    #[test]
    fn test_master_schema_is_standalone_from_ob_poc_namespace() {
        assert!(
            !MASTER_SCHEMA.contains("\"ob-poc\"."),
            "bpmn-lite schema must not reference ob-poc DB objects"
        );
    }
}
