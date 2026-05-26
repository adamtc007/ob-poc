//! formKey normalisation for Camunda 8 userTask elements.
//!
//! Camunda 8 `formKey` values come in several prefix-qualified forms.
//! This module maps each to a canonical `:form-ref` string for use in
//! the bpmn-lite DSL, or emits a HUMAN-RESOLVE diagnostic for unknown forms.

#[derive(Debug, Clone, PartialEq)]
pub enum FormKeyNormalised {
    /// Resolved to a canonical form ref — emit `:verb dsl.form :form-ref "<ref>"`.
    Resolved(String),
    /// No formKey present — emit plain `(node id :kind user-task)`.
    Absent,
    /// Prefix not supported — emit HUMAN-RESOLVE comment + plain node.
    NeedsReview { raw: String, reason: String },
}

/// Normalise a raw `formKey` attribute value from a Camunda 8 `bpmn:userTask`.
///
/// Mapping table (design doc §10.4):
///
/// | Prefix                    | Action         | `:form-ref` output     |
/// |---------------------------|----------------|------------------------|
/// | `camunda-forms:embedded:` | Strip prefix   | `embedded/<rest>`      |
/// | `deployment:`             | Strip prefix   | `deployment/<rest>`    |
/// | Plain (no `:`)            | Pass through   | `<key>`                |
/// | `classpath:`, `bpmn:`…    | NeedsReview    | `[HUMAN-RESOLVE]`      |
/// | Absent / empty            | Absent         | (no dsl.form verb)     |
pub fn normalise_form_key(raw: Option<&str>) -> FormKeyNormalised {
    let raw = match raw {
        None | Some("") => return FormKeyNormalised::Absent,
        Some(s) => s.trim(),
    };

    if let Some(rest) = raw.strip_prefix("camunda-forms:embedded:") {
        return FormKeyNormalised::Resolved(format!("embedded/{}", rest));
    }
    if let Some(rest) = raw.strip_prefix("deployment:") {
        return FormKeyNormalised::Resolved(format!("deployment/{}", rest));
    }
    // Plain key — no colon at all, or colon only inside the key value
    if !raw.contains(':') {
        return FormKeyNormalised::Resolved(raw.to_owned());
    }
    // Any other prefix is not supported in v1
    FormKeyNormalised::NeedsReview {
        raw: raw.to_owned(),
        reason: format!(
            "formKey prefix not supported (use camunda-forms:embedded:, deployment:, or plain key): {}",
            raw
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_form_key() {
        assert_eq!(normalise_form_key(None), FormKeyNormalised::Absent);
        assert_eq!(normalise_form_key(Some("")), FormKeyNormalised::Absent);
    }

    #[test]
    fn embedded_prefix_stripped() {
        assert_eq!(
            normalise_form_key(Some("camunda-forms:embedded:kyc-review")),
            FormKeyNormalised::Resolved("embedded/kyc-review".into())
        );
    }

    #[test]
    fn deployment_prefix_stripped() {
        assert_eq!(
            normalise_form_key(Some("deployment:doc-checklist.json")),
            FormKeyNormalised::Resolved("deployment/doc-checklist.json".into())
        );
    }

    #[test]
    fn plain_key_passthrough() {
        assert_eq!(
            normalise_form_key(Some("kyc.review-summary")),
            FormKeyNormalised::Resolved("kyc.review-summary".into())
        );
    }

    #[test]
    fn classpath_prefix_needs_review() {
        match normalise_form_key(Some("classpath:forms/onboarding.json")) {
            FormKeyNormalised::NeedsReview { raw, .. } => {
                assert!(raw.contains("classpath:"));
            }
            other => panic!("expected NeedsReview, got {:?}", other),
        }
    }

    #[test]
    fn bpmn_prefix_needs_review() {
        assert!(matches!(
            normalise_form_key(Some("bpmn:somePath")),
            FormKeyNormalised::NeedsReview { .. }
        ));
    }
}
