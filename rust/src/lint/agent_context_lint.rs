//! Agent Context Lint Rule
//!
//! AGENT001: Rejects `context: interactive_only` verbs inside template bodies
//! or BPMN task definitions. Interactive-only verbs (agent.start, agent.pause, etc.)
//! are runtime session controls and must not appear in automated pipelines.

use super::diagnostic::{Diagnostic, Severity};

/// Rule code for interactive-only verb in template/BPMN context
const AGENT001: &str = "AGENT001";

/// Known interactive-only verb FQNs (loaded from metadata or hardcoded as fallback)
const INTERACTIVE_ONLY_VERBS: &[&str] = &[
    "agent.start",
    "agent.pause",
    "agent.resume",
    "agent.stop",
    "agent.confirm",
    "agent.reject",
    "agent.select",
    "agent.set-threshold",
    "agent.set-mode",
];

/// Check a template body for interactive-only verb references.
///
/// Scans the DSL template body string for any verb FQN that is classified
/// as `context: interactive_only`. Returns diagnostics for each violation.
pub fn lint_template_body(template_body: &str, template_path: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let interactive_verbs = interactive_only_verbs();

    for verb_fqn in &interactive_verbs {
        // Check if the verb appears in the template body as a verb call
        // Template bodies use s-expression syntax: (verb.name :arg value)
        let pattern = format!("({}", verb_fqn);
        if template_body.contains(&pattern) {
            diagnostics.push(Diagnostic {
                code: AGENT001.to_string(),
                severity: Severity::Error,
                path: template_path.to_string(),
                message: format!(
                    "Interactive-only verb '{}' cannot be used inside a template body. \
                         Interactive verbs are session controls that require user presence.",
                    verb_fqn
                ),
                hint: Some(format!(
                    "Remove '{}' from the template or use a scripted-ok alternative",
                    verb_fqn
                )),
            });
        }
    }

    diagnostics
}

/// Check a BPMN task definition for interactive-only verb references.
///
/// BPMN service tasks specify a `task_type` which maps to a verb FQN.
/// Interactive-only verbs must not be used as BPMN task types.
pub fn lint_bpmn_task_type(task_type: &str, task_path: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let interactive_verbs = interactive_only_verbs();

    if interactive_verbs.iter().any(|v| v == task_type) {
        diagnostics.push(Diagnostic {
            code: AGENT001.to_string(),
            severity: Severity::Error,
            path: task_path.to_string(),
            message: format!(
                "Interactive-only verb '{}' cannot be used as a BPMN task type. \
                     BPMN tasks execute without user interaction.",
                task_type
            ),
            hint: Some("Use a verb with context: scripted_ok instead".to_string()),
        });
    }

    diagnostics
}

/// Returns the set of interactive-only verb FQNs.
fn interactive_only_verbs() -> Vec<String> {
    INTERACTIVE_ONLY_VERBS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_with_interactive_verb_rejected() {
        let body = r#"
            (research.import-run.begin :case-id $case-id)
            (agent.start :task "resolve-gaps")
            (research.import-run.complete :run-id @run)
        "#;
        let diagnostics = lint_template_body(body, "skeleton-build.template.body");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "AGENT001");
        assert!(diagnostics[0].severity == Severity::Error);
        assert!(diagnostics[0].message.contains("agent.start"));
    }

    #[test]
    fn test_template_with_scripted_verb_ok() {
        let body = r#"
            (research.import-run.begin :case-id $case-id)
            (graph.validate :case-id $case-id)
            (ubo.compute-chains :case-id $case-id)
        "#;
        let diagnostics = lint_template_body(body, "skeleton-build.template.body");
        assert!(
            diagnostics.is_empty(),
            "Expected no diagnostics for scripted verbs"
        );
    }

    #[test]
    fn test_bpmn_task_with_interactive_verb_rejected() {
        let diagnostics = lint_bpmn_task_type("agent.start", "process.task[0].task_type");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "AGENT001");
    }

    #[test]
    fn test_bpmn_task_with_resolve_gaps_ok() {
        let diagnostics =
            lint_bpmn_task_type("research.import-run.begin", "process.task[0].task_type");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_multiple_interactive_verbs_in_template() {
        let body = r#"
            (agent.start :task "enrich")
            (agent.pause)
            (agent.resume)
        "#;
        let diagnostics = lint_template_body(body, "bad-template.body");
        assert_eq!(diagnostics.len(), 3);
    }
}
