//! Document Bundles Module
//!
//! Provides versioned document requirement bundles for structure macros.
//! Bundles define sets of required documents for fund structures (UCITS, AIF, etc.)
//! and support inheritance for specialization (hedge extends AIF).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                    DOCUMENT BUNDLES SYSTEM                               │
//! │                                                                          │
//! │  YAML Definitions          Registry              Database                │
//! │  ┌─────────────┐          ┌─────────────┐       ┌─────────────┐        │
//! │  │ ucits.yaml  │──────────│  DocBundle  │───────│  document_  │        │
//! │  │ aif.yaml    │  load()  │  Registry   │apply()│ requirements│        │
//! │  │ hedge.yaml  │          │             │       │             │        │
//! │  └─────────────┘          └─────────────┘       └─────────────┘        │
//! │                                  │                                      │
//! │                                  ▼                                      │
//! │                           Inheritance Resolution                        │
//! │                           hedge extends aif                             │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Concepts
//!
//! - **DocsBundleDef**: Definition of a document bundle (from YAML)
//! - **BundleDocumentDef**: Individual document requirement within a bundle
//! - **DocsBundleRegistry**: In-memory registry loaded from YAML
//! - **DocsBundleService**: Database operations for applying bundles
//!
//! ## Example Usage
//!
//! ```ignore
//! // Load registry from YAML
//! let registry = DocsBundleRegistry::load_from_dir("config/document_bundles")?;
//!
//! // Get bundle with inheritance resolved
//! let bundle = registry.get_resolved("docs.bundle.hedge-baseline")?;
//!
//! // Apply bundle to CBU (creates document_requirements)
//! let service = DocsBundleService::new(pool);
//! let requirements = service.apply_bundle(cbu_id, "docs.bundle.hedge-baseline", context).await?;
//! ```

pub mod registry;
pub mod service;
pub mod types;

pub use registry::DocsBundleRegistry;
pub use service::DocsBundleService;
pub use types::{
    AppliedBundle, BundleContext, BundleDocumentDef, DocsBundleDef, ResolvedBundleDocument,
};
