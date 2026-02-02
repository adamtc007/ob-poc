//! MCP (Model Context Protocol) Server Module
//!
//! Exposes the DSL pipeline as an MCP server for Claude integration.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        Claude Agent                          │
//! └─────────────────────────────────────────────────────────────┘
//!                               │
//!                               │ MCP Protocol (JSON-RPC over stdio)
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      MCP Server (Rust)                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Tools:                                                      │
//! │  ├── dsl_validate    - Parse + compile                      │
//! │  ├── dsl_execute     - Full execution to DB                 │
//! │  ├── dsl_plan        - Show execution plan                  │
//! │  ├── cbu_get         - Get CBU with all related data        │
//! │  ├── cbu_list        - List/search CBUs                     │
//! │  ├── entity_get      - Get entity details                   │
//! │  ├── verbs_list      - List available DSL verbs             │
//! │  └── schema_info     - Get entity types, roles, doc types   │
//! └─────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     DSL Pipeline + PostgreSQL                │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```bash
//! # Build
//! cargo build --features mcp --bin dsl_mcp
//!
//! # Run
//! DATABASE_URL=postgresql://localhost/ob-poc ./target/debug/dsl_mcp
//! ```

pub mod enrichment;
pub mod handlers;
pub mod intent_pipeline;
pub mod macro_integration;
pub mod protocol;
pub mod resolution;
pub mod schema;
pub mod scope_resolution;
pub mod server;
pub mod session;
pub mod tools;
pub mod types;
pub mod utterance;
pub mod verb_search;

pub use enrichment::{EntityContext, EntityEnricher, EntityType, OwnershipContext, RoleContext};
pub use macro_integration::{
    check_macro_prereqs, get_all_verb_readiness, get_macro_schema, get_ready_verbs,
    get_verb_readiness, init_macro_registry, is_macro, is_verb_ready, list_macros, macro_registry,
    macros_by_mode, try_expand_macro, update_dag_after_execution, MacroAttemptResult, MacroInfo,
    VerbReadiness, VerbReadinessInfo,
};
pub use resolution::{
    ConversationContext, EnrichedMatch, ResolutionConfidence, ResolutionResult, ResolutionStrategy,
    SuggestedAction,
};
pub use scope_resolution::{
    EntityMatch, ScopeCandidate, ScopeContext, ScopeResolutionOutcome, ScopeResolver,
};
pub use server::McpServer;
pub use types::*;
