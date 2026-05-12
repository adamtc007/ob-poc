//! Entity Gateway - Read-only Entity Resolution Service
//!
//! The EntityGateway is a high-performance, read-only search service that
//! resolves entity references by nickname and input value(s). It buffers
//! the primary database, providing sub-50ms fuzzy search for IDE autocomplete
//! and efficient exact-match resolution for validation and runtime.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  Consumers: LSP, Parser, Linter, Runtime, Batch Agent           │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    gRPC Service                                  │
//! │                  (EntityGateway)                                 │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   Index Registry                                 │
//! │          nickname -> SearchIndex (Tantivy)                      │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                  Refresh Pipeline                                │
//! │               (Postgres -> Indexes)                              │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use entity_gateway::{GatewayConfig, IndexRegistry, TantivyIndex, RefreshPipeline};
//!
//! // Load configuration
//! let config = GatewayConfig::from_file("config/entity_index.yaml")?;
//!
//! // Create registry and indexes
//! let registry = Arc::new(IndexRegistry::new(config.entities.clone()));
//! for (nickname, entity_config) in &config.entities {
//!     let index = TantivyIndex::new(entity_config.clone())?;
//!     registry.register(nickname.clone(), Arc::new(index)).await;
//! }
//!
//! // Initialize refresh pipeline
//! let pipeline = RefreshPipeline::new(config.clone()).await?;
//! pipeline.refresh_all(&registry).await?;
//!
//! // Start gRPC server
//! let service = EntityGatewayService::new(registry);
//! ```

mod config;
mod index;
pub mod proto;
mod refresh;
mod search_engine;
mod search_expr;
mod server;

// Re-export the external API surface only. Workspace-grep (Phase 2 audit,
// 2026-05-12) confirmed:
//   - GatewayConfig, EntityConfig, RefreshConfig, StartupMode — consumed
//     by ob-poc-web's main.rs (Clone, field traversal, struct iteration).
//   - IndexRegistry, TantivyIndex, run_refresh_loop, RefreshPipeline,
//     EntityGatewayService — same caller.
//   - The rest (SearchEngine/SearchResult/RankedMatch; SearchExpr +
//     SearchExprQuery/SearchSchema/search_expr::ParseError; IndexError,
//     IndexRecord, MatchMode, SearchIndex, SearchMatch, SearchQuery) had
//     zero external imports. Removed from the public surface so
//     unreachable_pub can flag the now-orphaned items for demotion to
//     pub(crate).
pub use config::{EntityConfig, GatewayConfig, RefreshConfig, StartupMode};
pub use index::{IndexRegistry, TantivyIndex};
pub use refresh::{run_refresh_loop, RefreshPipeline};
pub use server::EntityGatewayService;
