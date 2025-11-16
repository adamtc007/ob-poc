//! Taxonomy module for Product-Service-Resource management
//!
//! This module provides DSL operations and management for the complete
//! product-service-resource taxonomy system with enhanced features:
//! - Transaction management
//! - Comprehensive validation
//! - Error recovery and retry logic
//! - Caching layer
//! - Audit logging
//! - Enhanced resource allocation strategies

pub mod allocator;
pub mod audit;
pub mod cache;
pub mod manager;
pub mod operations;
pub mod recovery;
pub mod transaction;
pub mod validation;

pub use allocator::{AllocationStrategy, ResourceAllocator};
pub use audit::{AuditEntry, AuditLogger, AuditRecord};
pub use cache::{CacheStats, ServiceDiscoveryCache};
pub use manager::TaxonomyDslManager;
pub use operations::{DslOperation, DslResult};
pub use recovery::{CompensationHandler, RecoveryStrategy};
pub use transaction::TaxonomyTransaction;
pub use validation::{OptionValidator, ValidationResult};
