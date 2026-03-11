# Entity Name Extraction from Utterances

## Problem

`try_semtaxonomy_path` currently passes the full utterance to `discovery.search-entities`:

```rust
("query", serde_json::json!(request.message.clone()))
// "show me the deals for Allianz Global Investors"
// becomes ILIKE '%show me the deals for Allianz Global Investors%'
// which matches nothing
```

The entity name must be extracted before the search fires.

## Design

### Principle: subtract the known, search the residual

The utterance is composed of:
1. **Intent words** — show, list, create, add, what, who, which, run, check, set up, etc.
2. **Domain nouns** — deals, CBUs, documents, screening, onboarding, ownership, fund, etc.
3. **Connective tissue** — for, the, a, an, on, of, in, this, that, me, my, all, new, etc.
4. **Entity names** — Allianz Global Investors, BNP Paribas SA, Deutsche Bank AG, etc.

Category 4 is what we want. Categories 1-3 are known vocabulary. Strip 1-3, what remains is the candidate entity name(s).

### Implementation

```rust
/// Extract candidate entity name(s) from a raw utterance.
/// Returns one or more candidate strings, ranked by likelihood.
/// Returns empty vec if no entity name can be extracted.
pub fn extract_entity_candidates(utterance: &str) -> Vec<String> {
    // ... implementation below
}
```

### Step 1: Build the stop vocabulary (compile once, reuse)

Three sets, all lowercased:

**Intent words** (from existing classifiers in semtaxonomy/mod.rs):
```
show, list, what, which, read, view, inspect, describe, display,
create, add, update, change, delete, remove, assign, set, open,
who, how, can, do, does, is, are, was, were, will, would, could,
run, check, tell, find, get, give, look, search, help,
move, progress, forward, next, start, begin, continue, resume,
verify, validate, review, approve, reject, close, complete,
need, want, like, should, must, try, able
```

**Domain nouns** (from noun_index.yaml + domain vocabulary):
```
cbu, cbus, deal, deals, document, documents, doc, docs,
entity, entities, onboarding, screening, sanctions, pep,
ownership, ubo, beneficial, owner, owners, fund, funds,
subfund, subfunds, share, class, umbrella, structure,
relationship, relationships, graph, party, parties,
group, groups, client, clients, mandate, mandates,
kyc, aml, compliance, case, cases, evidence, rate, card,
adverse, media, workstream, checklist
```

**Connective tissue:**
```
the, a, an, for, of, on, in, at, to, by, with, from,
this, that, these, those, it, its, them, their,
me, my, i, we, our, us, you, your,
all, every, each, some, any, no,
new, current, existing, active, pending, missing, available,
up, out, about, into, around, through, between, against,
and, or, but, if, then, so, yet, not, also, just, only,
please, ok, sure, yes, hey, hi, right, actually,
what's, who's, how's, where's, there's, let's, don't, doesn't, isn't, aren't,
whats, whos, hows
```

All three sets are static. Build a `HashSet<&str>` at startup.

### Step 2: Tokenise and strip

```rust
fn extract_entity_candidates(utterance: &str) -> Vec<String> {
    let stop_words: &HashSet<&str> = &STOP_VOCABULARY; // lazy_static or OnceCell
    
    // Tokenise on whitespace, preserving original case
    let tokens: Vec<&str> = utterance.split_whitespace().collect();
    
    // Mark each token as stop or candidate
    let mut is_candidate: Vec<bool> = tokens.iter()
        .map(|token| {
            let lower = token.to_ascii_lowercase();
            let cleaned = lower.trim_matches(|c: char| !c.is_alphanumeric());
            !stop_words.contains(cleaned.as_str()) && cleaned.len() > 1
        })
        .collect();
    
    // ... extract contiguous runs of candidate tokens
}
```

### Step 3: Extract contiguous candidate runs

After stripping, candidate entity names are contiguous runs of non-stop tokens:

```
"show me the deals for Allianz Global Investors"
 stop stop stop stop stop CAND   CAND   CAND
                          ^^^^^^^^^^^^^^^^^^^^^^^^
                          → "Allianz Global Investors"

"create a new CBU for BlackRock Luxembourg SICAV"
 stop  stop stop stop stop CAND     CAND       CAND
                           ^^^^^^^^^^^^^^^^^^^^^^^^^
                           → "BlackRock Luxembourg SICAV"

"who owns BNP Paribas"
 stop stop CAND CAND
           ^^^^^^^^^^^^
           → "BNP Paribas"

"what documents are missing for the Vanguard fund"
 stop stop      stop stop   stop stop CAND    stop
                                      ^^^^^^^^
                                      → "Vanguard"

"run sanctions screening on Deutsche Bank AG"
 stop stop      stop      stop CAND    CAND CAND
                               ^^^^^^^^^^^^^^^^^^^^
                               → "Deutsche Bank AG"
```

Collect contiguous runs of candidate tokens, preserving original case:

```rust
    // Collect contiguous runs of candidate tokens
    let mut runs: Vec<String> = Vec::new();
    let mut current_run: Vec<&str> = Vec::new();
    
    for (i, token) in tokens.iter().enumerate() {
        if is_candidate[i] {
            current_run.push(token);
        } else {
            if !current_run.is_empty() {
                runs.push(current_run.join(" "));
                current_run.clear();
            }
        }
    }
    if !current_run.is_empty() {
        runs.push(current_run.join(" "));
    }
    
    // Strip trailing punctuation from each run
    runs.iter_mut().for_each(|run| {
        *run = run.trim_matches(|c: char| !c.is_alphanumeric() && c != '.').to_string();
    });
    
    // Filter out empty/trivial runs
    runs.retain(|run| run.len() > 1);
    
    runs
```

### Step 4: Rank candidates

If multiple runs are extracted, rank by:
1. **Runs following positional markers** ("for", "called", "named", "on") rank highest — these are explicit entity references.
2. **Capitalised runs** rank above lowercase — proper nouns are more likely entity names.
3. **Longer runs** rank above shorter — "Allianz Global Investors" is more specific than "Allianz".
4. **Runs not matching any verb name or domain** rank above those that do — a run that coincidentally contains a verb fragment is less likely to be an entity name.

```rust
    // Boost runs that follow positional markers
    let marker_positions: Vec<usize> = tokens.iter().enumerate()
        .filter(|(_, t)| ["for", "called", "named", "on", "about", "regarding"]
            .contains(&t.to_ascii_lowercase().as_str()))
        .map(|(i, _)| i)
        .collect();
    
    runs.sort_by(|a, b| {
        let a_after_marker = marker_positions.iter()
            .any(|&pos| tokens.get(pos + 1)
                .map(|t| a.starts_with(t))
                .unwrap_or(false));
        let b_after_marker = marker_positions.iter()
            .any(|&pos| tokens.get(pos + 1)
                .map(|t| b.starts_with(t))
                .unwrap_or(false));
        
        b_after_marker.cmp(&a_after_marker)
            .then(b.len().cmp(&a.len()))
    });
    
    runs
```

### Step 5: Use in try_semtaxonomy_path

```rust
// BEFORE (current)
let search = self.run_discovery_op(
    "discovery", "search-entities",
    vec![("query", json!(request.message.clone()))],
).await?;

// AFTER
let entity_names = extract_entity_candidates(&request.message);
let search_query = entity_names.first()
    .cloned()
    .unwrap_or_else(|| request.message.clone()); // fallback to full utterance

let search = self.run_discovery_op(
    "discovery", "search-entities",
    vec![("query", json!(search_query))],
).await?;

// If first candidate returns no results, try subsequent candidates
// Also: try the full utterance as fallback if extraction yields nothing useful
```

### Edge Cases

**No entity name found:**
```
"what can I do next?"
→ all tokens are stop words
→ empty candidate list
→ fall back to session's active_entity or return options/state response
```

**Multiple entity names:**
```
"show me the relationship between Allianz and Deutsche Bank"
→ two runs: "Allianz", "Deutsche Bank"
→ search both, return candidates from both
→ composition can reason about multi-entity queries
```

**Entity name contains a domain noun:**
```
"show me BlackRock Fund Services"
→ "Fund" is in domain nouns, "Services" is not
→ naive stripping gives: "BlackRock" + "Services" (two runs)
→ FIX: before splitting into runs, check if a stop-word token is 
  sandwiched between two candidate tokens with a capital letter
  → "BlackRock [Fund] Services" → preserve as one run if Fund is
  capitalised and adjacent candidates are capitalised
```

This is the main edge case. Proper-noun-aware run merging:

```rust
    // Merge runs separated by a single capitalised stop word
    // "BlackRock Fund Services" → "BlackRock Fund Services" (not "BlackRock" + "Services")
    // Only if the stop word starts with uppercase in the original
    let mut merged_runs: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if is_candidate[i] {
            let mut run_tokens = vec![tokens[i]];
            let mut j = i + 1;
            while j < tokens.len() {
                if is_candidate[j] {
                    run_tokens.push(tokens[j]);
                    j += 1;
                } else if j + 1 < tokens.len()
                    && is_candidate[j + 1]
                    && tokens[j].chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                {
                    // Stop word is capitalised and followed by another candidate
                    // → likely part of an entity name
                    run_tokens.push(tokens[j]);
                    run_tokens.push(tokens[j + 1]);
                    j += 2;
                } else {
                    break;
                }
            }
            merged_runs.push(run_tokens.join(" "));
            i = j;
        } else {
            i += 1;
        }
    }
```

**Possessives:**
```
"show me BNP's deals"
→ "BNP's" → strip trailing 's → "BNP"
```

```rust
    // Strip possessive suffixes
    runs.iter_mut().for_each(|run| {
        if run.ends_with("'s") || run.ends_with("'s") {
            run.truncate(run.len() - 2);
        }
    });
```

## Integration with cascade-research

The same extraction should feed `cascade-research` when it fires:

```rust
let entity_names = extract_entity_candidates(&request.message);
let search_query = entity_names.first()
    .cloned()
    .unwrap_or_else(|| request.message.clone());

// Use for both search-entities and cascade-research
if entity_candidates.len() > 1 || domain_scope.is_none() {
    if let Ok(cascade) = self.run_discovery_op(
        "discovery", "cascade-research",
        vec![
            ("query", json!(search_query)),  // extracted name, not full utterance
            ("top-n", json!(3)),
            ("include-relationships", json!(true)),
        ],
    ).await { ... }
}
```

## Multi-candidate search

When extraction produces multiple candidates ("show me the relationship between Allianz and Deutsche Bank"), search each independently and merge results:

```rust
let entity_names = extract_entity_candidates(&request.message);

let mut all_candidates = Vec::new();
for name in &entity_names {
    let search = self.run_discovery_op(
        "discovery", "search-entities",
        vec![("query", json!(name))],
    ).await?;
    all_candidates.extend(Self::parse_entity_candidates(&search));
}
// Deduplicate by entity_id
all_candidates.sort_by(|a, b| b.match_score.partial_cmp(&a.match_score).unwrap_or(std::cmp::Ordering::Equal));
all_candidates.dedup_by(|a, b| a.entity_id == b.entity_id);
```

## Testing

Add unit tests for extraction:

```rust
#[test]
fn extract_simple_entity_after_for() {
    let result = extract_entity_candidates("show me the deals for Allianz");
    assert_eq!(result, vec!["Allianz"]);
}

#[test]
fn extract_multi_word_entity() {
    let result = extract_entity_candidates("create a CBU for Allianz Global Investors");
    assert_eq!(result, vec!["Allianz Global Investors"]);
}

#[test]
fn extract_entity_at_end() {
    let result = extract_entity_candidates("who owns BNP Paribas");
    assert_eq!(result, vec!["BNP Paribas"]);
}

#[test]
fn extract_entity_with_ag_suffix() {
    let result = extract_entity_candidates("run screening on Deutsche Bank AG");
    assert_eq!(result, vec!["Deutsche Bank AG"]);
}

#[test]
fn extract_no_entity() {
    let result = extract_entity_candidates("what can I do next");
    assert!(result.is_empty());
}

#[test]
fn extract_multiple_entities() {
    let result = extract_entity_candidates("show relationship between Allianz and Deutsche Bank");
    assert!(result.contains(&"Allianz".to_string()));
    assert!(result.contains(&"Deutsche Bank".to_string()));
}

#[test]
fn extract_entity_with_domain_noun_inside() {
    let result = extract_entity_candidates("show me BlackRock Fund Services");
    assert_eq!(result, vec!["BlackRock Fund Services"]);
}

#[test]
fn extract_possessive() {
    let result = extract_entity_candidates("show me BNP's deals");
    assert_eq!(result, vec!["BNP"]);
}

#[test]
fn extract_entity_from_natural_language() {
    let result = extract_entity_candidates("I need to onboard Vanguard as a new client");
    assert_eq!(result, vec!["Vanguard"]);
}
```

## Performance

This is pure string manipulation — no DB, no network, no LLM. Should be sub-microsecond. The stop vocabulary HashSet is built once at startup.

## What this fixes

From the review: "show me the deals for Allianz Global Investors" currently searches `%show me the deals for Allianz Global Investors%` via ILIKE and matches nothing. After this change, it searches `%Allianz Global Investors%` and gets a hit.

This is likely the single highest-impact fix for the no-proposal failure class. The entity search was never broken — it just never received a searchable input.
