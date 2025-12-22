//! Entity Extractor
//!
//! Extracts domain entities from user utterances using pattern matching,
//! lookup tables, and category expansion.
//!
//! The extractor uses multiple strategies:
//! 1. Regex pattern matching for structured values (MICs, currencies, etc.)
//! 2. Lookup table matching for known entities
//! 3. Category expansion (e.g., "European" → list of MIC codes)
//! 4. Coreference resolution for pronouns and anaphora

use crate::agentic::entity_types::{EntityTypeDefinition, EntityTypesConfig, PatternDefinition};
use crate::agentic::instrument_hierarchy::InstrumentHierarchyConfig;
use crate::agentic::intent_classifier::ConversationContext;
use crate::agentic::market_regions::MarketRegionsConfig;
use regex::Regex;
use std::collections::HashMap;

/// Result of entity extraction
#[derive(Debug, Clone, Default)]
pub struct ExtractedEntities {
    /// All extracted entities
    pub entities: Vec<ExtractedEntity>,
}

/// A single extracted entity
#[derive(Debug, Clone)]
pub struct ExtractedEntity {
    /// Unique ID for this extraction
    pub id: usize,
    /// The entity type (e.g., "currency", "market_reference")
    pub entity_type: String,
    /// The extracted/normalized value
    pub value: String,
    /// Original text that was matched
    pub original_text: String,
    /// Character span in the original utterance (start, end)
    pub span: (usize, usize),
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Source of extraction
    pub source: ExtractionSource,
    /// If this entity was derived from another (e.g., expansion)
    pub derived_from: Option<usize>,
    /// If this was resolved from an anaphoric reference
    pub resolved_from: Option<String>,
    /// For composite entities, child values
    pub components: HashMap<String, Vec<String>>,
}

/// Source of entity extraction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionSource {
    /// Extracted via regex pattern
    Pattern,
    /// Extracted via lookup table
    Lookup,
    /// Extracted via semantic similarity (future)
    Semantic,
    /// Expanded from a category reference
    Expansion,
    /// Resolved from coreference (pronoun, anaphora)
    Coreference,
    /// Inferred from context
    Inferred,
}

impl ExtractedEntities {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
        }
    }

    /// Add an entity
    pub fn add(&mut self, entity: ExtractedEntity) {
        self.entities.push(entity);
    }

    /// Merge another set of entities
    pub fn merge(&mut self, other: ExtractedEntities) {
        self.entities.extend(other.entities);
    }

    /// Get all entities of a specific type
    pub fn by_type(&self, entity_type: &str) -> Vec<&ExtractedEntity> {
        self.entities
            .iter()
            .filter(|e| e.entity_type == entity_type)
            .collect()
    }

    /// Get the first entity of a specific type
    pub fn first_of_type(&self, entity_type: &str) -> Option<&ExtractedEntity> {
        self.entities.iter().find(|e| e.entity_type == entity_type)
    }

    /// Check if an entity type is present
    pub fn has_type(&self, entity_type: &str) -> bool {
        self.entities.iter().any(|e| e.entity_type == entity_type)
    }

    /// Get all unique values for an entity type
    pub fn values_for_type(&self, entity_type: &str) -> Vec<&str> {
        self.entities
            .iter()
            .filter(|e| e.entity_type == entity_type)
            .map(|e| e.value.as_str())
            .collect()
    }

    /// Iterator over all entities
    pub fn iter(&self) -> impl Iterator<Item = &ExtractedEntity> {
        self.entities.iter()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Number of entities
    pub fn len(&self) -> usize {
        self.entities.len()
    }
}

/// The entity extractor
pub struct EntityExtractor {
    entity_types: EntityTypesConfig,
    market_regions: MarketRegionsConfig,
    instrument_hierarchy: InstrumentHierarchyConfig,
    compiled_patterns: HashMap<String, Vec<CompiledEntityPattern>>,
    next_id: usize,
}

/// A compiled pattern for entity extraction
struct CompiledEntityPattern {
    #[allow(dead_code)] // Used for debugging/logging
    pattern_type: String,
    regex: Regex,
    mappings: HashMap<String, String>,
    confidence: f32,
}

impl EntityExtractor {
    /// Create a new extractor from configuration
    pub fn new(
        entity_types: EntityTypesConfig,
        market_regions: MarketRegionsConfig,
        instrument_hierarchy: InstrumentHierarchyConfig,
    ) -> Self {
        let compiled_patterns = Self::compile_all_patterns(&entity_types);

        Self {
            entity_types,
            market_regions,
            instrument_hierarchy,
            compiled_patterns,
            next_id: 0,
        }
    }

    /// Compile all entity type patterns
    fn compile_all_patterns(
        config: &EntityTypesConfig,
    ) -> HashMap<String, Vec<CompiledEntityPattern>> {
        let mut result = HashMap::new();

        for (type_name, type_def) in &config.entity_types {
            let mut patterns = Vec::new();

            for pattern_def in &type_def.patterns {
                if let Some(compiled) = Self::compile_pattern(pattern_def) {
                    patterns.push(compiled);
                }
            }

            result.insert(type_name.clone(), patterns);
        }

        result
    }

    /// Compile a single pattern definition
    fn compile_pattern(pattern: &PatternDefinition) -> Option<CompiledEntityPattern> {
        // Build regex from pattern
        let regex = if let Some(re) = &pattern.regex {
            Regex::new(&format!(r"(?i)\b{}\b", re)).ok()?
        } else if !pattern.valid_values.is_empty() {
            // Build alternation from valid values
            let escaped: Vec<String> = pattern
                .valid_values
                .iter()
                .map(|v| regex::escape(v))
                .collect();
            Regex::new(&format!(r"(?i)\b({})\b", escaped.join("|"))).ok()?
        } else if !pattern.mappings.is_empty() {
            // Build alternation from mapping keys
            let escaped: Vec<String> = pattern.mappings.keys().map(|k| regex::escape(k)).collect();
            Regex::new(&format!(r"(?i)\b({})\b", escaped.join("|"))).ok()?
        } else {
            return None;
        };

        Some(CompiledEntityPattern {
            pattern_type: pattern.pattern_type.clone(),
            regex,
            mappings: pattern.mappings.clone(),
            confidence: if pattern.fuzzy_match { 0.85 } else { 0.95 },
        })
    }

    /// Extract all entities from an utterance
    pub fn extract(&mut self, utterance: &str, context: &ConversationContext) -> ExtractedEntities {
        let mut entities = ExtractedEntities::new();

        // Step 1: Pattern-based extraction
        let pattern_entities = self.extract_by_pattern(utterance);
        entities.merge(pattern_entities);

        // Step 2: Lookup-based extraction (for names, etc.)
        let lookup_entities = self.extract_by_lookup(utterance);
        entities.merge(lookup_entities);

        // Step 3: Expand category references
        let expanded = self.expand_categories(&entities);
        entities.merge(expanded);

        // Step 4: Resolve coreferences
        let resolved = self.resolve_coreferences(utterance, &entities, context);
        entities.merge(resolved);

        // Step 5: Infer missing entities from context
        let inferred = self.infer_from_context(&entities, context);
        entities.merge(inferred);

        entities
    }

    /// Extract entities using regex patterns
    fn extract_by_pattern(&mut self, utterance: &str) -> ExtractedEntities {
        let mut entities = ExtractedEntities::new();

        // Collect matches first to avoid borrow conflict with next_id()
        let mut matches: Vec<(String, String, String, usize, usize, f32)> = Vec::new();

        // Process in extraction order
        for (type_name, type_def) in self.entity_types.extraction_ordered() {
            if let Some(patterns) = self.compiled_patterns.get(type_name) {
                for pattern in patterns {
                    for captures in pattern.regex.captures_iter(utterance) {
                        let matched = captures.get(0).unwrap();
                        let value = matched.as_str();

                        // Normalize the value
                        let normalized =
                            self.normalize_value(value, type_name, &pattern.mappings, type_def);

                        matches.push((
                            type_name.to_string(),
                            normalized,
                            value.to_string(),
                            matched.start(),
                            matched.end(),
                            pattern.confidence,
                        ));
                    }
                }
            }
        }

        // Now create entities with unique IDs
        for (entity_type, value, original_text, start, end, confidence) in matches {
            entities.add(ExtractedEntity {
                id: self.next_id(),
                entity_type,
                value,
                original_text,
                span: (start, end),
                confidence,
                source: ExtractionSource::Pattern,
                derived_from: None,
                resolved_from: None,
                components: HashMap::new(),
            });
        }

        entities
    }

    /// Extract entities by looking for known values in text
    fn extract_by_lookup(&mut self, utterance: &str) -> ExtractedEntities {
        let mut entities = ExtractedEntities::new();
        let utterance_lower = utterance.to_lowercase();

        // Collect matches first to avoid borrow conflict with next_id()
        let mut matches: Vec<(String, String, String, usize, usize, f32)> = Vec::new();

        // Check for market regions
        for region_name in self.market_regions.market_regions.keys() {
            if utterance_lower.contains(&region_name.to_lowercase()) {
                if let Some(pos) = utterance_lower.find(&region_name.to_lowercase()) {
                    matches.push((
                        "market_reference".to_string(),
                        region_name.clone(),
                        region_name.clone(),
                        pos,
                        pos + region_name.len(),
                        0.9,
                    ));
                }
            }
        }

        // Check for instrument categories
        for (node_id, node) in &self.instrument_hierarchy.instrument_hierarchy {
            // Check node name
            if utterance_lower.contains(&node.name.to_lowercase()) {
                if let Some(pos) = utterance_lower.find(&node.name.to_lowercase()) {
                    let value = node.code.clone().unwrap_or_else(|| node_id.clone());
                    matches.push((
                        "instrument_class_reference".to_string(),
                        value,
                        node.name.clone(),
                        pos,
                        pos + node.name.len(),
                        0.9,
                    ));
                }
            }

            // Check aliases
            for alias in &node.aliases {
                if utterance_lower.contains(&alias.to_lowercase()) {
                    if let Some(pos) = utterance_lower.find(&alias.to_lowercase()) {
                        let value = node.code.clone().unwrap_or_else(|| node_id.clone());
                        matches.push((
                            "instrument_class_reference".to_string(),
                            value,
                            alias.clone(),
                            pos,
                            pos + alias.len(),
                            0.85,
                        ));
                    }
                }
            }
        }

        // Now create entities with unique IDs
        for (entity_type, value, original_text, start, end, confidence) in matches {
            entities.add(ExtractedEntity {
                id: self.next_id(),
                entity_type,
                value,
                original_text,
                span: (start, end),
                confidence,
                source: ExtractionSource::Lookup,
                derived_from: None,
                resolved_from: None,
                components: HashMap::new(),
            });
        }

        entities
    }

    /// Expand category references to concrete values
    fn expand_categories(&mut self, entities: &ExtractedEntities) -> ExtractedEntities {
        let mut expanded = ExtractedEntities::new();

        for entity in entities.iter() {
            // Expand market regions
            if entity.entity_type == "market_reference" {
                if let Some(markets) = self.market_regions.expand_region(&entity.value) {
                    for mic in markets {
                        expanded.add(ExtractedEntity {
                            id: self.next_id(),
                            entity_type: "market_reference".to_string(),
                            value: mic,
                            original_text: entity.original_text.clone(),
                            span: entity.span,
                            confidence: entity.confidence * 0.95,
                            source: ExtractionSource::Expansion,
                            derived_from: Some(entity.id),
                            resolved_from: None,
                            components: HashMap::new(),
                        });
                    }
                }
            }

            // Expand instrument categories
            if entity.entity_type == "instrument_class_reference" {
                if let Some(codes) = self.instrument_hierarchy.expand_category(&entity.value) {
                    // Only expand if it's a category, not already a code
                    if codes.len() > 1 || codes.first() != Some(&entity.value) {
                        for code in codes {
                            expanded.add(ExtractedEntity {
                                id: self.next_id(),
                                entity_type: "instrument_class_reference".to_string(),
                                value: code,
                                original_text: entity.original_text.clone(),
                                span: entity.span,
                                confidence: entity.confidence * 0.95,
                                source: ExtractionSource::Expansion,
                                derived_from: Some(entity.id),
                                resolved_from: None,
                                components: HashMap::new(),
                            });
                        }
                    }
                }
            }
        }

        expanded
    }

    /// Resolve coreferences (pronouns, anaphora)
    fn resolve_coreferences(
        &mut self,
        utterance: &str,
        _entities: &ExtractedEntities,
        context: &ConversationContext,
    ) -> ExtractedEntities {
        let mut resolved = ExtractedEntities::new();
        let utterance_lower = utterance.to_lowercase();

        // Common pronouns/anaphora patterns
        let patterns = [
            ("them", "manager_reference"),
            ("that manager", "manager_reference"),
            ("the first im", "manager_reference"),
            ("the im", "manager_reference"),
            ("it", "cbu_reference"),
            ("the fund", "cbu_reference"),
            ("the client", "cbu_reference"),
        ];

        for (pattern, entity_type) in patterns {
            if utterance_lower.contains(pattern) {
                // Look for this entity type in context
                if let Some(value) = context.session_entities.get(entity_type) {
                    if let Some(pos) = utterance_lower.find(pattern) {
                        resolved.add(ExtractedEntity {
                            id: self.next_id(),
                            entity_type: entity_type.to_string(),
                            value: value.clone(),
                            original_text: pattern.to_string(),
                            span: (pos, pos + pattern.len()),
                            confidence: 0.8,
                            source: ExtractionSource::Coreference,
                            derived_from: None,
                            resolved_from: Some(pattern.to_string()),
                            components: HashMap::new(),
                        });
                    }
                }
            }
        }

        resolved
    }

    /// Infer missing entities from context
    fn infer_from_context(
        &mut self,
        entities: &ExtractedEntities,
        context: &ConversationContext,
    ) -> ExtractedEntities {
        let mut inferred = ExtractedEntities::new();

        // If we have no CBU reference but there's one in context, add it
        if !entities.has_type("cbu_reference") {
            if let Some(cbu) = context.session_entities.get("cbu_reference") {
                inferred.add(ExtractedEntity {
                    id: self.next_id(),
                    entity_type: "cbu_reference".to_string(),
                    value: cbu.clone(),
                    original_text: String::new(),
                    span: (0, 0),
                    confidence: 0.9,
                    source: ExtractionSource::Inferred,
                    derived_from: None,
                    resolved_from: None,
                    components: HashMap::new(),
                });
            }
        }

        inferred
    }

    /// Normalize a value based on entity type configuration
    fn normalize_value(
        &self,
        value: &str,
        _type_name: &str,
        mappings: &HashMap<String, String>,
        type_def: &EntityTypeDefinition,
    ) -> String {
        // First check mappings
        let value_lower = value.to_lowercase();
        for (key, mapped_value) in mappings {
            if key.to_lowercase() == value_lower {
                return mapped_value.clone();
            }
        }

        // Apply normalization rules
        let mut result = value.to_string();

        if type_def.normalization.uppercase {
            result = result.to_uppercase();
        }

        if type_def.normalization.remove_commas {
            result = result.replace(',', "");
        }

        result
    }

    /// Get next unique ID
    fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_configs() -> (
        EntityTypesConfig,
        MarketRegionsConfig,
        InstrumentHierarchyConfig,
    ) {
        let entity_types_yaml = r#"
version: "1.0"
description: "Test"
entity_types:
  currency:
    description: "Currency"
    patterns:
      - type: ISO_CODE
        regex: "\\b(USD|EUR|GBP|JPY|CHF)\\b"
      - type: NAME
        mappings:
          "dollars": USD
          "euros": EUR
    normalization:
      uppercase: true
  market_reference:
    description: "Market"
    patterns:
      - type: MIC
        regex: "X[A-Z]{3}"
extraction_config:
  extraction_order:
    - currency
    - market_reference
"#;

        let market_regions_yaml = r#"
version: "1.0"
description: "Test"
market_regions:
  European:
    description: "EU"
    markets: [XLON, XETR, XPAR]
    aliases: [Europe, EU]
  US:
    description: "US"
    markets: [XNYS, XNAS]
"#;

        let instrument_hierarchy_yaml = r#"
version: "1.0"
description: "Test"
instrument_hierarchy:
  equity:
    name: "Equity"
    code: "EQUITY"
    aliases: [equities, stocks]
  fixed_income:
    name: "Fixed Income"
    aliases: [bonds]
    children: [govt_bond, corp_bond]
  govt_bond:
    name: "Government Bonds"
    code: "GOVT_BOND"
  corp_bond:
    name: "Corporate Bonds"
    code: "CORP_BOND"
"#;

        (
            EntityTypesConfig::load_from_str(entity_types_yaml).unwrap(),
            MarketRegionsConfig::load_from_str(market_regions_yaml).unwrap(),
            InstrumentHierarchyConfig::load_from_str(instrument_hierarchy_yaml).unwrap(),
        )
    }

    #[test]
    fn test_extract_currency_code() {
        let (et, mr, ih) = sample_configs();
        let mut extractor = EntityExtractor::new(et, mr, ih);
        let context = ConversationContext::default();

        let entities = extractor.extract("use USD for settlement", &context);

        assert!(entities.has_type("currency"));
        assert_eq!(entities.first_of_type("currency").unwrap().value, "USD");
    }

    #[test]
    fn test_extract_currency_name() {
        let (et, mr, ih) = sample_configs();
        let mut extractor = EntityExtractor::new(et, mr, ih);
        let context = ConversationContext::default();

        let entities = extractor.extract("pay in dollars", &context);

        assert!(entities.has_type("currency"));
        assert_eq!(entities.first_of_type("currency").unwrap().value, "USD");
    }

    #[test]
    fn test_extract_market_mic() {
        let (et, mr, ih) = sample_configs();
        let mut extractor = EntityExtractor::new(et, mr, ih);
        let context = ConversationContext::default();

        let entities = extractor.extract("trade on XNYS", &context);

        assert!(entities.has_type("market_reference"));
        assert_eq!(
            entities.first_of_type("market_reference").unwrap().value,
            "XNYS"
        );
    }

    #[test]
    fn test_expand_market_region() {
        let (et, mr, ih) = sample_configs();
        let mut extractor = EntityExtractor::new(et, mr, ih);
        let context = ConversationContext::default();

        let entities = extractor.extract("European equities", &context);

        let markets = entities.values_for_type("market_reference");
        // Should have expanded "European" to XLON, XETR, XPAR
        assert!(markets.len() >= 3);
        assert!(markets.contains(&"XLON"));
    }

    #[test]
    fn test_expand_instrument_category() {
        let (et, mr, ih) = sample_configs();
        let mut extractor = EntityExtractor::new(et, mr, ih);
        let context = ConversationContext::default();

        let entities = extractor.extract("trade fixed income", &context);

        let instruments = entities.values_for_type("instrument_class_reference");
        // Should have expanded "fixed income" to GOVT_BOND, CORP_BOND
        assert!(instruments.contains(&"GOVT_BOND") || instruments.contains(&"CORP_BOND"));
    }

    #[test]
    fn test_infer_from_context() {
        let (et, mr, ih) = sample_configs();
        let mut extractor = EntityExtractor::new(et, mr, ih);
        let mut context = ConversationContext::default();
        context
            .session_entities
            .insert("cbu_reference".to_string(), "fund-123".to_string());

        let entities = extractor.extract("add equities", &context);

        // Should infer CBU from context
        assert!(entities.has_type("cbu_reference"));
        assert_eq!(
            entities.first_of_type("cbu_reference").unwrap().value,
            "fund-123"
        );
    }
}
