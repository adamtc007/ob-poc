//! UK Companies House Source Loader
//!
//! Provides access to the UK Companies House API for company information,
//! Persons with Significant Control (PSC), and officers.
//!
//! # Coverage
//!
//! - **Jurisdictions:** GB, UK
//! - **Key type:** Company Number (8 characters, e.g., "12345678" or "SC123456")
//! - **Provides:** Entity, ControlHolders (PSC), Officers
//! - **Does NOT provide:** ParentChain (use PSC corporate holders instead)
//!
//! # API Reference
//!
//! - Base URL: `https://api.company-information.service.gov.uk`
//! - Auth: HTTP Basic (API key as username, no password)
//! - Rate Limit: 600 requests per 5 minutes
//!
//! # Environment
//!
//! Requires `COMPANIES_HOUSE_API_KEY` environment variable.

mod client;
mod loader;
mod normalize;
mod types;

pub use loader::CompaniesHouseLoader;
