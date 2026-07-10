//! `dsl-migrate` — Camunda 8 BPMN XML → bpmn-lite DSL migration tool.
//!
//! No runtime deps, no database, no Sage. Pure XML-in, DSL-out.
#![deny(unreachable_pub)]

pub mod emitter;
pub mod feel_parser;
pub mod form_key;
pub mod mapper;
pub mod reporter;
pub mod verb_resolver;
pub mod xml_reader;

pub use emitter::{emit, MigrationResult};
pub use reporter::{CoverageReport, MigrationElement, MigrationStatus};
pub use xml_reader::{parse_bpmn_xml, BpmnProcess};
