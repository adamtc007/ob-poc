//! Server-rendered UI for DSL Agent
//!
//! This module provides HTML pages rendered directly from Rust,
//! using the DSL parser and executor infrastructure.

pub mod pages;
pub mod routes;

pub use routes::create_ui_router;
