//! RAG Context Provider for DSL Generation
//!
//! Retrieves relevant context from Runtime vocabulary, dictionary,
//! and example DSL corpus to guide LLM generation.
//!
//! The vocabulary now comes from the in-memory Runtime (vocab_registry.rs)
//! rather than the deprecated vocabulary_registry DB table.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::forth_engine::runtime::Runtime;

#[derive(Clone)]
pub struct RagContextProvider {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagContext {
    /// Valid vocabulary verbs for the operation
    pub vocabulary: Vec<VocabEntry>,

    /// Similar DSL examples from corpus
    pub examples: Vec<DslExample>,

    /// Relevant attributes from dictionary
    pub attributes: Vec<AttributeDefinition>,

    /// Grammar hints (EBNF snippets)
    pub grammar_hints: Vec<String>,

    /// Business constraints
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VocabEntry {
    pub verb_name: String,
    pub signature: String,
    pub description: Option<String>,
    pub examples: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DslExample {
    pub dsl_text: String,
    pub natural_language_input: Option<String>,
    pub confidence_score: Option<f64>,
    pub operation_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AttributeDefinition {
    pub semantic_id: String,
    pub name: String,
    pub data_type: String,
    pub description: Option<String>,
    pub validation_rules: Option<serde_json::Value>,
}

impl RagContextProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get comprehensive context for DSL generation using Runtime vocabulary
    ///
    /// This is the preferred method - vocabulary comes from in-memory Runtime
    /// rather than the deprecated DB table.
    pub fn get_context_with_runtime(
        &self,
        runtime: &Runtime,
        operation_type: &str,
        query: &str,
        domain: Option<&str>,
    ) -> Result<RagContext> {
        // Get vocabulary from Runtime (in-memory, not DB)
        let vocabulary = self.get_vocab_from_runtime(runtime, domain);

        // These still come from DB (they have user data)
        // Use block_on since we're in a sync context
        let handle = tokio::runtime::Handle::try_current();
        let (examples, attributes) = if let Ok(h) = handle {
            h.block_on(async {
                tokio::join!(
                    self.search_examples(query, operation_type),
                    self.query_attributes(query, domain)
                )
            })
        } else {
            // Fallback if no runtime - return empty results
            (Ok(Vec::new()), Ok(Vec::new()))
        };

        Ok(RagContext {
            vocabulary,
            examples: examples?,
            attributes: attributes?,
            grammar_hints: self.get_grammar_hints(operation_type),
            constraints: self.get_constraints(operation_type, domain),
        })
    }

    /// Extract vocabulary from Runtime
    fn get_vocab_from_runtime(&self, runtime: &Runtime, domain: Option<&str>) -> Vec<VocabEntry> {
        let words = if let Some(d) = domain {
            runtime.get_domain_words(d)
        } else {
            // Get all words
            runtime
                .get_all_word_names()
                .iter()
                .filter_map(|name| runtime.get_word(name))
                .collect()
        };

        words
            .iter()
            .map(|w| VocabEntry {
                verb_name: w.name.to_string(),
                signature: w.signature.to_string(),
                description: Some(w.description.to_string()),
                examples: Some(serde_json::json!(w.examples)),
            })
            .collect()
    }

    /// Get comprehensive context for DSL generation (DEPRECATED)
    ///
    /// Use get_context_with_runtime instead - vocabulary now comes from Runtime.
    #[allow(deprecated)]
    pub async fn get_context(
        &self,
        operation_type: &str,
        query: &str,
        domain: Option<&str>,
    ) -> Result<RagContext> {
        // Execute all queries concurrently
        let (vocabulary, examples, attributes) = tokio::join!(
            self.query_vocabulary(operation_type, domain),
            self.search_examples(query, operation_type),
            self.query_attributes(query, domain)
        );

        Ok(RagContext {
            vocabulary: vocabulary?,
            examples: examples?,
            attributes: attributes?,
            grammar_hints: self.get_grammar_hints(operation_type),
            constraints: self.get_constraints(operation_type, domain),
        })
    }

    /// Query vocabulary registry for valid verbs
    async fn query_vocabulary(
        &self,
        operation_type: &str,
        domain: Option<&str>,
    ) -> Result<Vec<VocabEntry>> {
        let results = if let Some(d) = domain {
            sqlx::query_as::<_, VocabEntry>(
                r#"
                SELECT verb_name, signature, description, examples
                FROM "ob-poc".vocabulary_registry
                WHERE
                    is_active = true
                    AND $1 = ANY(operation_types)
                    AND domain = $2
                ORDER BY usage_count DESC
                LIMIT 20
                "#,
            )
            .bind(operation_type)
            .bind(d)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query vocabulary with domain")?
        } else {
            sqlx::query_as::<_, VocabEntry>(
                r#"
                SELECT verb_name, signature, description, examples
                FROM "ob-poc".vocabulary_registry
                WHERE
                    is_active = true
                    AND $1 = ANY(operation_types)
                ORDER BY usage_count DESC
                LIMIT 20
                "#,
            )
            .bind(operation_type)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query vocabulary")?
        };

        Ok(results)
    }

    /// Search for similar DSL examples (keyword-based)
    async fn search_examples(&self, query: &str, operation_type: &str) -> Result<Vec<DslExample>> {
        let pattern = format!("%{}%", query);

        let results = sqlx::query_as::<_, DslExample>(
            r#"
            SELECT
                dsl_text, natural_language_input,
                confidence_score::float8 as confidence_score, operation_type
            FROM "ob-poc".dsl_instances
            WHERE
                execution_success = true
                AND operation_type = $1
                AND (
                    natural_language_input ILIKE $2
                    OR dsl_text ILIKE $2
                )
            ORDER BY
                confidence_score DESC NULLS LAST,
                created_at DESC
            LIMIT 5
            "#,
        )
        .bind(operation_type)
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await
        .context("Failed to search DSL examples")?;

        Ok(results)
    }

    /// Query relevant attributes from dictionary
    async fn query_attributes(
        &self,
        query: &str,
        domain: Option<&str>,
    ) -> Result<Vec<AttributeDefinition>> {
        let keywords = self.extract_keywords(query);

        if keywords.is_empty() {
            return Ok(Vec::new());
        }

        let pattern = format!("%{}%", keywords.join("%"));

        let results = if let Some(d) = domain {
            sqlx::query_as::<_, AttributeDefinition>(
                r#"
                SELECT semantic_id, name, data_type, description, validation_rules
                FROM "ob-poc".dictionary
                WHERE (
                    name ILIKE $1
                    OR semantic_id ILIKE $1
                    OR description ILIKE $1
                )
                AND business_domain = $2
                ORDER BY name
                LIMIT 10
                "#,
            )
            .bind(&pattern)
            .bind(d)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query attributes with domain")?
        } else {
            sqlx::query_as::<_, AttributeDefinition>(
                r#"
                SELECT semantic_id, name, data_type, description, validation_rules
                FROM "ob-poc".dictionary
                WHERE (
                    name ILIKE $1
                    OR semantic_id ILIKE $1
                    OR description ILIKE $1
                )
                ORDER BY name
                LIMIT 10
                "#,
            )
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query attributes")?
        };

        Ok(results)
    }

    /// Extract keywords from query for attribute matching
    fn extract_keywords(&self, query: &str) -> Vec<String> {
        Self::extract_keywords_from_text(query)
    }

    /// Static helper to extract keywords without requiring a RagContextProvider instance
    pub fn extract_keywords_from_text(query: &str) -> Vec<String> {
        let stop_words = [
            "with", "from", "create", "update", "delete", "read", "the", "a", "an", "for", "and",
            "or",
        ];
        query
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .filter(|w| !stop_words.contains(&w.to_lowercase().as_str()))
            .map(|w| w.to_lowercase())
            .collect()
    }

    /// Get EBNF grammar hints for operation type
    fn get_grammar_hints(&self, operation_type: &str) -> Vec<String> {
        match operation_type {
            "CREATE" | "CREATE_CBU" => vec![
                "s_expr ::= \"(\" word_call \")\"".to_string(),
                "word_call ::= SYMBOL { expr }".to_string(),
                "KEYWORD ::= \":\" SYMBOL".to_string(),
                "STRING ::= '\"' { char } '\"'".to_string(),
            ],
            "UPDATE" | "TRANSITION" => vec![
                "s_expr ::= \"(\" word_call \")\"".to_string(),
                "state_transition ::= \"(->\" state state verbs)".to_string(),
                "preconditions ::= \":preconditions\" attr_ref_list".to_string(),
            ],
            "READ" | "FETCH" => vec![
                "s_expr ::= \"(\" word_call \")\"".to_string(),
                "uuid_ref ::= \"@\" ref_type \"(\" UUID \")\"".to_string(),
            ],
            _ => vec![
                "s_expr ::= \"(\" word_call \")\"".to_string(),
                "word_call ::= SYMBOL { expr }".to_string(),
            ],
        }
    }

    /// Get business constraints for operation
    fn get_constraints(&self, operation_type: &str, domain: Option<&str>) -> Vec<String> {
        let mut constraints = Vec::new();

        if let Some("kyc") = domain {
            constraints.push("All KYC entities must have verification status".to_string());
            constraints.push("Documents must be verified before CBU approval".to_string());
        }

        if let Some("cbu") = domain {
            constraints.push("CBU must have legal name and jurisdiction".to_string());
            constraints.push("CBU state transitions must follow lifecycle rules".to_string());
        }

        match operation_type {
            "CREATE" | "CREATE_CBU" => {
                constraints.push("All required fields must be provided".to_string());
            }
            "UPDATE" | "TRANSITION" => {
                constraints.push("Entity must exist before update".to_string());
            }
            "DELETE" => {
                constraints.push("Entity must not have dependent records".to_string());
            }
            _ => {}
        }

        constraints
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_rag_context_retrieval() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/data_designer".to_string());
        let pool = PgPool::connect(&database_url).await.unwrap();

        let provider = RagContextProvider::new(pool);

        let context = provider
            .get_context("CREATE", "Create CBU for TechCorp", Some("cbu"))
            .await
            .unwrap();

        assert!(
            !context.grammar_hints.is_empty(),
            "Should have grammar hints"
        );
    }

    #[test]
    fn test_extract_keywords() {
        let keywords = RagContextProvider::extract_keywords_from_text(
            "Create a CBU for TechCorp with banking services",
        );
        assert!(keywords.contains(&"techcorp".to_string()));
        assert!(keywords.contains(&"banking".to_string()));
        assert!(keywords.contains(&"services".to_string()));
        assert!(!keywords.contains(&"create".to_string())); // stop word
        assert!(!keywords.contains(&"for".to_string())); // stop word
    }
}
