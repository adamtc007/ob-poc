# Refactor: Agentic RAG Integration with New Runtime

**Created:** 2025-11-25  
**Status:** TODO  
**Depends On:** REFACTOR_FORTH_TO_DIRECT_RUNTIME.md (COMPLETE)  
**Estimated Effort:** 4-6 hours with Claude Code  
**Risk:** Medium (integration work, existing code mostly works)

---

## Executive Summary

The agentic DSL generation system exists but is **disconnected** from the new direct runtime. Key issues:

1. **RAG queries dead DB vocabulary** — `vocabulary_registry` table is unused; vocab is now in Rust
2. **Two parallel agentic systems** — `dsl_source/agentic/*` (proper) vs `services/agentic_dsl_crud.rs` (regex NL parser)
3. **No integration with new Runtime** — LlmDslGenerator doesn't use `create_standard_runtime()` for vocab metadata
4. **Validation queries dead DB** — `ValidationPipeline` checks `vocabulary_registry` which is stale
5. **Missing orchestrator** — No single entry point: generate → validate → retry → execute

---

## Current Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ dsl_source/agentic/                                             │
│                                                                 │
│  RagContextProvider ──→ queries vocabulary_registry (DEAD)      │
│         │                queries dictionary (OK)                │
│         │                queries dsl_instances (OK)             │
│         ▼                                                       │
│  LlmDslGenerator ──→ builds prompt with RAG context             │
│         │            calls MultiProviderLlm                     │
│         │            basic retry on syntax error                │
│         ▼                                                       │
│  GeneratedDsl { dsl_text, confidence, reasoning }               │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ dsl_source/validation/                                          │
│                                                                 │
│  ValidationPipeline ──→ Stage 1: NomDslParser (syntax)          │
│                         Stage 2: query vocabulary_registry (DEAD)
│                         Stage 3: business rules (hardcoded)     │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ services/agentic_dsl_crud.rs (PARALLEL SYSTEM - SHOULD DELETE)  │
│                                                                 │
│  DslParser ──→ regex-based NL parsing (brittle)                 │
│  AiDslGenerator ──→ template-based, no LLM                      │
│  CrudExecutor ──→ direct SQL, bypasses Runtime                  │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ forth_engine/ (NEW - from previous refactor)                    │
│                                                                 │
│  Runtime ──→ vocab: HashMap<&str, WordEntry>                    │
│              WordEntry { name, domain, func, signature,         │
│                          description, examples }                │
│                                                                 │
│  create_standard_runtime() ──→ builds Runtime with 63 words     │
│                                HAS the RAG metadata we need!    │
└─────────────────────────────────────────────────────────────────┘
```

---

## Target Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ User Prompt                                                     │
│ "Create a hedge fund CBU in UK jurisdiction"                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ AgenticOrchestrator (NEW - single entry point)                  │
│                                                                 │
│  1. Build RAG context from Runtime + DB                         │
│  2. Call LLM with structured prompt                             │
│  3. Validate generated DSL                                      │
│  4. If invalid → retry with error feedback (max 3)              │
│  5. Execute via Runtime                                         │
│  6. Return result                                               │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│ RagContext    │    │ Validation    │    │ Runtime       │
│ Builder       │    │ Pipeline      │    │ Execution     │
│               │    │               │    │               │
│ - Runtime     │    │ - Syntax      │    │ - Direct AST  │
│   vocab       │    │ - Semantic    │    │ - CRUD emit   │
│ - Dictionary  │    │ - Business    │    │ - CrudExecutor│
│ - Examples    │    │   Rules       │    │               │
└───────────────┘    └───────────────┘    └───────────────┘
```

---

## Files to MODIFY

### 1. `rust/src/dsl_source/agentic/rag_context.rs`

**Problem:** Queries dead `vocabulary_registry` table.

**Solution:** Get vocab from Runtime instead.

```rust
// ADD: Import Runtime
use crate::forth_engine::vocab_registry::create_standard_runtime;
use crate::forth_engine::runtime::Runtime;

impl RagContextProvider {
    // NEW: Build context using Runtime vocabulary
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
        let (examples, attributes) = tokio::runtime::Handle::current().block_on(async {
            tokio::join!(
                self.search_examples(query, operation_type),
                self.query_attributes(query, domain)
            )
        });
        
        Ok(RagContext {
            vocabulary,
            examples: examples?,
            attributes: attributes?,
            grammar_hints: self.get_grammar_hints(operation_type),
            constraints: self.get_constraints(operation_type, domain),
        })
    }
    
    // NEW: Extract vocab from Runtime
    fn get_vocab_from_runtime(&self, runtime: &Runtime, domain: Option<&str>) -> Vec<VocabEntry> {
        let words = if let Some(d) = domain {
            runtime.get_domain_words(d)
        } else {
            runtime.get_all_word_names()
                .iter()
                .filter_map(|name| runtime.get_word(name))
                .collect()
        };
        
        words.iter().map(|w| VocabEntry {
            verb_name: w.name.to_string(),
            signature: w.signature.to_string(),
            description: Some(w.description.to_string()),
            examples: Some(serde_json::json!(w.examples)),
        }).collect()
    }
}

// DEPRECATE: Mark old method
impl RagContextProvider {
    #[deprecated(note = "Use get_context_with_runtime instead")]
    pub async fn get_context(...) -> Result<RagContext> {
        // Keep for backward compat, but warn
    }
}
```

---

### 2. `rust/src/dsl_source/agentic/llm_generator.rs`

**Problem:** Uses deprecated RAG method that queries dead DB.

**Solution:** Pass Runtime to generator, use new RAG method.

```rust
// MODIFY: Add Runtime to struct
pub struct LlmDslGenerator {
    llm_client: Arc<MultiProviderLlm>,
    rag_provider: Arc<RagContextProvider>,
    runtime: Arc<Runtime>,  // ADD THIS
    max_retries: usize,
}

impl LlmDslGenerator {
    // MODIFY: Constructor takes Runtime
    pub fn new(
        rag_provider: Arc<RagContextProvider>,
        runtime: Arc<Runtime>,
    ) -> Result<Self> {
        let llm_client = MultiProviderLlm::from_env()?;
        Ok(Self {
            llm_client: Arc::new(llm_client),
            rag_provider,
            runtime,
            max_retries: 3,
        })
    }
    
    // MODIFY: Use Runtime for context
    pub async fn generate(
        &self,
        instruction: &str,
        operation_type: &str,
        domain: Option<&str>,
    ) -> Result<GeneratedDsl> {
        // Use new method with Runtime
        let context = self.rag_provider
            .get_context_with_runtime(&self.runtime, operation_type, instruction, domain)?;
        
        // ... rest unchanged
    }
}
```

---

### 3. `rust/src/dsl_source/validation/pipeline.rs`

**Problem:** Semantic validation queries dead `vocabulary_registry`.

**Solution:** Validate against Runtime vocabulary.

```rust
// ADD: Import
use crate::forth_engine::runtime::Runtime;

pub struct ValidationPipeline {
    pool: PgPool,
    runtime: Runtime,  // ADD THIS
}

impl ValidationPipeline {
    pub fn new(pool: PgPool) -> Self {
        Self { 
            pool,
            runtime: create_standard_runtime(),
        }
    }
    
    // MODIFY: Use Runtime for semantic validation
    async fn validate_semantics(&self, dsl_text: &str) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        let verbs = self.extract_verbs(dsl_text);
        
        for verb in verbs {
            // Check against Runtime vocab, not DB
            if self.runtime.get_word(&verb).is_none() {
                // Suggest similar verbs
                let suggestions = self.find_similar_verbs(&verb);
                warnings.push(format!(
                    "Unknown verb '{}'. Did you mean: {}?",
                    verb,
                    suggestions.join(", ")
                ));
            }
        }
        
        Ok(warnings)
    }
    
    // ADD: Find similar verbs for suggestions
    fn find_similar_verbs(&self, verb: &str) -> Vec<String> {
        let all_verbs = self.runtime.get_all_word_names();
        
        // Simple prefix matching
        let prefix = verb.split('.').next().unwrap_or(verb);
        all_verbs.iter()
            .filter(|v| v.starts_with(prefix))
            .take(3)
            .map(|v| v.to_string())
            .collect()
    }
}
```

---

### 4. CREATE NEW: `rust/src/dsl_source/orchestrator.rs`

**The missing piece:** Single entry point that ties everything together.

```rust
//! Agentic DSL Orchestrator
//!
//! Single entry point for: prompt → generate → validate → retry → execute

use anyhow::{Context, Result};
use std::sync::Arc;
use sqlx::PgPool;

use crate::forth_engine::runtime::Runtime;
use crate::forth_engine::vocab_registry::create_standard_runtime;
use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::parser_nom::NomDslParser;
use crate::forth_engine::ast::DslParser;
use crate::database::CrudExecutor;

use super::agentic::{LlmDslGenerator, RagContextProvider, GeneratedDsl};
use super::validation::{ValidationPipeline, ValidationResult, ValidationStage};

/// Result of agentic DSL generation and execution
#[derive(Debug)]
pub struct AgenticResult {
    pub success: bool,
    pub dsl_text: String,
    pub validation: ValidationResult,
    pub execution_logs: Vec<String>,
    pub attempts: usize,
    pub confidence: f64,
}

/// Configuration for orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_retries: usize,
    pub min_confidence: f64,
    pub execute_on_success: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            min_confidence: 0.7,
            execute_on_success: true,
        }
    }
}

/// Agentic DSL Orchestrator - the main entry point
pub struct AgenticOrchestrator {
    runtime: Arc<Runtime>,
    generator: LlmDslGenerator,
    validator: ValidationPipeline,
    executor: Option<CrudExecutor>,
    config: OrchestratorConfig,
}

impl AgenticOrchestrator {
    /// Create orchestrator with database connection
    pub fn new(pool: PgPool, config: OrchestratorConfig) -> Result<Self> {
        let runtime = Arc::new(create_standard_runtime());
        let rag_provider = Arc::new(RagContextProvider::new(pool.clone()));
        let generator = LlmDslGenerator::new(rag_provider, runtime.clone())?;
        let validator = ValidationPipeline::new(pool.clone());
        let executor = Some(CrudExecutor::new(pool));
        
        Ok(Self {
            runtime,
            generator,
            validator,
            executor,
            config,
        })
    }
    
    /// Create orchestrator without database (validation only)
    pub fn without_db() -> Result<Self> {
        let runtime = Arc::new(create_standard_runtime());
        
        // Fake pool for RAG provider (won't be used for vocab)
        let pool = PgPool::connect_lazy("postgresql://localhost/unused")?;
        let rag_provider = Arc::new(RagContextProvider::new(pool.clone()));
        let generator = LlmDslGenerator::new(rag_provider, runtime.clone())?;
        let validator = ValidationPipeline::new(pool);
        
        Ok(Self {
            runtime,
            generator,
            validator,
            executor: None,
            config: OrchestratorConfig::default(),
        })
    }
    
    /// Main entry point: natural language → executed DSL
    pub async fn process(&self, instruction: &str, domain: Option<&str>) -> Result<AgenticResult> {
        let operation_type = self.infer_operation_type(instruction);
        let mut attempts = 0;
        let mut last_validation = None;
        let mut last_dsl = String::new();
        let mut feedback = String::new();
        
        while attempts < self.config.max_retries {
            attempts += 1;
            
            // Step 1: Generate DSL
            let prompt = if feedback.is_empty() {
                instruction.to_string()
            } else {
                format!("{}\n\nPREVIOUS ATTEMPT FAILED:\n{}", instruction, feedback)
            };
            
            let generated = self.generator
                .generate(&prompt, &operation_type, domain)
                .await
                .context("LLM generation failed")?;
            
            last_dsl = generated.dsl_text.clone();
            
            // Step 2: Validate
            let validation = self.validator
                .validate(&generated.dsl_text)
                .await
                .context("Validation failed")?;
            
            last_validation = Some(validation.clone());
            
            // Step 3: Check result
            if validation.is_valid && generated.confidence >= self.config.min_confidence {
                // Success! Execute if configured
                let execution_logs = if self.config.execute_on_success {
                    self.execute(&generated.dsl_text).await?
                } else {
                    vec!["Execution skipped (config)".to_string()]
                };
                
                return Ok(AgenticResult {
                    success: true,
                    dsl_text: generated.dsl_text,
                    validation,
                    execution_logs,
                    attempts,
                    confidence: generated.confidence,
                });
            }
            
            // Build feedback for retry
            feedback = self.build_feedback(&validation, &generated);
        }
        
        // Max retries exceeded
        Ok(AgenticResult {
            success: false,
            dsl_text: last_dsl,
            validation: last_validation.unwrap_or_else(|| ValidationResult {
                is_valid: false,
                errors: vec![],
                warnings: vec!["Max retries exceeded".to_string()],
                stage_reached: ValidationStage::Syntax,
            }),
            execution_logs: vec![],
            attempts,
            confidence: 0.0,
        })
    }
    
    /// Execute validated DSL
    async fn execute(&self, dsl_text: &str) -> Result<Vec<String>> {
        let parser = NomDslParser::new();
        let ast = parser.parse(dsl_text)?;
        
        let mut env = RuntimeEnv::new(
            crate::forth_engine::env::OnboardingRequestId(uuid::Uuid::new_v4().to_string())
        );
        
        // Execute via Runtime
        self.runtime.execute_sheet(&ast, &mut env)?;
        
        let mut logs = vec![format!("Executed {} statements", ast.len())];
        
        // Execute CRUD statements if we have executor
        if let Some(executor) = &self.executor {
            let pending = env.take_pending_crud();
            if !pending.is_empty() {
                let results = executor.execute_all(&pending).await?;
                for result in results {
                    logs.push(format!(
                        "CRUD {}: {} ({} rows)",
                        result.operation, result.asset, result.rows_affected
                    ));
                }
            }
        }
        
        Ok(logs)
    }
    
    /// Infer operation type from instruction
    fn infer_operation_type(&self, instruction: &str) -> String {
        let lower = instruction.to_lowercase();
        
        if lower.contains("create") || lower.contains("new") || lower.contains("add") {
            "CREATE".to_string()
        } else if lower.contains("update") || lower.contains("modify") || lower.contains("change") {
            "UPDATE".to_string()
        } else if lower.contains("delete") || lower.contains("remove") {
            "DELETE".to_string()
        } else if lower.contains("read") || lower.contains("get") || lower.contains("fetch") {
            "READ".to_string()
        } else {
            "CREATE".to_string() // Default
        }
    }
    
    /// Build feedback message for retry
    fn build_feedback(&self, validation: &ValidationResult, generated: &GeneratedDsl) -> String {
        let mut parts = Vec::new();
        
        if !validation.is_valid {
            let errors = self.validator.format_errors_for_llm(validation);
            parts.push(format!("Validation errors:\n{}", errors));
        }
        
        if generated.confidence < self.config.min_confidence {
            parts.push(format!(
                "Confidence {} below threshold {}",
                generated.confidence, self.config.min_confidence
            ));
        }
        
        if !validation.warnings.is_empty() {
            parts.push(format!("Warnings: {}", validation.warnings.join(", ")));
        }
        
        parts.join("\n\n")
    }
    
    /// Get available domains
    pub fn get_domains(&self) -> Vec<&'static str> {
        self.runtime.get_domains()
    }
    
    /// Get words for domain (for UI/help)
    pub fn get_domain_words(&self, domain: &str) -> Vec<&str> {
        self.runtime.get_domain_words(domain)
            .iter()
            .map(|w| w.name)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Requires LLM API key
    async fn test_orchestrator_create_cbu() {
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        
        let orchestrator = AgenticOrchestrator::new(
            pool,
            OrchestratorConfig {
                execute_on_success: false, // Don't actually execute
                ..Default::default()
            }
        ).unwrap();
        
        let result = orchestrator
            .process("Create a hedge fund CBU called AcmeFund in UK jurisdiction", Some("cbu"))
            .await
            .unwrap();
        
        println!("Success: {}", result.success);
        println!("DSL: {}", result.dsl_text);
        println!("Attempts: {}", result.attempts);
        println!("Confidence: {}", result.confidence);
        
        assert!(result.dsl_text.contains("cbu.create"));
    }
}
```

---

### 5. UPDATE: `rust/src/dsl_source/mod.rs`

```rust
//! DSL Source Generation and Validation

pub mod agentic;
pub mod context;
pub mod editor;
pub mod generation;
pub mod generator;
pub mod sources;
pub mod validation;
pub mod orchestrator;  // ADD THIS

// Re-export main entry point
pub use orchestrator::{AgenticOrchestrator, AgenticResult, OrchestratorConfig};
```

---

### 6. DELETE: `rust/src/services/agentic_dsl_crud.rs`

**Reason:** Parallel system with regex NL parser, direct SQL, bypasses Runtime.

The proper `dsl_source/agentic/*` system replaces it entirely.

**Also remove from `services/mod.rs`:**
```rust
// REMOVE: pub mod agentic_dsl_crud;
```

---

### 7. DELETE: `rust/src/vocabulary/` directory

**Reason:** Dead code. Vocabulary is now in Rust via `vocab_registry.rs`.

**Files to delete:**
- `rust/src/vocabulary/mod.rs`
- `rust/src/vocabulary/vocab_registry.rs`
- `rust/src/vocabulary/models.rs`
- Any other files in that directory

**Also remove from `lib.rs`:**
```rust
// REMOVE: pub mod vocabulary;
```

---

### 8. OPTIONAL: Drop DB table `vocabulary_registry`

```sql
-- Run if you want to clean up the database
DROP TABLE IF EXISTS "ob-poc".vocabulary_registry;
```

Or keep it as historical data, but it won't be queried.

---

## Integration with Runtime Vocabulary

The key insight: **Runtime already has RAG-ready metadata.**

```rust
// In vocab_registry.rs, each word has:
WordEntry {
    name: "cbu.create",
    domain: "cbu",
    func: words::cbu_create,
    signature: ":cbu-name STRING :client-type STRING? :jurisdiction STRING?",
    description: "Create a new Client Business Unit",
    examples: &[
        r#"(cbu.create :cbu-name "AcmeFund" :client-type "HEDGE_FUND" :jurisdiction "GB")"#,
    ],
}
```

This is **exactly** what RAG needs:
- `signature` → tells LLM what args to provide
- `description` → explains what the word does
- `examples` → shows valid syntax

No need for DB lookup — the Runtime IS the vocabulary.

---

## Updated System Prompt for LLM

The `build_system_prompt` in `llm_generator.rs` should produce something like:

```
You are an expert DSL generator for the ob-poc financial onboarding system.

VALID VOCABULARY (USE ONLY THESE):
  - cbu.create :cbu-name STRING :client-type STRING? :jurisdiction STRING?
    Create a new Client Business Unit
  - cbu.read :cbu-id UUID
    Read a CBU by ID
  - cbu.update :cbu-id UUID :name STRING? :status STRING?
    Update a CBU
  ...

EXAMPLE DSL:
  (cbu.create :cbu-name "AcmeFund" :client-type "HEDGE_FUND" :jurisdiction "GB")
  (cbu.read :cbu-id "550e8400-e29b-41d4-a716-446655440000")

EBNF GRAMMAR:
  s_expr ::= "(" word_call ")"
  word_call ::= SYMBOL { keyword_arg }
  keyword_arg ::= KEYWORD value
  KEYWORD ::= ":" SYMBOL
  ...

OUTPUT FORMAT:
{
  "dsl_text": "(your-dsl-here)",
  "confidence": 0.95,
  "reasoning": "Brief explanation"
}
```

---

## Verification Steps

1. **Unit tests pass:** `cargo test`
2. **Integration test:**
   ```rust
   let orchestrator = AgenticOrchestrator::new(pool, config)?;
   let result = orchestrator.process("Create CBU for TechCorp", Some("cbu")).await?;
   assert!(result.success);
   assert!(result.dsl_text.contains("cbu.create"));
   ```
3. **End-to-end:** Generated DSL executes without errors
4. **Retry logic:** Invalid DSL triggers retry with feedback

---

## Claude Code Instructions

1. **Start with rag_context.rs** — Add `get_context_with_runtime` method
2. **Update llm_generator.rs** — Accept Runtime, use new RAG method
3. **Update validation/pipeline.rs** — Validate against Runtime vocab
4. **Create orchestrator.rs** — New file, main entry point
5. **Update mod.rs files** — Add exports
6. **Delete dead code:**
   - `services/agentic_dsl_crud.rs`
   - `vocabulary/` directory
7. **Run tests** — `cargo test`
8. **Integration test** — Test full flow with LLM

---

## Post-Refactor: Usage Example

```rust
use ob_poc::dsl_source::{AgenticOrchestrator, OrchestratorConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    
    let orchestrator = AgenticOrchestrator::new(pool, OrchestratorConfig::default())?;
    
    // Natural language → Validated DSL → Executed
    let result = orchestrator.process(
        "Create a hedge fund CBU called AcmeFund in UK jurisdiction with custody services",
        Some("cbu")
    ).await?;
    
    if result.success {
        println!("Generated DSL: {}", result.dsl_text);
        println!("Executed in {} attempts", result.attempts);
        for log in result.execution_logs {
            println!("  {}", log);
        }
    } else {
        println!("Failed after {} attempts", result.attempts);
        for error in result.validation.errors {
            println!("  Error: {:?}", error);
        }
    }
    
    Ok(())
}
```

---

## Flow Diagram

```
User: "Create hedge fund CBU in UK"
              │
              ▼
┌─────────────────────────────────┐
│ AgenticOrchestrator.process()   │
└─────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────┐
│ 1. Build RAG Context            │
│    - Runtime.get_domain_words() │
│    - DB: dsl_instances examples │
│    - DB: dictionary attributes  │
└─────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────┐
│ 2. LLM Generation               │
│    - System prompt with vocab   │
│    - User instruction           │
│    - Parse JSON response        │
└─────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────┐
│ 3. Validation Pipeline          │
│    - Syntax: NomDslParser       │
│    - Semantic: Runtime lookup   │
│    - Business: rule checks      │
└─────────────────────────────────┘
              │
        ┌─────┴─────┐
        │           │
    VALID?      INVALID
        │           │
        ▼           ▼
┌──────────┐  ┌──────────────┐
│ Execute  │  │ Build error  │
│ Runtime  │  │ feedback     │
│ → CRUD   │  │ → Retry      │
└──────────┘  └──────────────┘
              │
              ▼
         (loop max 3)
```

---

## Summary

| Component | Before | After |
|-----------|--------|-------|
| Vocab source | Dead DB table | Runtime in-memory |
| RAG context | DB queries only | Runtime + DB |
| Validation | DB vocab check | Runtime vocab check |
| Entry point | Scattered | AgenticOrchestrator |
| NL parsing | Regex (brittle) | LLM (robust) |
| Execution | Direct SQL | Runtime → CrudExecutor |
| Dead code | vocabulary/, agentic_dsl_crud | Deleted |

This completes the integration: **User prompt → LLM + RAG → Valid DSL → Direct Runtime → CRUD → Database**
