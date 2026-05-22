//! Migration coverage report types.

#[derive(Debug, Clone, serde::Serialize)]
pub struct MigrationElement {
    pub element_id: String,
    pub element_name: Option<String>,
    pub element_type: String,
    pub status: MigrationStatus,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum MigrationStatus {
    /// Cleanly translated to bpmn-lite DSL.
    Clean,
    /// Needs human resolution (FEEL expression, unknown verb, etc.).
    HumanResolve,
    /// Explicitly rejected (complex gateway, etc.).
    Rejected,
    /// Skipped (handled as part of parent element or not user-visible).
    Skipped,
}

impl MigrationElement {
    pub fn clean(id: &str, name: Option<&str>, elem_type: &str) -> Self {
        Self {
            element_id: id.to_string(),
            element_name: name.map(String::from),
            element_type: elem_type.to_string(),
            status: MigrationStatus::Clean,
            note: None,
        }
    }

    pub fn human_resolve(id: &str, name: Option<&str>, elem_type: &str, reason: &str) -> Self {
        Self {
            element_id: id.to_string(),
            element_name: name.map(String::from),
            element_type: elem_type.to_string(),
            status: MigrationStatus::HumanResolve,
            note: Some(reason.to_string()),
        }
    }

    pub fn rejected(id: &str, name: Option<&str>, elem_type: &str, reason: &str) -> Self {
        Self {
            element_id: id.to_string(),
            element_name: name.map(String::from),
            element_type: elem_type.to_string(),
            status: MigrationStatus::Rejected,
            note: Some(reason.to_string()),
        }
    }

    pub fn skip(elem_type: &str) -> Self {
        Self {
            element_id: String::new(),
            element_name: None,
            element_type: elem_type.to_string(),
            status: MigrationStatus::Skipped,
            note: None,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct CoverageReport {
    pub total: usize,
    pub clean: usize,
    pub human_resolve: usize,
    pub rejected: usize,
    pub skipped: usize,
    pub elements: Vec<MigrationElement>,
}

impl CoverageReport {
    pub fn from_elements(elements: Vec<MigrationElement>) -> Self {
        let total = elements.len();
        let clean = elements
            .iter()
            .filter(|e| e.status == MigrationStatus::Clean)
            .count();
        let human_resolve = elements
            .iter()
            .filter(|e| e.status == MigrationStatus::HumanResolve)
            .count();
        let rejected = elements
            .iter()
            .filter(|e| e.status == MigrationStatus::Rejected)
            .count();
        let skipped = elements
            .iter()
            .filter(|e| e.status == MigrationStatus::Skipped)
            .count();
        Self {
            total,
            clean,
            human_resolve,
            rejected,
            skipped,
            elements,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "Migration: {}/{} clean ({} human-resolve, {} rejected, {} skipped)",
            self.clean, self.total, self.human_resolve, self.rejected, self.skipped
        )
    }
}
