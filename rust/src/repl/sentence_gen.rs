//! Sentence Generator — Template-based, deterministic, LLM-free
//!
//! Produces human-readable sentences from verb + args. Used for:
//! - Runbook entry display ("Add IRS product to Allianz Lux")
//! - Sentence playback in the REPL
//! - Pack-level summary generation
//!
//! # Fallback Chain (Phase 0)
//!
//! 1. `invocation_phrases` from VerbConfig — best match by arg slot coverage.
//! 2. `phrase_gen::generate_phrases()` — auto-generated from synonym dictionaries.
//! 3. Structured fallback — "{action} {domain} with {args}".
//!
//! Phase 1 will add `sentences.step[]` on verb YAML as the highest-priority source.

use std::collections::HashMap;

/// Deterministic sentence generator — no LLM, no network.
pub struct SentenceGenerator;

impl SentenceGenerator {
    /// Generate a human-readable sentence for a verb invocation.
    ///
    /// # Arguments
    ///
    /// * `verb` — Fully-qualified verb name (e.g. "cbu.assign-product").
    /// * `args` — Extracted arguments (e.g. {"product": "IRS", "cbu-name": "Allianz Lux"}).
    /// * `invocation_phrases` — Invocation phrases from VerbConfig (may be empty).
    /// * `description` — Verb description from VerbConfig (fallback).
    pub fn generate(
        &self,
        verb: &str,
        args: &HashMap<String, String>,
        invocation_phrases: &[String],
        description: &str,
    ) -> String {
        // 1. Try invocation_phrases — pick best template by arg coverage.
        if let Some(sentence) = Self::best_phrase_template(invocation_phrases, args) {
            return sentence;
        }

        // 2. Try auto-generated phrases from phrase_gen.
        //    Only use phrase_gen if it produced synonym-enhanced phrases (more than
        //    the single base "{action} {domain}" combo). Otherwise fall through to
        //    the description-based fallback which is more readable.
        let (domain, action) = Self::split_verb(verb);
        let generated = dsl_core::config::phrase_gen::generate_phrases(domain, action, &[]);
        if generated.len() > 1 {
            if let Some(sentence) = Self::best_phrase_template(&generated, args) {
                return sentence;
            }
        }

        // 3. Structured fallback (uses description if available).
        Self::structured_fallback(verb, args, description)
    }

    /// Pick the best invocation phrase as a sentence template.
    ///
    /// Strategy: find phrases that contain arg-like placeholders or that
    /// we can augment with arg values. The "best" phrase is the one whose
    /// words overlap most with the arg keys/values.
    fn best_phrase_template(phrases: &[String], args: &HashMap<String, String>) -> Option<String> {
        if phrases.is_empty() {
            return None;
        }

        // Build a word set from arg keys and values for scoring.
        let arg_words: Vec<String> = args
            .iter()
            .flat_map(|(k, v)| {
                let mut words: Vec<String> = k
                    .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
                    .map(|s| s.to_lowercase())
                    .collect();
                words.extend(v.split_whitespace().map(|s| s.to_lowercase()));
                words
            })
            .collect();

        // Score each phrase by word overlap with args.
        let mut best_phrase: Option<&str> = None;
        let mut best_score = -1i32;

        for phrase in phrases {
            let phrase_lower = phrase.to_lowercase();
            let score: i32 = arg_words
                .iter()
                .filter(|w| w.len() > 2 && phrase_lower.contains(w.as_str()))
                .count() as i32;

            if score > best_score {
                best_score = score;
                best_phrase = Some(phrase.as_str());
            }
        }

        let base_phrase = best_phrase?;

        // Substitute any {placeholder} patterns in the phrase.
        let mut sentence = Self::substitute(base_phrase, args);

        // If the phrase is short and we have arg values, append them.
        if args.is_empty() {
            return Some(Self::capitalize_first(&sentence));
        }

        // Append arg values that aren't already mentioned in the sentence.
        let sentence_lower = sentence.to_lowercase();
        let unmentioned: Vec<&str> = args
            .values()
            .filter(|v| !v.is_empty() && !sentence_lower.contains(&v.to_lowercase()))
            .map(|v| v.as_str())
            .collect();

        if !unmentioned.is_empty() {
            sentence.push_str(" — ");
            sentence.push_str(&unmentioned.join(", "));
        }

        Some(Self::capitalize_first(&sentence))
    }

    /// Substitute `{key}` placeholders in a template with arg values.
    fn substitute(template: &str, args: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in args {
            // Try both {key} and {key-with-dashes}
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
            let placeholder_underscore = format!("{{{}}}", key.replace('-', "_"));
            result = result.replace(&placeholder_underscore, value);
        }
        result
    }

    /// Structured fallback: "Action domain with arg1, arg2".
    fn structured_fallback(
        verb: &str,
        args: &HashMap<String, String>,
        description: &str,
    ) -> String {
        // Use description if available and short enough.
        if !description.is_empty() && description.len() < 80 {
            if args.is_empty() {
                return Self::capitalize_first(description);
            }
            let arg_summary = Self::format_arg_summary(args);
            return format!("{} — {}", Self::capitalize_first(description), arg_summary);
        }

        // Otherwise build from verb structure.
        let (domain, action) = Self::split_verb(verb);
        let action_display = action.replace('-', " ");
        let domain_display = domain.replace('-', " ");

        if args.is_empty() {
            return Self::capitalize_first(&format!("{} {}", action_display, domain_display));
        }

        let arg_summary = Self::format_arg_summary(args);
        Self::capitalize_first(&format!(
            "{} {} — {}",
            action_display, domain_display, arg_summary
        ))
    }

    /// Format args as a readable summary string.
    fn format_arg_summary(args: &HashMap<String, String>) -> String {
        let mut pairs: Vec<_> = args.iter().collect();
        pairs.sort_by_key(|(k, _)| *k);

        pairs
            .iter()
            .map(|(k, v)| {
                let display_key = k.replace(['-', '_'], " ");
                format!("{}: {}", display_key, v)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Split "domain.action" → ("domain", "action").
    fn split_verb(verb: &str) -> (&str, &str) {
        verb.split_once('.').unwrap_or(("unknown", verb))
    }

    /// Capitalize the first character of a string.
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        }
    }

    /// Format a list with Oxford comma.
    ///
    /// - 0 items → ""
    /// - 1 item  → "A"
    /// - 2 items → "A and B"
    /// - 3+ items → "A, B, and C"
    pub fn format_list(values: &[String]) -> String {
        match values.len() {
            0 => String::new(),
            1 => values[0].clone(),
            2 => format!("{} and {}", values[0], values[1]),
            _ => {
                let (last, rest) = values.split_last().unwrap();
                format!("{}, and {}", rest.join(", "), last)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn gen() -> SentenceGenerator {
        SentenceGenerator
    }

    fn args(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // -- Oxford comma --

    #[test]
    fn test_format_list_empty() {
        assert_eq!(SentenceGenerator::format_list(&[]), "");
    }

    #[test]
    fn test_format_list_single() {
        assert_eq!(SentenceGenerator::format_list(&["IRS".to_string()]), "IRS");
    }

    #[test]
    fn test_format_list_two() {
        assert_eq!(
            SentenceGenerator::format_list(&["IRS".to_string(), "EQUITY".to_string()]),
            "IRS and EQUITY"
        );
    }

    #[test]
    fn test_format_list_three() {
        assert_eq!(
            SentenceGenerator::format_list(&[
                "IRS".to_string(),
                "EQUITY".to_string(),
                "FX".to_string()
            ]),
            "IRS, EQUITY, and FX"
        );
    }

    #[test]
    fn test_format_list_five() {
        let items: Vec<String> = vec!["A", "B", "C", "D", "E"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(SentenceGenerator::format_list(&items), "A, B, C, D, and E");
    }

    // -- Sentence generation with invocation phrases --

    #[test]
    fn test_generate_with_invocation_phrase() {
        let sentence = gen().generate(
            "cbu.create",
            &args(&[("name", "Allianz Lux")]),
            &["create cbu".to_string(), "add new client unit".to_string()],
            "Create a new CBU",
        );
        // Should use an invocation phrase and include the arg value.
        assert!(sentence.contains("Allianz Lux"));
    }

    #[test]
    fn test_generate_with_matching_phrase() {
        let sentence = gen().generate(
            "cbu.assign-product",
            &args(&[("product", "IRS"), ("cbu-name", "Allianz Lux")]),
            &[
                "assign product to cbu".to_string(),
                "add product".to_string(),
            ],
            "Assign a product to a CBU",
        );
        // Should pick the phrase with best arg overlap and include values.
        assert!(sentence.contains("IRS") || sentence.contains("Allianz Lux"));
    }

    // -- Sentence generation with no invocation phrases (fallback) --

    #[test]
    fn test_generate_fallback_to_phrase_gen() {
        let sentence = gen().generate(
            "cbu.create",
            &args(&[("name", "Test Fund")]),
            &[], // no invocation phrases
            "",  // no description
        );
        // Should still produce something reasonable from phrase_gen.
        assert!(!sentence.is_empty());
        assert!(sentence.contains("Test Fund") || sentence.to_lowercase().contains("cbu"));
    }

    #[test]
    fn test_generate_structured_fallback() {
        let sentence = gen().generate(
            "custom-domain.exotic-action",
            &args(&[("target", "something")]),
            &[],
            "", // no description, no phrase_gen match
        );
        // Should fall back to structured format.
        assert!(!sentence.is_empty());
        assert!(sentence.contains("something") || sentence.contains("exotic"));
    }

    #[test]
    fn test_generate_description_fallback() {
        // Use a domain that has no phrase_gen coverage to force the description fallback.
        let sentence = gen().generate(
            "zzz-unknown-domain.exotic-verb",
            &args(&[("key", "value")]),
            &[],
            "Do something custom",
        );
        assert!(sentence.starts_with("Do something custom"));
        assert!(sentence.contains("value"));
    }

    #[test]
    fn test_generate_no_args() {
        let sentence = gen().generate(
            "session.clear",
            &HashMap::new(),
            &["clear session".to_string()],
            "Clear the current session",
        );
        assert_eq!(sentence, "Clear session");
    }

    // -- Substitution --

    #[test]
    fn test_substitute_placeholders() {
        let result = SentenceGenerator::substitute(
            "Create {name} in {jurisdiction}",
            &args(&[("name", "Allianz Lux"), ("jurisdiction", "LU")]),
        );
        assert_eq!(result, "Create Allianz Lux in LU");
    }

    #[test]
    fn test_substitute_no_match() {
        let result = SentenceGenerator::substitute("create cbu", &args(&[("name", "Test")]));
        assert_eq!(result, "create cbu");
    }

    // -- Capitalize --

    #[test]
    fn test_capitalize_first() {
        assert_eq!(SentenceGenerator::capitalize_first("hello"), "Hello");
        assert_eq!(SentenceGenerator::capitalize_first(""), "");
        assert_eq!(SentenceGenerator::capitalize_first("A"), "A");
        assert_eq!(SentenceGenerator::capitalize_first("already"), "Already");
    }

    // -- Split verb --

    #[test]
    fn test_split_verb() {
        assert_eq!(
            SentenceGenerator::split_verb("cbu.create"),
            ("cbu", "create")
        );
        assert_eq!(
            SentenceGenerator::split_verb("trading-profile.list"),
            ("trading-profile", "list")
        );
        assert_eq!(SentenceGenerator::split_verb("bare"), ("unknown", "bare"));
    }

    // -- Varied verb/arg combos --

    #[test]
    fn test_session_load_galaxy() {
        let s = gen().generate(
            "session.load-galaxy",
            &args(&[("apex-name", "Allianz")]),
            &["load the book".to_string(), "load galaxy".to_string()],
            "Load all CBUs under apex entity",
        );
        assert!(!s.is_empty());
    }

    #[test]
    fn test_kyc_case_create() {
        let s = gen().generate(
            "kyc-case.create",
            &args(&[("entity-id", "abc-123"), ("case-type", "onboarding")]),
            &["open kyc case".to_string()],
            "Create a KYC case",
        );
        assert!(!s.is_empty());
    }

    #[test]
    fn test_isda_create() {
        let s = gen().generate(
            "isda.create",
            &args(&[("counterparty", "Goldman Sachs"), ("governing-law", "NY")]),
            &["create isda agreement".to_string()],
            "Create ISDA master agreement",
        );
        assert!(s.contains("Goldman Sachs") || s.contains("isda"));
    }

    #[test]
    fn test_entity_create() {
        let s = gen().generate(
            "entity.create",
            &args(&[("name", "Acme Corp"), ("entity-type", "company")]),
            &["create entity".to_string()],
            "Create a new entity",
        );
        assert!(s.contains("Acme Corp"));
    }

    #[test]
    fn test_bulk_operation_with_list_args() {
        let s = gen().generate(
            "cbu.assign-product",
            &args(&[("product", "IRS, EQUITY, FX"), ("cbu-name", "Fund A")]),
            &["assign product".to_string()],
            "Assign product to CBU",
        );
        assert!(s.contains("IRS") || s.contains("Fund A"));
    }

    #[test]
    fn test_trading_profile_create() {
        let s = gen().generate(
            "trading-profile.create",
            &args(&[("cbu-id", "uuid-123")]),
            &[],
            "Create trading profile for CBU",
        );
        assert!(!s.is_empty());
    }

    #[test]
    fn test_view_drill() {
        let s = gen().generate(
            "view.drill",
            &args(&[("entity-id", "uuid-456")]),
            &["drill down".to_string(), "go deeper".to_string()],
            "Drill into entity detail",
        );
        assert!(!s.is_empty());
    }

    #[test]
    fn test_many_args() {
        let s = gen().generate(
            "contract.create",
            &args(&[
                ("client", "Allianz"),
                ("reference", "MSA-2024-001"),
                ("effective-date", "2024-01-01"),
                ("product", "CUSTODY"),
            ]),
            &["create contract".to_string()],
            "Create legal contract",
        );
        assert!(!s.is_empty());
        // Should mention at least some arg values.
        let has_arg = s.contains("Allianz") || s.contains("MSA-2024-001") || s.contains("CUSTODY");
        assert!(has_arg);
    }

    // -- Format arg summary --

    #[test]
    fn test_format_arg_summary() {
        let summary = SentenceGenerator::format_arg_summary(&args(&[
            ("name", "Test"),
            ("jurisdiction", "LU"),
        ]));
        assert!(summary.contains("name: Test"));
        assert!(summary.contains("jurisdiction: LU"));
    }

    #[test]
    fn test_format_arg_summary_dashes_to_spaces() {
        let summary = SentenceGenerator::format_arg_summary(&args(&[("cbu-name", "Fund A")]));
        assert!(summary.contains("cbu name: Fund A"));
    }
}
