//! SEC EDGAR Source Loader
//!
//! Provides access to SEC EDGAR for US company filings, particularly
//! 13D/13G beneficial ownership disclosures.
//!
//! # Coverage
//!
//! - **Jurisdictions:** US
//! - **Key type:** CIK (Central Index Key, up to 10 digits)
//! - **Provides:** Entity, ControlHolders (from 13D/13G), Filings
//! - **Does NOT provide:** Officers (would need DEF 14A parsing), ParentChain
//!
//! # API Reference
//!
//! - Base URL: `https://data.sec.gov`
//! - Auth: None (but User-Agent header REQUIRED)
//! - Rate Limit: 10 requests per second
//!
//! # Note
//!
//! SEC EDGAR 13D/13G filings are semi-structured XML/SGML documents.
//! Full parsing is complex; this implementation provides basic extraction.

mod client;
mod loader;
mod types;

pub use loader::SecEdgarLoader;
