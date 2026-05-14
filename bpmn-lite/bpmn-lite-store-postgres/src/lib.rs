//! BPMN-Lite PostgreSQL persistence.
//!
//! Owns `PostgresProcessStore` (impl of `ProcessStore`) and the
//! migration set that defines its schema. Separated from
//! `bpmn-lite-store` so a memory-only deployment (CI, integration
//! tests, smoke harnesses) doesn't link sqlx + the full Postgres
//! client.
//!
//! Empty at Phase 1 skeleton — the PostgreSQL impl lives in
//! `bpmn-lite-core/src/store_postgres.rs` and migrations under
//! `bpmn-lite-core/migrations/` until the Phase 2 migration slice
//! (`store_postgres + migrations → bpmn-lite-store-postgres`).
