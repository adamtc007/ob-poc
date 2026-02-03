//! Utterance Segmentation
//!
//! Explicit, deterministic, non-lossy segmentation of user input.
//! Every token belongs to exactly one segment OR ends up in `residual_terms`.
//!
//! ## Pipeline Position
//!
//! ```text
//! User Input
//!     ↓
//! segment_utterance()  ← THIS MODULE
//!     ↓
//! ├── group_phrase → ScopeResolver (sets client_group_id)
//! ├── verb_phrase  → HybridVerbSearcher (verb discovery)
//! └── scope_phrase → Entity search (search_entity_tags)
//! ```
//!
//! ## Algorithm (4-Pass Greedy Extraction)
//!
//! 1. **Pass 0**: Normalize & tokenize (preserve quotes, track spans)
//! 2. **Pass 1**: Group phrase extraction (highest priority)
//! 3. **Pass 2**: Verb phrase extraction (constrained window)
//! 4. **Pass 3**: Scope phrase extraction (remaining entity descriptors)
//! 5. **Pass 4**: Residual terms (filters, parameters)
//!
//! ## Key Invariant
//!
//! Verb phrase must NOT consume tokens that are entity descriptors
//! (jurisdiction codes, fund types, company suffixes, etc.)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// CONTRACT TYPES
// =============================================================================

/// Segmented user utterance with confidence scores and span tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtteranceSegmentation {
    /// Original input (preserved for audit)
    pub original: String,
    /// Normalized text (lowercase, collapsed whitespace)
    pub normalized: String,
    /// Tokenized with offsets
    pub tokens: Vec<Token>,
    /// Verb/action phrase (REQUIRED - may be low confidence)
    pub verb_phrase: Segment,
    /// Client group anchor (optional)
    pub group_phrase: Option<Segment>,
    /// Entity scope descriptor (optional)
    pub scope_phrase: Option<Segment>,
    /// Remaining unconsumed tokens
    pub residual_terms: Vec<Segment>,
    /// Debug/learning trace of segmentation steps
    pub method_trace: Vec<SegStep>,
}

/// A segment with text, span, confidence, and method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// The extracted text
    pub text: String,
    /// Byte offsets into normalized string (start, end)
    pub span: Span,
    /// Match confidence (0.0 - 1.0)
    pub confidence: f32,
    /// How this segment was identified
    pub method: SegmentMethod,
}

/// Byte span in the normalized string
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A token with text, span, and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// The token text
    pub text: String,
    /// Byte span in normalized string
    pub span: Span,
    /// Whether this token was quoted in the original
    pub is_quoted: bool,
    /// Token index (for tracking consumption)
    pub index: usize,
}

/// How a segment was identified
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SegmentMethod {
    /// Exact string match against lexicon
    ExactMatch,
    /// Matched via alias table (trigram/phonetic)
    AliasMatch,
    /// Matched verb invocation phrase
    VerbLexicon,
    /// Pattern-based extraction (heuristic)
    Heuristic,
    /// Default/residual assignment
    Fallback,
    /// No match found
    NoMatch,
}

/// A step in the segmentation trace (for debugging/learning)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegStep {
    /// Which pass (0-4)
    pub pass: u8,
    /// Description of action taken
    pub action: String,
    /// Span consumed (if any)
    pub consumed_span: Option<Span>,
    /// Confidence of this step
    pub confidence: Option<f32>,
}

// =============================================================================
// ENTITY DESCRIPTOR GUARDRAILS
// =============================================================================

/// Jurisdiction codes that indicate entity context (must not be eaten by verb)
const JURISDICTION_CODES: &[&str] = &[
    "ie",
    "irish",
    "ireland",
    "lu",
    "lux",
    "luxembourg",
    "uk",
    "british",
    "england",
    "us",
    "usa",
    "american",
    "de",
    "german",
    "germany",
    "fr",
    "french",
    "france",
    "ch",
    "swiss",
    "switzerland",
    "nl",
    "dutch",
    "netherlands",
    "be",
    "belgian",
    "belgium",
    "at",
    "austrian",
    "austria",
    "es",
    "spanish",
    "spain",
    "it",
    "italian",
    "italy",
    "pt",
    "portuguese",
    "portugal",
    "sg",
    "singapore",
    "hk",
    "hong kong",
    "jp",
    "japan",
    "japanese",
    "au",
    "australian",
    "australia",
    "ca",
    "canadian",
    "canada",
];

/// Fund/instrument types that indicate entity context
const INSTRUMENT_TYPES: &[&str] = &[
    "fund",
    "funds",
    "etf",
    "etfs",
    "sicav",
    "sicavs",
    "ucits",
    "aif",
    "aifs",
    "spv",
    "spvs",
    "subfund",
    "subfunds",
    "umbrella",
    "feeder",
    "master",
    "manco",
    "cbu",
    "cbus",
    "portfolio",
    "portfolios",
    "mandate",
    "mandates",
];

/// Company suffixes that indicate entity names
const COMPANY_SUFFIXES: &[&str] = &[
    "ltd",
    "limited",
    "plc",
    "inc",
    "incorporated",
    "corp",
    "corporation",
    "llc",
    "gmbh",
    "ag",
    "sa",
    "sarl",
    "bv",
    "nv",
];

/// Check if a token is an entity descriptor (should not be consumed by verb phrase)
fn is_entity_descriptor(token: &str) -> bool {
    let lower = token.to_lowercase();
    JURISDICTION_CODES.contains(&lower.as_str())
        || INSTRUMENT_TYPES.contains(&lower.as_str())
        || COMPANY_SUFFIXES.contains(&lower.as_str())
}

// =============================================================================
// GROUP PHRASE MARKERS
// =============================================================================

/// Explicit prefixes that introduce a group phrase
const GROUP_PREFIXES: &[&str] = &[
    "for ",
    "within ",
    "in ",
    "client ",
    "set client to ",
    "set client ",
    "work on ",
    "working on ",
    "switch to ",
];

/// Group prefixes that are ALSO verb phrases (maps prefix -> verb pattern)
/// When these prefixes are used, the verb is implicitly known
const VERB_GROUP_PREFIXES: &[(&str, &str)] = &[
    ("work on ", "work on"),
    ("working on ", "working on"),
    ("switch to ", "switch to"),
    ("set client to ", "set client to"),
    ("set client ", "set client"),
];

/// Check if input starts with a group prefix, return (prefix, remainder)
fn extract_group_prefix(input: &str) -> Option<(&str, &str)> {
    let lower = input.to_lowercase();
    for prefix in GROUP_PREFIXES {
        if lower.starts_with(prefix) {
            return Some((prefix, &input[prefix.len()..]));
        }
    }
    None
}

/// Check if a group prefix implies a verb pattern
fn get_implied_verb_for_prefix(prefix: &str) -> Option<&'static str> {
    let prefix_lower = prefix.to_lowercase();
    for (group_prefix, verb_pattern) in VERB_GROUP_PREFIXES {
        if prefix_lower == *group_prefix {
            return Some(verb_pattern);
        }
    }
    None
}

// =============================================================================
// VERB PHRASE PATTERNS
// =============================================================================

/// Known verb phrases (most specific first) with their canonical verb
const VERB_PATTERNS: &[(&str, &str)] = &[
    // Session/scope
    ("set session to", "session.load-cluster"),
    ("set session", "session.load-cluster"),
    ("set client to", "session.load-cluster"),
    ("set client", "session.load-cluster"),
    ("switch to", "session.load-cluster"),
    ("work on", "session.load-cluster"),
    ("working on", "session.load-cluster"),
    // Loading
    ("load the", "session.load-galaxy"),
    ("load", "session.load-galaxy"),
    // Showing/listing
    ("show me the", "view.universe"),
    ("show me", "view.cbu"),
    ("show", "view.cbu"),
    ("list all", "cbu.list"),
    ("list", "cbu.list"),
    // Navigation
    ("drill into", "view.drill"),
    ("drill down", "view.drill"),
    ("drill", "view.drill"),
    ("zoom into", "view.drill"),
    ("zoom in", "view.drill"),
    ("zoom out", "view.surface"),
    ("surface back", "view.surface"),
    ("surface", "view.surface"),
    ("go back", "session.undo"),
    ("undo", "session.undo"),
    ("redo", "session.redo"),
    // UBO/Control
    ("trace ubo chain", "ubo.trace-chain"),
    ("trace ubo", "ubo.trace-chain"),
    ("trace chain", "ubo.trace-chain"),
    ("trace", "ubo.trace-chain"),
    ("find ubos", "control.identify-ubos"),
    ("find ubo", "control.identify-ubos"),
    ("discover ubos", "control.identify-ubos"),
    ("discover ubo", "control.identify-ubos"),
    ("who owns", "control.build-graph"),
    ("who controls", "control.build-graph"),
    // Creation
    ("create a", "cbu.create"),
    ("create", "cbu.create"),
    ("spin up a", "cbu.create"),
    ("spin up", "cbu.create"),
    ("add a", "entity.create"),
    ("add", "entity.create"),
    // Search/find
    ("find", "entity.search"),
    ("search for", "entity.search"),
    ("search", "entity.search"),
    ("lookup", "gleif.search"),
    ("look up", "gleif.search"),
];

/// Maximum tokens to consider for verb phrase (constrained window)
const VERB_WINDOW_SIZE: usize = 5;

// =============================================================================
// PASS 0: NORMALIZE & TOKENIZE
// =============================================================================

/// Normalize input: lowercase, collapse whitespace, preserve structure
fn normalize(input: &str) -> String {
    // split_whitespace already trims and handles multiple spaces
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Tokenize normalized input, tracking spans and quoted phrases
fn tokenize(normalized: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut in_quote = false;
    let mut quote_start = 0;
    let mut current_start = 0;
    let mut index = 0;

    let chars: Vec<char> = normalized.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '"' {
            if in_quote {
                // End of quoted phrase
                let text: String = chars[quote_start + 1..i].iter().collect();
                if !text.is_empty() {
                    tokens.push(Token {
                        text,
                        span: Span::new(quote_start, i + 1),
                        is_quoted: true,
                        index,
                    });
                    index += 1;
                }
                in_quote = false;
                current_start = i + 1;
            } else {
                // Start of quoted phrase - flush any pending token
                if current_start < i {
                    let pending: String = chars[current_start..i].iter().collect();
                    for word in pending.split_whitespace() {
                        let word_start = normalized[current_start..]
                            .find(word)
                            .map(|p| current_start + p)
                            .unwrap_or(current_start);
                        tokens.push(Token {
                            text: word.to_string(),
                            span: Span::new(word_start, word_start + word.len()),
                            is_quoted: false,
                            index,
                        });
                        index += 1;
                    }
                }
                in_quote = true;
                quote_start = i;
            }
        }
        i += 1;
    }

    // Handle remaining text
    if !in_quote && current_start < normalized.len() {
        let remaining = &normalized[current_start..];
        let mut pos = current_start;
        for word in remaining.split_whitespace() {
            let word_start = remaining[pos - current_start..]
                .find(word)
                .map(|p| pos + p)
                .unwrap_or(pos);
            tokens.push(Token {
                text: word.to_string(),
                span: Span::new(word_start, word_start + word.len()),
                is_quoted: false,
                index,
            });
            index += 1;
            pos = word_start + word.len();
        }
    }

    tokens
}

// =============================================================================
// SEGMENTATION ENGINE
// =============================================================================

impl UtteranceSegmentation {
    /// Create a default segmentation with no matches
    pub fn empty(original: &str) -> Self {
        let normalized = normalize(original);
        let tokens = tokenize(&normalized);
        Self {
            original: original.to_string(),
            normalized,
            tokens,
            verb_phrase: Segment {
                text: String::new(),
                span: Span::new(0, 0),
                confidence: 0.0,
                method: SegmentMethod::NoMatch,
            },
            group_phrase: None,
            scope_phrase: None,
            residual_terms: vec![],
            method_trace: vec![],
        }
    }

    /// Check if input is likely garbage (nothing meaningful extracted)
    pub fn is_likely_garbage(&self) -> bool {
        // Too short
        if self.original.trim().len() < 2 {
            return true;
        }

        // No meaningful segments
        let has_verb = self.verb_phrase.confidence >= 0.3;
        let has_group = self
            .group_phrase
            .as_ref()
            .is_some_and(|g| g.confidence >= 0.5);
        let has_scope = self.scope_phrase.is_some();

        !has_verb && !has_group && !has_scope
    }

    /// Check if input is likely a typo (group resolved but verb weak)
    ///
    /// Thresholds:
    /// - group_resolved: confidence >= 0.6 (fuzzy match threshold)
    /// - verb_weak: confidence < 0.5 (no verb pattern match)
    pub fn is_likely_typo(&self) -> bool {
        let group_resolved = self
            .group_phrase
            .as_ref()
            .is_some_and(|g| g.confidence >= 0.6);
        let verb_weak = self.verb_phrase.confidence < 0.5;

        group_resolved && verb_weak
    }

    /// Get the extracted verb for downstream processing
    pub fn get_verb_text(&self) -> &str {
        &self.verb_phrase.text
    }

    /// Get the extracted group name for scope resolution
    pub fn get_group_text(&self) -> Option<&str> {
        self.group_phrase.as_ref().map(|g| g.text.as_str())
    }

    /// Get the extracted scope for entity search
    pub fn get_scope_text(&self) -> Option<&str> {
        self.scope_phrase.as_ref().map(|s| s.text.as_str())
    }
}

/// Main segmentation function (with database for group resolution)
#[cfg(feature = "database")]
pub async fn segment_utterance(input: &str, pool: &PgPool) -> UtteranceSegmentation {
    let original = input.to_string();
    let normalized = normalize(input);
    let tokens = tokenize(&normalized);
    let mut trace = Vec::new();
    let mut consumed: HashSet<usize> = HashSet::new();

    // Pass 0: Already done (normalize + tokenize)
    trace.push(SegStep {
        pass: 0,
        action: format!("Normalized to {} tokens", tokens.len()),
        consumed_span: None,
        confidence: None,
    });

    // Check if input starts with a verb-group prefix (e.g., "work on allianz")
    // These prefixes imply both a verb AND introduce a group
    let implied_verb = extract_group_prefix(&normalized)
        .and_then(|(prefix, _)| get_implied_verb_for_prefix(prefix));

    // Pass 1: Group phrase extraction
    let group_phrase =
        extract_group_phrase(&normalized, &tokens, &mut consumed, &mut trace, pool).await;

    // Pass 2: Verb phrase extraction
    // If we have an implied verb from a verb-group prefix, use that with high confidence
    let verb_phrase = if let Some(implied) = implied_verb {
        if group_phrase.is_some() {
            // The prefix was a verb-group prefix and group resolved successfully
            trace.push(SegStep {
                pass: 2,
                action: format!("Verb '{}' implied by group prefix", implied),
                consumed_span: None,
                confidence: Some(0.95),
            });
            Segment {
                text: implied.to_string(),
                span: Span::new(0, implied.len()),
                confidence: 0.95, // High confidence - explicit pattern match
                method: SegmentMethod::VerbLexicon,
            }
        } else {
            // Group prefix found but group didn't resolve - fall back to normal extraction
            extract_verb_phrase(&normalized, &tokens, &mut consumed, &mut trace)
        }
    } else {
        // Normal verb extraction (constrained window, respects entity descriptors)
        extract_verb_phrase(&normalized, &tokens, &mut consumed, &mut trace)
    };

    // Pass 3: Scope phrase extraction (remaining entity descriptors)
    let scope_phrase = extract_scope_phrase(&normalized, &tokens, &consumed, &mut trace);

    // Pass 4: Residual terms
    let residual_terms = extract_residuals(&tokens, &consumed, &mut trace);

    UtteranceSegmentation {
        original,
        normalized,
        tokens,
        verb_phrase,
        group_phrase,
        scope_phrase,
        residual_terms,
        method_trace: trace,
    }
}

/// Non-database version (for testing without DB)
#[cfg(not(feature = "database"))]
pub fn segment_utterance_sync(input: &str) -> UtteranceSegmentation {
    let original = input.to_string();
    let normalized = normalize(input);
    let tokens = tokenize(&normalized);
    let mut trace = Vec::new();
    let mut consumed: HashSet<usize> = HashSet::new();

    trace.push(SegStep {
        pass: 0,
        action: format!("Normalized to {} tokens", tokens.len()),
        consumed_span: None,
        confidence: None,
    });

    // Pass 2: Verb phrase (no group resolution without DB)
    let verb_phrase = extract_verb_phrase(&normalized, &tokens, &mut consumed, &mut trace);

    // Pass 3: Scope phrase
    let scope_phrase = extract_scope_phrase(&normalized, &tokens, &consumed, &mut trace);

    // Pass 4: Residuals
    let residual_terms = extract_residuals(&tokens, &consumed, &mut trace);

    UtteranceSegmentation {
        original,
        normalized,
        tokens,
        verb_phrase,
        group_phrase: None,
        scope_phrase,
        residual_terms,
        method_trace: trace,
    }
}

// =============================================================================
// PASS 1: GROUP PHRASE EXTRACTION
// =============================================================================

#[cfg(feature = "database")]
async fn extract_group_phrase(
    normalized: &str,
    tokens: &[Token],
    consumed: &mut HashSet<usize>,
    trace: &mut Vec<SegStep>,
    pool: &PgPool,
) -> Option<Segment> {
    // Check for explicit group prefix
    if let Some((prefix, remainder)) = extract_group_prefix(normalized) {
        let remainder_trimmed = remainder.trim();
        if !remainder_trimmed.is_empty() {
            // Try to resolve the remainder as a group
            if let Ok(Some((_group_id, group_name, confidence))) =
                resolve_group(remainder_trimmed, pool).await
            {
                // Mark tokens as consumed
                let prefix_tokens = prefix.split_whitespace().count();
                for (i, _) in tokens.iter().enumerate() {
                    if i < prefix_tokens {
                        consumed.insert(i);
                    }
                }
                // Mark group tokens as consumed
                let group_start_idx = prefix_tokens;
                for (i, tok) in tokens.iter().enumerate() {
                    if i >= group_start_idx && remainder_trimmed.contains(&tok.text) {
                        consumed.insert(i);
                    }
                }

                trace.push(SegStep {
                    pass: 1,
                    action: format!(
                        "Group phrase '{}' resolved to '{}'",
                        remainder_trimmed, group_name
                    ),
                    consumed_span: Some(Span::new(0, prefix.len() + remainder_trimmed.len())),
                    confidence: Some(confidence),
                });

                return Some(Segment {
                    text: remainder_trimmed.to_string(),
                    span: Span::new(prefix.len(), prefix.len() + remainder_trimmed.len()),
                    confidence,
                    method: SegmentMethod::AliasMatch,
                });
            }
        }
    }

    // No explicit prefix - try to find group mention anywhere
    // Look for tokens that might be client group names (after prepositions like "to", "for")
    let prepositions = ["to", "for", "on", "in"];
    for (i, tok) in tokens.iter().enumerate() {
        if prepositions.contains(&tok.text.as_str()) && i + 1 < tokens.len() {
            // Check if next token(s) form a group name
            let candidate: String = tokens[i + 1..]
                .iter()
                .take(3) // Max 3 tokens for group name
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            if let Ok(Some((_, _group_name, confidence))) = resolve_group(&candidate, pool).await {
                // Mark consumed
                consumed.insert(i); // preposition
                for j in (i + 1)..tokens.len().min(i + 4) {
                    consumed.insert(j);
                }

                trace.push(SegStep {
                    pass: 1,
                    action: format!("Group phrase '{}' found after '{}'", candidate, tok.text),
                    consumed_span: Some(Span::new(
                        tok.span.start,
                        tokens.get(i + 3).map_or(tok.span.end, |t| t.span.end),
                    )),
                    confidence: Some(confidence),
                });

                return Some(Segment {
                    text: candidate,
                    span: Span::new(
                        tokens[i + 1].span.start,
                        tokens
                            .get(i + 3)
                            .map_or(tokens[i + 1].span.end, |t| t.span.end),
                    ),
                    confidence,
                    method: SegmentMethod::AliasMatch,
                });
            }

            // Try just the immediate next token
            let single = &tokens[i + 1].text;
            if let Ok(Some((_, _group_name, confidence))) = resolve_group(single, pool).await {
                consumed.insert(i);
                consumed.insert(i + 1);

                trace.push(SegStep {
                    pass: 1,
                    action: format!("Group phrase '{}' found after '{}'", single, tok.text),
                    consumed_span: Some(tokens[i + 1].span),
                    confidence: Some(confidence),
                });

                return Some(Segment {
                    text: single.clone(),
                    span: tokens[i + 1].span,
                    confidence,
                    method: SegmentMethod::AliasMatch,
                });
            }
        }
    }

    trace.push(SegStep {
        pass: 1,
        action: "No group phrase found".to_string(),
        consumed_span: None,
        confidence: None,
    });

    None
}

/// Resolve a phrase against client_group_alias table
#[cfg(feature = "database")]
async fn resolve_group(
    phrase: &str,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, String, f32)>, sqlx::Error> {
    let phrase_norm = phrase.to_lowercase();

    let result = sqlx::query!(
        r#"
        SELECT
            cg.id as "group_id!",
            cg.canonical_name as "group_name!",
            CASE
                WHEN cga.alias_norm = $1 THEN 1.0
                WHEN dmetaphone(cga.alias_norm) = dmetaphone($1) THEN 0.9
                ELSE GREATEST(similarity(cga.alias_norm, $1), 0.4)
            END as "confidence!"
        FROM "ob-poc".client_group_alias cga
        JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
        WHERE cga.alias_norm = $1
           OR similarity(cga.alias_norm, $1) > 0.4
           OR dmetaphone(cga.alias_norm) = dmetaphone($1)
        ORDER BY
            (cga.alias_norm = $1) DESC,
            (dmetaphone(cga.alias_norm) = dmetaphone($1)) DESC,
            similarity(cga.alias_norm, $1) DESC
        LIMIT 1
        "#,
        phrase_norm
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| (r.group_id, r.group_name, r.confidence)))
}

// =============================================================================
// PASS 2: VERB PHRASE EXTRACTION
// =============================================================================

fn extract_verb_phrase(
    _normalized: &str,
    tokens: &[Token],
    consumed: &mut HashSet<usize>,
    trace: &mut Vec<SegStep>,
) -> Segment {
    // Build text from unconsumed tokens in the verb window
    let available_tokens: Vec<&Token> = tokens
        .iter()
        .filter(|t| !consumed.contains(&t.index))
        .take(VERB_WINDOW_SIZE)
        .collect();

    if available_tokens.is_empty() {
        trace.push(SegStep {
            pass: 2,
            action: "No tokens available for verb extraction".to_string(),
            consumed_span: None,
            confidence: None,
        });
        return Segment {
            text: String::new(),
            span: Span::new(0, 0),
            confidence: 0.0,
            method: SegmentMethod::NoMatch,
        };
    }

    // Try longest match first against verb patterns
    for window_size in (1..=available_tokens.len()).rev() {
        let candidate: String = available_tokens[..window_size]
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        // Check if this candidate ends with an entity descriptor - if so, shrink
        let last_token = &available_tokens[window_size - 1].text;
        if is_entity_descriptor(last_token) && window_size > 1 {
            continue; // Try smaller window
        }

        // Check against verb patterns
        for (pattern, verb) in VERB_PATTERNS {
            if candidate == *pattern || candidate.starts_with(&format!("{} ", pattern)) {
                // Found a match
                let pattern_tokens = pattern.split_whitespace().count();
                let actual_tokens = pattern_tokens.min(window_size);

                for t in available_tokens.iter().take(actual_tokens) {
                    consumed.insert(t.index);
                }

                let span = Span::new(
                    available_tokens[0].span.start,
                    available_tokens[actual_tokens - 1].span.end,
                );

                trace.push(SegStep {
                    pass: 2,
                    action: format!(
                        "Verb phrase '{}' matched pattern '{}' → {}",
                        candidate, pattern, verb
                    ),
                    consumed_span: Some(span),
                    confidence: Some(1.0),
                });

                return Segment {
                    text: pattern.to_string(),
                    span,
                    confidence: 1.0,
                    method: SegmentMethod::VerbLexicon,
                };
            }
        }
    }

    // No exact pattern match - take first non-entity-descriptor token as verb
    for tok in &available_tokens {
        if !is_entity_descriptor(&tok.text) {
            consumed.insert(tok.index);

            trace.push(SegStep {
                pass: 2,
                action: format!(
                    "Verb phrase '{}' extracted (heuristic, no pattern match)",
                    tok.text
                ),
                consumed_span: Some(tok.span),
                confidence: Some(0.3),
            });

            return Segment {
                text: tok.text.clone(),
                span: tok.span,
                confidence: 0.3,
                method: SegmentMethod::Heuristic,
            };
        }
    }

    // All tokens are entity descriptors - no verb found
    trace.push(SegStep {
        pass: 2,
        action: "No verb phrase found (all tokens are entity descriptors)".to_string(),
        consumed_span: None,
        confidence: None,
    });

    Segment {
        text: String::new(),
        span: Span::new(0, 0),
        confidence: 0.0,
        method: SegmentMethod::NoMatch,
    }
}

// =============================================================================
// PASS 3: SCOPE PHRASE EXTRACTION
// =============================================================================

fn extract_scope_phrase(
    _normalized: &str, // Reserved for future span reconstruction
    tokens: &[Token],
    consumed: &HashSet<usize>,
    trace: &mut Vec<SegStep>,
) -> Option<Segment> {
    // Collect unconsumed tokens that look like entity descriptors
    let scope_tokens: Vec<&Token> = tokens
        .iter()
        .filter(|t| !consumed.contains(&t.index))
        .filter(|t| !is_stopword(&t.text))
        .collect();

    if scope_tokens.is_empty() {
        trace.push(SegStep {
            pass: 3,
            action: "No scope phrase (no remaining tokens)".to_string(),
            consumed_span: None,
            confidence: None,
        });
        return None;
    }

    let text: String = scope_tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let span = Span::new(
        scope_tokens.first().unwrap().span.start,
        scope_tokens.last().unwrap().span.end,
    );

    trace.push(SegStep {
        pass: 3,
        action: format!(
            "Scope phrase '{}' extracted from {} tokens",
            text,
            scope_tokens.len()
        ),
        consumed_span: Some(span),
        confidence: Some(0.8),
    });

    Some(Segment {
        text,
        span,
        confidence: 0.8,
        method: SegmentMethod::Heuristic,
    })
}

/// Stopwords to filter from scope phrase
fn is_stopword(word: &str) -> bool {
    const STOPWORDS: &[&str] = &[
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "as", "is", "was", "are", "were", "been", "be", "have", "has", "had", "do", "does",
        "did", "will", "would", "could", "should", "may", "might", "must", "shall", "can", "need",
        "this", "that", "these", "those", "i", "you", "he", "she", "it", "we", "they", "me", "him",
        "her", "us", "them", "my", "your", "his", "its", "our", "their", "all", "each", "every",
        "both", "few", "more", "most", "other", "some", "such", "no", "nor", "not", "only", "same",
        "so", "than", "too", "very", "just", "also",
    ];
    STOPWORDS.contains(&word.to_lowercase().as_str())
}

// =============================================================================
// PASS 4: RESIDUAL TERMS
// =============================================================================

fn extract_residuals(
    tokens: &[Token],
    consumed: &HashSet<usize>,
    trace: &mut Vec<SegStep>,
) -> Vec<Segment> {
    let residuals: Vec<Segment> = tokens
        .iter()
        .filter(|t| !consumed.contains(&t.index))
        .filter(|t| !is_stopword(&t.text))
        .map(|t| Segment {
            text: t.text.clone(),
            span: t.span,
            confidence: 0.5,
            method: SegmentMethod::Fallback,
        })
        .collect();

    if !residuals.is_empty() {
        trace.push(SegStep {
            pass: 4,
            action: format!(
                "{} residual terms: {:?}",
                residuals.len(),
                residuals.iter().map(|r| &r.text).collect::<Vec<_>>()
            ),
            consumed_span: None,
            confidence: None,
        });
    }

    residuals
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("  Hello   World  "), "hello world");
        assert_eq!(normalize("FOR ALLIANZ"), "for allianz");
    }

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize("show irish funds");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].text, "show");
        assert_eq!(tokens[1].text, "irish");
        assert_eq!(tokens[2].text, "funds");
    }

    #[test]
    fn test_tokenize_quoted() {
        let tokens = tokenize("find \"blackrock inc\" in lux");
        assert!(tokens
            .iter()
            .any(|t| t.text == "blackrock inc" && t.is_quoted));
    }

    #[test]
    fn test_is_entity_descriptor() {
        assert!(is_entity_descriptor("irish"));
        assert!(is_entity_descriptor("funds"));
        assert!(is_entity_descriptor("LU"));
        assert!(is_entity_descriptor("SICAV"));
        assert!(!is_entity_descriptor("show"));
        assert!(!is_entity_descriptor("list"));
    }

    #[test]
    fn test_empty_segmentation() {
        let seg = UtteranceSegmentation::empty("test");
        assert_eq!(seg.original, "test");
        assert!(seg.verb_phrase.confidence == 0.0);
    }

    #[test]
    fn test_garbage_detection() {
        let mut seg = UtteranceSegmentation::empty("x");
        assert!(seg.is_likely_garbage());

        seg = UtteranceSegmentation::empty("xyz abc 123");
        assert!(seg.is_likely_garbage());
    }

    #[test]
    fn test_verb_extraction_simple() {
        let normalized = "show irish funds";
        let tokens = tokenize(&normalized);
        let mut consumed = HashSet::new();
        let mut trace = Vec::new();

        let verb = extract_verb_phrase(&normalized, &tokens, &mut consumed, &mut trace);

        assert_eq!(verb.text, "show");
        assert!(verb.confidence > 0.0);
        // "irish" should NOT be consumed by verb
        assert!(!consumed.contains(&1)); // index of "irish"
    }

    #[test]
    fn test_verb_does_not_eat_jurisdiction() {
        let normalized = "list irish etf funds";
        let tokens = tokenize(&normalized);
        let mut consumed = HashSet::new();
        let mut trace = Vec::new();

        let verb = extract_verb_phrase(&normalized, &tokens, &mut consumed, &mut trace);

        assert_eq!(verb.text, "list");
        // "irish", "etf", "funds" should NOT be consumed
        assert!(!consumed.contains(&1));
        assert!(!consumed.contains(&2));
        assert!(!consumed.contains(&3));
    }

    #[test]
    fn test_scope_extraction() {
        let normalized = "show irish funds";
        let tokens = tokenize(normalized);
        let mut consumed = HashSet::new();
        consumed.insert(0); // "show" consumed by verb
        let mut trace = Vec::new();

        let scope = extract_scope_phrase(normalized, &tokens, &consumed, &mut trace);

        assert!(scope.is_some());
        let scope = scope.unwrap();
        assert!(scope.text.contains("irish"));
        assert!(scope.text.contains("funds"));
    }

    #[test]
    fn test_verb_pattern_matching() {
        let normalized = "trace ubo chain for irish funds";
        let tokens = tokenize(normalized);
        let mut consumed = HashSet::new();
        let mut trace = Vec::new();

        let verb = extract_verb_phrase(normalized, &tokens, &mut consumed, &mut trace);

        assert_eq!(verb.text, "trace ubo chain");
        assert_eq!(verb.confidence, 1.0);
        assert_eq!(verb.method, SegmentMethod::VerbLexicon);
    }
}
