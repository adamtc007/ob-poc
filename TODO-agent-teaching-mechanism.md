# TODO: Implement Agent Teaching Mechanism for Verb Pattern Learning

**Created:** 2026-01-21
**Status:** READY FOR IMPLEMENTATION
**Priority:** HIGH
**Complexity:** Medium

---

## Executive Summary

Implement a direct teaching mechanism that allows users to explicitly teach phrase→verb mappings to the semantic matching system. This bypasses the slow feedback loop (which learns from usage over time) and provides immediate, trusted pattern additions.

**Key insight:** We're not training the Candle embedding model - we're training the pattern database. The model is already good at semantic similarity; what we need is more patterns per verb to improve coverage.

```
User phrase → Candle embed → pgvector search → Match against YOUR TAUGHT PATTERNS → Return verb
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TEACHING MECHANISMS                                  │
│                                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │  DSL Verb       │  │  MCP Tool       │  │  YAML Batch     │             │
│  │  agent.teach    │  │  teach_phrase   │  │  Import         │             │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘             │
│           │                    │                    │                       │
│           └────────────────────┼────────────────────┘                       │
│                                │                                            │
│                                ▼                                            │
│                    ┌───────────────────────┐                               │
│                    │  agent.teach_phrase() │  ← SQL function               │
│                    │  (trusted source)     │                               │
│                    └───────────┬───────────┘                               │
│                                │                                            │
│                                ▼                                            │
│                    ┌───────────────────────┐                               │
│                    │  dsl_verbs            │                               │
│                    │  .intent_patterns[]   │  ← Pattern added directly     │
│                    └───────────┬───────────┘                               │
│                                │                                            │
│                                ▼                                            │
│                    ┌───────────────────────┐                               │
│                    │  populate_embeddings  │  ← Run to activate            │
│                    │  (creates vectors)    │                               │
│                    └───────────┬───────────┘                               │
│                                │                                            │
│                                ▼                                            │
│                    ┌───────────────────────┐                               │
│                    │  verb_pattern_        │                               │
│                    │  embeddings           │  ← Now searchable             │
│                    └───────────────────────┘                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: SQL Foundation

**File:** `migrations/044_agent_teaching.sql`

```sql
-- Migration 044: Agent teaching mechanism
-- Direct phrase→verb teaching that bypasses candidate staging

-- ============================================================================
-- 1. Teaching function (trusted source, no staging)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.teach_phrase(
    p_phrase TEXT,
    p_verb TEXT,
    p_source TEXT DEFAULT 'direct_teaching'
) RETURNS BOOLEAN AS $$
DECLARE
    v_normalized TEXT;
    v_added BOOLEAN;
    v_word_count INT;
BEGIN
    -- Normalize phrase
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));
    
    -- Basic validation: not empty
    IF v_normalized = '' OR v_normalized IS NULL THEN
        RAISE EXCEPTION 'Phrase cannot be empty';
    END IF;
    
    -- Basic validation: verb exists
    IF NOT EXISTS (SELECT 1 FROM "ob-poc".dsl_verbs WHERE full_name = p_verb) THEN
        RAISE EXCEPTION 'Unknown verb: %. Use verbs.list to see available verbs.', p_verb;
    END IF;
    
    -- Word count check (warn but don't block for teaching)
    v_word_count := array_length(string_to_array(v_normalized, ' '), 1);
    IF v_word_count < 2 THEN
        RAISE WARNING 'Very short phrase (% words) - may cause false positives', v_word_count;
    END IF;
    
    -- Add to dsl_verbs.intent_patterns
    SELECT "ob-poc".add_learned_pattern(p_verb, v_normalized) INTO v_added;
    
    IF v_added THEN
        -- Audit the teaching
        INSERT INTO agent.learning_audit (
            action, 
            learning_type, 
            actor, 
            details
        ) VALUES (
            'taught',
            'invocation_phrase', 
            p_source,
            jsonb_build_object(
                'phrase', v_normalized, 
                'verb', p_verb,
                'word_count', v_word_count
            )
        );
    END IF;
    
    RETURN v_added;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.teach_phrase IS 
    'Directly teach a phrase→verb mapping. Bypasses candidate staging (trusted source).';

-- ============================================================================
-- 2. Batch teaching function
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.teach_phrases_batch(
    p_phrases JSONB,  -- Array of {phrase, verb} objects
    p_source TEXT DEFAULT 'batch_teaching'
) RETURNS TABLE (
    phrase TEXT,
    verb TEXT,
    success BOOLEAN,
    message TEXT
) AS $$
DECLARE
    v_item JSONB;
    v_phrase TEXT;
    v_verb TEXT;
    v_added BOOLEAN;
BEGIN
    FOR v_item IN SELECT * FROM jsonb_array_elements(p_phrases)
    LOOP
        v_phrase := v_item->>'phrase';
        v_verb := v_item->>'verb';
        
        BEGIN
            SELECT agent.teach_phrase(v_phrase, v_verb, p_source) INTO v_added;
            
            phrase := v_phrase;
            verb := v_verb;
            success := v_added;
            message := CASE 
                WHEN v_added THEN 'Learned'
                ELSE 'Already exists'
            END;
            RETURN NEXT;
            
        EXCEPTION WHEN OTHERS THEN
            phrase := v_phrase;
            verb := v_verb;
            success := FALSE;
            message := SQLERRM;
            RETURN NEXT;
        END;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.teach_phrases_batch IS 
    'Batch teach multiple phrase→verb mappings from JSON array.';

-- ============================================================================
-- 3. View: Recently taught patterns
-- ============================================================================

CREATE OR REPLACE VIEW agent.v_recently_taught AS
SELECT 
    la.id,
    la.details->>'phrase' as phrase,
    la.details->>'verb' as verb,
    la.actor as source,
    la.timestamp as taught_at,
    -- Check if embedding exists yet
    EXISTS (
        SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
        WHERE vpe.verb_name = la.details->>'verb'
          AND vpe.pattern_normalized = la.details->>'phrase'
          AND vpe.embedding IS NOT NULL
    ) as has_embedding
FROM agent.learning_audit la
WHERE la.action = 'taught'
  AND la.learning_type = 'invocation_phrase'
ORDER BY la.timestamp DESC
LIMIT 100;

COMMENT ON VIEW agent.v_recently_taught IS 
    'Recently taught patterns with embedding status. Run populate_embeddings to activate patterns without embeddings.';

-- ============================================================================
-- 4. Function: Get patterns pending embedding
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_pending_embeddings()
RETURNS TABLE (
    verb TEXT,
    phrase TEXT,
    taught_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        la.details->>'verb' as verb,
        la.details->>'phrase' as phrase,
        la.timestamp as taught_at
    FROM agent.learning_audit la
    WHERE la.action = 'taught'
      AND la.learning_type = 'invocation_phrase'
      AND NOT EXISTS (
          SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
          WHERE vpe.verb_name = la.details->>'verb'
            AND vpe.pattern_normalized = la.details->>'phrase'
            AND vpe.embedding IS NOT NULL
      )
    ORDER BY la.timestamp DESC;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 5. Function: Unteach a pattern (with audit)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.unteach_phrase(
    p_phrase TEXT,
    p_verb TEXT,
    p_reason TEXT DEFAULT NULL,
    p_actor TEXT DEFAULT 'manual'
) RETURNS BOOLEAN AS $$
DECLARE
    v_normalized TEXT;
    v_removed BOOLEAN := FALSE;
BEGIN
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));
    
    -- Remove from dsl_verbs.intent_patterns
    UPDATE "ob-poc".dsl_verbs
    SET intent_patterns = array_remove(intent_patterns, v_normalized),
        updated_at = NOW()
    WHERE full_name = p_verb
      AND v_normalized = ANY(intent_patterns);
    
    v_removed := FOUND;
    
    IF v_removed THEN
        -- Remove from embeddings cache
        DELETE FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_name = p_verb
          AND pattern_normalized = v_normalized;
        
        -- Audit
        INSERT INTO agent.learning_audit (
            action, learning_type, actor, details
        ) VALUES (
            'untaught',
            'invocation_phrase',
            p_actor,
            jsonb_build_object(
                'phrase', v_normalized,
                'verb', p_verb,
                'reason', p_reason
            )
        );
    END IF;
    
    RETURN v_removed;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.unteach_phrase IS 
    'Remove a taught pattern (with audit). Use when a pattern causes problems.';

-- ============================================================================
-- 6. Teaching stats view
-- ============================================================================

CREATE OR REPLACE VIEW agent.v_teaching_stats AS
SELECT
    DATE_TRUNC('day', la.timestamp) as day,
    la.actor as source,
    COUNT(*) as patterns_taught,
    COUNT(DISTINCT la.details->>'verb') as verbs_affected
FROM agent.learning_audit la
WHERE la.action = 'taught'
  AND la.learning_type = 'invocation_phrase'
  AND la.timestamp > NOW() - INTERVAL '30 days'
GROUP BY 1, 2
ORDER BY 1 DESC, 3 DESC;
```

---

### Phase 2: DSL Verb Definition

**File:** `rust/config/verbs/agent.yaml`

```yaml
domains:
  agent:
    description: "Agent learning, teaching, and configuration"
    
    invocation_hints:
      - "teach"
      - "learn"
      - "train"
      - "agent"
    
    verbs:
      teach:
        description: "Teach the agent phrase→verb mappings for better intent recognition"
        behavior: plugin
        
        invocation_phrases:
          - "teach the agent"
          - "teach a phrase"
          - "train the agent"
          - "add a learning pattern"
          - "start teaching session"
          - "teach phrase to verb mapping"
        
        metadata:
          tier: meta
          source_of_truth: operational
          scope: global
          noun: teaching_session
        
        args:
          - name: mode
            type: string
            required: false
            valid_values: [interactive, batch, single]
            default: single
            description: |
              - single: Teach one phrase→verb mapping
              - batch: Load mappings from YAML file
              - interactive: Start Q&A teaching session
            
          - name: phrase
            type: string
            required: false
            description: "The phrase to teach (required for single mode)"
            
          - name: verb
            type: string
            required: false
            description: "Target verb full name, e.g. 'cbu.create' (required for single mode)"
            
          - name: from-file
            type: string
            required: false
            description: "Path to YAML file with phrase mappings (for batch mode)"
        
        returns:
          type: record
          fields:
            - name: phrases_learned
              type: integer
              description: "Number of new patterns added"
            - name: phrases_skipped
              type: integer
              description: "Number of patterns skipped (already exist)"
            - name: message
              type: string
              description: "Status message"
            - name: pending_embeddings
              type: integer
              description: "Patterns awaiting populate_embeddings"

      unteach:
        description: "Remove a previously taught phrase→verb mapping"
        behavior: plugin
        
        invocation_phrases:
          - "unteach a phrase"
          - "remove a learning pattern"
          - "delete phrase mapping"
          - "forget this phrase"
        
        metadata:
          tier: meta
          source_of_truth: operational
          scope: global
        
        args:
          - name: phrase
            type: string
            required: true
            description: "The phrase to remove"
            
          - name: verb
            type: string
            required: true
            description: "The verb it was mapped to"
            
          - name: reason
            type: string
            required: false
            description: "Why this mapping is being removed (for audit)"
        
        returns:
          type: record
          fields:
            - name: removed
              type: boolean
            - name: message
              type: string

      teaching-status:
        description: "Show recently taught patterns and embedding status"
        behavior: plugin
        
        invocation_phrases:
          - "show taught patterns"
          - "teaching status"
          - "what have I taught"
          - "pending embeddings"
        
        metadata:
          tier: meta
          source_of_truth: operational
          scope: global
        
        args:
          - name: limit
            type: integer
            required: false
            default: 20
            description: "Max patterns to show"
            
          - name: pending-only
            type: boolean
            required: false
            default: false
            description: "Only show patterns awaiting embeddings"
        
        returns:
          type: record_set
```

---

### Phase 3: Rust Plugin Handler

**File:** `rust/src/dsl_v2/custom_ops/agent_ops.rs` (NEW or update)

```rust
//! Agent teaching operations - direct phrase→verb learning

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::info;

use crate::dsl_v2::{DslArgs, ExecutionContext, ExecutionResult};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct TeachResult {
    pub phrases_learned: i32,
    pub phrases_skipped: i32,
    pub message: String,
    pub pending_embeddings: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnteachResult {
    pub removed: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaughtPattern {
    pub phrase: String,
    pub verb: String,
    pub source: String,
    pub taught_at: String,
    pub has_embedding: bool,
}

#[derive(Debug, Deserialize)]
pub struct TrainingFile {
    pub phrases: Vec<TrainingPhrase>,
}

#[derive(Debug, Deserialize)]
pub struct TrainingPhrase {
    pub phrase: String,
    pub verb: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// Handle agent.teach verb
pub async fn handle_agent_teach(
    ctx: &ExecutionContext,
    args: &DslArgs,
) -> Result<ExecutionResult> {
    let mode = args.get_string("mode").unwrap_or_else(|| "single".to_string());
    
    match mode.as_str() {
        "single" => handle_teach_single(ctx, args).await,
        "batch" => handle_teach_batch(ctx, args).await,
        "interactive" => handle_teach_interactive(ctx).await,
        _ => Err(anyhow!("Invalid mode: {}. Use 'single', 'batch', or 'interactive'", mode)),
    }
}

/// Single phrase teaching
async fn handle_teach_single(
    ctx: &ExecutionContext,
    args: &DslArgs,
) -> Result<ExecutionResult> {
    let phrase = args.require_string("phrase")
        .context("'phrase' is required for single mode")?;
    let verb = args.require_string("verb")
        .context("'verb' is required for single mode")?;
    
    let added = teach_single_phrase(&ctx.pool, &phrase, &verb, "dsl_teaching").await?;
    let pending = count_pending_embeddings(&ctx.pool).await?;
    
    let result = TeachResult {
        phrases_learned: if added { 1 } else { 0 },
        phrases_skipped: if added { 0 } else { 1 },
        message: if added {
            format!("✓ Learned: '{}' → {}", phrase, verb)
        } else {
            format!("Skipped (already exists): '{}' → {}", phrase, verb)
        },
        pending_embeddings: pending,
    };
    
    if pending > 0 {
        info!(
            "Taught phrase. {} patterns pending embeddings - run populate_embeddings to activate.",
            pending
        );
    }
    
    Ok(ExecutionResult::Record(serde_json::to_value(result)?))
}

/// Batch teaching from YAML file
async fn handle_teach_batch(
    ctx: &ExecutionContext,
    args: &DslArgs,
) -> Result<ExecutionResult> {
    let file_path = args.require_string("from-file")
        .context("'from-file' is required for batch mode")?;
    
    // Load and parse YAML
    let content = std::fs::read_to_string(&file_path)
        .context(format!("Failed to read file: {}", file_path))?;
    
    let training: TrainingFile = serde_yaml::from_str(&content)
        .context("Failed to parse YAML. Expected format: { phrases: [{phrase, verb}, ...] }")?;
    
    if training.phrases.is_empty() {
        return Err(anyhow!("No phrases found in file"));
    }
    
    // Teach each phrase
    let mut learned = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();
    
    for item in &training.phrases {
        match teach_single_phrase(&ctx.pool, &item.phrase, &item.verb, "batch_teaching").await {
            Ok(true) => learned += 1,
            Ok(false) => skipped += 1,
            Err(e) => {
                errors.push(format!("'{}' → {}: {}", item.phrase, item.verb, e));
            }
        }
    }
    
    let pending = count_pending_embeddings(&ctx.pool).await?;
    
    let mut message = format!(
        "Batch complete: {} learned, {} skipped",
        learned, skipped
    );
    
    if !errors.is_empty() {
        message.push_str(&format!(", {} errors", errors.len()));
    }
    
    if pending > 0 {
        message.push_str(&format!(
            "\n\n⚠️  {} patterns pending embeddings. Run:\n   populate_embeddings\nto activate them for semantic search.",
            pending
        ));
    }
    
    let result = TeachResult {
        phrases_learned: learned,
        phrases_skipped: skipped,
        message,
        pending_embeddings: pending,
    };
    
    Ok(ExecutionResult::Record(serde_json::to_value(result)?))
}

/// Interactive teaching mode
/// Returns a special result that triggers the UI to enter teaching mode
async fn handle_teach_interactive(
    ctx: &ExecutionContext,
) -> Result<ExecutionResult> {
    // For now, return instructions. Full interactive mode requires UI support.
    let message = r#"
Interactive Teaching Mode
=========================

Type phrases you'd use to invoke verbs. For each phrase, I'll show you 
the current best matches and ask which verb it should trigger.

Example session:
  You: "set up an ISDA with Goldman"
  Agent: Best matches:
         1. isda.create (0.82)
         2. isda.assign-counterparty (0.71)
         Which verb? [1/2/other]: 1
  Agent: ✓ Learned: "set up an isda with goldman" → isda.create

Commands:
  - Type a phrase to teach it
  - Type 'done' to finish
  - Type 'status' to see pending embeddings
  - Type 'undo' to remove the last taught phrase

Note: After teaching, run `populate_embeddings` to activate new patterns.
"#;
    
    // TODO: When UI supports interactive mode, return:
    // Ok(ExecutionResult::Interactive(InteractiveSession::Teaching { ... }))
    
    Ok(ExecutionResult::Record(serde_json::json!({
        "mode": "interactive",
        "message": message,
        "status": "not_yet_implemented",
        "hint": "Use single mode: (agent.teach :phrase \"your phrase\" :verb target.verb)"
    })))
}

/// Handle agent.unteach verb
pub async fn handle_agent_unteach(
    ctx: &ExecutionContext,
    args: &DslArgs,
) -> Result<ExecutionResult> {
    let phrase = args.require_string("phrase")?;
    let verb = args.require_string("verb")?;
    let reason = args.get_string("reason");
    
    let removed: bool = sqlx::query_scalar(
        r#"SELECT agent.unteach_phrase($1, $2, $3, 'dsl_unteach')"#
    )
    .bind(&phrase)
    .bind(&verb)
    .bind(&reason)
    .fetch_one(&ctx.pool)
    .await?;
    
    let result = UnteachResult {
        removed,
        message: if removed {
            format!("✓ Removed: '{}' → {}", phrase, verb)
        } else {
            format!("Not found: '{}' → {}", phrase, verb)
        },
    };
    
    Ok(ExecutionResult::Record(serde_json::to_value(result)?))
}

/// Handle agent.teaching-status verb
pub async fn handle_teaching_status(
    ctx: &ExecutionContext,
    args: &DslArgs,
) -> Result<ExecutionResult> {
    let limit = args.get_i32("limit").unwrap_or(20);
    let pending_only = args.get_bool("pending-only").unwrap_or(false);
    
    let patterns: Vec<TaughtPattern> = if pending_only {
        sqlx::query_as(
            r#"
            SELECT 
                phrase, 
                verb, 
                'pending' as source,
                taught_at::text,
                false as has_embedding
            FROM agent.get_pending_embeddings()
            LIMIT $1
            "#
        )
        .bind(limit)
        .fetch_all(&ctx.pool)
        .await?
    } else {
        sqlx::query_as(
            r#"
            SELECT 
                phrase, 
                verb, 
                source,
                taught_at::text,
                has_embedding
            FROM agent.v_recently_taught
            LIMIT $1
            "#
        )
        .bind(limit)
        .fetch_all(&ctx.pool)
        .await?
    };
    
    let pending_count = count_pending_embeddings(&ctx.pool).await?;
    
    // Add summary as first record
    let mut results: Vec<serde_json::Value> = vec![
        serde_json::json!({
            "_summary": true,
            "total_shown": patterns.len(),
            "pending_embeddings": pending_count,
            "hint": if pending_count > 0 {
                "Run populate_embeddings to activate pending patterns"
            } else {
                "All patterns have embeddings"
            }
        })
    ];
    
    for p in patterns {
        results.push(serde_json::to_value(p)?);
    }
    
    Ok(ExecutionResult::RecordSet(results))
}

// ============================================================================
// Helpers
// ============================================================================

async fn teach_single_phrase(
    pool: &PgPool,
    phrase: &str,
    verb: &str,
    source: &str,
) -> Result<bool> {
    let result: Result<bool, sqlx::Error> = sqlx::query_scalar(
        r#"SELECT agent.teach_phrase($1, $2, $3)"#
    )
    .bind(phrase)
    .bind(verb)
    .bind(source)
    .fetch_one(pool)
    .await;
    
    match result {
        Ok(added) => Ok(added),
        Err(e) => {
            // Extract the RAISE EXCEPTION message if present
            let msg = e.to_string();
            if msg.contains("Unknown verb") {
                Err(anyhow!("Unknown verb: {}. Use (verbs.list) to see available verbs.", verb))
            } else {
                Err(anyhow!("Teaching failed: {}", msg))
            }
        }
    }
}

async fn count_pending_embeddings(pool: &PgPool) -> Result<i32> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM agent.get_pending_embeddings()"#
    )
    .fetch_one(pool)
    .await?;
    
    Ok(count as i32)
}
```

---

### Phase 4: MCP Tool for Quick Teaching

**File:** `rust/src/mcp/tools/learning_tools.rs` (add to existing or create)

```rust
//! MCP tools for teaching phrase→verb mappings

use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Teach a single phrase→verb mapping
/// 
/// Example: teach_phrase("spin up a fund", "cbu.create")
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TeachPhraseInput {
    /// The phrase users might say
    pub phrase: String,
    /// The DSL verb it should trigger (e.g., "cbu.create")
    pub verb: String,
}

#[derive(Debug, Serialize)]
pub struct TeachPhraseOutput {
    pub success: bool,
    pub message: String,
    pub pending_embeddings: i32,
}

pub async fn teach_phrase(
    pool: &PgPool,
    input: TeachPhraseInput,
) -> Result<TeachPhraseOutput> {
    // Teach via SQL function
    let added: Result<bool, sqlx::Error> = sqlx::query_scalar(
        r#"SELECT agent.teach_phrase($1, $2, 'mcp_teaching')"#
    )
    .bind(&input.phrase)
    .bind(&input.verb)
    .fetch_one(pool)
    .await;
    
    let pending: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM agent.get_pending_embeddings()"#
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    
    match added {
        Ok(true) => Ok(TeachPhraseOutput {
            success: true,
            message: format!(
                "✓ Taught: '{}' → {}\n\nRun `populate_embeddings` to activate ({} pending).",
                input.phrase, input.verb, pending
            ),
            pending_embeddings: pending as i32,
        }),
        Ok(false) => Ok(TeachPhraseOutput {
            success: false,
            message: format!("Already exists: '{}' → {}", input.phrase, input.verb),
            pending_embeddings: pending as i32,
        }),
        Err(e) => Ok(TeachPhraseOutput {
            success: false,
            message: format!("Failed: {}", e),
            pending_embeddings: pending as i32,
        }),
    }
}

/// List pending patterns that need embeddings
pub async fn list_pending_patterns(pool: &PgPool) -> Result<Vec<PendingPattern>> {
    let patterns: Vec<PendingPattern> = sqlx::query_as(
        r#"SELECT verb, phrase, taught_at::text FROM agent.get_pending_embeddings() LIMIT 50"#
    )
    .fetch_all(pool)
    .await?;
    
    Ok(patterns)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PendingPattern {
    pub verb: String,
    pub phrase: String,
    pub taught_at: String,
}

/// Unteach a pattern that's causing problems
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnteachPhraseInput {
    pub phrase: String,
    pub verb: String,
    pub reason: Option<String>,
}

pub async fn unteach_phrase(
    pool: &PgPool,
    input: UnteachPhraseInput,
) -> Result<String> {
    let removed: bool = sqlx::query_scalar(
        r#"SELECT agent.unteach_phrase($1, $2, $3, 'mcp_unteach')"#
    )
    .bind(&input.phrase)
    .bind(&input.verb)
    .bind(&input.reason)
    .fetch_one(pool)
    .await?;
    
    if removed {
        Ok(format!("✓ Removed: '{}' → {}", input.phrase, input.verb))
    } else {
        Ok(format!("Not found: '{}' → {}", input.phrase, input.verb))
    }
}
```

---

### Phase 5: Training File Format

**File:** `docs/training-phrases-format.md` (documentation)

```markdown
# Training Phrases File Format

YAML file for batch teaching phrase→verb mappings.

## Basic Format

```yaml
phrases:
  - phrase: "spin up a fund"
    verb: cbu.create
    
  - phrase: "create a trading book"
    verb: cbu.create
    
  - phrase: "onboard a new client"
    verb: cbu.create
```

## Usage

```clojure
(agent.teach :mode batch :from-file "path/to/training.yaml")
```

## Guidelines

1. **Use natural language** - Write phrases as users would actually say them
2. **Multiple phrases per verb** - Add 5-10 variations for good coverage
3. **Include domain vocabulary** - "ISDA", "KYC", "UBO" should be in phrases
4. **Avoid generic phrases** - "create" alone is too vague
5. **Test after teaching** - Run `populate_embeddings` and test matches

## Example: Complete Training File

```yaml
# training-phrases.yaml
# Phrase→verb mappings for ob-poc semantic matching

phrases:
  # CBU creation
  - phrase: "spin up a fund"
    verb: cbu.create
  - phrase: "create a trading book"
    verb: cbu.create
  - phrase: "onboard a new client"
    verb: cbu.create
  - phrase: "set up a new cbu"
    verb: cbu.create
  - phrase: "add a client business unit"
    verb: cbu.create

  # ISDA management
  - phrase: "set up an ISDA"
    verb: isda.create
  - phrase: "create a master agreement"
    verb: isda.create
  - phrase: "add an ISDA with"
    verb: isda.create
  - phrase: "new derivatives agreement"
    verb: isda.create

  # UBO discovery
  - phrase: "who owns"
    verb: ubo.discover
  - phrase: "show me the ownership chain"
    verb: ubo.discover
  - phrase: "beneficial owners of"
    verb: ubo.discover
  - phrase: "ultimate beneficial owner"
    verb: ubo.discover
  - phrase: "ownership structure for"
    verb: ubo.discover

  # GLEIF lookup
  - phrase: "look up LEI for"
    verb: gleif.lookup
  - phrase: "find the LEI"
    verb: gleif.lookup
  - phrase: "search GLEIF for"
    verb: gleif.lookup

  # KYC workflows
  - phrase: "start KYC for"
    verb: kyc.initiate
  - phrase: "begin due diligence on"
    verb: kyc.initiate
  - phrase: "run KYC checks"
    verb: kyc.initiate
```
```

---

## Wire-up: Register Plugin Handlers

**File:** `rust/src/dsl_v2/custom_ops/mod.rs` (update)

```rust
// Add to plugin registry
pub fn register_agent_ops(registry: &mut PluginRegistry) {
    registry.register("agent.teach", handle_agent_teach);
    registry.register("agent.unteach", handle_agent_unteach);
    registry.register("agent.teaching-status", handle_teaching_status);
}
```

---

## Testing Checklist

- [ ] Migration 044 applies cleanly
- [ ] `agent.teach_phrase()` SQL function works
- [ ] `agent.unteach_phrase()` removes patterns and embeddings
- [ ] DSL verb `(agent.teach :phrase "..." :verb ...)` works
- [ ] Batch mode loads YAML file correctly
- [ ] `(agent.teaching-status)` shows recent patterns
- [ ] MCP `teach_phrase` tool works
- [ ] Audit log captures all teaching events
- [ ] `populate_embeddings` picks up new patterns
- [ ] Semantic search finds newly taught patterns

---

## Usage Examples

### Quick Teaching via DSL

```clojure
;; Single phrase
(agent.teach :phrase "spin up a fund for Acme" :verb cbu.create)
;; → ✓ Learned: 'spin up a fund for acme' → cbu.create

;; Check status
(agent.teaching-status)
;; → Shows recent patterns and pending embeddings

;; Remove a bad pattern
(agent.unteach :phrase "spin up a fund for acme" :verb cbu.create :reason "too specific")
```

### Batch Teaching

```clojure
;; Load from file
(agent.teach :mode batch :from-file "config/training-phrases.yaml")
;; → Batch complete: 25 learned, 3 skipped
;; → ⚠️  25 patterns pending embeddings. Run populate_embeddings to activate.
```

### Via MCP Tool (in Claude chat)

```
User: teach "who are the beneficial owners of" to trigger ubo.discover

Claude: [calls teach_phrase tool]
✓ Taught: 'who are the beneficial owners of' → ubo.discover

Run `populate_embeddings` to activate (1 pending).
```

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `migrations/044_agent_teaching.sql` | CREATE | Teaching SQL functions |
| `rust/config/verbs/agent.yaml` | UPDATE | Add teach/unteach/teaching-status verbs |
| `rust/src/dsl_v2/custom_ops/agent_ops.rs` | CREATE | Plugin handlers |
| `rust/src/dsl_v2/custom_ops/mod.rs` | UPDATE | Register handlers |
| `rust/src/mcp/tools/learning_tools.rs` | UPDATE | MCP tools |
| `docs/training-phrases-format.md` | CREATE | Documentation |
| `config/training-phrases.yaml` | CREATE | Example training file |

---

## Post-Implementation: Seed Initial Patterns

After implementing, create a comprehensive training file with domain-specific phrases:

```bash
# Create initial training file
cat > config/training-phrases.yaml << 'EOF'
phrases:
  # ... 100+ phrases covering all key verbs
EOF

# Load them
(agent.teach :mode batch :from-file "config/training-phrases.yaml")

# Activate embeddings
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

This gives you immediate coverage improvement without waiting for the feedback loop.
