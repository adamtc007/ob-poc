//! sem_os_server — standalone REST server for Semantic OS.
//!
//! Provides JWT-authenticated REST endpoints backed by `CoreService`.
//! Routes:
//!   GET  /health                         — health check (no auth)
//!   POST /resolve_context                — context resolution (auth required)
//!   GET  /snapshot_sets/:id/manifest     — get manifest (auth required)
//!   POST /publish                        — admin publish (auth required)
//!   GET  /exports/snapshot_set/:id       — export snapshot set (auth required)
//!   POST /bootstrap/seed_bundle          — admin bootstrap (auth required)
//!   POST /tools/call                     — invoke an MCP tool (auth required)
//!   GET  /tools/list                     — list available MCP tools (auth required)

pub mod dispatcher;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod router;
