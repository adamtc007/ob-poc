//! Template Instantiation — Expand pack templates into runbook entries
//!
//! Takes a `PackTemplate`, client context, and user answers, then produces
//! `RunbookEntry` items with full slot provenance tracking.
//!
//! # Provenance Guarantees
//!
//! Every arg on every entry gets a `SlotSource`:
//! - `TemplateDefault` — value came from the template step's static args.
//! - `UserProvided` — value came from user answers.
//! - `InferredFromContext` — value came from client/session context.
//!
//! Template provenance fields (`template_id`, `template_hash`) are set on
//! the parent Runbook by the caller after instantiation.

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::journey::pack::{PackTemplate, TemplateStep};
use crate::repl::runbook::{
    ConfirmPolicy, EntryStatus, ExecutionMode, RunbookEntry, SlotProvenance, SlotSource,
};
use crate::repl::sentence_gen::SentenceGenerator;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Expand a pack template into runbook entries.
///
/// # Arguments
///
/// * `template` — The pack template to instantiate.
/// * `context_vars` — Context values (e.g. `context.client_name`, `context.jurisdiction`).
/// * `answers` — User answers to pack questions.
/// * `sentence_gen` — Sentence generator for producing human-readable descriptions.
/// * `verb_phrases` — Map of verb FQN → invocation phrases (from VerbConfig).
/// * `verb_descriptions` — Map of verb FQN → description (from VerbConfig).
///
/// # Returns
///
/// A tuple of `(entries, template_hash)` where the hash is derived from the
/// template structure for provenance tracking.
pub fn instantiate_template(
    template: &PackTemplate,
    context_vars: &HashMap<String, String>,
    answers: &HashMap<String, serde_json::Value>,
    sentence_gen: &SentenceGenerator,
    verb_phrases: &HashMap<String, Vec<String>>,
    verb_descriptions: &HashMap<String, String>,
) -> Result<(Vec<RunbookEntry>, String), TemplateError> {
    let template_hash = compute_template_hash(template);
    let mut entries = Vec::new();

    for step in &template.steps {
        // Evaluate `when` condition — skip step if condition is false.
        if let Some(ref condition) = step.when {
            if !evaluate_condition(condition, context_vars, answers) {
                continue;
            }
        }

        // Handle `repeat_for` — expand step once per item in the named list.
        if let Some(ref repeat_key) = step.repeat_for {
            let items = resolve_list(repeat_key, context_vars, answers)?;
            for item in &items {
                let entry = build_entry(
                    step,
                    context_vars,
                    answers,
                    Some(item),
                    sentence_gen,
                    verb_phrases,
                    verb_descriptions,
                );
                entries.push(entry);
            }
        } else {
            let entry = build_entry(
                step,
                context_vars,
                answers,
                None,
                sentence_gen,
                verb_phrases,
                verb_descriptions,
            );
            entries.push(entry);
        }
    }

    Ok((entries, template_hash))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a single runbook entry from a template step.
fn build_entry(
    step: &TemplateStep,
    context_vars: &HashMap<String, String>,
    answers: &HashMap<String, serde_json::Value>,
    repeat_item: Option<&str>,
    sentence_gen: &SentenceGenerator,
    verb_phrases: &HashMap<String, Vec<String>>,
    verb_descriptions: &HashMap<String, String>,
) -> RunbookEntry {
    let mut args = HashMap::new();
    let mut provenance = SlotProvenance::default();

    // Resolve each arg in the template step.
    for (key, value) in &step.args {
        let value_str = json_value_to_string(value);
        let (resolved, source) = resolve_arg_value(&value_str, context_vars, answers, repeat_item);
        args.insert(key.clone(), resolved);
        provenance.slots.insert(key.clone(), source);
    }

    // Generate sentence.
    let phrases = verb_phrases
        .get(&step.verb)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let description = verb_descriptions
        .get(&step.verb)
        .map(|s| s.as_str())
        .unwrap_or("");
    let sentence = sentence_gen.generate(&step.verb, &args, phrases, description);

    // Build DSL string.
    let dsl = build_dsl(&step.verb, &args);

    // Determine execution mode.
    let execution_mode = match step.execution_mode.as_deref() {
        Some("durable") => ExecutionMode::Durable,
        Some("human_gate") => ExecutionMode::HumanGate,
        _ => ExecutionMode::Sync,
    };

    RunbookEntry {
        id: Uuid::new_v4(),
        sequence: 0, // Set by Runbook::add_entry()
        sentence,
        labels: HashMap::new(),
        dsl,
        verb: step.verb.clone(),
        args,
        slot_provenance: provenance,
        arg_extraction_audit: None,
        status: EntryStatus::Proposed,
        execution_mode,
        confirm_policy: ConfirmPolicy::PackConfigured,
        unresolved_refs: Vec::new(),
        depends_on: Vec::new(),
        compiled_runbook_id: None,
        result: None,
        invocation: None,
    }
}

/// Resolve an arg value string, substituting `{context.*}`, `{answers.*}`, and `{item}`.
///
/// Returns the resolved value and its provenance source.
fn resolve_arg_value(
    value: &str,
    context_vars: &HashMap<String, String>,
    answers: &HashMap<String, serde_json::Value>,
    repeat_item: Option<&str>,
) -> (String, SlotSource) {
    // {item} — from repeat_for iteration
    if value == "{item}" {
        if let Some(item) = repeat_item {
            return (item.to_string(), SlotSource::UserProvided);
        }
    }

    // {context.*} — from session/client context
    if value.starts_with("{context.") && value.ends_with('}') {
        let key = &value[9..value.len() - 1];
        if let Some(val) = context_vars.get(key) {
            return (val.clone(), SlotSource::InferredFromContext);
        }
    }

    // {answers.*} — from user answers
    if value.starts_with("{answers.") && value.ends_with('}') {
        let key = &value[9..value.len() - 1];
        if let Some(val) = answers.get(key) {
            return (json_value_to_string(val), SlotSource::UserProvided);
        }
    }

    // No substitution — it's a template default.
    (value.to_string(), SlotSource::TemplateDefault)
}

/// Evaluate a simple condition string against context and answers.
///
/// Phase 0 supports:
/// - `"answers.field == value"` — equality check.
/// - `"answers.field == true"` / `"answers.field == false"` — boolean check.
/// - Anything else → true (permissive by default).
fn evaluate_condition(
    condition: &str,
    _context_vars: &HashMap<String, String>,
    answers: &HashMap<String, serde_json::Value>,
) -> bool {
    // Parse "answers.key == value" pattern.
    if let Some(rest) = condition.strip_prefix("answers.") {
        if let Some((key, expected)) = rest.split_once("==") {
            let key = key.trim();
            let expected = expected.trim();
            if let Some(actual) = answers.get(key) {
                return match actual {
                    serde_json::Value::Bool(b) => {
                        (expected == "true" && *b) || (expected == "false" && !*b)
                    }
                    serde_json::Value::String(s) => s == expected,
                    _ => json_value_to_string(actual) == expected,
                };
            }
            return false; // Key not found → condition not met.
        }
    }

    // Bare field name → check if it exists in answers and is non-empty.
    let key = condition.trim();
    if let Some(val) = answers.get(key) {
        return match val {
            serde_json::Value::Null => false,
            serde_json::Value::Bool(b) => *b,
            serde_json::Value::String(s) => !s.is_empty(),
            serde_json::Value::Array(a) => !a.is_empty(),
            _ => true,
        };
    }

    // Not in answers — condition not met.
    false
}

/// Resolve a list from answers or context for `repeat_for`.
fn resolve_list(
    key: &str,
    _context_vars: &HashMap<String, String>,
    answers: &HashMap<String, serde_json::Value>,
) -> Result<Vec<String>, TemplateError> {
    // Try answers.* first.
    let lookup_key = key.strip_prefix("answers.").unwrap_or(key);

    if let Some(value) = answers.get(lookup_key) {
        match value {
            serde_json::Value::Array(arr) => {
                return Ok(arr.iter().map(json_value_to_string).collect());
            }
            serde_json::Value::String(s) => {
                // Comma-separated fallback.
                return Ok(s.split(',').map(|s| s.trim().to_string()).collect());
            }
            _ => {}
        }
    }

    Err(TemplateError::MissingListSlot {
        key: key.to_string(),
    })
}

/// Build a DSL s-expression from verb + args.
fn build_dsl(verb: &str, args: &HashMap<String, String>) -> String {
    let mut dsl = format!("({}", verb);
    let mut sorted_args: Vec<_> = args.iter().collect();
    sorted_args.sort_by_key(|(k, _)| *k);
    for (key, value) in sorted_args {
        dsl.push_str(&format!(" :{} \"{}\"", key, value));
    }
    dsl.push(')');
    dsl
}

/// Convert a JSON value to a string for arg substitution.
fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(json_value_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Compute a deterministic hash of the template structure.
fn compute_template_hash(template: &PackTemplate) -> String {
    let mut hasher = Sha256::new();
    hasher.update(template.template_id.as_bytes());
    hasher.update(template.when_to_use.as_bytes());
    for step in &template.steps {
        hasher.update(step.verb.as_bytes());
        // Sort args for determinism.
        let mut sorted_args: Vec<_> = step.args.iter().collect();
        sorted_args.sort_by_key(|(k, _)| *k);
        for (k, v) in sorted_args {
            hasher.update(k.as_bytes());
            hasher.update(v.to_string().as_bytes());
        }
        if let Some(ref r) = step.repeat_for {
            hasher.update(r.as_bytes());
        }
        if let Some(ref w) = step.when {
            hasher.update(w.as_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from template instantiation.
#[derive(Debug)]
pub enum TemplateError {
    MissingListSlot { key: String },
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingListSlot { key } => {
                write!(
                    f,
                    "Template repeat_for references missing list slot: {}",
                    key
                )
            }
        }
    }
}

impl std::error::Error for TemplateError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey::pack::TemplateStep;

    fn make_template() -> PackTemplate {
        PackTemplate {
            template_id: "basic-onboarding".to_string(),
            when_to_use: "Standard onboarding".to_string(),
            steps: vec![
                TemplateStep {
                    verb: "cbu.create".to_string(),
                    args: HashMap::from([
                        (
                            "name".to_string(),
                            serde_json::json!("{context.client_name}"),
                        ),
                        (
                            "jurisdiction".to_string(),
                            serde_json::json!("{answers.jurisdiction}"),
                        ),
                    ]),
                    repeat_for: None,
                    when: None,
                    execution_mode: None,
                },
                TemplateStep {
                    verb: "cbu.assign-product".to_string(),
                    args: HashMap::from([("product".to_string(), serde_json::json!("{item}"))]),
                    repeat_for: Some("answers.products".to_string()),
                    when: None,
                    execution_mode: None,
                },
                TemplateStep {
                    verb: "isda.create".to_string(),
                    args: HashMap::from([(
                        "counterparty".to_string(),
                        serde_json::json!("{answers.counterparty}"),
                    )]),
                    repeat_for: None,
                    when: Some("answers.has_otc == true".to_string()),
                    execution_mode: Some("human_gate".to_string()),
                },
            ],
        }
    }

    fn context() -> HashMap<String, String> {
        HashMap::from([("client_name".to_string(), "Allianz Lux".to_string())])
    }

    fn answers_with_otc() -> HashMap<String, serde_json::Value> {
        HashMap::from([
            ("jurisdiction".to_string(), serde_json::json!("LU")),
            ("products".to_string(), serde_json::json!(["IRS", "EQUITY"])),
            ("has_otc".to_string(), serde_json::json!(true)),
            (
                "counterparty".to_string(),
                serde_json::json!("Goldman Sachs"),
            ),
        ])
    }

    fn answers_no_otc() -> HashMap<String, serde_json::Value> {
        HashMap::from([
            ("jurisdiction".to_string(), serde_json::json!("LU")),
            ("products".to_string(), serde_json::json!(["IRS", "EQUITY"])),
            ("has_otc".to_string(), serde_json::json!(false)),
        ])
    }

    #[test]
    fn test_basic_instantiation() {
        let template = make_template();
        let (entries, hash) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        // 1 cbu.create + 2 cbu.assign-product (repeat) + 1 isda.create = 4
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].verb, "cbu.create");
        assert_eq!(entries[1].verb, "cbu.assign-product");
        assert_eq!(entries[2].verb, "cbu.assign-product");
        assert_eq!(entries[3].verb, "isda.create");

        // Hash should be non-empty.
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_when_condition_false_skips_step() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_no_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        // No isda.create because has_otc == false.
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| e.verb != "isda.create"));
    }

    #[test]
    fn test_repeat_for_expansion() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let product_entries: Vec<_> = entries
            .iter()
            .filter(|e| e.verb == "cbu.assign-product")
            .collect();
        assert_eq!(product_entries.len(), 2);
        assert_eq!(product_entries[0].args["product"], "IRS");
        assert_eq!(product_entries[1].args["product"], "EQUITY");
    }

    #[test]
    fn test_context_substitution() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(entries[0].args["name"], "Allianz Lux");
    }

    #[test]
    fn test_answers_substitution() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(entries[0].args["jurisdiction"], "LU");
    }

    #[test]
    fn test_slot_provenance_context() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let create_entry = &entries[0];
        assert_eq!(
            create_entry.slot_provenance.slots.get("name"),
            Some(&SlotSource::InferredFromContext)
        );
    }

    #[test]
    fn test_slot_provenance_user_provided() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(
            entries[0].slot_provenance.slots.get("jurisdiction"),
            Some(&SlotSource::UserProvided)
        );
    }

    #[test]
    fn test_slot_provenance_repeat_items_user_provided() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        // Each repeat_for item should be UserProvided.
        for entry in entries.iter().filter(|e| e.verb == "cbu.assign-product") {
            assert_eq!(
                entry.slot_provenance.slots.get("product"),
                Some(&SlotSource::UserProvided)
            );
        }
    }

    #[test]
    fn test_slot_provenance_template_default() {
        let template = PackTemplate {
            template_id: "default-test".to_string(),
            when_to_use: "test".to_string(),
            steps: vec![TemplateStep {
                verb: "cbu.create".to_string(),
                args: HashMap::from([("kind".to_string(), serde_json::json!("sicav"))]),
                repeat_for: None,
                when: None,
                execution_mode: None,
            }],
        };

        let (entries, _) = instantiate_template(
            &template,
            &HashMap::new(),
            &HashMap::new(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(
            entries[0].slot_provenance.slots.get("kind"),
            Some(&SlotSource::TemplateDefault)
        );
    }

    #[test]
    fn test_execution_mode_human_gate() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let isda = entries.iter().find(|e| e.verb == "isda.create").unwrap();
        assert_eq!(isda.execution_mode, ExecutionMode::HumanGate);
    }

    #[test]
    fn test_template_hash_stability() {
        let template = make_template();
        let hash1 = compute_template_hash(&template);
        let hash2 = compute_template_hash(&template);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_template_hash_changes_on_modification() {
        let mut template = make_template();
        let hash1 = compute_template_hash(&template);

        template.steps.push(TemplateStep {
            verb: "extra.step".to_string(),
            args: HashMap::new(),
            repeat_for: None,
            when: None,
            execution_mode: None,
        });
        let hash2 = compute_template_hash(&template);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_entries_have_sentences() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        for entry in &entries {
            assert!(
                !entry.sentence.is_empty(),
                "Entry for {} has empty sentence",
                entry.verb
            );
        }
    }

    #[test]
    fn test_entries_have_dsl() {
        let template = make_template();
        let (entries, _) = instantiate_template(
            &template,
            &context(),
            &answers_with_otc(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        for entry in &entries {
            assert!(entry.dsl.starts_with('('), "DSL should start with '('");
            assert!(entry.dsl.ends_with(')'), "DSL should end with ')'");
            assert!(
                entry.dsl.contains(&entry.verb),
                "DSL should contain the verb"
            );
        }
    }

    #[test]
    fn test_missing_repeat_list_returns_error() {
        let template = PackTemplate {
            template_id: "bad".to_string(),
            when_to_use: "test".to_string(),
            steps: vec![TemplateStep {
                verb: "cbu.assign-product".to_string(),
                args: HashMap::from([("product".to_string(), serde_json::json!("{item}"))]),
                repeat_for: Some("answers.nonexistent".to_string()),
                when: None,
                execution_mode: None,
            }],
        };

        let result = instantiate_template(
            &template,
            &HashMap::new(),
            &HashMap::new(),
            &SentenceGenerator,
            &HashMap::new(),
            &HashMap::new(),
        );

        assert!(result.is_err());
    }
}
