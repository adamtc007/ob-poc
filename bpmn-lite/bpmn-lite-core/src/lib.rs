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

pub mod authoring;
pub mod compiler;
pub mod engine;
pub mod events;
pub mod store;
pub mod store_memory;
#[cfg(feature = "postgres")]
pub mod store_postgres;
pub mod types;
pub mod vm;

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
