# Entity Name Extraction Plan

## Objective

Stop sending full user utterances into `discovery.search-entities`.

Instead:

1. extract candidate entity names from the utterance
2. search each candidate independently
3. merge and rank the results
4. fall back to the full utterance only when extraction yields nothing

## Scope

- add deterministic extraction to `rust/src/semtaxonomy/mod.rs`
- integrate it into `try_semtaxonomy_path()` in `rust/src/api/agent_service.rs`
- use the same extracted query for:
  - `discovery.search-entities`
  - `discovery.cascade-research`
- add focused unit tests for extraction

## Implementation

### E1. Stop Vocabulary

Build static lowercased stop sets for:

- intent words
- domain nouns
- connective tissue

Use a `OnceLock<HashSet<&'static str>>`.

### E2. Candidate Extraction

Add:

- `pub fn extract_entity_candidates(utterance: &str) -> Vec<String>`

Rules:

- preserve original token case in output
- strip stop words
- collect contiguous candidate runs
- support possessives
- merge runs across a single capitalized stop word
- return ranked candidates

### E3. Search Integration

In `try_semtaxonomy_path()`:

- extract candidate names first
- query `discovery.search-entities` for each candidate
- merge and deduplicate by `entity_id`
- use the top candidate string for `cascade-research`
- fall back to full utterance only if no candidates are extracted

### E4. Tests

Add unit tests for:

- `show me the deals for Allianz`
- `create a CBU for Allianz Global Investors`
- `who owns BNP Paribas`
- `run screening on Deutsche Bank AG`
- no-entity utterance
- multi-entity utterance
- capitalized domain noun inside entity name
- possessive form
- natural-language onboarding request

## Gate

- `cargo check -p ob-poc`
- focused extraction tests compile and pass

