//! Pipeline Integration Tests
//!
//! Tests for the Phase 3 agent intelligence pipeline.
//! These tests verify end-to-end behavior from user utterance to DSL generation.

#[cfg(test)]
mod tests {
    use crate::agentic::dsl_generator::{DslGenerator, GenerationContext};
    use crate::agentic::entity_extractor::EntityExtractor;
    use crate::agentic::entity_types::EntityTypesConfig;
    use crate::agentic::instrument_hierarchy::InstrumentHierarchyConfig;
    use crate::agentic::intent_classifier::{ConversationContext, IntentClassifier};
    use crate::agentic::market_regions::MarketRegionsConfig;
    #[allow(unused_imports)]
    use crate::agentic::pipeline::{AgentPipeline, ResponseType, SessionContext};
    use crate::agentic::taxonomy::IntentTaxonomy;
    use uuid::Uuid;

    // Sample YAML configurations for testing
    const SAMPLE_TAXONOMY: &str = r#"
version: "1.0"
description: "Test taxonomy"
intent_taxonomy:
  trading_matrix:
    description: "Trading matrix domain"
    investment_manager:
      description: "IM subdomain"
      intents:
        - intent: im_assign
          description: "Assign investment manager"
          canonical_verb: investment-manager.assign
          trigger_phrases:
            - "add {manager} as investment manager"
            - "add {manager} as IM"
            - "also add {manager}"
            - "{manager} will handle"
            - "use {manager} for"
            - "{manager} manages"
          required_entities:
            - manager_reference
          optional_entities:
            - market_reference
            - instrument_class_reference
            - instruction_method
          default_inferences:
            priority: 100
            can-trade: true
          examples:
            - input: "Add BlackRock for European equities"
              entities:
                manager_reference: "BlackRock"
                market_reference: "European"
                instrument_class_reference: "EQUITY"
    pricing:
      description: "Pricing subdomain"
      intents:
        - intent: pricing_set
          description: "Configure pricing source"
          canonical_verb: pricing-config.set
          trigger_phrases:
            - "use {source} for pricing"
            - "get prices from {source}"
            - "{source} for all pricing"
          required_entities:
            - pricing_source
          optional_entities:
            - instrument_class_reference
intent_relationships:
  natural_followups:
    im_assign:
      - im_assign
      - pricing_set
confidence_thresholds:
  defaults:
    execute_threshold: 0.85
    confirm_threshold: 0.65
    suggest_threshold: 0.45
"#;

    const SAMPLE_ENTITY_TYPES: &str = r#"
version: "1.0"
description: "Test entity types"
entity_types:
  manager_reference:
    description: "Investment manager reference"
    patterns:
      - type: NAME
        fuzzy_match: true
        mappings:
          "blackrock": "BlackRock"
          "vanguard": "Vanguard"
          "pimco": "PIMCO"
          "fidelity": "Fidelity"
    normalization:
      uppercase: false
  market_reference:
    description: "Market reference"
    patterns:
      - type: MIC
        regex: "X[A-Z]{3}"
      - type: REGION
        mappings:
          "european": "European"
          "us": "US"
          "asian": "Asian"
    normalization:
      uppercase: true
  instrument_class_reference:
    description: "Instrument class reference"
    patterns:
      - type: NAME
        mappings:
          "equities": "EQUITY"
          "equity": "EQUITY"
          "bonds": "FIXED_INCOME"
          "fixed income": "FIXED_INCOME"
    normalization:
      uppercase: true
  instruction_method:
    description: "Instruction method"
    patterns:
      - type: CODE
        valid_values: ["CTM", "SWIFT", "FIX", "MANUAL"]
    normalization:
      uppercase: true
  pricing_source:
    description: "Pricing source"
    patterns:
      - type: NAME
        mappings:
          "bloomberg": "BLOOMBERG"
          "refinitiv": "REFINITIV"
          "reuters": "REFINITIV"
    normalization:
      uppercase: true
  currency:
    description: "Currency"
    patterns:
      - type: ISO_CODE
        regex: "[A-Z]{3}"
    normalization:
      uppercase: true
extraction_config:
  extraction_order:
    - manager_reference
    - market_reference
    - instrument_class_reference
    - instruction_method
    - pricing_source
    - currency
"#;

    const SAMPLE_MARKET_REGIONS: &str = r#"
version: "1.0"
description: "Test market regions"
market_regions:
  European:
    description: "European markets"
    markets: ["XLON", "XETR", "XPAR", "XAMS", "XMIL"]
    aliases: ["Europe", "EU", "european"]
  US:
    description: "US markets"
    markets: ["XNYS", "XNAS", "XASE"]
    aliases: ["America", "american", "us"]
  Asian:
    description: "Asian markets"
    markets: ["XTKS", "XHKG", "XSES"]
    aliases: ["Asia", "asia", "asian"]
"#;

    const SAMPLE_INSTRUMENT_HIERARCHY: &str = r#"
version: "1.0"
description: "Test instrument hierarchy"
instrument_hierarchy:
  all:
    name: "All Instruments"
    children: ["equity", "fixed_income"]
  equity:
    name: "Equity"
    code: "EQUITY"
    aliases: ["equities", "stocks"]
  fixed_income:
    name: "Fixed Income"
    code: "FIXED_INCOME"
    aliases: ["bonds", "debt"]
    children: ["govt_bond", "corp_bond"]
  govt_bond:
    name: "Government Bonds"
    code: "GOVT_BOND"
    aliases: ["government bonds", "sovereigns"]
  corp_bond:
    name: "Corporate Bonds"
    code: "CORP_BOND"
    aliases: ["corporate bonds", "corporates"]
"#;

    const SAMPLE_PARAMETER_MAPPINGS: &str = r#"
version: "1.0"
description: "Test parameter mappings"
parameter_mappings:
  investment-manager.assign:
    description: "Assign investment manager"
    mappings:
      - entity_type: manager_reference
        param: manager-name
        required: true
      - entity_type: market_reference
        param: scope-markets
        is_list: true
      - entity_type: instrument_class_reference
        param: scope-instrument-classes
        is_list: true
      - entity_type: instruction_method
        param: instruction-method
        default_if_missing: "SWIFT"
      - entity_type: cbu_reference
        param: cbu-id
        source: context
        fallback: session.current_cbu
    defaults:
      priority: 100
      can-trade: true
      can-settle: true
    symbol_template: "@im-{manager-name}"
    symbol_transform: lowercase_hyphenate

  pricing-config.set:
    description: "Set pricing configuration"
    mappings:
      - entity_type: pricing_source
        param: source
        required: true
      - entity_type: instrument_class_reference
        param: instrument-class
        is_list: true
        expansion_mode: cartesian
      - entity_type: cbu_reference
        param: cbu-id
        source: context
        fallback: session.current_cbu
    defaults:
      priority: 1
    symbol_template: "@pricing-{instrument-class}"
    symbol_transform: lowercase_hyphenate
"#;

    /// Create test configurations
    fn create_test_configs() -> (
        IntentTaxonomy,
        EntityTypesConfig,
        MarketRegionsConfig,
        InstrumentHierarchyConfig,
        DslGenerator,
    ) {
        let taxonomy = IntentTaxonomy::load_from_str(SAMPLE_TAXONOMY).unwrap();
        let entity_types = EntityTypesConfig::load_from_str(SAMPLE_ENTITY_TYPES).unwrap();
        let market_regions = MarketRegionsConfig::load_from_str(SAMPLE_MARKET_REGIONS).unwrap();
        let instrument_hierarchy =
            InstrumentHierarchyConfig::load_from_str(SAMPLE_INSTRUMENT_HIERARCHY).unwrap();
        let dsl_generator = DslGenerator::load_from_str(SAMPLE_PARAMETER_MAPPINGS).unwrap();

        (
            taxonomy,
            entity_types,
            market_regions,
            instrument_hierarchy,
            dsl_generator,
        )
    }

    /// Create a test session ID
    #[allow(dead_code)]
    fn test_session_id() -> Uuid {
        Uuid::new_v4()
    }

    // ==================== Intent Classification Tests ====================

    #[test]
    fn test_taxonomy_parsing() {
        let taxonomy = IntentTaxonomy::load_from_str(SAMPLE_TAXONOMY).unwrap();
        let all_intents = taxonomy.all_intents();
        println!("Total intents found: {}", all_intents.len());
        for intent in &all_intents {
            println!(
                "  Intent: {} - {} trigger phrases",
                intent.intent,
                intent.trigger_phrases.len()
            );
        }
        assert!(
            !all_intents.is_empty(),
            "Should have parsed at least one intent"
        );
    }

    #[test]
    fn test_classify_simple_im_intent() {
        let (taxonomy, _, _, _, _) = create_test_configs();
        let classifier = IntentClassifier::new(taxonomy);
        let context = ConversationContext::default();

        let result = classifier.classify("Add BlackRock as investment manager", &context);

        assert!(
            !result.intents.is_empty(),
            "Should classify at least one intent"
        );
        assert_eq!(result.intents[0].intent_id, "im_assign");
        assert!(result.intents[0].confidence > 0.5);
    }

    #[test]
    fn test_classify_pricing_intent() {
        let (taxonomy, _, _, _, _) = create_test_configs();
        let classifier = IntentClassifier::new(taxonomy);
        let context = ConversationContext::default();

        let result = classifier.classify("Use Bloomberg for pricing", &context);

        assert!(!result.intents.is_empty());
        assert_eq!(result.intents[0].intent_id, "pricing_set");
    }

    #[test]
    fn test_classify_with_context_boost() {
        let (taxonomy, _, _, _, _) = create_test_configs();
        let classifier = IntentClassifier::new(taxonomy);

        // First classification
        let context = ConversationContext::default();
        let _result1 = classifier.classify("Add BlackRock as IM", &context);

        // Second classification with context - should boost followup intents
        let mut context2 = ConversationContext::default();
        context2.last_intent = Some("im_assign".to_string());

        let result2 = classifier.classify("also add Vanguard", &context2);

        assert!(!result2.intents.is_empty());
        // im_assign should be boosted due to natural followup
        assert_eq!(result2.intents[0].intent_id, "im_assign");
    }

    // ==================== Entity Extraction Tests ====================

    #[test]
    fn test_extract_manager_reference() {
        let (_, entity_types, market_regions, instrument_hierarchy, _) = create_test_configs();
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let entities = extractor.extract("Add BlackRock as investment manager", &context);

        assert!(entities.has_type("manager_reference"));
        let manager = entities.first_of_type("manager_reference").unwrap();
        assert_eq!(manager.value, "BlackRock");
    }

    #[test]
    fn test_extract_market_mic() {
        let (_, entity_types, market_regions, instrument_hierarchy, _) = create_test_configs();
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let entities = extractor.extract("Trade on XLON", &context);

        assert!(entities.has_type("market_reference"));
        let market = entities.first_of_type("market_reference").unwrap();
        assert_eq!(market.value, "XLON");
    }

    #[test]
    fn test_expand_market_region() {
        let (_, entity_types, market_regions, instrument_hierarchy, _) = create_test_configs();
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let entities = extractor.extract("European equities", &context);

        // Should expand "European" to individual markets
        let markets = entities.values_for_type("market_reference");
        assert!(markets.contains(&"XLON") || markets.contains(&"European"));
    }

    #[test]
    fn test_extract_instruction_method() {
        let (_, entity_types, market_regions, instrument_hierarchy, _) = create_test_configs();
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let entities = extractor.extract("Use CTM for instructions", &context);

        assert!(entities.has_type("instruction_method"));
        let method = entities.first_of_type("instruction_method").unwrap();
        assert_eq!(method.value, "CTM");
    }

    #[test]
    fn test_extract_multiple_entities() {
        let (_, entity_types, market_regions, instrument_hierarchy, _) = create_test_configs();
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let entities = extractor.extract("Add BlackRock for European equities via CTM", &context);

        assert!(entities.has_type("manager_reference"));
        assert!(entities.has_type("market_reference"));
        assert!(entities.has_type("instrument_class_reference"));
        assert!(entities.has_type("instruction_method"));
    }

    // ==================== DSL Generation Tests ====================

    #[test]
    fn test_generate_simple_im_dsl() {
        let (taxonomy, entity_types, market_regions, instrument_hierarchy, dsl_generator) =
            create_test_configs();

        let classifier = IntentClassifier::new(taxonomy);
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let utterance = "Add BlackRock as investment manager";
        let classification = classifier.classify(utterance, &context);
        let entities = extractor.extract(utterance, &context);

        let gen_context = GenerationContext::default();
        let generated =
            dsl_generator.generate(&classification.intents, &entities, &context, &gen_context);

        assert!(!generated.statements.is_empty());
        let dsl = generated.to_dsl_string();
        assert!(dsl.contains("investment-manager.assign"));
        assert!(dsl.contains("BlackRock"));
    }

    #[test]
    fn test_generate_dsl_with_defaults() {
        let (taxonomy, entity_types, market_regions, instrument_hierarchy, dsl_generator) =
            create_test_configs();

        let classifier = IntentClassifier::new(taxonomy);
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let utterance = "Add Vanguard as IM";
        let classification = classifier.classify(utterance, &context);
        let entities = extractor.extract(utterance, &context);

        let gen_context = GenerationContext::default();
        let generated =
            dsl_generator.generate(&classification.intents, &entities, &context, &gen_context);

        let dsl = generated.to_dsl_string();
        // Should include default values like priority and instruction-method
        assert!(dsl.contains("investment-manager.assign"));
    }

    #[test]
    fn test_generate_symbol() {
        let (taxonomy, entity_types, market_regions, instrument_hierarchy, dsl_generator) =
            create_test_configs();

        let classifier = IntentClassifier::new(taxonomy);
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let utterance = "Add BlackRock as investment manager";
        let classification = classifier.classify(utterance, &context);
        let entities = extractor.extract(utterance, &context);

        let gen_context = GenerationContext::default();
        let generated =
            dsl_generator.generate(&classification.intents, &entities, &context, &gen_context);

        assert!(!generated.statements.is_empty());
        // Check that a symbol was generated
        let stmt = &generated.statements[0];
        assert!(stmt.capture_symbol.is_some());
        let symbol = stmt.capture_symbol.as_ref().unwrap();
        assert!(symbol.starts_with("@im-"));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_empty_utterance() {
        let (taxonomy, entity_types, market_regions, instrument_hierarchy, _) =
            create_test_configs();
        let classifier = IntentClassifier::new(taxonomy);
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let classification = classifier.classify("", &context);
        let entities = extractor.extract("", &context);

        assert!(classification.intents.is_empty() || classification.intents[0].confidence < 0.3);
        assert!(entities.is_empty());
    }

    #[test]
    fn test_ambiguous_utterance() {
        let (taxonomy, _, _, _, _) = create_test_configs();
        let classifier = IntentClassifier::new(taxonomy);
        let context = ConversationContext::default();

        // Very ambiguous - should have low confidence
        let result = classifier.classify("set up the thing", &context);

        if !result.intents.is_empty() {
            // Should have low confidence
            assert!(result.intents[0].confidence < 0.7);
        }
    }

    #[test]
    fn test_unknown_entity() {
        let (_, entity_types, market_regions, instrument_hierarchy, _) = create_test_configs();
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        // "XYZ Corp" is not in our mappings
        let entities = extractor.extract("Add XYZ Corp as manager", &context);

        // Should not extract unknown manager
        let managers = entities.values_for_type("manager_reference");
        assert!(!managers.contains(&"XYZ Corp"));
    }

    // ==================== Context Tests ====================

    #[test]
    fn test_cbu_from_context() {
        let (taxonomy, entity_types, market_regions, instrument_hierarchy, dsl_generator) =
            create_test_configs();

        let classifier = IntentClassifier::new(taxonomy);
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);
        let context = ConversationContext::default();

        let utterance = "Add BlackRock as IM";
        let classification = classifier.classify(utterance, &context);
        let entities = extractor.extract(utterance, &context);

        // Provide CBU in generation context
        let gen_context = GenerationContext {
            cbu_id: Some("test-cbu-123".to_string()),
            profile_id: None,
            available_symbols: vec![],
            created_entities: std::collections::HashMap::new(),
        };

        let generated =
            dsl_generator.generate(&classification.intents, &entities, &context, &gen_context);

        assert!(!generated.statements.is_empty());
        // The DSL should include the CBU reference
        let stmt = &generated.statements[0];
        if let Some(cbu_param) = stmt.params.get("cbu-id") {
            assert_eq!(cbu_param.value.as_str().unwrap(), "test-cbu-123");
        }
    }

    #[test]
    fn test_session_entity_resolution() {
        let (_, entity_types, market_regions, instrument_hierarchy, _) = create_test_configs();
        let mut extractor =
            EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);

        // Set up context with previous entity
        let mut context = ConversationContext::default();
        context
            .session_entities
            .insert("manager_reference".to_string(), "BlackRock".to_string());

        // "them" should resolve to BlackRock
        let entities = extractor.extract("now connect them via CTM", &context);

        // Should resolve coreference
        let managers = entities.values_for_type("manager_reference");
        assert!(managers.contains(&"BlackRock"));
    }
}
