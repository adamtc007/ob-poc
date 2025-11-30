# TASK: DSL Generation Log - Capture Agent Interactions for Training

## Goal

Capture the full agent generation flow (intent → prompts → responses → lint → corrections) to enable:
1. Fine-tuning dataset generation
2. Few-shot RAG retrieval of successful examples
3. Error recovery pattern learning
4. Prompt effectiveness analysis

---

## Part 1: Database Schema

### 1.1 New Table

**File:** `sql/migrations/XXXXXX_add_dsl_generation_log.sql`

```sql
-- DSL Generation Log
-- Captures agent prompt/response iterations for training data extraction

CREATE TABLE "ob-poc".dsl_generation_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Link to persisted DSL (nullable - might fail before persisting)
    instance_id UUID REFERENCES "ob-poc".dsl_instances(instance_id),
    
    -- === THE GOLD PAIR FOR TRAINING ===
    user_intent TEXT NOT NULL,           -- Natural language: "Create hedge fund with John as director"
    final_valid_dsl TEXT,                -- The DSL that passed validation (NULL if never succeeded)
    
    -- === ITERATION HISTORY ===
    -- JSONB array capturing each attempt
    iterations JSONB NOT NULL DEFAULT '[]',
    /*
      Structure:
      [
        {
          "attempt": 1,
          "timestamp": "2025-01-15T10:30:00Z",
          "prompt_template": "cbu_create_v2",
          "prompt_text": "Given the vocabulary...",      -- Full prompt sent
          "raw_response": "I'll create a CBU...",        -- Full LLM response
          "extracted_dsl": "(cbu.create :name ...)",     -- DSL extracted from response
          "parse_result": {
            "success": true,
            "error": null
          },
          "lint_result": {
            "valid": false,
            "errors": ["Unknown verb: cbu.créate"],
            "warnings": []
          },
          "compile_result": {
            "success": false,
            "error": "Unknown verb: cbu.créate",
            "step_count": 0
          }
        },
        {
          "attempt": 2,
          "timestamp": "2025-01-15T10:30:05Z",
          "prompt_template": "cbu_create_v2",
          "prompt_text": "The previous DSL had errors...",
          "raw_response": "Let me fix that...",
          "extracted_dsl": "(cbu.create :name ...)",
          "parse_result": {"success": true, "error": null},
          "lint_result": {"valid": true, "errors": [], "warnings": []},
          "compile_result": {"success": true, "error": null, "step_count": 3}
        }
      ]
    */
    
    -- === CONTEXT ===
    domain_name VARCHAR(50) NOT NULL,           -- "cbu", "entity", "document"
    session_id UUID,                            -- Link to agent session if applicable
    cbu_id UUID,                                -- Target CBU if applicable
    
    -- === METRICS ===
    model_used VARCHAR(100),                    -- "claude-sonnet-4-20250514"
    total_attempts INT NOT NULL DEFAULT 1,
    success BOOLEAN NOT NULL DEFAULT false,
    total_latency_ms INT,                       -- Sum of all attempts
    total_input_tokens INT,
    total_output_tokens INT,
    
    -- === TIMESTAMPS ===
    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

-- Indexes for training data extraction
CREATE INDEX idx_gen_log_success ON "ob-poc".dsl_generation_log(success) WHERE success = true;
CREATE INDEX idx_gen_log_domain ON "ob-poc".dsl_generation_log(domain_name);
CREATE INDEX idx_gen_log_created ON "ob-poc".dsl_generation_log(created_at DESC);
CREATE INDEX idx_gen_log_instance ON "ob-poc".dsl_generation_log(instance_id) WHERE instance_id IS NOT NULL;

-- GIN index for JSONB queries on iterations
CREATE INDEX idx_gen_log_iterations ON "ob-poc".dsl_generation_log USING GIN (iterations);

COMMENT ON TABLE "ob-poc".dsl_generation_log IS 'Captures agent DSL generation iterations for training data extraction';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.user_intent IS 'Natural language description of what user wanted - the input side of training pairs';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.final_valid_dsl IS 'Successfully validated DSL - the output side of training pairs';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.iterations IS 'JSONB array of each generation attempt with prompts, responses, and validation results';
```

### 1.2 Optional: Prompt Template Registry

If you want to track which prompt templates work best:

```sql
CREATE TABLE "ob-poc".prompt_templates (
    template_id VARCHAR(100) PRIMARY KEY,       -- "cbu_create_v2"
    template_text TEXT NOT NULL,                -- The actual prompt template with {{placeholders}}
    domain_name VARCHAR(50) NOT NULL,
    version INT NOT NULL DEFAULT 1,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    deprecated_at TIMESTAMPTZ                   -- NULL = active
);
```

---

## Part 2: Rust Implementation

### 2.1 New Repository

**File:** `rust/src/database/generation_log_repository.rs`

```rust
//! DSL Generation Log Repository
//! 
//! Captures and queries agent generation iterations for training.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// A single generation attempt within an iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationAttempt {
    pub attempt: i32,
    pub timestamp: DateTime<Utc>,
    pub prompt_template: Option<String>,
    pub prompt_text: String,
    pub raw_response: String,
    pub extracted_dsl: Option<String>,
    pub parse_result: ParseResult,
    pub lint_result: LintResult,
    pub compile_result: CompileResult,
    pub latency_ms: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub success: bool,
    pub error: Option<String>,
    pub step_count: i32,
}

/// Builder for creating generation log entries
pub struct GenerationLogBuilder {
    user_intent: String,
    domain_name: String,
    session_id: Option<Uuid>,
    cbu_id: Option<Uuid>,
    model_used: Option<String>,
    iterations: Vec<GenerationAttempt>,
}

impl GenerationLogBuilder {
    pub fn new(user_intent: &str, domain_name: &str) -> Self {
        Self {
            user_intent: user_intent.to_string(),
            domain_name: domain_name.to_string(),
            session_id: None,
            cbu_id: None,
            model_used: None,
            iterations: Vec::new(),
        }
    }

    pub fn session(mut self, session_id: Uuid) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn cbu(mut self, cbu_id: Uuid) -> Self {
        self.cbu_id = Some(cbu_id);
        self
    }

    pub fn model(mut self, model: &str) -> Self {
        self.model_used = Some(model.to_string());
        self
    }

    pub fn add_attempt(&mut self, attempt: GenerationAttempt) {
        self.iterations.push(attempt);
    }
}

/// Repository for generation log operations
pub struct GenerationLogRepository {
    pool: PgPool,
}

impl GenerationLogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Start a new generation log entry
    /// Returns log_id for adding iterations
    pub async fn start_log(
        &self,
        user_intent: &str,
        domain_name: &str,
        session_id: Option<Uuid>,
        cbu_id: Option<Uuid>,
        model_used: Option<&str>,
    ) -> Result<Uuid, sqlx::Error> {
        let log_id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_generation_log 
            (log_id, user_intent, domain_name, session_id, cbu_id, model_used, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#
        )
        .bind(log_id)
        .bind(user_intent)
        .bind(domain_name)
        .bind(session_id)
        .bind(cbu_id)
        .bind(model_used)
        .execute(&self.pool)
        .await?;
        
        Ok(log_id)
    }

    /// Add an iteration attempt to existing log
    pub async fn add_attempt(
        &self,
        log_id: Uuid,
        attempt: &GenerationAttempt,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_generation_log
            SET 
                iterations = iterations || $2::jsonb,
                total_attempts = jsonb_array_length(iterations) + 1,
                total_latency_ms = COALESCE(total_latency_ms, 0) + COALESCE($3, 0),
                total_input_tokens = COALESCE(total_input_tokens, 0) + COALESCE($4, 0),
                total_output_tokens = COALESCE(total_output_tokens, 0) + COALESCE($5, 0)
            WHERE log_id = $1
            "#
        )
        .bind(log_id)
        .bind(serde_json::to_value(attempt).unwrap())
        .bind(attempt.latency_ms)
        .bind(attempt.input_tokens)
        .bind(attempt.output_tokens)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    /// Mark generation as successful and store final DSL
    pub async fn mark_success(
        &self,
        log_id: Uuid,
        final_dsl: &str,
        instance_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_generation_log
            SET 
                success = true,
                final_valid_dsl = $2,
                instance_id = $3,
                completed_at = NOW()
            WHERE log_id = $1
            "#
        )
        .bind(log_id)
        .bind(final_dsl)
        .bind(instance_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    /// Mark generation as failed
    pub async fn mark_failed(&self, log_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_generation_log
            SET success = false, completed_at = NOW()
            WHERE log_id = $1
            "#
        )
        .bind(log_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

### 2.2 Training Data Extraction Queries

Add these methods to the repository:

```rust
impl GenerationLogRepository {
    /// Export successful intent → DSL pairs for fine-tuning
    pub async fn export_training_pairs(
        &self,
        domain: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<TrainingPair>, sqlx::Error> {
        sqlx::query_as::<_, TrainingPair>(
            r#"
            SELECT user_intent, final_valid_dsl as valid_dsl
            FROM "ob-poc".dsl_generation_log
            WHERE success = true
              AND final_valid_dsl IS NOT NULL
              AND ($1::text IS NULL OR domain_name = $1)
            ORDER BY created_at DESC
            LIMIT $2
            "#
        )
        .bind(domain)
        .bind(limit.unwrap_or(1000))
        .fetch_all(&self.pool)
        .await
    }

    /// Export error correction pairs (bad DSL + error → fixed DSL)
    pub async fn export_correction_pairs(
        &self,
        limit: Option<i32>,
    ) -> Result<Vec<CorrectionPair>, sqlx::Error> {
        // Query logs where total_attempts > 1 and success = true
        // Extract iteration N (failed) and N+1 (succeeded) as pairs
        sqlx::query_as::<_, CorrectionPair>(
            r#"
            WITH corrections AS (
                SELECT 
                    log_id,
                    user_intent,
                    iterations,
                    total_attempts
                FROM "ob-poc".dsl_generation_log
                WHERE success = true AND total_attempts > 1
            )
            SELECT 
                user_intent,
                iterations->0->>'extracted_dsl' as bad_dsl,
                iterations->0->'lint_result'->>'errors' as error_message,
                iterations->1->>'extracted_dsl' as fixed_dsl
            FROM corrections
            WHERE iterations->1->>'extracted_dsl' IS NOT NULL
            LIMIT $1
            "#
        )
        .bind(limit.unwrap_or(500))
        .fetch_all(&self.pool)
        .await
    }

    /// Find similar successful generations for few-shot RAG
    /// Requires pg_trgm extension for similarity search
    pub async fn find_similar_examples(
        &self,
        intent: &str,
        domain: &str,
        limit: i32,
    ) -> Result<Vec<TrainingPair>, sqlx::Error> {
        sqlx::query_as::<_, TrainingPair>(
            r#"
            SELECT user_intent, final_valid_dsl as valid_dsl
            FROM "ob-poc".dsl_generation_log
            WHERE success = true
              AND domain_name = $2
              AND user_intent % $1
            ORDER BY similarity(user_intent, $1) DESC
            LIMIT $3
            "#
        )
        .bind(intent)
        .bind(domain)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Get prompt template effectiveness stats
    pub async fn prompt_effectiveness_stats(
        &self,
    ) -> Result<Vec<PromptStats>, sqlx::Error> {
        sqlx::query_as::<_, PromptStats>(
            r#"
            SELECT 
                iterations->0->>'prompt_template' as template_name,
                COUNT(*) as total_uses,
                SUM(CASE WHEN total_attempts = 1 AND success THEN 1 ELSE 0 END) as first_try_success,
                AVG(total_attempts)::float as avg_attempts,
                AVG(total_latency_ms)::float as avg_latency_ms
            FROM "ob-poc".dsl_generation_log
            WHERE iterations->0->>'prompt_template' IS NOT NULL
            GROUP BY iterations->0->>'prompt_template'
            ORDER BY total_uses DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrainingPair {
    pub user_intent: String,
    pub valid_dsl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CorrectionPair {
    pub user_intent: String,
    pub bad_dsl: String,
    pub error_message: String,
    pub fixed_dsl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PromptStats {
    pub template_name: String,
    pub total_uses: i64,
    pub first_try_success: i64,
    pub avg_attempts: f64,
    pub avg_latency_ms: f64,
}
```

### 2.3 Wire Into mod.rs

**File:** `rust/src/database/mod.rs`

```rust
pub mod generation_log_repository;
pub use generation_log_repository::{
    GenerationLogRepository, GenerationAttempt, 
    ParseResult, LintResult, CompileResult,
    TrainingPair, CorrectionPair
};
```

---

## Part 3: Integration Points

### 3.1 Where to Capture

The generation log should be called from wherever the agent generates DSL. Likely locations:

1. **MCP tool handler** — if agent calls `dsl_generate` tool
2. **API route** — if there's a `/api/agent/generate` endpoint
3. **Session execute** — if DSL is generated inline during session

### 3.2 Example Integration

```rust
// In your agent/generation code:

async fn generate_dsl_with_logging(
    user_intent: &str,
    domain: &str,
    gen_log_repo: &GenerationLogRepository,
    // ... other deps
) -> Result<String, Error> {
    // Start log
    let log_id = gen_log_repo.start_log(
        user_intent,
        domain,
        session_id,
        cbu_id,
        Some("claude-sonnet-4-20250514"),
    ).await?;

    let mut attempt_num = 0;
    let mut last_dsl = None;
    let mut last_error = None;

    loop {
        attempt_num += 1;
        if attempt_num > MAX_ATTEMPTS {
            gen_log_repo.mark_failed(log_id).await?;
            return Err(anyhow!("Max attempts exceeded"));
        }

        // Build prompt (include last_error for retry)
        let prompt = build_prompt(user_intent, last_error.as_deref());
        
        // Call LLM
        let start = Instant::now();
        let response = call_llm(&prompt).await?;
        let latency = start.elapsed().as_millis() as i32;

        // Extract DSL from response
        let extracted_dsl = extract_dsl_from_response(&response);

        // Parse
        let parse_result = match parse_program(&extracted_dsl) {
            Ok(_) => ParseResult { success: true, error: None },
            Err(e) => ParseResult { success: false, error: Some(e.to_string()) },
        };

        // Lint (if parse succeeded)
        let lint_result = if parse_result.success {
            run_linter(&extracted_dsl)
        } else {
            LintResult { valid: false, errors: vec![], warnings: vec![] }
        };

        // Compile (if lint succeeded)
        let compile_result = if lint_result.valid {
            match compile_dsl(&extracted_dsl) {
                Ok(plan) => CompileResult { 
                    success: true, 
                    error: None, 
                    step_count: plan.len() as i32 
                },
                Err(e) => CompileResult { 
                    success: false, 
                    error: Some(e.to_string()), 
                    step_count: 0 
                },
            }
        } else {
            CompileResult { success: false, error: None, step_count: 0 }
        };

        // Log this attempt
        let attempt = GenerationAttempt {
            attempt: attempt_num,
            timestamp: Utc::now(),
            prompt_template: Some("cbu_create_v1".to_string()),
            prompt_text: prompt,
            raw_response: response,
            extracted_dsl: Some(extracted_dsl.clone()),
            parse_result: parse_result.clone(),
            lint_result: lint_result.clone(),
            compile_result: compile_result.clone(),
            latency_ms: Some(latency),
            input_tokens: None,  // Fill if available
            output_tokens: None,
        };
        gen_log_repo.add_attempt(log_id, &attempt).await?;

        // Check if successful
        if compile_result.success {
            gen_log_repo.mark_success(log_id, &extracted_dsl, None).await?;
            return Ok(extracted_dsl);
        }

        // Prepare for retry
        last_dsl = Some(extracted_dsl);
        last_error = compile_result.error
            .or(lint_result.errors.first().cloned())
            .or(parse_result.error);
    }
}
```

---

## Part 4: Export Scripts

### 4.1 JSONL Export for Fine-tuning

**File:** `scripts/export_training_data.py` (or Rust CLI)

```python
#!/usr/bin/env python3
"""Export training pairs as JSONL for fine-tuning."""

import json
import psycopg2

conn = psycopg2.connect("postgresql://...")
cur = conn.cursor()

# Export intent → DSL pairs
cur.execute("""
    SELECT user_intent, final_valid_dsl
    FROM "ob-poc".dsl_generation_log
    WHERE success = true AND final_valid_dsl IS NOT NULL
""")

with open("training_pairs.jsonl", "w") as f:
    for intent, dsl in cur.fetchall():
        record = {
            "messages": [
                {"role": "user", "content": intent},
                {"role": "assistant", "content": dsl}
            ]
        }
        f.write(json.dumps(record) + "\n")

print(f"Exported {cur.rowcount} training pairs")
```

### 4.2 Correction Pairs Export

```python
# Export error → fix pairs
cur.execute("""
    SELECT 
        user_intent,
        iterations->0->>'extracted_dsl' as bad_dsl,
        iterations->0->'lint_result'->'errors'->0 as error,
        final_valid_dsl
    FROM "ob-poc".dsl_generation_log
    WHERE success = true AND total_attempts > 1
""")

with open("correction_pairs.jsonl", "w") as f:
    for intent, bad, error, good in cur.fetchall():
        record = {
            "messages": [
                {"role": "user", "content": f"Fix this DSL:\n{bad}\n\nError: {error}"},
                {"role": "assistant", "content": good}
            ]
        }
        f.write(json.dumps(record) + "\n")
```

---

## Part 5: Implementation Order

1. [ ] Create migration: `sql/migrations/XXXXXX_add_dsl_generation_log.sql`
2. [ ] Run migration
3. [ ] Create `rust/src/database/generation_log_repository.rs`
4. [ ] Add to `database/mod.rs`
5. [ ] Identify integration point (where agent generates DSL)
6. [ ] Wire in logging calls
7. [ ] Test with a few generations
8. [ ] Verify data with:
   ```sql
   SELECT log_id, user_intent, success, total_attempts, 
          jsonb_array_length(iterations) as logged_attempts
   FROM "ob-poc".dsl_generation_log
   ORDER BY created_at DESC LIMIT 10;
   ```

---

## Part 6: Future Enhancements

### 6.1 Embedding-based RAG

Add vector column for semantic similarity search:

```sql
ALTER TABLE "ob-poc".dsl_generation_log 
ADD COLUMN intent_embedding vector(1536);

CREATE INDEX idx_gen_log_embedding 
ON "ob-poc".dsl_generation_log 
USING ivfflat (intent_embedding vector_cosine_ops);
```

### 6.2 A/B Testing Prompts

Track which prompt template version was used and compare success rates.

### 6.3 Feedback Loop

Add human review column to mark "good" vs "acceptable" vs "poor" outputs for weighted training.

---

## Notes

- Keep `raw_response` for debugging but may want to exclude from training exports (noisy)
- `user_intent` should be the clean natural language, not the full prompt
- Consider retention policy — these logs can grow large
- The `pg_trgm` extension is needed for similarity search; ensure it's enabled:
  ```sql
  CREATE EXTENSION IF NOT EXISTS pg_trgm;
  ```
