//! Tests for ESPER command registry

use super::*;
use ob_poc_types::AgentCommand;
use std::collections::HashMap;

fn test_config_yaml() -> &'static str {
    r#"
version: "1.0"
commands:
  stop:
    canonical: "stop"
    response: "Stopping."
    priority: 1000
    agent_command:
      type: Stop
    aliases:
      exact:
        - "stop"
        - "hold"
        - "freeze"
        - "that's good"

  zoom_in:
    canonical: "enhance"
    response: "Enhancing..."
    priority: 100
    agent_command:
      type: ZoomIn
      params:
        factor: extract
    aliases:
      exact:
        - "enhance"
        - "closer"
      prefix:
        - "zoom in"
        - "enhance "

  zoom_out:
    canonical: "zoom out"
    response: "Zooming out..."
    agent_command:
      type: ZoomOut
      params:
        factor: extract
    aliases:
      exact:
        - "wider"
      prefix:
        - "zoom out"
        - "pull back"

  scale_universe:
    canonical: "show universe"
    response: "Showing full universe..."
    agent_command:
      type: ScaleUniverse
    aliases:
      contains:
        - "universe"
        - "full book"
        - "god view"

  search:
    canonical: "find"
    response: "Searching..."
    priority: 80
    agent_command:
      type: Search
      params:
        query: rest_of_phrase
    aliases:
      prefix:
        - "find "
        - "search "

  pan:
    canonical: "track"
    response: "Tracking..."
    priority: 90
    agent_command:
      type: Pan
      params:
        direction: extract
        amount: extract
    aliases:
      exact:
        - "left"
        - "right"
        - "up"
        - "down"
      prefix:
        - "track "
        - "pan "

  export:
    canonical: "give me a hard copy"
    response: "Generating hard copy..."
    agent_command:
      type: Export
      params:
        format: extract
    aliases:
      contains:
        - "hard copy"
        - "export"
"#
}

fn test_registry() -> EsperCommandRegistry {
    let config = EsperConfig::from_yaml(test_config_yaml()).unwrap();
    EsperCommandRegistry::new(config, HashMap::new())
}

#[test]
fn test_exact_match() {
    let registry = test_registry();
    let m = registry.lookup("enhance").unwrap();
    assert_eq!(m.command_key, "zoom_in");
    assert!(matches!(m.command, AgentCommand::ZoomIn { factor: None }));
    assert_eq!(m.response, "Enhancing...");
    assert_eq!(m.source, MatchSource::Builtin);
}

#[test]
fn test_exact_match_stop_priority() {
    let registry = test_registry();

    // "stop" has priority 1000, should match
    let m = registry.lookup("stop").unwrap();
    assert_eq!(m.command_key, "stop");
    assert!(matches!(m.command, AgentCommand::Stop));
}

#[test]
fn test_contains_match() {
    let registry = test_registry();

    // "universe" is in contains list
    let m = registry.lookup("show me the universe please").unwrap();
    assert_eq!(m.command_key, "scale_universe");
    assert!(matches!(m.command, AgentCommand::ScaleUniverse));

    // "god view" also works
    let m = registry.lookup("give me the god view").unwrap();
    assert_eq!(m.command_key, "scale_universe");
}

#[test]
fn test_prefix_match_with_query() {
    let registry = test_registry();

    let m = registry.lookup("find Goldman Sachs").unwrap();
    assert_eq!(m.command_key, "search");
    match &m.command {
        AgentCommand::Search { query } => {
            // Query is extracted from normalized (lowercase) phrase
            assert_eq!(query, "goldman sachs");
        }
        _ => panic!("Expected Search command"),
    }

    let m = registry.lookup("search for something").unwrap();
    assert_eq!(m.command_key, "search");
}

#[test]
fn test_factor_extraction() {
    let registry = test_registry();

    // Plain number
    let m = registry.lookup("zoom in 2.5").unwrap();
    match &m.command {
        AgentCommand::ZoomIn { factor } => {
            let f = factor.unwrap();
            assert!((f - 2.5).abs() < 0.01);
        }
        _ => panic!("Expected ZoomIn"),
    }

    // With 'x' suffix
    let m = registry.lookup("enhance 3x").unwrap();
    match &m.command {
        AgentCommand::ZoomIn { factor } => {
            let f = factor.unwrap();
            assert!((f - 3.0).abs() < 0.01);
        }
        _ => panic!("Expected ZoomIn"),
    }
}

#[test]
fn test_percentage_extraction() {
    let registry = test_registry();

    let m = registry.lookup("enhance 50%").unwrap();
    match &m.command {
        AgentCommand::ZoomIn { factor } => {
            let f = factor.unwrap();
            assert!((f - 0.5).abs() < 0.01);
        }
        _ => panic!("Expected ZoomIn"),
    }
}

#[test]
fn test_direction_extraction() {
    let registry = test_registry();

    let m = registry.lookup("track left").unwrap();
    match &m.command {
        AgentCommand::Pan { direction, .. } => {
            assert_eq!(*direction, ob_poc_types::PanDirection::Left);
        }
        _ => panic!("Expected Pan"),
    }

    // Simple direction command
    let m = registry.lookup("right").unwrap();
    match &m.command {
        AgentCommand::Pan { direction, .. } => {
            assert_eq!(*direction, ob_poc_types::PanDirection::Right);
        }
        _ => panic!("Expected Pan"),
    }
}

#[test]
fn test_case_insensitivity() {
    let registry = test_registry();

    assert!(registry.lookup("ENHANCE").is_some());
    assert!(registry.lookup("Zoom In 2x").is_some());
    assert!(registry.lookup("UNIVERSE").is_some());
    assert!(registry.lookup("Stop").is_some());
}

#[test]
fn test_whitespace_normalization() {
    let registry = test_registry();

    assert!(registry.lookup("  enhance  ").is_some());
    assert!(registry.lookup("\tstop\n").is_some());
}

#[test]
fn test_learned_alias() {
    let mut registry = test_registry();

    // Before learning
    assert!(registry.lookup("make it bigger").is_none());

    // Add learned alias
    registry.add_learned_alias("make it bigger", "zoom_in");

    // After learning
    let m = registry.lookup("make it bigger").unwrap();
    assert_eq!(m.command_key, "zoom_in");
    assert_eq!(m.source, MatchSource::Learned);
    assert!(matches!(m.command, AgentCommand::ZoomIn { .. }));
}

#[test]
fn test_no_match_returns_none() {
    let registry = test_registry();

    assert!(registry.lookup("what is the weather").is_none());
    assert!(registry.lookup("hello world").is_none());
    assert!(registry.lookup("create entity").is_none());
    assert!(registry.lookup("").is_none());
}

#[test]
fn test_format_extraction() {
    let registry = test_registry();

    let m = registry.lookup("export as svg").unwrap();
    match &m.command {
        AgentCommand::Export { format } => {
            assert_eq!(format.as_deref(), Some("svg"));
        }
        _ => panic!("Expected Export"),
    }

    let m = registry.lookup("give me a hard copy in pdf").unwrap();
    match &m.command {
        AgentCommand::Export { format } => {
            assert_eq!(format.as_deref(), Some("pdf"));
        }
        _ => panic!("Expected Export"),
    }

    // Default to png
    let m = registry.lookup("hard copy").unwrap();
    match &m.command {
        AgentCommand::Export { format } => {
            assert_eq!(format.as_deref(), Some("png"));
        }
        _ => panic!("Expected Export"),
    }
}

#[test]
fn test_command_count() {
    let registry = test_registry();
    assert_eq!(registry.command_count(), 7);
}

#[test]
fn test_has_command() {
    let registry = test_registry();
    assert!(registry.has_command("zoom_in"));
    assert!(registry.has_command("stop"));
    assert!(!registry.has_command("nonexistent"));
}

#[test]
fn test_list_commands() {
    let registry = test_registry();
    let commands: Vec<_> = registry.list_commands().collect();
    assert_eq!(commands.len(), 7);

    let keys: Vec<_> = commands.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"stop"));
    assert!(keys.contains(&"zoom_in"));
    assert!(keys.contains(&"scale_universe"));
}

#[test]
fn test_load_actual_config() {
    // Test loading the actual esper-commands.yaml file
    let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join("esper-commands.yaml");

    if config_path.exists() {
        let config = EsperConfig::load(&config_path).expect("Failed to load esper-commands.yaml");

        // Should have 48 commands
        assert!(
            config.commands.len() >= 40,
            "Expected at least 40 commands, got {}",
            config.commands.len()
        );

        // Key commands should exist
        assert!(config.commands.contains_key("stop"));
        assert!(config.commands.contains_key("zoom_in"));
        assert!(config.commands.contains_key("zoom_out"));
        assert!(config.commands.contains_key("scale_universe"));
        assert!(config.commands.contains_key("xray"));
        assert!(config.commands.contains_key("drill_through"));

        // Build registry and test some lookups
        let registry = EsperCommandRegistry::new(config, HashMap::new());

        // Test classic Blade Runner commands
        assert!(registry.lookup("enhance").is_some());
        assert!(registry.lookup("give me a hard copy").is_some());
        assert!(registry.lookup("stop").is_some());

        // Test astronomical navigation
        assert!(registry.lookup("show me the universe").is_some());
        assert!(registry.lookup("galaxy view").is_some());

        // Test investigation commands
        assert!(registry.lookup("follow the money").is_some());
        assert!(registry.lookup("red flag scan").is_some());
    }
}

// ============================================================================
// Semantic Index Tests (Phase 8)
// ============================================================================

#[test]
fn test_semantic_index_empty() {
    let index = registry::SemanticIndex::new();
    assert!(!index.ready);
    assert!(index.is_empty());
    assert_eq!(index.len(), 0);

    // Search on empty index returns empty
    let results = index.search(&[0.1, 0.2, 0.3], 3);
    assert!(results.is_empty());
}

#[test]
fn test_semantic_index_add_and_search() {
    let mut index = registry::SemanticIndex::new();

    // Add some test embeddings (L2 normalized for cosine similarity)
    // These are fake embeddings but normalized to unit length
    let emb1 = normalize(&[1.0, 0.0, 0.0]);
    let emb2 = normalize(&[0.0, 1.0, 0.0]);
    let emb3 = normalize(&[0.7, 0.7, 0.0]);

    index.add_alias("enhance".to_string(), "zoom_in".to_string(), emb1.clone());
    index.add_alias("zoom in".to_string(), "zoom_in".to_string(), emb3.clone());
    index.add_alias("stop".to_string(), "stop".to_string(), emb2.clone());
    index.mark_ready();

    assert!(index.ready);
    assert_eq!(index.len(), 3);

    // Query similar to emb1 should match "enhance"
    let query = normalize(&[0.9, 0.1, 0.0]);
    let results = index.search(&query, 2);

    assert_eq!(results.len(), 2);
    // First result should be closest to query
    assert!(results[0].confidence > results[1].confidence);
}

#[test]
fn test_semantic_index_not_ready() {
    let mut index = registry::SemanticIndex::new();
    index.add_alias("test".to_string(), "cmd".to_string(), vec![0.1, 0.2, 0.3]);
    // Don't call mark_ready()

    // Search should return empty when not ready
    let results = index.search(&[0.1, 0.2, 0.3], 3);
    assert!(results.is_empty());
}

#[test]
fn test_lookup_with_semantic_trie_hit() {
    let registry = test_registry();

    // Trie hit - should not need semantic
    let result = registry.lookup_with_semantic("enhance", None);
    match result {
        registry::LookupResult::Matched(m) => {
            assert_eq!(m.command_key, "zoom_in");
        }
        _ => panic!("Expected Matched result"),
    }
}

#[test]
fn test_lookup_with_semantic_no_index() {
    let registry = test_registry();

    // Phrase that doesn't match trie
    let fake_embedding = vec![0.1; 384];
    let result = registry.lookup_with_semantic("make it bigger", Some(&fake_embedding));

    // Without semantic index ready, should return NoMatch
    match result {
        registry::LookupResult::NoMatch => {}
        _ => panic!("Expected NoMatch without semantic index"),
    }
}

#[test]
fn test_all_aliases_extraction() {
    let registry = test_registry();
    let aliases = registry.all_aliases();

    // Should have aliases from all commands
    assert!(!aliases.is_empty());

    // Check some expected aliases exist
    let alias_texts: Vec<_> = aliases.iter().map(|(text, _)| text.as_str()).collect();
    assert!(alias_texts.contains(&"enhance"));
    assert!(alias_texts.contains(&"stop"));
    assert!(alias_texts.contains(&"universe"));
}

#[test]
fn test_registry_stats() {
    let registry = test_registry();
    let stats = registry.stats();

    assert_eq!(stats.total_commands, 7);
    assert!(stats.exact_count > 0);
    assert!(stats.contains_count > 0);
    assert!(stats.prefix_count > 0);
    assert_eq!(stats.learned_count, 0);
}

/// Helper to normalize a vector to unit length (for cosine similarity)
fn normalize(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        v.to_vec()
    } else {
        v.iter().map(|x| x / norm).collect()
    }
}
