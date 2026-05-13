//! Traceability boundary types — pure DTOs for utterance trace persistence.
//!
//! Repl-coupled phase builders, replay comparators, and the Postgres
//! repository continue to live in `ob_poc::traceability` (under
//! `feature = "database"`). The types here are the durable contract
//! they all key off.

pub mod types;
