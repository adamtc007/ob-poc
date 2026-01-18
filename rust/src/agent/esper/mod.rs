//! ESPER Navigation Command Registry
//!
//! Blade Runner-inspired voice/chat navigation commands with:
//! - YAML configuration (no code changes to add phrases)
//! - Trie-based O(k) lookup for instant response
//! - Learnable aliases from user corrections
//!
//! # Architecture
//!
//! ```text
//! User Input ("enhance", "zoom in 2x", "make it bigger")
//!     │
//!     ├─ Trie Lookup (O(k)) ──► Instant match
//!     │       │
//!     │       └─ Miss? Check learned aliases
//!     │               │
//!     │               └─ Miss? Fall through to DSL pipeline
//!     │
//!     ▼
//! AgentCommand::ZoomIn { factor: Some(2.0) }
//! ```

mod config;
mod registry;
mod warmup;

pub use config::{AgentCommandSpec, AliasSpec, EsperCommandDef, EsperConfig, ParamSource};
pub use registry::{
    EsperCommandRegistry, EsperMatch, LookupResult, MatchSource, MatchType, RegistryStats,
    SemanticIndex, SemanticMatch,
};
pub use warmup::{EsperWarmup, EsperWarmupStats};

#[cfg(test)]
mod tests;
