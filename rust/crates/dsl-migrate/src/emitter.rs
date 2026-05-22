//! DSL source text generation from a parsed BpmnProcess.

use crate::mapper::map_process;
use crate::reporter::CoverageReport;
use crate::xml_reader::BpmnProcess;

pub struct MigrationResult {
    /// The generated bpmn-lite DSL source text.
    pub dsl_source: String,
    /// Coverage report for the migration.
    pub coverage: CoverageReport,
    /// Human-readable process name.
    pub process_name: String,
}

pub fn emit(process: &BpmnProcess) -> MigrationResult {
    let mapped = map_process(process);
    let coverage = CoverageReport::from_elements(mapped.element_statuses);
    let dsl_source = mapped.atom_lines.join("\n");

    MigrationResult {
        dsl_source,
        coverage,
        process_name: process.name.clone().unwrap_or_else(|| process.id.clone()),
    }
}
