//! ConstellationVerbIndex — two-way lookup between utterance clues and live constellation state.
//!
//! Given a hydrated constellation (slots with state-gated `available_verbs`), builds:
//!
//! 1. **Forward:** `(noun, action_stem) → Vec<verb_fqn>` — "user said 'screen the entities', what verbs match?"
//! 2. **Reverse:** `verb_fqn → Vec<SlotContext>` — "given screening.run, which slots/entities does it live on?"
//! 3. **By noun:** `noun → Vec<verb_fqn>` — all verbs available on slots matching this noun
//! 4. **By action:** `action_stem → Vec<verb_fqn>` — all verbs matching this action across all slots
//!
//! The index is rebuilt each time the constellation is re-hydrated (after execution).
//! It reflects **current state only** — verbs gated behind future states are excluded.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::sem_os_runtime::constellation_runtime::{HydratedCardinality, HydratedSlot};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Context about where a verb lives in the constellation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotContext {
    pub slot_name: String,
    pub slot_path: String,
    pub effective_state: String,
    pub cardinality: HydratedCardinality,
    pub has_entity: bool,
}

/// A single entry in the forward index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbMatch {
    pub verb_fqn: String,
    pub slot_name: String,
    pub effective_state: String,
    /// Match priority: 1 = verb domain prefix matches noun (high confidence),
    /// 2 = slot name matches noun (lower confidence, broader).
    /// Used to prefer domain-specific matches over slot-name fallbacks
    /// when entity_workstream hosts verbs from 6+ sub-domains.
    pub priority: u8,
}

/// Summary statistics for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_slots: usize,
    pub total_available_verbs: usize,
    pub distinct_verbs: usize,
    pub distinct_nouns: usize,
    pub distinct_actions: usize,
    pub forward_entries: usize,
}

/// The two-way index over a hydrated constellation's verb surface.
#[derive(Debug, Clone)]
pub struct ConstellationVerbIndex {
    /// (noun_key, action_stem) → matching verbs with slot context
    forward: HashMap<(String, String), Vec<VerbMatch>>,
    /// verb_fqn → slot contexts where it appears
    reverse: HashMap<String, Vec<SlotContext>>,
    /// noun_key → all available verbs on matching slots
    by_noun: HashMap<String, Vec<String>>,
    /// action_stem → all available verbs matching this action
    by_action: HashMap<String, Vec<String>>,
    /// Stats
    stats: IndexStats,
}

impl ConstellationVerbIndex {
    /// Build the index from a hydrated constellation's slot tree.
    pub fn build(slots: &[HydratedSlot]) -> Self {
        let mut forward: HashMap<(String, String), Vec<VerbMatch>> = HashMap::new();
        let mut reverse: HashMap<String, Vec<SlotContext>> = HashMap::new();
        let mut by_noun: HashMap<String, Vec<String>> = HashMap::new();
        let mut by_action: HashMap<String, Vec<String>> = HashMap::new();

        let mut total_slots = 0usize;
        let mut total_available_verbs = 0usize;

        // Flatten the slot tree and process each slot.
        let flat = flatten_slots(slots);

        for slot in &flat {
            total_slots += 1;
            let noun_keys = slot_to_noun_keys(&slot.name);

            for verb_fqn in &slot.available_verbs {
                total_available_verbs += 1;
                let action = extract_action_from_fqn(verb_fqn);

                let sc = SlotContext {
                    slot_name: slot.name.clone(),
                    slot_path: slot.path.clone(),
                    effective_state: slot.effective_state.clone(),
                    cardinality: slot.cardinality,
                    has_entity: slot.entity_id.is_some(),
                };

                // Combine slot noun keys with verb-domain noun keys.
                // e.g., "document.solicit" on entity_workstream → nouns include "document"
                let domain_nouns = verb_domain_to_noun_keys(verb_fqn);

                // Forward: every (noun, action) pair — with priority.
                // Priority 1 = noun came from verb's domain prefix (high confidence)
                // Priority 2 = noun came from slot name (broader, lower confidence)
                // This ensures "document solicit" matches document.solicit (priority 1)
                // over entity-workstream.update-status (priority 2 via slot name "entity").
                let all_nouns_with_priority: Vec<(String, u8)> = {
                    let mut pairs = Vec::new();
                    for dn in &domain_nouns {
                        pairs.push((dn.clone(), 1));
                    }
                    for sn in &noun_keys {
                        if !domain_nouns.contains(sn) {
                            pairs.push((sn.clone(), 2));
                        }
                    }
                    pairs
                };

                for (noun, priority) in &all_nouns_with_priority {
                    let vm = VerbMatch {
                        verb_fqn: verb_fqn.clone(),
                        slot_name: slot.name.clone(),
                        effective_state: slot.effective_state.clone(),
                        priority: *priority,
                    };
                    forward
                        .entry((noun.clone(), action.clone()))
                        .or_default()
                        .push(vm);

                    // By noun
                    let noun_verbs = by_noun.entry(noun.clone()).or_default();
                    if !noun_verbs.contains(verb_fqn) {
                        noun_verbs.push(verb_fqn.clone());
                    }
                }

                // By action
                let action_verbs = by_action.entry(action.clone()).or_default();
                if !action_verbs.contains(verb_fqn) {
                    action_verbs.push(verb_fqn.clone());
                }

                // Reverse
                reverse.entry(verb_fqn.clone()).or_default().push(sc);
            }
        }

        let distinct_verbs = reverse.len();
        let distinct_nouns = by_noun.len();
        let distinct_actions = by_action.len();
        let forward_entries = forward.len();

        ConstellationVerbIndex {
            forward,
            reverse,
            by_noun,
            by_action,
            stats: IndexStats {
                total_slots,
                total_available_verbs,
                distinct_verbs,
                distinct_nouns,
                distinct_actions,
                forward_entries,
            },
        }
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Look up verbs matching both a noun and action stem.
    /// This is the primary "two-clue" resolution path.
    /// Results are sorted by priority (1 = domain-prefix match first, 2 = slot-name fallback).
    pub fn lookup(&self, noun: &str, action: &str) -> Vec<&VerbMatch> {
        let key = (normalize(noun), normalize_action(action));
        let mut matches: Vec<&VerbMatch> = self.forward.get(&key).map_or_else(Vec::new, |v| v.iter().collect());
        matches.sort_by_key(|m| m.priority);
        matches
    }

    /// Look up verbs matching a noun (any action).
    pub fn lookup_by_noun(&self, noun: &str) -> &[String] {
        self.by_noun
            .get(&normalize(noun))
            .map_or(&[] as &[String], |v| v.as_slice())
    }

    /// Look up verbs matching an action stem (any noun).
    pub fn lookup_by_action(&self, action: &str) -> &[String] {
        self.by_action
            .get(&normalize_action(action))
            .map_or(&[] as &[String], |v| v.as_slice())
    }

    /// Reverse: given a verb FQN, where does it live in the constellation?
    pub fn slot_contexts(&self, verb_fqn: &str) -> &[SlotContext] {
        self.reverse
            .get(verb_fqn)
            .map_or(&[] as &[SlotContext], |v| v.as_slice())
    }

    /// All verb FQNs currently available in the constellation.
    pub fn all_verbs(&self) -> Vec<&str> {
        self.reverse.keys().map(|s| s.as_str()).collect()
    }

    /// All noun keys present in the index.
    pub fn all_nouns(&self) -> Vec<&str> {
        self.by_noun.keys().map(|s| s.as_str()).collect()
    }

    /// All action stems present in the index.
    pub fn all_actions(&self) -> Vec<&str> {
        self.by_action.keys().map(|s| s.as_str()).collect()
    }

    /// Diagnostic statistics.
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }

    /// Produce a human-readable dump for inspection.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "ConstellationVerbIndex: {} slots, {} available verbs, {} distinct verbs\n",
            self.stats.total_slots, self.stats.total_available_verbs, self.stats.distinct_verbs,
        ));
        out.push_str(&format!(
            "  {} nouns × {} actions = {} forward entries\n\n",
            self.stats.distinct_nouns, self.stats.distinct_actions, self.stats.forward_entries,
        ));

        // By noun
        out.push_str("=== By Noun ===\n");
        let mut nouns: Vec<_> = self.by_noun.iter().collect();
        nouns.sort_by_key(|(k, _)| *k);
        for (noun, verbs) in &nouns {
            out.push_str(&format!("  {} → [{}]\n", noun, verbs.join(", ")));
        }

        // By action
        out.push_str("\n=== By Action ===\n");
        let mut actions: Vec<_> = self.by_action.iter().collect();
        actions.sort_by_key(|(k, _)| *k);
        for (action, verbs) in &actions {
            out.push_str(&format!("  {} → [{}]\n", action, verbs.join(", ")));
        }

        // Forward (noun, action) pairs
        out.push_str("\n=== Forward (noun, action) → verbs ===\n");
        let mut pairs: Vec<_> = self.forward.iter().collect();
        pairs.sort_by(|((n1, a1), _), ((n2, a2), _)| (n1, a1).cmp(&(n2, a2)));
        for ((noun, action), matches) in &pairs {
            let verbs: Vec<_> = matches.iter().map(|m| m.verb_fqn.as_str()).collect();
            out.push_str(&format!("  ({}, {}) → [{}]\n", noun, action, verbs.join(", ")));
        }

        // Reverse
        out.push_str("\n=== Reverse: verb → slots ===\n");
        let mut rev: Vec<_> = self.reverse.iter().collect();
        rev.sort_by_key(|(k, _)| *k);
        for (verb, contexts) in &rev {
            let slots: Vec<_> = contexts
                .iter()
                .map(|c| format!("{}[{}]", c.slot_name, c.effective_state))
                .collect();
            out.push_str(&format!("  {} → [{}]\n", verb, slots.join(", ")));
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Flatten the slot tree into a list (depth-first).
fn flatten_slots(slots: &[HydratedSlot]) -> Vec<&HydratedSlot> {
    let mut out = Vec::new();
    for slot in slots {
        out.push(slot);
        out.extend(flatten_slots(&slot.children));
    }
    out
}

/// Extract the action part from a verb FQN.
///
/// `"kyc-case.create"` → `"create"`
/// `"entity-workstream.update-status"` → `"update"` (first segment before hyphen)
/// `"screening.run"` → `"run"`
fn extract_action_from_fqn(fqn: &str) -> String {
    let action_part = fqn
        .rsplit_once('.')
        .map_or(fqn, |(_, action)| action);

    // Normalize compound actions: "update-status" → "update", "trace-chain" → "trace"
    // This aligns with how extract_action_stem produces single-word stems.
    let stem = action_part
        .split('-')
        .next()
        .unwrap_or(action_part);

    normalize_action(stem)
}

/// Extract noun keys from a verb's domain prefix (the part before the dot).
///
/// `"document.solicit"` → `["document", "doc"]`
/// `"screening.run"` → `["screening", "sanctions", "pep"]`
/// `"red-flag.raise"` → `["flag", "red-flag"]`
/// `"kyc-case.create"` → `["case", "kyc"]`
///
/// This is the key insight: verbs carry their own domain noun, independent
/// of which slot they're declared on. A `document.solicit` verb on an
/// `entity_workstream` slot should be findable via noun "document".
fn verb_domain_to_noun_keys(fqn: &str) -> Vec<String> {
    let domain = fqn.split('.').next().unwrap_or("");
    let mut keys = Vec::new();

    // The full domain as a key
    if !domain.is_empty() {
        keys.push(domain.to_string());
    }

    // Domain-specific expansions
    match domain {
        "kyc-case" => {
            keys.push("case".into());
            keys.push("kyc".into());
        }
        "entity-workstream" => {
            keys.push("workstream".into());
            keys.push("entity".into());
        }
        "screening" => {
            keys.push("sanctions".into());
            keys.push("pep".into());
        }
        "red-flag" => {
            keys.push("flag".into());
        }
        "document" => {
            keys.push("doc".into());
            keys.push("documents".into());
        }
        "requirement" => {
            keys.push("requirements".into());
        }
        "tollgate" => {
            keys.push("gate".into());
        }
        "identifier" => {
            keys.push("identity".into());
        }
        "kyc-agreement" => {
            keys.push("agreement".into());
            keys.push("kyc".into());
        }
        "ubo" => {
            keys.push("ownership".into());
            keys.push("control".into());
        }
        "ownership" => {
            keys.push("ubo".into());
        }
        "entity" => {
            keys.push("party".into());
        }
        "party" => {
            keys.push("entity".into());
        }
        "cbu" => {
            keys.push("unit".into());
        }
        "case" => {
            keys.push("kyc".into());
        }
        "mandate" => {
            keys.push("trading".into());
        }
        "fund" => {
            keys.push("subfund".into());
        }
        "share-class" => {
            keys.push("share".into());
        }
        "deal" => {}
        "custody" => {
            keys.push("account".into());
        }
        "billing" => {
            keys.push("fee".into());
        }
        _ => {
            // For unknown domains, split on hyphens and add segments
            for seg in domain.split('-') {
                if seg.len() >= 3 && !is_noise_word(seg) && seg != domain {
                    keys.push(seg.to_string());
                }
            }
        }
    }

    keys.sort();
    keys.dedup();
    keys
}

/// Map slot names to noun keys that users might say.
///
/// Slot names in constellation YAML are like:
///   `kyc_case`, `entity_workstream`, `screening`, `custody_account`, `ubo_graph`
///
/// Users say things like:
///   "case", "kyc", "workstream", "entity", "screening", "custody", "ubo"
///
/// We generate multiple noun keys per slot to maximize match surface.
fn slot_to_noun_keys(slot_name: &str) -> Vec<String> {
    let mut keys = Vec::new();

    // Full name normalized (underscores → hyphens)
    let full = slot_name.replace('_', "-");
    keys.push(full.clone());

    // Individual segments
    let segments: Vec<&str> = slot_name.split('_').collect();
    for seg in &segments {
        let s = seg.to_lowercase();
        if s.len() >= 3 && !is_noise_word(&s) {
            keys.push(s);
        }
    }

    // Known expansions: slot name → common user nouns.
    // These supplement the mechanical segment split with domain-specific synonyms.
    // Grouped by constellation family for maintainability.
    match slot_name {
        // --- CBU / Group ---
        "cbu" => {
            keys.push("cbu".into());
            keys.push("unit".into());
        }
        "client_group" => {
            keys.push("group".into());
            keys.push("client".into());
        }

        // --- KYC ---
        "kyc_case" | "group_kyc_clearance" => {
            keys.push("case".into());
            keys.push("kyc".into());
        }
        "kyc_agreement" => {
            keys.push("agreement".into());
            keys.push("kyc".into());
        }
        "entity_workstream" => {
            keys.push("workstream".into());
            keys.push("entity".into());
        }
        "screening" => {
            keys.push("screening".into());
            keys.push("sanctions".into());
            keys.push("pep".into());
        }
        "identifier" => {
            keys.push("identifier".into());
            keys.push("identity".into());
        }
        "request" => {
            keys.push("request".into());
            keys.push("requirement".into());
        }
        "tollgate" => {
            keys.push("gate".into());
            keys.push("tollgate".into());
        }

        // --- Ownership / UBO ---
        "ubo_graph" | "ownership_graph" | "ownership_chain" => {
            keys.push("ubo".into());
            keys.push("ownership".into());
            keys.push("control".into());
        }

        // --- Fund structure roles ---
        "management_company" => {
            keys.push("management".into());
            keys.push("manco".into());
            keys.push("management-company".into());
        }
        "depositary" => {
            keys.push("depositary".into());
            keys.push("custodian".into());
        }
        "investment_manager" => {
            keys.push("investment".into());
            keys.push("investment-manager".into());
        }
        "fund_administrator" | "fund_admin" => {
            keys.push("administrator".into());
            keys.push("fund-admin".into());
        }
        "transfer_agent" => {
            keys.push("transfer".into());
            keys.push("transfer-agent".into());
            keys.push("ta".into());
        }
        "auditor" => {
            keys.push("auditor".into());
        }

        // --- Trading / Custody / Deal ---
        "trading_profile" => {
            keys.push("trading".into());
            keys.push("profile".into());
            keys.push("mandate".into());
        }
        "custody_account" | "custody" => {
            keys.push("custody".into());
            keys.push("account".into());
        }
        "deal" => {
            keys.push("deal".into());
        }
        "mandate" => {
            keys.push("mandate".into());
            keys.push("trading".into());
        }

        // --- Fund structure ---
        "fund" | "fund_structure" => {
            keys.push("fund".into());
            keys.push("subfund".into());
            keys.push("umbrella".into());
        }
        "share_class" => {
            keys.push("share".into());
            keys.push("share-class".into());
        }
        "case" => {
            keys.push("case".into());
        }

        // --- Documents ---
        "document" | "documents" => {
            keys.push("document".into());
            keys.push("documents".into());
            keys.push("doc".into());
        }

        _ => {}
    }

    keys.sort();
    keys.dedup();
    keys
}

fn is_noise_word(s: &str) -> bool {
    matches!(s, "the" | "a" | "an" | "of" | "for" | "in" | "by" | "to")
}

fn normalize(s: &str) -> String {
    s.to_lowercase().replace('_', "-").trim().to_string()
}

/// Normalize action stems — map common synonyms to canonical form.
/// Mirrors the stem map in `compound_intent::extract_action_stem` but
/// only for the FQN action part (which is already fairly canonical).
fn normalize_action(s: &str) -> String {
    match s.to_lowercase().as_str() {
        // These match extract_action_stem's output vocabulary
        "show" | "display" | "view" | "get" | "read" => "read".into(),
        "list" => "list".into(),
        "create" | "open" | "start" | "begin" | "ensure" => "create".into(),
        "update" | "modify" | "change" | "edit" | "set" | "rename" | "assign" => "update".into(),
        "delete" | "remove" => "remove".into(),
        "run" | "execute" => "run".into(),
        "compute" | "calculate" | "derive" => "compute".into(),
        "check" | "verify" | "validate" => "check".into(),
        "search" | "find" | "query" | "identify" | "discover" => "search".into(),
        "import" | "load" | "fetch" | "sync" => "import".into(),
        "close" => "close".into(),
        "approve" | "publish" => "approve".into(),
        "reject" => "reject".into(),
        "solicit" | "request" | "chase" | "remind" => "solicit".into(),
        "trace" => "trace".into(),
        "escalate" => "escalate".into(),
        "link" => "link".into(),
        "record" => "record".into(),
        "mark" => "mark".into(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_slot(name: &str, state: &str, verbs: &[&str]) -> HydratedSlot {
        HydratedSlot {
            name: name.into(),
            path: name.into(),
            slot_type: crate::sem_os_runtime::constellation_runtime::HydratedSlotType::Entity,
            cardinality: HydratedCardinality::Mandatory,
            entity_id: Some(Uuid::new_v4()),
            record_id: None,
            computed_state: state.into(),
            effective_state: state.into(),
            progress: 50,
            blocking: false,
            warnings: vec![],
            overlays: vec![],
            graph_node_count: None,
            graph_edge_count: None,
            graph_nodes: vec![],
            graph_edges: vec![],
            available_verbs: verbs.iter().map(|s| s.to_string()).collect(),
            blocked_verbs: vec![],
            children: vec![],
        }
    }

    #[test]
    fn test_two_clue_lookup() {
        let slots = vec![
            make_slot("kyc_case", "open", &["kyc-case.read", "kyc-case.update-status", "kyc-case.close"]),
            make_slot("screening", "empty", &["screening.run", "screening.sanctions", "screening.pep"]),
        ];

        let idx = ConstellationVerbIndex::build(&slots);

        // Two-clue: noun=case, action=close → kyc-case.close
        let matches = idx.lookup("case", "close");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].verb_fqn, "kyc-case.close");

        // Two-clue: noun=screening, action=run → screening.run + screening.sanctions + screening.pep
        // (all three have action stems that normalize to "run")
        let matches = idx.lookup("screening", "run");
        assert!(matches.iter().any(|m| m.verb_fqn == "screening.run"));
    }

    #[test]
    fn test_by_noun_lookup() {
        let slots = vec![
            make_slot("kyc_case", "open", &["kyc-case.read", "kyc-case.close"]),
        ];
        let idx = ConstellationVerbIndex::build(&slots);

        let verbs = idx.lookup_by_noun("case");
        assert!(verbs.contains(&"kyc-case.read".to_string()));
        assert!(verbs.contains(&"kyc-case.close".to_string()));
    }

    #[test]
    fn test_by_action_lookup() {
        let slots = vec![
            make_slot("kyc_case", "open", &["kyc-case.close"]),
            make_slot("deal", "negotiating", &["deal.close"]),
        ];
        let idx = ConstellationVerbIndex::build(&slots);

        let verbs = idx.lookup_by_action("close");
        assert!(verbs.contains(&"kyc-case.close".to_string()));
        assert!(verbs.contains(&"deal.close".to_string()));
    }

    #[test]
    fn test_reverse_lookup() {
        let slots = vec![
            make_slot("kyc_case", "open", &["kyc-case.read"]),
        ];
        let idx = ConstellationVerbIndex::build(&slots);

        let contexts = idx.slot_contexts("kyc-case.read");
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].slot_name, "kyc_case");
        assert_eq!(contexts[0].effective_state, "open");
    }

    #[test]
    fn test_children_flattened() {
        let mut parent = make_slot("kyc_case", "open", &["kyc-case.read"]);
        parent.children = vec![
            make_slot("tollgate", "empty", &["tollgate.evaluate"]),
        ];

        let idx = ConstellationVerbIndex::build(&[parent]);
        assert_eq!(idx.stats().total_slots, 2);

        let matches = idx.lookup("tollgate", "evaluate");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].verb_fqn, "tollgate.evaluate");
    }

    #[test]
    fn test_action_normalization() {
        assert_eq!(extract_action_from_fqn("kyc-case.update-status"), "update");
        assert_eq!(extract_action_from_fqn("screening.run"), "run");
        assert_eq!(extract_action_from_fqn("entity.ensure"), "create");
        assert_eq!(extract_action_from_fqn("ubo.compute-chains"), "compute");
        assert_eq!(extract_action_from_fqn("document.solicit"), "solicit");
        assert_eq!(extract_action_from_fqn("cbu.rename"), "update");
    }

    #[test]
    fn test_synonym_normalization() {
        // "show" and "read" should both resolve to "read"
        let slots = vec![
            make_slot("kyc_case", "open", &["kyc-case.read"]),
        ];
        let idx = ConstellationVerbIndex::build(&slots);

        // lookup with "show" should find verbs whose action normalizes to "read"
        let matches = idx.lookup("case", "show");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].verb_fqn, "kyc-case.read");
    }

    #[test]
    fn test_empty_constellation() {
        let idx = ConstellationVerbIndex::build(&[]);
        assert_eq!(idx.stats().total_slots, 0);
        assert_eq!(idx.stats().distinct_verbs, 0);
        assert!(idx.all_verbs().is_empty());
        assert!(idx.lookup("anything", "create").is_empty());
    }

    #[test]
    fn test_dump_does_not_panic() {
        let slots = vec![
            make_slot("kyc_case", "open", &["kyc-case.read", "kyc-case.close"]),
            make_slot("screening", "empty", &["screening.run"]),
        ];
        let idx = ConstellationVerbIndex::build(&slots);
        let dump = idx.dump();
        assert!(dump.contains("kyc-case.read"));
        assert!(dump.contains("screening.run"));
    }

    #[test]
    fn test_stats() {
        let slots = vec![
            make_slot("kyc_case", "open", &["kyc-case.read", "kyc-case.close"]),
            make_slot("screening", "empty", &["screening.run", "screening.pep"]),
        ];
        let idx = ConstellationVerbIndex::build(&slots);

        assert_eq!(idx.stats().total_slots, 2);
        assert_eq!(idx.stats().total_available_verbs, 4);
        assert_eq!(idx.stats().distinct_verbs, 4);
    }

    // -----------------------------------------------------------------------
    // Diagnostic: real KYC onboarding constellation at mid-lifecycle
    // -----------------------------------------------------------------------
    // Run with: cargo test --lib agent::constellation_verb_index::tests::diag_kyc_onboarding -- --nocapture

    #[test]
    fn diag_kyc_onboarding() {
        // Simulate KYC onboarding constellation at mid-lifecycle:
        // - CBU exists (filled)
        // - KYC case open, entity workstream in documents_requested
        // - Screening not yet started
        // - Tollgate not yet evaluated
        // - Identifier captured, request pending

        let mut kyc_case = make_slot(
            "kyc_case",
            "open",
            &[
                "kyc-case.read",
                "kyc-case.list-by-cbu",
                "kyc-case.summarize",
                "kyc-case.assign",
                "kyc-case.update-status",
                "kyc-case.set-risk-rating",
                "kyc-case.close",
                "kyc-case.reopen",
                "kyc-case.escalate",
            ],
        );
        kyc_case.children = vec![make_slot(
            "tollgate",
            "empty",
            &[
                "tollgate.evaluate",
                "tollgate.check-gate",
            ],
        )];

        let entity_ws = make_slot(
            "entity_workstream",
            "documents_requested",
            &[
                "entity-workstream.read",
                "entity-workstream.list-by-case",
                "entity-workstream.state",
                "entity-workstream.update-status",
                "entity-workstream.escalate-dd",
                "red-flag.raise",
                "red-flag.read",
                "red-flag.list",
                "red-flag.update",
                "requirement.check",
                "requirement.list",
                "requirement.list-for-entity",
                "requirement.list-outstanding",
                "requirement.waive",
                "document.solicit",
                "document.solicit-batch",
                "document.upload",
                "document.read",
                "document.list",
                "document.compute-requirements",
            ],
        );

        let screening = make_slot(
            "screening",
            "empty",
            &[
                "screening.run",
                "screening.sanctions",
                "screening.pep",
                "screening.adverse-media",
            ],
        );

        let identifier = make_slot(
            "identifier",
            "captured",
            &[
                "identifier.read",
                "identifier.list",
                "identifier.verify",
                "identifier.expire",
                "identifier.update",
                "identifier.search",
                "identifier.resolve",
                "identifier.list-by-type",
            ],
        );

        let request = make_slot(
            "request",
            "pending",
            &[
                "request.create",
                "request.read",
                "request.list",
                "request.assign",
                "request.cancel",
            ],
        );

        let cbu = make_slot("cbu", "filled", &["cbu.inspect"]);

        let slots = vec![cbu, kyc_case, entity_ws, screening, identifier, request];
        let idx = ConstellationVerbIndex::build(&slots);

        println!("\n{}", idx.dump());
        println!("--- Sample two-clue lookups ---");

        let queries = [
            ("case", "close"),
            ("case", "read"),
            ("screening", "run"),
            ("screening", "check"),
            ("document", "solicit"),
            ("document", "upload"),
            ("entity", "update"),
            ("workstream", "read"),
            ("identifier", "verify"),
            ("requirement", "check"),
            ("tollgate", "evaluate"),
            ("kyc", "escalate"),
            ("pep", "run"),
            ("sanctions", "check"),
            ("flag", "create"),   // will the stem "create" → "raise"? No — but "raise" isn't in stems
            ("ubo", "trace"),     // not in this constellation — should be empty
        ];

        for (noun, action) in &queries {
            let matches = idx.lookup(noun, action);
            let verbs: Vec<_> = matches.iter().map(|m| m.verb_fqn.as_str()).collect();
            println!(
                "  ({:>15}, {:>10}) → {}",
                noun,
                action,
                if verbs.is_empty() {
                    "∅ (no match)".to_string()
                } else {
                    format!("[{}]", verbs.join(", "))
                }
            );
        }
    }

    // Diagnostic: Lux UCITS SICAV fund structure constellation
    // Run with: cargo test --lib agent::constellation_verb_index::tests::diag_lux_sicav -- --nocapture

    #[test]
    fn diag_lux_sicav() {
        // Simulate Lux UCITS SICAV at early lifecycle:
        // - CBU created, roles being assigned (placeholders)
        let cbu = make_slot("cbu", "filled", &[
            "cbu.create", "cbu.read", "cbu.inspect", "cbu.parties",
        ]);
        let manco = make_slot("management_company", "placeholder", &[
            "cbu.assign-role", "party.search", "entity.resolve-placeholder",
        ]);
        let depositary = make_slot("depositary", "empty", &[
            "entity.ensure-or-placeholder", "party.add",
        ]);
        let investment_mgr = make_slot("investment_manager", "empty", &[
            "entity.ensure-or-placeholder", "entity.resolve-placeholder",
        ]);
        let ownership = make_slot("ownership_chain", "empty", &[
            "ubo.discover", "ubo.allege",
        ]);
        let case = make_slot("case", "intake", &[
            "case.open", "case.submit",
        ]);
        let mandate = make_slot("mandate", "empty", &[
            "mandate.create",
        ]);

        let slots = vec![cbu, manco, depositary, investment_mgr, ownership, case, mandate];
        let idx = ConstellationVerbIndex::build(&slots);

        println!("\n{}", idx.dump());
        println!("--- Sample two-clue lookups ---");

        let queries = [
            ("management", "assign"),
            ("manco", "assign"),
            ("depositary", "create"),
            ("depositary", "add"),
            ("ubo", "trace"),        // "discover" maps to "search", not "trace"
            ("ubo", "search"),
            ("ownership", "compute"),
            ("case", "create"),
            ("mandate", "create"),
            ("cbu", "read"),
            ("custodian", "create"),  // synonym for depositary
        ];

        for (noun, action) in &queries {
            let matches = idx.lookup(noun, action);
            let verbs: Vec<_> = matches.iter().map(|m| m.verb_fqn.as_str()).collect();
            println!(
                "  ({:>15}, {:>10}) → {}",
                noun,
                action,
                if verbs.is_empty() {
                    "∅ (no match)".to_string()
                } else {
                    format!("[{}]", verbs.join(", "))
                }
            );
        }
    }
}
