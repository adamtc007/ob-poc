//! Layer 1 extraction parser and validator for canonical NLCI structured intent.

use anyhow::{anyhow, Context, Result};

use super::StructuredIntentPlan;

/// Parse raw extractor output into the canonical NLCI structured intent plan.
///
/// # Examples
/// ```ignore
/// use ob_poc::semtaxonomy_v2::parse_structured_intent_plan;
///
/// let raw = r#"{
///   "steps": [{
///     "action": "read",
///     "entity": "cbu",
///     "target": null,
///     "qualifiers": [],
///     "parameters": [],
///     "confidence": "high"
///   }],
///   "composition": "single_step",
///   "data_flow": []
/// }"#;
/// let plan = parse_structured_intent_plan(raw)?;
/// assert_eq!(plan.steps.len(), 1);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn parse_structured_intent_plan(raw: &str) -> Result<StructuredIntentPlan> {
    let json = extract_json_payload(raw);
    let plan: StructuredIntentPlan = serde_json::from_str(json)
        .with_context(|| format!("failed to parse canonical structured intent JSON: {json}"))?;
    plan.validate_invariants()
        .map_err(|error| anyhow!("structured intent validation failed: {error}"))?;
    Ok(plan)
}

fn extract_json_payload(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.contains("```json") {
        return trimmed
            .split("```json")
            .nth(1)
            .and_then(|block| block.split("```").next())
            .map(str::trim)
            .unwrap_or(trimmed);
    }
    if trimmed.contains("```") {
        return trimmed
            .split("```")
            .nth(1)
            .and_then(|block| block.split("```").next())
            .map(str::trim)
            .unwrap_or(trimmed);
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_structured_intent_plan_accepts_valid_json() {
        let raw = r#"{
          "steps": [{
            "action": "read",
            "entity": "cbu",
            "target": null,
            "qualifiers": [],
            "parameters": [],
            "confidence": "high"
          }],
          "composition": "single_step",
          "data_flow": []
        }"#;

        let plan = parse_structured_intent_plan(raw).expect("plan should parse");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].entity, "cbu");
    }

    #[test]
    fn parse_structured_intent_plan_rejects_dsl_shaped_action() {
        let raw = r#"{
          "steps": [{
            "action": "(cbu.read)",
            "entity": "cbu",
            "target": null,
            "qualifiers": [],
            "parameters": [],
            "confidence": "high"
          }],
          "composition": "single_step",
          "data_flow": []
        }"#;

        let error = parse_structured_intent_plan(raw).expect_err("plan should fail");
        assert!(error
            .to_string()
            .contains("structured intent validation failed"));
    }
}
