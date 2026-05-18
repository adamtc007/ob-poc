//! Automatic phrase generation for verb discovery
//!
//! This module provides deterministic generation of invocation phrases
//! for DSL verbs based on synonym dictionaries. These phrases enable
//! semantic search without requiring manual curation.
//!
//! # Architecture
//!
//! ```text
//! V1 YAML verb definition (domain, action)
//!     ↓
//! generate_phrases(domain, action, existing)
//!     ↓
//! Merge existing + generated phrases
//!     ↓
//! Store in dsl_verbs.yaml_intent_patterns
//! ```

use std::collections::HashMap;
use std::sync::OnceLock;

/// Consumer-registered domain noun vocabulary for phrase generation.
pub type PhraseGenNouns = HashMap<String, Vec<String>>;

static PHRASE_GEN_NOUNS: OnceLock<PhraseGenNouns> = OnceLock::new();

/// Register the consumer's domain noun vocabulary.
///
/// Must be called before `load_verbs()` so phrase enrichment uses the right
/// vocabulary. Subsequent calls are silently ignored (OnceLock semantics).
pub fn set_phrase_gen_nouns(nouns: PhraseGenNouns) {
    let _ = PHRASE_GEN_NOUNS.set(nouns);
}

/// Verb action synonyms for phrase generation.
///
/// Maps common verb actions (create, list, get, etc.) to natural language
/// alternatives that users might type.
pub fn verb_synonyms() -> HashMap<&'static str, Vec<&'static str>> {
    let mut synonyms = HashMap::new();

    // CRUD operations
    // Note: list and read/get are deliberately differentiated to avoid collisions.
    // - list → "list all", "show all", "display", "enumerate" (plural/collection)
    // - read/get → "get details", "fetch", "retrieve", "view" (singular/detail)
    synonyms.insert("create", vec!["add", "new", "make", "register"]);
    synonyms.insert("list", vec!["show all", "list all", "display", "enumerate"]);
    synonyms.insert("get", vec!["show", "fetch", "retrieve", "read"]);
    synonyms.insert("read", vec!["get", "fetch", "view", "retrieve"]);
    synonyms.insert("update", vec!["edit", "modify", "change", "set"]);
    synonyms.insert("delete", vec!["remove", "drop", "terminate"]);
    synonyms.insert("remove", vec!["delete", "drop", "clear"]);

    // Computation
    synonyms.insert("compute", vec!["calculate", "derive", "run"]);
    synonyms.insert("calculate", vec!["compute", "derive", "determine"]);
    synonyms.insert("analyze", vec!["examine", "inspect", "review"]);
    synonyms.insert("validate", vec!["verify", "check", "confirm"]);

    // Navigation
    // "zoom in" removed — collides with view.zoom-in; "enter" removed — collides with view.book
    synonyms.insert("drill", vec!["dive", "expand", "go into", "dig into"]);
    // "zoom out" removed — collides with view.zoom-out in same domain
    synonyms.insert("surface", vec!["back", "up", "parent", "ascend"]);
    synonyms.insert("load", vec!["open", "switch", "select"]);
    synonyms.insert("unload", vec!["close", "remove", "clear"]);

    // Discovery
    synonyms.insert("trace", vec!["follow", "track", "path"]);
    synonyms.insert("discover", vec!["find", "identify", "detect"]);
    // "lookup" removed from find/search synonym sets — collides with
    // dedicated `lookup` verbs (e.g. gleif.lookup, subcustodian.lookup)
    // when both lookup + search live in the same domain.
    synonyms.insert("find", vec!["search", "locate"]);
    synonyms.insert("search", vec!["find", "query"]);

    // Workflow
    synonyms.insert("approve", vec!["accept", "confirm", "authorize"]);
    synonyms.insert("reject", vec!["decline", "deny", "refuse"]);
    synonyms.insert("submit", vec!["send", "complete", "finish"]);
    synonyms.insert("assign", vec!["allocate", "set", "give"]);

    // State changes
    synonyms.insert("activate", vec!["enable", "start", "turn on"]);
    synonyms.insert("deactivate", vec!["disable", "stop", "turn off"]);
    synonyms.insert("suspend", vec!["pause", "hold", "freeze"]);
    synonyms.insert("provision", vec!["setup", "configure", "initialize"]);

    // Linking
    synonyms.insert("link", vec!["connect", "attach", "associate"]);
    synonyms.insert("attach", vec!["link", "connect", "add"]);
    synonyms.insert("sync", vec!["synchronize", "refresh", "update"]);

    // Deal/Billing specific
    synonyms.insert("record", vec!["log", "capture", "enter"]);
    // "enter" removed — collides with drill synonyms in view domain
    synonyms.insert("book", vec!["record", "register", "log"]);
    synonyms.insert("generate", vec!["create", "produce", "build"]);
    synonyms.insert("invoice", vec!["bill", "charge"]);
    synonyms.insert("reconcile", vec!["match", "balance", "verify"]);

    synonyms
}

/// Generate invocation phrases for a verb.
///
/// Creates phrases by combining verb action synonyms with domain noun variations.
/// Existing phrases are preserved and deduped.
///
/// # Arguments
///
/// * `domain` - The verb's domain (e.g., "cbu", "deal", "billing")
/// * `action` - The verb's action (e.g., "create", "list", "get")
/// * `existing` - Any existing phrases to preserve
///
/// # Returns
///
/// A vector of unique phrases, limited to 20 entries.
///
/// # Example
///
/// ```
/// use dsl_core::config::phrase_gen::generate_phrases;
///
/// let phrases = generate_phrases("deal", "create", &[]);
/// // Returns: ["create deal", "add deal", "new deal record", "make client deal", ...]
/// ```
pub fn generate_phrases(domain: &str, action: &str, existing: &[String]) -> Vec<String> {
    let mut phrases: Vec<String> = existing.to_vec();

    let synonyms = verb_synonyms();

    // Get action words (original action + synonyms)
    let mut action_words: Vec<&str> = vec![action];
    if let Some(syns) = synonyms.get(action) {
        action_words.extend(syns.iter());
    }

    // Get domain words from consumer-registered noun vocabulary.
    // When no vocabulary is registered (dsl-lsp, bpmn-lite) only the domain
    // name itself is used as a phrase component — no ob-poc-specific expansions.
    let mut domain_words: Vec<&str> = vec![domain];
    let extra: Vec<&str> = PHRASE_GEN_NOUNS
        .get()
        .and_then(|nouns| nouns.get(domain))
        .map(|v| v.iter().map(String::as_str).collect())
        .unwrap_or_default();
    domain_words.extend(extra.iter());

    // Generate combinations: action + domain
    for action_word in &action_words {
        for domain_word in &domain_words {
            let phrase = format!("{} {}", action_word, domain_word);
            if !phrases.contains(&phrase) {
                phrases.push(phrase);
            }
        }
    }

    // Dedupe (preserving generation order — primary action+domain first) and limit
    let mut seen = std::collections::HashSet::new();
    phrases.retain(|p| seen.insert(p.clone()));
    phrases.truncate(20);

    phrases
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_nouns() {
        // OnceLock: only the first call actually registers; subsequent calls are no-ops.
        set_phrase_gen_nouns(
            [
                ("cbu", vec!["cbu", "client business unit", "trading unit"]),
                ("entity", vec!["entity", "company", "person"]),
                (
                    "deal",
                    vec!["deal", "deal record", "client deal", "sales deal"],
                ),
                (
                    "billing",
                    vec!["billing", "billing profile", "fee billing", "invoice"],
                ),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into_iter().map(str::to_string).collect()))
            .collect(),
        );
    }

    #[test]
    fn test_generate_phrases_cbu_create() {
        setup_test_nouns();
        let phrases = generate_phrases("cbu", "create", &[]);
        assert!(phrases.contains(&"create cbu".to_string()));
        assert!(phrases.contains(&"add cbu".to_string()));
        assert!(phrases.contains(&"new client business unit".to_string()));
        assert!(!phrases.is_empty());
        assert!(phrases.len() <= 20);
    }

    #[test]
    fn test_generate_phrases_deal_create() {
        setup_test_nouns();
        let phrases = generate_phrases("deal", "create", &[]);
        assert!(phrases.contains(&"create deal".to_string()));
        assert!(phrases.contains(&"add deal record".to_string()));
        assert!(phrases.contains(&"new client deal".to_string()));
    }

    #[test]
    fn test_generate_phrases_billing_list() {
        setup_test_nouns();
        let phrases = generate_phrases("billing", "list", &[]);
        assert!(phrases.contains(&"list billing".to_string()));
        assert!(phrases.contains(&"show all billing".to_string()));
        assert!(phrases.len() >= 5);
        assert!(phrases.len() <= 20);
    }

    #[test]
    fn test_generate_phrases_preserves_existing() {
        setup_test_nouns();
        let existing = vec!["custom phrase".to_string()];
        let phrases = generate_phrases("cbu", "create", &existing);
        assert!(phrases.contains(&"custom phrase".to_string()));
        assert!(phrases.contains(&"create cbu".to_string()));
    }

    #[test]
    fn test_generate_phrases_dedupes() {
        setup_test_nouns();
        let existing = vec!["create cbu".to_string()];
        let phrases = generate_phrases("cbu", "create", &existing);
        let count = phrases.iter().filter(|p| *p == "create cbu").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_verb_synonyms_coverage() {
        let synonyms = verb_synonyms();
        assert!(synonyms.contains_key("create"));
        assert!(synonyms.contains_key("list"));
        assert!(synonyms.contains_key("get"));
        assert!(synonyms.contains_key("update"));
        assert!(synonyms.contains_key("delete"));
    }

    #[test]
    fn test_noun_expansion_without_registration() {
        // Without registration, only the domain name itself is used.
        // (setup_test_nouns() may already have registered in this binary run;
        //  this test is meaningful when it runs first — documents the contract)
        let phrases = generate_phrases("unknown-domain", "create", &[]);
        assert!(phrases.contains(&"create unknown-domain".to_string()));
        assert!(phrases.contains(&"add unknown-domain".to_string()));
    }
}
