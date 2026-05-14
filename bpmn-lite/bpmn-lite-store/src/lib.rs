//! BPMN-Lite persistence boundary.
//!
//! Owns the `ProcessStore` async trait that every storage backend
//! implements, plus the `MemoryProcessStore` in-process default. The
//! PostgreSQL backend is a separate crate (`bpmn-lite-store-postgres`)
//! so binaries that don't need Postgres don't link sqlx.
//!
//! Empty at Phase 1 skeleton — the trait and memory impl live in
//! `bpmn-lite-core/src/{store.rs, store_memory.rs}` until the Phase 2
//! migration slice (`store + store_memory → bpmn-lite-store`).
