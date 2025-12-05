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

pub mod config;
pub mod index;
pub mod proto;
pub mod refresh;
pub mod server;

// Re-export main types
pub use config::{EntityConfig, GatewayConfig, RefreshConfig, StartupMode};
pub use index::{
    IndexError, IndexRecord, IndexRegistry, MatchMode, SearchIndex, SearchMatch, SearchQuery,
    TantivyIndex,
};
pub use refresh::{run_refresh_loop, RefreshPipeline};
pub use server::EntityGatewayService;
