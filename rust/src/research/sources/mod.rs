//! Source-loader modules — slim remnant after T11.1b (2026-07-12).
//!
//! `companies_house`/`sec_edgar`/`traits`/`normalized`/`registry` moved to
//! `ob-poc-agent::research::sources` (pure data/logic, no capability
//! coupling) and are re-exported here so every existing
//! `crate::research::sources::*` caller continues to resolve unchanged.
//!
//! `gleif` stays local — see the parent module's doc for why.

pub(crate) use ob_poc_agent::research::sources::{
    normalized, registry, traits, CompaniesHouseLoader, SecEdgarLoader,
};

pub(crate) mod gleif;
pub(crate) use gleif::GleifLoader;
