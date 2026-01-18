//! ESPER Command Registry with Trie-based lookup
//!
//! Provides O(k) lookup for exact matches, with fallback to
//! contains and prefix patterns sorted by priority.

use super::config::{AgentCommandSpec, EsperCommandDef, EsperConfig, ParamSource};
use ob_poc_types::{AgentCommand, PanDirection};
use radix_trie::{Trie, TrieCommon};
use std::collections::HashMap;

/// Match type for alias lookup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    Exact,
    Contains,
    Prefix,
}

/// Source of the match (builtin vs learned)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchSource {
    Builtin,
    Learned,
}

/// Internal trie entry
#[derive(Debug, Clone)]
struct TrieEntry {
    command_key: String,
    priority: u32,
}

/// Result of a successful ESPER command match
#[derive(Debug, Clone)]
pub struct EsperMatch {
    /// The matched command key (e.g., "zoom_in", "scale_universe")
    pub command_key: String,
    /// The AgentCommand to send to UI
    pub command: AgentCommand,
    /// Response text to show user
    pub response: String,
    /// Whether this was a builtin or learned alias
    pub source: MatchSource,
    /// Parameters extracted from the phrase
    pub extracted_params: HashMap<String, String>,
}

/// Result of lookup with semantic fallback
#[derive(Debug, Clone)]
pub enum LookupResult {
    /// Fast path: matched via trie (exact/contains/prefix)
    Matched(EsperMatch),

    /// Slow path: matched via semantic with high confidence (>0.80)
    /// Should auto-execute and learn the alias
    SemanticMatch {
        esper_match: EsperMatch,
        semantic: SemanticMatch,
        should_learn: bool,
    },

    /// Medium confidence (0.50-0.80): needs user disambiguation
    NeedsDisambiguation {
        candidates: Vec<SemanticMatch>,
        original_phrase: String,
    },

    /// No match found (confidence < 0.50 or no semantic index)
    NoMatch,
}

/// ESPER command registry with trie-based lookup and semantic fallback
pub struct EsperCommandRegistry {
    /// Trie for O(k) exact phrase lookup
    exact_trie: Trie<String, TrieEntry>,

    /// Contains patterns (checked if exact miss) - sorted by priority desc
    contains_patterns: Vec<(String, TrieEntry)>,

    /// Prefix patterns (checked if contains miss) - sorted by priority desc
    prefix_patterns: Vec<(String, TrieEntry)>,

    /// Learned aliases (phrase → command_key)
    learned: HashMap<String, String>,

    /// Command definitions (for building AgentCommand)
    commands: HashMap<String, EsperCommandDef>,

    /// Semantic index for fallback on trie miss (Phase 8)
    semantic_index: SemanticIndex,
}

impl EsperCommandRegistry {
    /// Build from config + learned aliases
    pub fn new(config: EsperConfig, learned: HashMap<String, String>) -> Self {
        let mut exact_trie = Trie::new();
        let mut contains_patterns = Vec::new();
        let mut prefix_patterns = Vec::new();

        for (key, def) in &config.commands {
            let priority = def.priority;

            // Insert exact aliases into trie
            for alias in &def.aliases.exact {
                let normalized = alias.to_lowercase().trim().to_string();
                exact_trie.insert(
                    normalized,
                    TrieEntry {
                        command_key: key.clone(),
                        priority,
                    },
                );
            }

            // Collect contains patterns
            for pattern in &def.aliases.contains {
                contains_patterns.push((
                    pattern.to_lowercase(),
                    TrieEntry {
                        command_key: key.clone(),
                        priority,
                    },
                ));
            }

            // Collect prefix patterns
            for pattern in &def.aliases.prefix {
                prefix_patterns.push((
                    pattern.to_lowercase(),
                    TrieEntry {
                        command_key: key.clone(),
                        priority,
                    },
                ));
            }
        }

        // Sort by priority descending for deterministic matching
        contains_patterns.sort_by(|a, b| b.1.priority.cmp(&a.1.priority));
        prefix_patterns.sort_by(|a, b| b.1.priority.cmp(&a.1.priority));

        // Add learned aliases to exact trie (lower priority than builtins)
        for (phrase, command_key) in &learned {
            exact_trie.insert(
                phrase.clone(),
                TrieEntry {
                    command_key: command_key.clone(),
                    priority: 50, // Lower than builtin default of 100
                },
            );
        }

        Self {
            exact_trie,
            contains_patterns,
            prefix_patterns,
            learned,
            commands: config.commands,
            semantic_index: SemanticIndex::new(),
        }
    }

    /// Get all aliases for pre-computing embeddings
    ///
    /// Returns (alias_text, command_key) pairs for all builtin aliases.
    pub fn all_aliases(&self) -> Vec<(String, String)> {
        let mut aliases = Vec::new();
        for (key, def) in &self.commands {
            // Exact aliases
            for alias in &def.aliases.exact {
                aliases.push((alias.to_lowercase().trim().to_string(), key.clone()));
            }
            // Contains patterns (use as aliases too)
            for pattern in &def.aliases.contains {
                aliases.push((pattern.to_lowercase(), key.clone()));
            }
            // Prefix patterns (without trailing space)
            for pattern in &def.aliases.prefix {
                aliases.push((pattern.to_lowercase().trim().to_string(), key.clone()));
            }
        }
        aliases
    }

    /// Set the semantic index (after warmup computes embeddings)
    pub fn set_semantic_index(&mut self, index: SemanticIndex) {
        self.semantic_index = index;
    }

    /// Check if semantic index is ready
    pub fn semantic_ready(&self) -> bool {
        self.semantic_index.ready
    }

    /// O(k) lookup for exact, then O(n) for contains/prefix
    ///
    /// This is the fast path - use `lookup_with_semantic_fallback` for
    /// semantic matching on trie miss.
    pub fn lookup(&self, phrase: &str) -> Option<EsperMatch> {
        let normalized = phrase.to_lowercase();
        let normalized = normalized.trim();

        // 1. Try exact match (O(k))
        if let Some(entry) = self.exact_trie.get(normalized) {
            let source = if self.learned.contains_key(normalized) {
                MatchSource::Learned
            } else {
                MatchSource::Builtin
            };
            return self.build_match(&entry.command_key, normalized, MatchType::Exact, source);
        }

        // 2. Try contains match (O(n) but patterns sorted by priority)
        for (pattern, entry) in &self.contains_patterns {
            if normalized.contains(pattern.as_str()) {
                return self.build_match(
                    &entry.command_key,
                    normalized,
                    MatchType::Contains,
                    MatchSource::Builtin,
                );
            }
        }

        // 3. Try prefix match
        for (pattern, entry) in &self.prefix_patterns {
            if normalized.starts_with(pattern.as_str()) {
                return self.build_match(
                    &entry.command_key,
                    normalized,
                    MatchType::Prefix,
                    MatchSource::Builtin,
                );
            }
        }

        None
    }

    /// Lookup with semantic fallback on trie miss
    ///
    /// Flow:
    /// 1. Try fast trie lookup (exact/contains/prefix)
    /// 2. On miss + semantic ready: search semantic index
    /// 3. Return best match if confidence > threshold
    ///
    /// The `query_embedding` should be pre-computed by caller using
    /// `CandleEmbedder::embed_blocking()` - this keeps the lookup itself fast.
    ///
    /// Returns:
    /// - `Ok(Some(match))` - Found via trie or semantic with confidence > threshold
    /// - `Ok(None)` - No match above threshold
    /// - The `SemanticMatch` in the tuple indicates if semantic was used
    pub fn lookup_with_semantic(
        &self,
        phrase: &str,
        query_embedding: Option<&[f32]>,
    ) -> LookupResult {
        // Fast path: trie lookup
        if let Some(esper_match) = self.lookup(phrase) {
            return LookupResult::Matched(esper_match);
        }

        // Slow path: semantic fallback (only if index is ready)
        let Some(embedding) = query_embedding else {
            return LookupResult::NoMatch;
        };

        if !self.semantic_index.ready {
            return LookupResult::NoMatch;
        }

        let semantic_matches = self.semantic_index.search(embedding, 3);
        if semantic_matches.is_empty() {
            return LookupResult::NoMatch;
        }

        let best = &semantic_matches[0];

        // High confidence: auto-execute
        if best.confidence >= thresholds::AUTO_EXECUTE {
            if let Some(esper_match) = self.build_match(
                &best.command_key,
                phrase,
                MatchType::Exact,
                MatchSource::Learned,
            ) {
                return LookupResult::SemanticMatch {
                    esper_match,
                    semantic: best.clone(),
                    should_learn: true,
                };
            }
        }

        // Medium confidence: disambiguation needed
        if best.confidence >= thresholds::DISAMBIGUATION {
            return LookupResult::NeedsDisambiguation {
                candidates: semantic_matches,
                original_phrase: phrase.to_string(),
            };
        }

        // Low confidence: no match
        LookupResult::NoMatch
    }

    /// Check if a command key exists
    pub fn has_command(&self, command_key: &str) -> bool {
        self.commands.contains_key(command_key)
    }

    /// List all commands (for MCP esper_list tool)
    pub fn list_commands(&self) -> impl Iterator<Item = (&String, &EsperCommandDef)> {
        self.commands.iter()
    }

    /// Get count of commands
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Get count of learned aliases
    pub fn learned_count(&self) -> usize {
        self.learned.len()
    }

    /// Add a learned alias (will persist to DB separately)
    pub fn add_learned_alias(&mut self, phrase: &str, command_key: &str) {
        let normalized = phrase.to_lowercase().trim().to_string();
        self.learned
            .insert(normalized.clone(), command_key.to_string());
        self.exact_trie.insert(
            normalized,
            TrieEntry {
                command_key: command_key.to_string(),
                priority: 50,
            },
        );
    }

    fn build_match(
        &self,
        command_key: &str,
        phrase: &str,
        match_type: MatchType,
        source: MatchSource,
    ) -> Option<EsperMatch> {
        let def = self.commands.get(command_key)?;
        let mut extracted = HashMap::new();

        // Extract parameters based on spec
        for (param_name, param_source) in &def.agent_command.params {
            match param_source {
                ParamSource::Extract => {
                    if let Some(val) = self.extract_param(param_name, phrase) {
                        extracted.insert(param_name.clone(), val);
                    }
                }
                ParamSource::RestOfPhrase => {
                    // For prefix matches, get the rest after prefix
                    if match_type == MatchType::Prefix {
                        for (pattern, entry) in &self.prefix_patterns {
                            if entry.command_key == command_key
                                && phrase.starts_with(pattern.as_str())
                            {
                                let rest = phrase[pattern.len()..].trim().to_string();
                                if !rest.is_empty() {
                                    extracted.insert(param_name.clone(), rest);
                                }
                                break;
                            }
                        }
                    }
                }
                ParamSource::Context => {
                    // Will be filled by caller from session context
                }
            }
        }

        // Build AgentCommand from spec + extracted params
        let command = self.build_agent_command(&def.agent_command, &extracted)?;

        Some(EsperMatch {
            command_key: command_key.to_string(),
            command,
            response: def.response.clone(),
            source,
            extracted_params: extracted,
        })
    }

    fn extract_param(&self, param_name: &str, phrase: &str) -> Option<String> {
        match param_name {
            "factor" | "amount" => {
                // Parse number or percentage
                for word in phrase.split_whitespace() {
                    // Try parsing as float directly
                    if let Ok(n) = word.parse::<f32>() {
                        return Some(n.to_string());
                    }
                    // Try parsing with 'x' suffix (e.g., "2x")
                    if word.ends_with('x') {
                        if let Ok(n) = word.trim_end_matches('x').parse::<f32>() {
                            return Some(n.to_string());
                        }
                    }
                    // Try parsing percentage (e.g., "50%")
                    if word.ends_with('%') {
                        if let Ok(n) = word.trim_end_matches('%').parse::<f32>() {
                            return Some((n / 100.0).to_string());
                        }
                    }
                }
                None
            }
            "direction" => {
                if phrase.contains("left") {
                    return Some("left".into());
                }
                if phrase.contains("right") {
                    return Some("right".into());
                }
                if phrase.contains("up") {
                    return Some("up".into());
                }
                if phrase.contains("down") {
                    return Some("down".into());
                }
                None
            }
            "layer" => {
                if phrase.contains("ownership") {
                    return Some("ownership".into());
                }
                if phrase.contains("control") {
                    return Some("control".into());
                }
                if phrase.contains("service") {
                    return Some("services".into());
                }
                if phrase.contains("custody") {
                    return Some("custody".into());
                }
                None
            }
            "format" => {
                if phrase.contains("svg") {
                    return Some("svg".into());
                }
                if phrase.contains("pdf") {
                    return Some("pdf".into());
                }
                Some("png".into()) // Default
            }
            "segment" => {
                if phrase.contains("hedge fund") {
                    return Some("hedge_fund".into());
                }
                if phrase.contains("pension") {
                    return Some("pension".into());
                }
                if phrase.contains("sovereign") {
                    return Some("sovereign_wealth".into());
                }
                if phrase.contains("family office") {
                    return Some("family_office".into());
                }
                if phrase.contains("insurance") {
                    return Some("insurance".into());
                }
                None
            }
            "aspect" => {
                if phrase.contains("ownership") {
                    return Some("ownership".into());
                }
                if phrase.contains("control") {
                    return Some("control".into());
                }
                if phrase.contains("risk") {
                    return Some("risk".into());
                }
                if phrase.contains("change") {
                    return Some("changes".into());
                }
                Some("all".into()) // Default
            }
            "dimension" => {
                if phrase.contains("time") {
                    return Some("time".into());
                }
                if phrase.contains("service") {
                    return Some("services".into());
                }
                if phrase.contains("ownership") {
                    return Some("ownership".into());
                }
                Some("default".into())
            }
            _ => None,
        }
    }

    fn build_agent_command(
        &self,
        spec: &AgentCommandSpec,
        params: &HashMap<String, String>,
    ) -> Option<AgentCommand> {
        Some(match spec.command_type.as_str() {
            // Stop/Hold
            "Stop" => AgentCommand::Stop,

            // Zoom
            "ZoomIn" => AgentCommand::ZoomIn {
                factor: params.get("factor").and_then(|s| s.parse().ok()),
            },
            "ZoomOut" => AgentCommand::ZoomOut {
                factor: params.get("factor").and_then(|s| s.parse().ok()),
            },
            "ZoomFit" => AgentCommand::ZoomFit,

            // Pan
            "Pan" => AgentCommand::Pan {
                direction: match params.get("direction").map(|s| s.as_str()) {
                    Some("left") => PanDirection::Left,
                    Some("right") => PanDirection::Right,
                    Some("up") => PanDirection::Up,
                    Some("down") => PanDirection::Down,
                    _ => return None,
                },
                amount: params.get("amount").and_then(|s| s.parse().ok()),
            },

            // Center/Layout
            "Center" => AgentCommand::Center,
            "ResetLayout" => AgentCommand::ResetLayout,
            "ToggleOrientation" => AgentCommand::ToggleOrientation,

            // Hierarchy
            "TaxonomyZoomOut" => AgentCommand::TaxonomyZoomOut,

            // Export
            "Export" => AgentCommand::Export {
                format: params.get("format").cloned(),
            },

            // Search/Help
            "Search" => AgentCommand::Search {
                query: params.get("query").cloned().unwrap_or_default(),
            },
            "ShowHelp" => AgentCommand::ShowHelp {
                topic: params.get("topic").cloned(),
            },

            // Scale Navigation
            "ScaleUniverse" => AgentCommand::ScaleUniverse,
            "ScaleGalaxy" => AgentCommand::ScaleGalaxy {
                segment: params.get("segment").cloned(),
            },
            "ScaleSystem" => AgentCommand::ScaleSystem { cbu_id: None },
            "ScalePlanet" => AgentCommand::ScalePlanet { entity_id: None },
            "ScaleSurface" => AgentCommand::ScaleSurface,
            "ScaleCore" => AgentCommand::ScaleCore,

            // Depth Navigation
            "DrillThrough" => AgentCommand::DrillThrough,
            "SurfaceReturn" => AgentCommand::SurfaceReturn,
            "XRay" => AgentCommand::XRay,
            "Peel" => AgentCommand::Peel,
            "CrossSection" => AgentCommand::CrossSection,
            "DepthIndicator" => AgentCommand::DepthIndicator,

            // Orbital Navigation
            "Orbit" => AgentCommand::Orbit { entity_id: None },
            "RotateLayer" => AgentCommand::RotateLayer {
                layer: params
                    .get("layer")
                    .cloned()
                    .unwrap_or_else(|| "ownership".into()),
            },
            "Flip" => AgentCommand::Flip,
            "Tilt" => AgentCommand::Tilt {
                dimension: params
                    .get("dimension")
                    .cloned()
                    .unwrap_or_else(|| "default".into()),
            },

            // Temporal Navigation
            "TimeRewind" => AgentCommand::TimeRewind { target_date: None },
            "TimePlay" => AgentCommand::TimePlay {
                from_date: None,
                to_date: None,
            },
            "TimeFreeze" => AgentCommand::TimeFreeze,
            "TimeSlice" => AgentCommand::TimeSlice {
                date1: None,
                date2: None,
            },
            "TimeTrail" => AgentCommand::TimeTrail { entity_id: None },

            // Investigation
            "FollowTheMoney" => AgentCommand::FollowTheMoney { from_entity: None },
            "WhoControls" => AgentCommand::WhoControls { entity_id: None },
            "Illuminate" => AgentCommand::Illuminate {
                aspect: params
                    .get("aspect")
                    .cloned()
                    .unwrap_or_else(|| "all".into()),
            },
            "Shadow" => AgentCommand::Shadow,
            "RedFlagScan" => AgentCommand::RedFlagScan,
            "BlackHole" => AgentCommand::BlackHole,

            // Context
            "ContextReview" => AgentCommand::ContextReview,
            "ContextInvestigation" => AgentCommand::ContextInvestigation,
            "ContextOnboarding" => AgentCommand::ContextOnboarding,
            "ContextMonitoring" => AgentCommand::ContextMonitoring,
            "ContextRemediation" => AgentCommand::ContextRemediation,

            _ => return None,
        })
    }

    /// Get registry statistics
    pub fn stats(&self) -> RegistryStats {
        RegistryStats {
            total_commands: self.commands.len(),
            exact_count: self.exact_trie.len(),
            contains_count: self.contains_patterns.len(),
            prefix_count: self.prefix_patterns.len(),
            learned_count: self.learned.len(),
        }
    }
}

/// Statistics about the ESPER command registry
#[derive(Debug, Clone)]
pub struct RegistryStats {
    /// Number of command definitions in YAML
    pub total_commands: usize,
    /// Number of exact alias entries in trie
    pub exact_count: usize,
    /// Number of contains patterns
    pub contains_count: usize,
    /// Number of prefix patterns
    pub prefix_count: usize,
    /// Number of learned aliases
    pub learned_count: usize,
}

// ============================================================================
// SEMANTIC FALLBACK (Phase 8)
// ============================================================================

/// Result of semantic search on trie miss
#[derive(Debug, Clone)]
pub struct SemanticMatch {
    /// Command key that matched
    pub command_key: String,
    /// Alias phrase that matched
    pub matched_alias: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
}

/// Confidence thresholds for semantic matching
pub mod thresholds {
    /// Above this: auto-execute and learn
    pub const AUTO_EXECUTE: f32 = 0.80;
    /// Above this: show disambiguation UI
    pub const DISAMBIGUATION: f32 = 0.50;
    // Below DISAMBIGUATION: escalate to chat (no constant needed)
}

/// Pre-computed embedding for a command alias
#[derive(Debug, Clone)]
pub struct EmbeddedAlias {
    /// Original alias text
    pub alias: String,
    /// Command key this maps to
    pub command_key: String,
    /// Pre-computed embedding vector
    pub embedding: Vec<f32>,
}

/// Semantic search index for ESPER commands
///
/// Used as fallback when trie lookup misses.
/// Pre-computes embeddings for all builtin aliases at startup.
#[derive(Debug, Clone, Default)]
pub struct SemanticIndex {
    /// Embedded aliases for semantic search
    pub aliases: Vec<EmbeddedAlias>,
    /// Whether the index is ready
    pub ready: bool,
}

impl SemanticIndex {
    /// Create an empty index
    pub fn new() -> Self {
        Self {
            aliases: Vec::new(),
            ready: false,
        }
    }

    /// Search for closest matching aliases
    ///
    /// Returns top-k matches sorted by confidence (descending).
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<SemanticMatch> {
        if !self.ready || query_embedding.is_empty() {
            return Vec::new();
        }

        let mut matches: Vec<_> = self
            .aliases
            .iter()
            .map(|entry| {
                let similarity = cosine_similarity(query_embedding, &entry.embedding);
                SemanticMatch {
                    command_key: entry.command_key.clone(),
                    matched_alias: entry.alias.clone(),
                    confidence: similarity,
                }
            })
            .collect();

        // Sort by confidence descending
        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(top_k);
        matches
    }

    /// Add an embedded alias to the index
    pub fn add_alias(&mut self, alias: String, command_key: String, embedding: Vec<f32>) {
        self.aliases.push(EmbeddedAlias {
            alias,
            command_key,
            embedding,
        });
    }

    /// Mark index as ready for use
    pub fn mark_ready(&mut self) {
        self.ready = true;
    }

    /// Number of indexed aliases
    pub fn len(&self) -> usize {
        self.aliases.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.aliases.is_empty()
    }
}

/// Compute cosine similarity between two vectors
///
/// Assumes vectors are L2-normalized (dot product = cosine similarity).
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
