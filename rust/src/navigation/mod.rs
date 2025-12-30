//! Navigation Module for EntityGraph
//!
//! This module provides natural language navigation commands for the unified
//! EntityGraph. It includes:
//!
//! - `commands`: NavCommand enum with all supported commands
//! - `parser`: Nom-based parser for natural language input
//! - `executor`: Execute commands against an EntityGraph
//!
//! ## Usage Example
//!
//! ```ignore
//! use ob_poc::navigation::{parse_nav_command, NavCommand};
//! use ob_poc::graph::EntityGraph;
//!
//! let input = "show me the Allianz book";
//! match parse_nav_command(input) {
//!     Ok((_, cmd)) => {
//!         let result = graph.execute(cmd);
//!         // Handle result
//!     }
//!     Err(e) => eprintln!("Parse error: {:?}", e),
//! }
//! ```
//!
//! ## Supported Command Patterns
//!
//! ### Scope Commands
//! - `load cbu "Fund Name"` / `show cbu "Fund Name"`
//! - `show [the] Allianz book` / `load book "Client Name"`
//! - `show jurisdiction LU` / `focus on Luxembourg`
//!
//! ### Filter Commands
//! - `filter jurisdiction LU, IE` / `focus on Lux`
//! - `show ownership [prong]` / `show control [prong]`
//! - `clear filters`
//! - `as of 2024-01-01`
//!
//! ### Navigation Commands
//! - `go to "Entity Name"` / `navigate to "Entity Name"`
//! - `go up` / `up` / `parent` / `owner`
//! - `go down` / `down` / `child` / `down to "Name"` / `down 0`
//! - `go back` / `back` / `<`
//! - `go forward` / `forward` / `>`
//! - `go to terminus` / `terminus` / `top`
//!
//! ### Query Commands
//! - `find "Name Pattern"`
//! - `where is "Person" [a director]`
//! - `find by role director`
//! - `list children` / `list owners` / `list controllers`
//!
//! ### Display Commands
//! - `show path` / `path to UBO`
//! - `show context` / `context`
//! - `show tree 3` / `tree depth 3`
//! - `expand cbu` / `collapse cbu`
//! - `zoom in` / `zoom out` / `zoom 1.5` / `fit`

pub mod commands;
pub mod executor;
pub mod parser;

pub use commands::{CommandCategory, Direction, NavCommand, ZoomLevel};
pub use executor::{NavExecutor, NavResult, QueryResultItem};
pub use parser::parse_nav_command;
