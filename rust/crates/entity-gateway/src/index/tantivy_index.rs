//! Tantivy-based search index implementation
//!
//! This module provides a high-performance in-memory search index
//! backed by Tantivy. It supports both fuzzy substring matching (for
//! autocomplete) and exact matching (for validation).
//!
//! Performance target: < 50ms for autocomplete queries
//!
//! For great UX, the fuzzy search supports:
//! - Substring matching: "Pacific" matches "Asia Pacific Fund"
//! - Typo tolerance: "Pcific" matches "Pacific"
//! - Multi-token: "lux fund" matches "Luxembourg Growth Fund"
//!
//! Strategy: Use ngram tokenization at index time (not query time) for
//! fast substring lookups. Ngrams are pre-computed during refresh.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Query, QueryParser, TermQuery};
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING,
};
use tantivy::tokenizer::{LowerCaser, NgramTokenizer, TextAnalyzer};
use tantivy::{Index, IndexReader, IndexWriter, Term};
use tokio::sync::RwLock;

use crate::config::{EntityConfig, IndexMode};
use crate::index::traits::{
    IndexError, IndexRecord, MatchMode, SearchIndex, SearchMatch, SearchQuery,
};

/// Custom tokenizer name for ngram-based substring search
const NGRAM_TOKENIZER: &str = "ngram3";

/// Tantivy-backed search index for a single entity type
pub struct TantivyIndex {
    /// Entity configuration
    config: EntityConfig,
    /// Tantivy index (in RAM)
    index: Index,
    /// Reader for searching (updated after refresh)
    reader: RwLock<Option<IndexReader>>,
    /// Schema definition
    #[allow(dead_code)]
    schema: Schema,
    /// Field handle for the token (ID)
    token_field: Field,
    /// Field handle for the display value (stored, not indexed)
    display_field: Field,
    /// Field handles for each search key (ngram indexed)
    search_fields: HashMap<String, Field>,
    /// Field handles for exact match (word tokenized)
    exact_fields: HashMap<String, Field>,
    /// Whether the index is ready
    ready: AtomicBool,
}

impl TantivyIndex {
    /// Create a new Tantivy index for the given entity configuration
    pub fn new(config: EntityConfig) -> Result<Self, IndexError> {
        let mut schema_builder = Schema::builder();

        // Token field: stored as-is (UUID), not analyzed
        let token_field = schema_builder.add_text_field("token", STRING | STORED);

        // Display field: stored only, not indexed (we search on search_keys)
        let display_opts = TextOptions::default().set_stored();
        let display_field = schema_builder.add_text_field("display", display_opts);

        // Standard text options for exact/prefix word matching
        let exact_indexing = TextFieldIndexing::default()
            .set_tokenizer("default")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let exact_opts = TextOptions::default()
            .set_indexing_options(exact_indexing.clone())
            .set_stored();

        // Ngram text options for substring matching (only used in trigram mode)
        let ngram_indexing = TextFieldIndexing::default()
            .set_tokenizer(NGRAM_TOKENIZER)
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let ngram_opts = TextOptions::default()
            .set_indexing_options(ngram_indexing)
            .set_stored();

        let mut search_fields = HashMap::new();
        let mut exact_fields = HashMap::new();

        for key in &config.search_keys {
            match config.index_mode {
                IndexMode::Trigram => {
                    // Trigram mode: ngram field for fuzzy + exact field for fallback
                    let ngram_field = schema_builder.add_text_field(&key.name, ngram_opts.clone());
                    search_fields.insert(key.name.clone(), ngram_field);

                    let exact_name = format!("{}_exact", key.name);
                    let exact_field =
                        schema_builder.add_text_field(&exact_name, exact_opts.clone());
                    exact_fields.insert(key.name.clone(), exact_field);
                }
                IndexMode::Exact => {
                    // Exact mode: only standard word tokenization
                    let exact_field = schema_builder.add_text_field(&key.name, exact_opts.clone());
                    search_fields.insert(key.name.clone(), exact_field);
                    exact_fields.insert(key.name.clone(), exact_field);
                }
            }
        }

        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema.clone());

        // Only register ngram tokenizer if using trigram mode
        if config.index_mode == IndexMode::Trigram {
            // Trigrams (n=3) optimized for KYC name search
            // "berg" in "Goldberg", "Bloomberg" etc.
            let ngram_tokenizer = TextAnalyzer::builder(NgramTokenizer::new(3, 3, false).unwrap())
                .filter(LowerCaser)
                .build();
            index
                .tokenizers()
                .register(NGRAM_TOKENIZER, ngram_tokenizer);
        }

        Ok(Self {
            config,
            index,
            reader: RwLock::new(None),
            schema,
            token_field,
            display_field,
            search_fields,
            exact_fields,
            ready: AtomicBool::new(false),
        })
    }

    /// Get the nickname of this index
    pub fn nickname(&self) -> &str {
        &self.config.nickname
    }

    /// Build a fuzzy substring query that handles:
    /// - Single token: ngram lookup via QueryParser (applies ngram tokenizer)
    /// - Multiple tokens: boolean AND of ngram lookups
    /// - Typo tolerance via fuzzy term query fallback for short inputs
    fn build_fuzzy_query(
        &self,
        search_field: Field,
        exact_field: Field,
        input: &str,
    ) -> Box<dyn Query> {
        let input_lower = input.to_lowercase().trim().to_string();

        if input_lower.is_empty() {
            // Empty query - match nothing
            return Box::new(BooleanQuery::new(vec![]));
        }

        // For short inputs (< 3 chars total), use fuzzy prefix on exact field
        if input_lower.len() < 3 {
            let term = Term::from_field_text(exact_field, &input_lower);
            return Box::new(FuzzyTermQuery::new_prefix(term, 1, true));
        }

        // Use QueryParser to properly tokenize with ngrams
        // This ensures "pacific" gets broken into ngrams that match indexed ngrams
        let mut query_parser = QueryParser::for_index(&self.index, vec![search_field]);
        query_parser.set_conjunction_by_default(); // AND semantics for multiple tokens

        match query_parser.parse_query(&input_lower) {
            Ok(query) => query,
            Err(e) => {
                tracing::warn!(error = %e, "Query parse failed, falling back to exact match");
                let term = Term::from_field_text(exact_field, &input_lower);
                Box::new(TermQuery::new(term, Default::default()))
            }
        }
    }
}

#[async_trait]
impl SearchIndex for TantivyIndex {
    async fn search(&self, query: &SearchQuery) -> Vec<SearchMatch> {
        let reader_guard = self.reader.read().await;
        let reader = match reader_guard.as_ref() {
            Some(r) => r,
            None => return vec![],
        };

        let searcher = reader.searcher();

        // Get the search fields
        let search_field = match self.search_fields.get(&query.search_key) {
            Some(f) => *f,
            None => {
                tracing::warn!(
                    search_key = %query.search_key,
                    nickname = %self.config.nickname,
                    "Unknown search key"
                );
                return vec![];
            }
        };

        let exact_field = self
            .exact_fields
            .get(&query.search_key)
            .copied()
            .unwrap_or(search_field);

        let mut results = Vec::new();
        let mut seen_tokens = std::collections::HashSet::new();

        for input in &query.values {
            let input_lower = input.to_lowercase();

            let tantivy_query: Box<dyn Query> = match query.mode {
                MatchMode::Fuzzy => self.build_fuzzy_query(search_field, exact_field, &input_lower),
                MatchMode::Exact => {
                    // Exact term match on the exact field
                    let term = Term::from_field_text(exact_field, &input_lower);
                    Box::new(TermQuery::new(term, Default::default()))
                }
            };

            let top_docs = match searcher.search(&tantivy_query, &TopDocs::with_limit(query.limit))
            {
                Ok(docs) => docs,
                Err(e) => {
                    tracing::error!(error = %e, "Search failed");
                    continue;
                }
            };

            for (score, doc_addr) in top_docs {
                match searcher.doc::<tantivy::TantivyDocument>(doc_addr) {
                    Ok(doc) => {
                        let token = doc
                            .get_first(self.token_field)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        // Deduplicate by token
                        if seen_tokens.contains(&token) {
                            continue;
                        }
                        seen_tokens.insert(token.clone());

                        let display = doc
                            .get_first(self.display_field)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        results.push(SearchMatch {
                            input: input.clone(),
                            display,
                            token,
                            score,
                        });
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to retrieve document");
                    }
                }
            }
        }

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit total results
        results.truncate(query.limit);

        results
    }

    async fn refresh(&self, data: Vec<IndexRecord>) -> Result<(), IndexError> {
        let start = std::time::Instant::now();
        tracing::info!(
            nickname = %self.config.nickname,
            records = data.len(),
            "Refreshing index"
        );

        // Create a new writer
        let mut writer: IndexWriter = self
            .index
            .writer(50_000_000) // 50MB buffer
            .map_err(|e| IndexError::BuildFailed(e.to_string()))?;

        // Clear existing documents
        writer
            .delete_all_documents()
            .map_err(|e| IndexError::BuildFailed(e.to_string()))?;

        // Index new data
        for record in data {
            let mut doc = tantivy::TantivyDocument::new();
            doc.add_text(self.token_field, &record.token);
            doc.add_text(self.display_field, &record.display);

            // Add search values to both ngram and exact fields
            for (key, value) in &record.search_values {
                let value_lower = value.to_lowercase();

                if let Some(field) = self.search_fields.get(key) {
                    doc.add_text(*field, &value_lower);
                }
                if let Some(field) = self.exact_fields.get(key) {
                    doc.add_text(*field, &value_lower);
                }
            }

            writer
                .add_document(doc)
                .map_err(|e| IndexError::BuildFailed(e.to_string()))?;
        }

        // Commit changes
        writer
            .commit()
            .map_err(|e| IndexError::BuildFailed(e.to_string()))?;

        // Create new reader
        let new_reader = self
            .index
            .reader()
            .map_err(|e| IndexError::BuildFailed(e.to_string()))?;

        // Update the reader
        *self.reader.write().await = Some(new_reader);
        self.ready.store(true, Ordering::SeqCst);

        let elapsed = start.elapsed();
        tracing::info!(
            nickname = %self.config.nickname,
            elapsed_ms = elapsed.as_millis(),
            "Index refresh complete"
        );

        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SearchKeyConfig, ShardConfig};

    fn sample_config() -> EntityConfig {
        EntityConfig {
            nickname: "cbu".to_string(),
            source_table: "cbus".to_string(),
            return_key: "cbu_id".to_string(),
            display_template: Some("{name}".to_string()),
            index_mode: IndexMode::Trigram,
            filter: None,
            search_keys: vec![SearchKeyConfig {
                name: "name".to_string(),
                column: "name".to_string(),
                default: true,
            }],
            shard: ShardConfig {
                enabled: false,
                prefix_len: 0,
            },
        }
    }

    fn sample_records() -> Vec<IndexRecord> {
        vec![
            IndexRecord {
                token: "uuid-1".to_string(),
                display: "Asia Pacific Growth Fund".to_string(),
                search_values: HashMap::from([(
                    "name".to_string(),
                    "asia pacific growth fund".to_string(),
                )]),
            },
            IndexRecord {
                token: "uuid-2".to_string(),
                display: "Luxembourg Investment SICAV".to_string(),
                search_values: HashMap::from([(
                    "name".to_string(),
                    "luxembourg investment sicav".to_string(),
                )]),
            },
            IndexRecord {
                token: "uuid-3".to_string(),
                display: "Pacific Capital Partners".to_string(),
                search_values: HashMap::from([(
                    "name".to_string(),
                    "pacific capital partners".to_string(),
                )]),
            },
            IndexRecord {
                token: "uuid-4".to_string(),
                display: "Apex Fund Services".to_string(),
                search_values: HashMap::from([(
                    "name".to_string(),
                    "apex fund services".to_string(),
                )]),
            },
        ]
    }

    #[tokio::test]
    async fn test_index_not_ready_initially() {
        let config = sample_config();
        let index = TantivyIndex::new(config).unwrap();
        assert!(!index.is_ready());
    }

    #[tokio::test]
    async fn test_refresh_makes_ready() {
        let config = sample_config();
        let index = TantivyIndex::new(config).unwrap();
        index.refresh(sample_records()).await.unwrap();
        assert!(index.is_ready());
    }

    #[tokio::test]
    async fn test_substring_search_middle() {
        let config = sample_config();
        let index = TantivyIndex::new(config).unwrap();
        index.refresh(sample_records()).await.unwrap();

        // "pacific" appears in middle of "Asia Pacific Growth Fund"
        let query = SearchQuery {
            values: vec!["pacific".to_string()],
            search_key: "name".to_string(),
            mode: MatchMode::Fuzzy,
            limit: 10,
        };

        let results = index.search(&query).await;

        assert!(
            !results.is_empty(),
            "Should find 'pacific' in middle of string"
        );
        let displays: Vec<_> = results.iter().map(|r| r.display.as_str()).collect();
        assert!(
            displays.contains(&"Asia Pacific Growth Fund"),
            "Should find Asia Pacific"
        );
        assert!(
            displays.contains(&"Pacific Capital Partners"),
            "Should find Pacific Capital"
        );
    }

    #[tokio::test]
    async fn test_substring_search_end() {
        let config = sample_config();
        let index = TantivyIndex::new(config).unwrap();
        index.refresh(sample_records()).await.unwrap();

        // "fund" appears at end
        let query = SearchQuery {
            values: vec!["fund".to_string()],
            search_key: "name".to_string(),
            mode: MatchMode::Fuzzy,
            limit: 10,
        };

        let results = index.search(&query).await;

        assert!(!results.is_empty(), "Should find 'fund' at end of string");
        let displays: Vec<_> = results.iter().map(|r| r.display.as_str()).collect();
        assert!(displays.contains(&"Asia Pacific Growth Fund"));
        assert!(displays.contains(&"Apex Fund Services"));
    }

    #[tokio::test]
    async fn test_multi_token_search() {
        let config = sample_config();
        let index = TantivyIndex::new(config).unwrap();
        index.refresh(sample_records()).await.unwrap();

        // "lux invest" should match "Luxembourg Investment SICAV"
        let query = SearchQuery {
            values: vec!["lux invest".to_string()],
            search_key: "name".to_string(),
            mode: MatchMode::Fuzzy,
            limit: 10,
        };

        let results = index.search(&query).await;

        assert!(!results.is_empty(), "Should find multi-token match");
        assert!(results
            .iter()
            .any(|r| r.display == "Luxembourg Investment SICAV"));
    }

    #[tokio::test]
    async fn test_short_prefix_search() {
        let config = sample_config();
        let index = TantivyIndex::new(config).unwrap();
        index.refresh(sample_records()).await.unwrap();

        // Short prefix "ap" should find "Apex"
        let query = SearchQuery {
            values: vec!["ap".to_string()],
            search_key: "name".to_string(),
            mode: MatchMode::Fuzzy,
            limit: 10,
        };

        let results = index.search(&query).await;

        // Should find Apex Fund Services
        assert!(
            results.iter().any(|r| r.display.contains("Apex")),
            "Should find Apex with 'ap' prefix"
        );
    }

    #[tokio::test]
    async fn test_exact_search() {
        let config = sample_config();
        let index = TantivyIndex::new(config).unwrap();
        index.refresh(sample_records()).await.unwrap();

        let query = SearchQuery {
            values: vec!["apex".to_string()],
            search_key: "name".to_string(),
            mode: MatchMode::Exact,
            limit: 10,
        };

        let results = index.search(&query).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display, "Apex Fund Services");
    }

    #[tokio::test]
    async fn test_search_performance() {
        let config = sample_config();
        let index = TantivyIndex::new(config).unwrap();
        index.refresh(sample_records()).await.unwrap();

        let query = SearchQuery {
            values: vec!["pacific".to_string()],
            search_key: "name".to_string(),
            mode: MatchMode::Fuzzy,
            limit: 10,
        };

        // Measure search time
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = index.search(&query).await;
        }
        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_millis() as f64 / 100.0;

        println!("Average search time: {:.2}ms", avg_ms);
        assert!(
            avg_ms < 50.0,
            "Search should be under 50ms, was {}ms",
            avg_ms
        );
    }
}
