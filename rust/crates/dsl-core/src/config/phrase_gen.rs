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
    synonyms.insert("find", vec!["search", "locate", "lookup"]);
    synonyms.insert("search", vec!["find", "lookup", "query"]);

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

/// Domain noun mappings for phrase generation.
///
/// Maps DSL domain names to user-friendly terms that might appear
/// in natural language queries.
pub fn domain_nouns() -> HashMap<&'static str, Vec<&'static str>> {
    let mut nouns = HashMap::new();

    // Core entities
    nouns.insert("entity", vec!["entity", "company", "person"]);
    nouns.insert("cbu", vec!["cbu", "client business unit", "trading unit"]);
    nouns.insert("fund", vec!["fund", "investment vehicle", "sicav"]);

    // Ownership/control
    nouns.insert("ownership", vec!["ownership", "stake", "holding"]);
    nouns.insert("ubo", vec!["ubo", "beneficial owner", "ultimate owner"]);
    nouns.insert("control", vec!["control", "ownership chain", "hierarchy"]);

    // KYC/Compliance
    nouns.insert("kyc", vec!["kyc", "case", "compliance check"]);
    nouns.insert("kyc-case", vec!["kyc case", "compliance case"]);
    nouns.insert("screening", vec!["screening", "check", "verification"]);
    nouns.insert("document", vec!["document", "file", "attachment"]);
    nouns.insert("requirement", vec!["requirement", "document requirement"]);

    // Session/Navigation
    nouns.insert("session", vec!["session", "scope", "workspace"]);
    // "display"/"visualization" removed — collides with graph domain and view sub-verbs
    nouns.insert("view", vec!["view", "viewport"]);
    // "visualization" removed — collides with view domain
    nouns.insert("graph", vec!["graph", "diagram"]);

    // Trading/Settlement
    nouns.insert("trading-profile", vec!["trading profile", "profile"]);
    nouns.insert("custody", vec!["custody", "safekeeping", "account"]);
    // "agreement"/"contract" removed — collides with contract domain
    nouns.insert("isda", vec!["isda", "isda agreement", "isda contract"]);
    nouns.insert("ssi", vec!["ssi", "settlement instruction"]);

    // Products/Services
    // "service" removed — collides with service domain
    nouns.insert("product", vec!["product", "offering"]);
    // "agreement" removed — collides with isda domain
    nouns.insert("contract", vec!["contract", "legal contract"]);
    nouns.insert("service-resource", vec!["service resource", "resource"]);
    nouns.insert("service-intent", vec!["service intent", "intent"]);

    // Identifiers
    nouns.insert("identifier", vec!["identifier", "id", "reference"]);
    nouns.insert("gleif", vec!["gleif", "lei", "legal entity identifier"]);
    nouns.insert(
        "bods",
        vec!["bods", "beneficial ownership", "ownership data"],
    );

    // Reference data
    nouns.insert("jurisdiction", vec!["jurisdiction", "country"]);
    nouns.insert("currency", vec!["currency", "money"]);
    // "position" removed — collides with holding domain
    nouns.insert("role", vec!["role", "entity role"]);

    // Workflow
    nouns.insert("runbook", vec!["runbook", "command", "staged command"]);
    nouns.insert("agent", vec!["agent", "assistant"]);
    nouns.insert("batch", vec!["batch", "bulk operation"]);

    // Investor
    nouns.insert("investor", vec!["investor", "shareholder"]);
    // "position" removed — collides with role domain
    nouns.insert("holding", vec!["holding", "investment holding"]);
    nouns.insert("share-class", vec!["share class", "class"]);

    // Deal/Billing (067)
    nouns.insert(
        "deal",
        vec!["deal", "deal record", "client deal", "sales deal"],
    );
    nouns.insert(
        "billing",
        vec!["billing", "billing profile", "fee billing", "invoice"],
    );
    nouns.insert("fee", vec!["fee", "charge", "cost"]);
    nouns.insert("rate-card", vec!["rate card", "pricing", "fee schedule"]);
    nouns.insert("invoice", vec!["invoice", "bill", "statement"]);

    nouns
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
    let nouns = domain_nouns();

    // Get action words (original action + synonyms)
    let mut action_words: Vec<&str> = vec![action];
    if let Some(syns) = synonyms.get(action) {
        action_words.extend(syns.iter());
    }

    // Get domain words (original domain + noun variations)
    let mut domain_words: Vec<&str> = vec![domain];
    if let Some(domain_nouns) = nouns.get(domain) {
        domain_words.extend(domain_nouns.iter());
    }

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

    #[test]
    fn test_generate_phrases_cbu_create() {
        let phrases = generate_phrases("cbu", "create", &[]);

        assert!(phrases.contains(&"create cbu".to_string()));
        assert!(phrases.contains(&"add cbu".to_string()));
        assert!(phrases.contains(&"new client business unit".to_string()));
        assert!(!phrases.is_empty());
        assert!(phrases.len() <= 15);
    }

    #[test]
    fn test_generate_phrases_deal_create() {
        let phrases = generate_phrases("deal", "create", &[]);

        assert!(phrases.contains(&"create deal".to_string()));
        assert!(phrases.contains(&"add deal record".to_string()));
        assert!(phrases.contains(&"new client deal".to_string()));
    }

    #[test]
    fn test_generate_phrases_billing_list() {
        let phrases = generate_phrases("billing", "list", &[]);

        // Core phrase should always be present
        assert!(phrases.contains(&"list billing".to_string()));
        // list synonyms should use "show all"/"list all" (not bare "show")
        assert!(phrases.contains(&"show all billing".to_string()));
        // Should have multiple phrases generated
        assert!(phrases.len() >= 5);
        // Should be limited to max 20
        assert!(phrases.len() <= 20);
    }

    #[test]
    fn test_generate_phrases_preserves_existing() {
        let existing = vec!["custom phrase".to_string()];
        let phrases = generate_phrases("cbu", "create", &existing);

        assert!(phrases.contains(&"custom phrase".to_string()));
        assert!(phrases.contains(&"create cbu".to_string()));
    }

    #[test]
    fn test_generate_phrases_dedupes() {
        let existing = vec!["create cbu".to_string()];
        let phrases = generate_phrases("cbu", "create", &existing);

        let count = phrases.iter().filter(|p| *p == "create cbu").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_verb_synonyms_coverage() {
        let synonyms = verb_synonyms();

        // Key verbs should have synonyms
        assert!(synonyms.contains_key("create"));
        assert!(synonyms.contains_key("list"));
        assert!(synonyms.contains_key("get"));
        assert!(synonyms.contains_key("update"));
        assert!(synonyms.contains_key("delete"));
    }

    #[test]
    fn test_domain_nouns_coverage() {
        let nouns = domain_nouns();

        // Key domains should have noun mappings
        assert!(nouns.contains_key("cbu"));
        assert!(nouns.contains_key("entity"));
        assert!(nouns.contains_key("deal"));
        assert!(nouns.contains_key("billing"));
    }
}
