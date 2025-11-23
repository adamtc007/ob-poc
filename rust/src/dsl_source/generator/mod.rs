//! DSL Generation - Programmatic builders and templates

pub mod builder;
pub mod templates;
pub mod domains;

pub use builder::{DslBuilder, CbuCreateBuilder, ProductCreateBuilder, ServiceCreateBuilder, ResourceCreateBuilder};
pub use templates::DslTemplate;
pub use domains::{CbuDslGenerator, ProductDslGenerator, ServiceDslGenerator, LifecycleResourceDslGenerator};
