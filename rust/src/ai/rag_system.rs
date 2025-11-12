//! RAG (Retrieval-Augmented Generation) System for Agentic CRUD Operations
//!
//! This module provides contextual information retrieval for AI-powered CRUD operations.
//! It maintains a knowledge base of schemas, grammar rules, and examples to help
//! the AI generate accurate DSL statements from natural language instructions.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RAG system that provides contextual information for CRUD operations
#[derive(Debug, Clone)]
pub struct CrudRagSystem {
    /// Asset schemas with their database mappings
    asset_schemas: HashMap<String, AssetSchemaInfo>,
    /// CRUD grammar rules and patterns
    grammar_rules: GrammarRules,
    /// Example mappings from natural language to DSL
    examples: Vec<CrudExample>,
}

/// Information about an asset type for RAG context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AssetSchemaInfo {
    pub asset_name: String,
    pub table_name: String,
    pub description: String,
    pub fields: Vec<FieldInfo>,
    pub common_operations: Vec<String>,
}

/// Information about a field in an asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FieldInfo {
    pub field_name: String,
    pub db_column: String,
    pub data_type: String,
    pub description: String,
    pub required: bool,
    pub examples: Vec<String>,
}

/// CRUD grammar rules and patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GrammarRules {
    pub ebnf_grammar: String,
    pub verb_patterns: HashMap<String, VerbPattern>,
    pub common_mistakes: Vec<String>,
}

/// Pattern information for a specific CRUD verb
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VerbPattern {
    pub verb: String,
    pub description: String,
    pub required_fields: Vec<String>,
    pub optional_fields: Vec<String>,
    pub syntax_template: String,
}

/// Example mapping from natural language to DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CrudExample {
    pub id: String,
    pub category: String,
    pub natural_language: String,
    pub dsl_output: String,
    pub explanation: String,
    pub assets_used: Vec<String>,
}

/// Context retrieved for a specific query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedContext {
    pub relevant_schemas: Vec<AssetSchemaInfo>,
    pub applicable_grammar: Vec<VerbPattern>,
    pub similar_examples: Vec<CrudExample>,
    pub confidence_score: f64,
    pub sources: Vec<String>,
}

impl CrudRagSystem {
    /// Creates a new RAG system with predefined knowledge base
    pub fn new() -> Self {
        Self {
            asset_schemas: Self::initialize_asset_schemas(),
            grammar_rules: Self::initialize_grammar_rules(),
            examples: Self::initialize_examples(),
        }
    }

    /// Retrieves relevant context for a natural language query
    pub fn retrieve_context(&self, query: &str) -> Result<RetrievedContext> {
        let query_lower = query.to_lowercase();

        // Determine relevant asset types based on keywords
        let relevant_assets = self.identify_relevant_assets(&query_lower);

        // Get schemas for relevant assets
        let relevant_schemas: Vec<AssetSchemaInfo> = relevant_assets
            .iter()
            .filter_map(|asset| self.asset_schemas.get(asset))
            .cloned()
            .collect();

        // Determine CRUD operation type
        let operation_type = self.identify_operation_type(&query_lower);

        // Get applicable grammar patterns
        let applicable_grammar: Vec<VerbPattern> = operation_type
            .iter()
            .filter_map(|op| self.grammar_rules.verb_patterns.get(op))
            .cloned()
            .collect();

        // Find similar examples
        let similar_examples = self.find_similar_examples(&query_lower, &relevant_assets);

        // Calculate confidence score
        let confidence_score =
            self.calculate_confidence(&relevant_schemas, &applicable_grammar, &similar_examples);

        Ok(RetrievedContext {
            relevant_schemas,
            applicable_grammar,
            similar_examples,
            confidence_score,
            sources: vec![
                "Asset schema matching".to_string(),
                "Operation type detection".to_string(),
                "Similar example retrieval".to_string(),
            ],
        })
    }

    /// Retrieves context for a specific operation type and instruction
    pub async fn retrieve_context_for_operation(
        &self,
        operation_verb: &str,
        instruction: &str,
    ) -> Result<RetrievedContext> {
        // Extract asset type from operation verb (e.g., "attribute.create" -> "attribute")
        let asset_type = if let Some(dot_pos) = operation_verb.find('.') {
            &operation_verb[..dot_pos]
        } else {
            "unknown"
        };

        // Get schema for this asset type
        let relevant_schemas = if let Some(schema) = self.asset_schemas.get(asset_type) {
            vec![schema.clone()]
        } else {
            // Fallback to general context retrieval
            return self.retrieve_context(instruction);
        };

        // Find applicable grammar patterns
        let applicable_grammar: Vec<VerbPattern> = self
            .grammar_rules
            .verb_patterns
            .values()
            .filter(|pattern| {
                pattern.verb.starts_with(asset_type)
                    || pattern.verb.contains(&format!(
                        ".{}",
                        operation_verb.split('.').last().unwrap_or("")
                    ))
            })
            .cloned()
            .collect();

        // Find examples for this asset type and operation
        let similar_examples: Vec<CrudExample> = self
            .examples
            .iter()
            .filter(|example| {
                example.assets_used.contains(&asset_type.to_string())
                    || example.dsl_output.contains(operation_verb)
                    || self.calculate_similarity(
                        &example.natural_language.to_lowercase(),
                        &instruction.to_lowercase(),
                    ) > 0.3
            })
            .take(3)
            .cloned()
            .collect();

        // Calculate confidence
        let confidence_score =
            self.calculate_confidence(&relevant_schemas, &applicable_grammar, &similar_examples);

        Ok(RetrievedContext {
            relevant_schemas,
            applicable_grammar,
            similar_examples,
            confidence_score,
            sources: vec![
                format!("Schema for {}", asset_type),
                format!("Grammar patterns for {}", operation_verb),
                format!("Examples for {} operations", asset_type),
            ],
        })
    }

    /// Gets all available asset types
    pub(crate) fn get_available_assets(&self) -> Vec<String> {
        self.asset_schemas.keys().cloned().collect()
    }

    /// Gets schema information for a specific asset
    pub(crate) fn get_asset_schema(&self, asset_name: &str) -> Option<&AssetSchemaInfo> {
        self.asset_schemas.get(asset_name)
    }

    /// Identifies relevant asset types from the query
    fn identify_relevant_assets(&self, query: &str) -> Vec<String> {
        let mut assets = Vec::new();

        // CBU-related keywords
        if query.contains("client")
            || query.contains("cbu")
            || query.contains("business unit")
            || query.contains("company")
            || query.contains("fund")
            || query.contains("entity")
        {
            assets.push("cbu".to_string());
        }

        // Document-related keywords
        if query.contains("document")
            || query.contains("passport")
            || query.contains("certificate")
            || query.contains("license")
            || query.contains("file")
        {
            assets.push("document".to_string());
        }

        // Attribute-related keywords
        if query.contains("attribute")
            || query.contains("field")
            || query.contains("property")
            || query.contains("dictionary")
            || query.contains("metadata")
        {
            assets.push("attribute".to_string());
        }

        // If no specific assets identified, include all
        if assets.is_empty() {
            assets = self.asset_schemas.keys().cloned().collect();
        }

        assets
    }

    /// Identifies the CRUD operation type from the query
    fn identify_operation_type(&self, query: &str) -> Vec<String> {
        let mut operations = Vec::new();

        // Create operations
        if query.contains("create")
            || query.contains("add")
            || query.contains("register")
            || query.contains("new")
            || query.contains("insert")
        {
            operations.push("data.create".to_string());
        }

        // Read operations
        if query.contains("find")
            || query.contains("search")
            || query.contains("get")
            || query.contains("list")
            || query.contains("show")
            || query.contains("query")
        {
            operations.push("data.read".to_string());
        }

        // Update operations
        if query.contains("update")
            || query.contains("modify")
            || query.contains("change")
            || query.contains("edit")
            || query.contains("set")
        {
            operations.push("data.update".to_string());
        }

        // Delete operations
        if query.contains("delete") || query.contains("remove") || query.contains("drop") {
            operations.push("data.delete".to_string());
        }

        // If no specific operation identified, include read as default
        if operations.is_empty() {
            operations.push("data.read".to_string());
        }

        operations
    }

    /// Finds examples similar to the query
    fn find_similar_examples(&self, query: &str, relevant_assets: &[String]) -> Vec<CrudExample> {
        self.examples
            .iter()
            .filter(|example| {
                // Check if example uses relevant assets
                example
                    .assets_used
                    .iter()
                    .any(|asset| relevant_assets.contains(asset))
                    || self.calculate_similarity(&example.natural_language.to_lowercase(), query)
                        > 0.3
            })
            .take(3) // Limit to top 3 examples
            .cloned()
            .collect()
    }

    /// Simple similarity calculation based on common words
    fn calculate_similarity(&self, text1: &str, text2: &str) -> f64 {
        let words1: Vec<&str> = text1.split_whitespace().collect();
        let words2: Vec<&str> = text2.split_whitespace().collect();

        let common_words = words1.iter().filter(|word| words2.contains(word)).count();

        if words1.is_empty() || words2.is_empty() {
            0.0
        } else {
            common_words as f64 / (words1.len() + words2.len()) as f64 * 2.0
        }
    }

    /// Calculates confidence score for the retrieved context
    fn calculate_confidence(
        &self,
        schemas: &[AssetSchemaInfo],
        grammar: &[VerbPattern],
        examples: &[CrudExample],
    ) -> f64 {
        let schema_score = if schemas.is_empty() { 0.0 } else { 0.4 };
        let grammar_score = if grammar.is_empty() { 0.0 } else { 0.3 };
        let example_score = (examples.len() as f64 / 3.0).min(1.0) * 0.3;

        schema_score + grammar_score + example_score
    }

    /// Initializes the asset schemas knowledge base
    fn initialize_asset_schemas() -> HashMap<String, AssetSchemaInfo> {
        let mut schemas = HashMap::new();

        // CBU Schema
        let cbu_schema = AssetSchemaInfo {
            asset_name: "cbu".to_string(),
            table_name: "ob-poc.cbus".to_string(),
            description: "Client Business Units - entities that represent clients in the system"
                .to_string(),
            fields: vec![
                FieldInfo {
                    field_name: "name".to_string(),
                    db_column: "name".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "The name of the client business unit".to_string(),
                    required: true,
                    examples: vec![
                        "Quantum Ventures LP".to_string(),
                        "Alpha Tech Fund".to_string(),
                    ],
                },
                FieldInfo {
                    field_name: "description".to_string(),
                    db_column: "description".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Description of the business unit".to_string(),
                    required: false,
                    examples: vec![
                        "Technology investment fund".to_string(),
                        "Healthcare venture capital".to_string(),
                    ],
                },
                FieldInfo {
                    field_name: "jurisdiction".to_string(),
                    db_column: "jurisdiction".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Legal jurisdiction code".to_string(),
                    required: false,
                    examples: vec!["US".to_string(), "GB".to_string(), "US-DE".to_string()],
                },
                FieldInfo {
                    field_name: "entity_type".to_string(),
                    db_column: "entity_type".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Type of legal entity".to_string(),
                    required: false,
                    examples: vec![
                        "CORP".to_string(),
                        "LIMITED_PARTNERSHIP".to_string(),
                        "LLC".to_string(),
                    ],
                },
            ],
            common_operations: vec![
                "create new clients".to_string(),
                "search clients".to_string(),
                "update client info".to_string(),
            ],
        };
        schemas.insert("cbu".to_string(), cbu_schema);

        // Document Schema
        let document_schema = AssetSchemaInfo {
            asset_name: "document".to_string(),
            table_name: "ob-poc.document_catalog".to_string(),
            description: "Document catalog for storing and managing various document types"
                .to_string(),
            fields: vec![
                FieldInfo {
                    field_name: "type".to_string(),
                    db_column: "document_type".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Type of document".to_string(),
                    required: true,
                    examples: vec![
                        "PASSPORT".to_string(),
                        "DRIVERS_LICENSE".to_string(),
                        "CERTIFICATE".to_string(),
                    ],
                },
                FieldInfo {
                    field_name: "title".to_string(),
                    db_column: "title".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Document title or name".to_string(),
                    required: false,
                    examples: vec![
                        "John Smith Passport".to_string(),
                        "Company Certificate".to_string(),
                    ],
                },
                FieldInfo {
                    field_name: "issuer".to_string(),
                    db_column: "issuer".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Document issuing authority".to_string(),
                    required: false,
                    examples: vec![
                        "US_STATE_DEPARTMENT".to_string(),
                        "UK_HOME_OFFICE".to_string(),
                    ],
                },
                FieldInfo {
                    field_name: "status".to_string(),
                    db_column: "status".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Current status of the document".to_string(),
                    required: false,
                    examples: vec![
                        "ACTIVE".to_string(),
                        "EXPIRED".to_string(),
                        "PENDING".to_string(),
                    ],
                },
            ],
            common_operations: vec![
                "catalog documents".to_string(),
                "find documents by type".to_string(),
                "update document status".to_string(),
            ],
        };
        schemas.insert("document".to_string(), document_schema);

        // Attribute Schema
        let attribute_schema = AssetSchemaInfo {
            asset_name: "attribute".to_string(),
            table_name: "ob-poc.dictionary".to_string(),
            description: "Data dictionary for managing attribute definitions".to_string(),
            fields: vec![
                FieldInfo {
                    field_name: "name".to_string(),
                    db_column: "name".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Name of the attribute".to_string(),
                    required: true,
                    examples: vec!["customer_id".to_string(), "email_address".to_string()],
                },
                FieldInfo {
                    field_name: "description".to_string(),
                    db_column: "description".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Description of what the attribute represents".to_string(),
                    required: false,
                    examples: vec![
                        "Unique customer identifier".to_string(),
                        "Primary email address".to_string(),
                    ],
                },
                FieldInfo {
                    field_name: "data_type".to_string(),
                    db_column: "data_type".to_string(),
                    data_type: "TEXT".to_string(),
                    description: "Data type of the attribute".to_string(),
                    required: true,
                    examples: vec![
                        "TEXT".to_string(),
                        "INTEGER".to_string(),
                        "BOOLEAN".to_string(),
                    ],
                },
                FieldInfo {
                    field_name: "is_pii".to_string(),
                    db_column: "is_pii".to_string(),
                    data_type: "BOOLEAN".to_string(),
                    description:
                        "Whether this attribute contains personally identifiable information"
                            .to_string(),
                    required: false,
                    examples: vec!["true".to_string(), "false".to_string()],
                },
            ],
            common_operations: vec![
                "define new attributes".to_string(),
                "search attribute definitions".to_string(),
                "update attribute metadata".to_string(),
            ],
        };
        schemas.insert("attribute".to_string(), attribute_schema);

        schemas
    }

    /// Initializes the grammar rules knowledge base
    fn initialize_grammar_rules() -> GrammarRules {
        let mut verb_patterns = HashMap::new();

        // Data Create Pattern
        verb_patterns.insert("data.create".to_string(), VerbPattern {
            verb: "data.create".to_string(),
            description: "Creates a new record for the specified asset type".to_string(),
            required_fields: vec![":asset".to_string(), ":values".to_string()],
            optional_fields: vec![],
            syntax_template: "(data.create :asset \"asset_type\" :values {:field1 \"value1\" :field2 \"value2\"})".to_string(),
        });

        // Data Read Pattern
        verb_patterns.insert("data.read".to_string(), VerbPattern {
            verb: "data.read".to_string(),
            description: "Reads records from the specified asset type with optional filtering and field selection".to_string(),
            required_fields: vec![":asset".to_string()],
            optional_fields: vec![":where".to_string(), ":select".to_string()],
            syntax_template: "(data.read :asset \"asset_type\" :where {:field \"value\"} :select [\"field1\" \"field2\"])".to_string(),
        });

        // Data Update Pattern
        verb_patterns.insert("data.update".to_string(), VerbPattern {
            verb: "data.update".to_string(),
            description: "Updates records in the specified asset type that match the WHERE conditions".to_string(),
            required_fields: vec![":asset".to_string(), ":where".to_string(), ":values".to_string()],
            optional_fields: vec![],
            syntax_template: "(data.update :asset \"asset_type\" :where {:field \"condition\"} :values {:field \"new_value\"})".to_string(),
        });

        // Data Delete Pattern
        verb_patterns.insert(
            "data.delete".to_string(),
            VerbPattern {
                verb: "data.delete".to_string(),
                description:
                    "Deletes records from the specified asset type that match the WHERE conditions"
                        .to_string(),
                required_fields: vec![":asset".to_string(), ":where".to_string()],
                optional_fields: vec![],
                syntax_template:
                    "(data.delete :asset \"asset_type\" :where {:field \"condition\"})".to_string(),
            },
        );

        GrammarRules {
            ebnf_grammar: r#"
statement ::= create_op | read_op | update_op | delete_op
create_op ::= "(" "data.create" asset_type value_block ")"
read_op   ::= "(" "data.read" asset_type filter_block? select_block? ")"
update_op ::= "(" "data.update" asset_type filter_block value_block ")"
delete_op ::= "(" "data.delete" asset_type filter_block ")"
asset_type ::= "cbu" | "document" | "attribute"
value_block  ::= "(" "set" (field_value)+ ")"
filter_block ::= "(" "where" expression ")"
select_block ::= "(" "select" "[" (symbol)+ "]" ")"
"#.to_string(),
            verb_patterns,
            common_mistakes: vec![
                "Using unsupported asset types - only 'cbu', 'document', and 'attribute' are supported".to_string(),
                "Missing required :where clause in UPDATE and DELETE operations".to_string(),
                "Incorrect field names - check the asset schema for valid field names".to_string(),
                "Missing quotes around string values".to_string(),
            ],
        }
    }

    /// Initializes the examples knowledge base
    fn initialize_examples() -> Vec<CrudExample> {
        vec![
            // CBU Examples
            CrudExample {
                id: "cbu_create_fund".to_string(),
                category: "CBU Creation".to_string(),
                natural_language: "Create a new client called 'Quantum Tech Fund LP', it's a Delaware LP that invests in quantum computing".to_string(),
                dsl_output: r#"(data.create :asset "cbu" :values {:name "Quantum Tech Fund LP" :description "Delaware limited partnership specializing in quantum computing investments" :jurisdiction "US-DE" :entity_type "LIMITED_PARTNERSHIP"})"#.to_string(),
                explanation: "Creates a new CBU with name, description, jurisdiction, and entity type".to_string(),
                assets_used: vec!["cbu".to_string()],
            },
            CrudExample {
                id: "cbu_search_us".to_string(),
                category: "CBU Search".to_string(),
                natural_language: "Find all US clients that are corporations".to_string(),
                dsl_output: r#"(data.read :asset "cbu" :where {:jurisdiction "US" :entity_type "CORP"} :select ["name" "description"])"#.to_string(),
                explanation: "Searches for CBUs with specific jurisdiction and entity type, returning selected fields".to_string(),
                assets_used: vec!["cbu".to_string()],
            },
            CrudExample {
                id: "cbu_update_description".to_string(),
                category: "CBU Update".to_string(),
                natural_language: "Update the description of 'Alpha Tech Ventures' to say they focus on AI and quantum computing".to_string(),
                dsl_output: r#"(data.update :asset "cbu" :where {:name "Alpha Tech Ventures"} :values {:description "Technology investment fund focusing on AI and quantum computing"})"#.to_string(),
                explanation: "Updates the description field for a specific CBU identified by name".to_string(),
                assets_used: vec!["cbu".to_string()],
            },

            // Document Examples
            CrudExample {
                id: "doc_create_passport".to_string(),
                category: "Document Creation".to_string(),
                natural_language: "Add a new passport document for John Smith issued by the US State Department".to_string(),
                dsl_output: r#"(data.create :asset "document" :values {:type "PASSPORT" :title "John Smith Passport" :issuer "US_STATE_DEPARTMENT" :status "ACTIVE"})"#.to_string(),
                explanation: "Creates a new document entry with type, title, issuer, and status".to_string(),
                assets_used: vec!["document".to_string()],
            },
            CrudExample {
                id: "doc_search_uk_passports".to_string(),
                category: "Document Search".to_string(),
                natural_language: "Find all passports issued by the UK that are still active".to_string(),
                dsl_output: r#"(data.read :asset "document" :where {:type "PASSPORT" :issuer "UK_HOME_OFFICE" :status "ACTIVE"})"#.to_string(),
                explanation: "Searches for documents with specific type, issuer, and status".to_string(),
                assets_used: vec!["document".to_string()],
            },

            // Attribute Examples
            CrudExample {
                id: "attr_create_email".to_string(),
                category: "Attribute Creation".to_string(),
                natural_language: "Create a new attribute definition for email addresses that contains PII".to_string(),
                dsl_output: r#"(data.create :asset "attribute" :values {:name "email_address" :description "Primary email address for customer contact" :data_type "TEXT" :is_pii true})"#.to_string(),
                explanation: "Creates a new attribute definition with name, description, data type, and PII flag".to_string(),
                assets_used: vec!["attribute".to_string()],
            },
            CrudExample {
                id: "attr_search_pii".to_string(),
                category: "Attribute Search".to_string(),
                natural_language: "Show me all attributes that contain personally identifiable information".to_string(),
                dsl_output: r#"(data.read :asset "attribute" :where {:is_pii true} :select ["name" "description" "data_type"])"#.to_string(),
                explanation: "Searches for attributes where the PII flag is true, returning selected fields".to_string(),
                assets_used: vec!["attribute".to_string()],
            },

            // Complex Examples
            CrudExample {
                id: "multi_condition_search".to_string(),
                category: "Complex Search".to_string(),
                natural_language: "Find all Delaware LPs in our system".to_string(),
                dsl_output: r#"(data.read :asset "cbu" :where {:jurisdiction "US-DE" :entity_type "LIMITED_PARTNERSHIP"})"#.to_string(),
                explanation: "Searches with multiple conditions in the WHERE clause".to_string(),
                assets_used: vec!["cbu".to_string()],
            },
            CrudExample {
                id: "cleanup_expired".to_string(),
                category: "Data Cleanup".to_string(),
                natural_language: "Remove all expired documents from the system".to_string(),
                dsl_output: r#"(data.delete :asset "document" :where {:status "EXPIRED"})"#.to_string(),
                explanation: "Deletes records matching a specific condition".to_string(),
                assets_used: vec!["document".to_string()],
            },
        ]
    }
}

impl Default for CrudRagSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_system_creation() {
        let rag = CrudRagSystem::new();
        assert_eq!(rag.get_available_assets().len(), 3);
        assert!(rag.get_available_assets().contains(&"cbu".to_string()));
        assert!(rag.get_available_assets().contains(&"document".to_string()));
        assert!(rag
            .get_available_assets()
            .contains(&"attribute".to_string()));
    }

    #[test]
    fn test_asset_identification() {
        let rag = CrudRagSystem::new();

        // Test CBU identification
        let cbu_query = "create a new client business unit";
        let context = rag.retrieve_context(cbu_query).unwrap();
        assert!(context
            .relevant_schemas
            .iter()
            .any(|s| s.asset_name == "cbu"));

        // Test Document identification
        let doc_query = "find all passport documents";
        let context = rag.retrieve_context(doc_query).unwrap();
        assert!(context
            .relevant_schemas
            .iter()
            .any(|s| s.asset_name == "document"));
    }

    #[test]
    fn test_operation_identification() {
        let rag = CrudRagSystem::new();

        // Test CREATE operation
        let create_query = "add a new client";
        let context = rag.retrieve_context(create_query).unwrap();
        assert!(context
            .applicable_grammar
            .iter()
            .any(|g| g.verb == "data.create"));

        // Test READ operation
        let read_query = "find clients in Delaware";
        let context = rag.retrieve_context(read_query).unwrap();
        assert!(context
            .applicable_grammar
            .iter()
            .any(|g| g.verb == "data.read"));
    }

    #[test]
    fn test_example_retrieval() {
        let rag = CrudRagSystem::new();

        let query = "create quantum fund";
        let context = rag.retrieve_context(query).unwrap();

        assert!(!context.similar_examples.is_empty());
        assert!(context.confidence_score > 0.0);
    }

    #[test]
    fn test_schema_retrieval() {
        let rag = CrudRagSystem::new();

        let cbu_schema = rag.get_asset_schema("cbu").unwrap();
        assert_eq!(cbu_schema.asset_name, "cbu");
        assert!(!cbu_schema.fields.is_empty());

        let name_field = cbu_schema
            .fields
            .iter()
            .find(|f| f.field_name == "name")
            .unwrap();
        assert!(name_field.required);
        assert_eq!(name_field.data_type, "TEXT");
    }
}
