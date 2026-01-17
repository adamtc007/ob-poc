# TODO: MCP Intent Feedback Tool

**Priority**: HIGH (complements semantic pipeline)  
**Created**: 2025-01-17  
**Status**: NOT STARTED  
**Depends On**: TODO-MCP-SEMANTIC-INTENT-PIPELINE.md (Phase 3 learning loop)

## Overview

Add an explicit feedback tool that Claude can invoke when users correct the system's verb selection or entity resolution. This provides an unambiguous correction signal rather than inferring from edited DSL.

**Current state**: Learning relies on comparing generated DSL vs executed DSL to infer corrections. This is noisy — edits might be refinements, not corrections.

**Target state**: Claude explicitly calls `intent_feedback` when user says "no, I meant X" — creating a clean learning signal with user intent captured directly.

## Existing Infrastructure (Already Built)

| Component | Location | What It Does |
|-----------|----------|--------------|
| `AgentLearningInspector` | `src/agent/learning/inspector.rs` | Manages learning lifecycle |
| `LearningType` enum | `src/agent/learning/inspector.rs` | `EntityAlias`, `LexiconToken`, `InvocationPhrase`, `PromptChange` |
| `RiskLevel` enum | `src/agent/learning/inspector.rs` | `Low`, `Medium`, `High` |
| `LearningCandidate` | `src/agent/learning/inspector.rs` | Pending learning record |
| `agent.learning_candidates` | `migrations/032_agent_learning.sql` | DB table for candidates |
| `agent.invocation_phrases` | `migrations/032_agent_learning.sql` | Applied phrase mappings |
| `agent.entity_aliases` | `migrations/032_agent_learning.sql` | Applied entity aliases |
| `SharedLearnedData` | `src/agent/learning/warmup.rs` | In-memory learned data |

**Key insight**: We already have the full infrastructure. This TODO just adds an MCP tool to expose it explicitly.

---

## User Interaction Patterns

```
User: "Set up a CSA for Apex Fund"
Claude: [calls verb_search] → trading-profile.add-csa-config (0.82)
Claude: "I'll set up a CSA using trading-profile.add-csa-config..."
User: "No, I need an ISDA master agreement, not just the CSA annex"

Claude: [calls intent_feedback with correction]
Claude: "Got it — you need trading-profile.add-isda-config for the master agreement. 
         I've recorded this so I'll get it right next time."
```

```
User: "Add John Smith as director"
Claude: [resolves to wrong John Smith]
User: "Wrong one — I mean John Smith from the London office"

Claude: [calls intent_feedback with entity correction]
Claude: "Updated to John Smith (London). I've noted the disambiguation for future reference."
```

---

## Tool Definition

**File**: `rust/src/mcp/tools.rs`

Add after `verb_search` tool:

```rust
Tool {
    name: "intent_feedback".into(),
    description: r#"Record user correction to improve future intent matching.

Call this when the user indicates the system chose the wrong verb, entity, 
or interpretation. Creates an explicit learning signal.

Feedback types:
- verb_correction: Wrong verb was selected
- entity_correction: Wrong entity was resolved  
- phrase_mapping: User provides explicit phrase→verb mapping

Examples:
- User says "no, I meant ISDA not CSA" → verb_correction
- User says "wrong John Smith" → entity_correction
- User says "when I say 'onboard' I mean cbu.create" → phrase_mapping

The system will:
1. Record the correction immediately
2. Apply to future requests (low-risk: immediate, medium-risk: after threshold)
3. Return confirmation of what was learned"#.into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "feedback_type": {
                "type": "string",
                "enum": ["verb_correction", "entity_correction", "phrase_mapping"],
                "description": "Type of correction"
            },
            "original_input": {
                "type": "string",
                "description": "What the user originally said/asked"
            },
            "system_choice": {
                "type": "string",
                "description": "What the system selected (verb name, entity UUID)"
            },
            "correct_choice": {
                "type": "string",
                "description": "What the user actually wanted"
            },
            "user_explanation": {
                "type": "string",
                "description": "User's explanation of the correction (optional but valuable)"
            },
            "context": {
                "type": "object",
                "description": "Additional context",
                "properties": {
                    "session_id": { "type": "string", "format": "uuid" },
                    "cbu_id": { "type": "string", "format": "uuid" },
                    "domain": { "type": "string" }
                }
            }
        },
        "required": ["feedback_type", "original_input", "correct_choice"]
    }),
},
```

---

## Handler Implementation

**File**: `rust/src/mcp/handlers/core.rs`

### Add to dispatch

```rust
"intent_feedback" => self.intent_feedback(args).await,
```

### Handler method

```rust
/// Record user correction for learning
///
/// Uses existing AgentLearningInspector infrastructure.
async fn intent_feedback(&self, args: Value) -> Result<Value> {
    use crate::agent::learning::inspector::{
        AgentLearningInspector, LearningType, RiskLevel,
    };

    let feedback_type = args["feedback_type"]
        .as_str()
        .ok_or_else(|| anyhow!("feedback_type required"))?;
    let original_input = args["original_input"]
        .as_str()
        .ok_or_else(|| anyhow!("original_input required"))?;
    let system_choice = args["system_choice"].as_str();
    let correct_choice = args["correct_choice"]
        .as_str()
        .ok_or_else(|| anyhow!("correct_choice required"))?;
    let user_explanation = args["user_explanation"].as_str();

    // Map feedback_type to learning infrastructure
    let (learning_type, risk_level) = match feedback_type {
        "verb_correction" => (LearningType::InvocationPhrase, RiskLevel::Medium),
        "entity_correction" => (LearningType::EntityAlias, RiskLevel::Low),
        "phrase_mapping" => (LearningType::InvocationPhrase, RiskLevel::Medium),
        _ => return Err(anyhow!("Unknown feedback_type: {}", feedback_type)),
    };

    let pool = self.require_pool()?;
    let inspector = AgentLearningInspector::new(pool.clone());

    // Create fingerprint for deduplication
    let fingerprint = format!(
        "{}:{}:{}",
        learning_type.as_str(),
        original_input.to_lowercase().trim(),
        correct_choice.trim()
    );

    // Insert or increment learning candidate
    let (candidate_id, occurrence_count, was_created) = sqlx::query_as::<_, (i64, i32, bool)>(
        r#"
        INSERT INTO agent.learning_candidates (
            fingerprint, learning_type, input_pattern, suggested_output,
            risk_level, auto_applicable
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (fingerprint) DO UPDATE SET
            occurrence_count = agent.learning_candidates.occurrence_count + 1,
            last_seen = NOW(),
            updated_at = NOW()
        RETURNING id, occurrence_count, (xmax = 0)
        "#
    )
    .bind(&fingerprint)
    .bind(learning_type.as_str())
    .bind(original_input)
    .bind(correct_choice)
    .bind(risk_level.as_str())
    .bind(risk_level == RiskLevel::Low)
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow!("Failed to record learning: {}", e))?;

    // For low-risk corrections (entity aliases), apply immediately
    let auto_applied = if risk_level == RiskLevel::Low {
        sqlx::query_scalar!(
            r#"SELECT agent.upsert_entity_alias($1, $2, NULL, 'explicit_feedback')"#,
            original_input.to_lowercase().trim(),
            correct_choice
        )
        .fetch_one(pool)
        .await
        .is_ok()
    } else {
        false
    };

    // Hot-reload into memory if learned_data available and applied
    if auto_applied {
        if let Some(learned) = &self.learned_data {
            let mut guard = learned.write().await;
            guard.entity_aliases.insert(
                original_input.to_lowercase(),
                (correct_choice.to_string(), None),
            );
        }
    }

    // For medium-risk (phrase mappings), check if we hit threshold
    let threshold_applied = if risk_level == RiskLevel::Medium && occurrence_count >= 3 {
        // Apply the phrase mapping
        let applied = sqlx::query(
            r#"
            INSERT INTO agent.invocation_phrases (phrase, verb, source)
            VALUES ($1, $2, 'threshold_auto')
            ON CONFLICT (phrase, verb) DO UPDATE SET
                occurrence_count = agent.invocation_phrases.occurrence_count + 1,
                updated_at = NOW()
            "#
        )
        .bind(original_input.to_lowercase().trim())
        .bind(correct_choice)
        .execute(pool)
        .await
        .is_ok();

        if applied {
            // Mark candidate as applied
            let _ = sqlx::query(
                "UPDATE agent.learning_candidates SET status = 'applied', applied_at = NOW() WHERE id = $1"
            )
            .bind(candidate_id)
            .execute(pool)
            .await;

            // Hot-reload into memory
            if let Some(learned) = &self.learned_data {
                let mut guard = learned.write().await;
                guard.invocation_phrases.insert(
                    original_input.to_lowercase(),
                    correct_choice.to_string(),
                );
            }
        }
        applied
    } else {
        false
    };

    let message = self.build_feedback_message(
        feedback_type, 
        correct_choice, 
        auto_applied || threshold_applied,
        occurrence_count,
    );

    tracing::info!(
        feedback_type = feedback_type,
        input = original_input,
        correction = correct_choice,
        auto_applied = auto_applied,
        threshold_applied = threshold_applied,
        occurrence_count = occurrence_count,
        "Intent feedback recorded"
    );

    Ok(json!({
        "recorded": true,
        "candidate_id": candidate_id,
        "occurrence_count": occurrence_count,
        "was_new": was_created,
        "learning_type": learning_type.as_str(),
        "risk_level": risk_level.as_str(),
        "auto_applied": auto_applied,
        "threshold_applied": threshold_applied,
        "message": message,
        "what_was_learned": {
            "input": original_input,
            "maps_to": correct_choice,
            "type": feedback_type
        }
    }))
}

fn build_feedback_message(
    &self, 
    feedback_type: &str, 
    correct_choice: &str, 
    applied: bool,
    occurrence_count: i32,
) -> String {
    let base = match feedback_type {
        "verb_correction" => format!("Noted: '{}' is the right verb for this.", correct_choice),
        "entity_correction" => format!("Got it — using '{}' for future lookups.", correct_choice),
        "phrase_mapping" => format!("Learned: this phrase maps to '{}'.", correct_choice),
        _ => format!("Correction recorded."),
    };

    if applied {
        format!("{} Applied immediately.", base)
    } else {
        format!("{} Will apply after {} more confirmation(s).", base, 3 - occurrence_count)
    }
}
```

---

## Claude Usage Patterns

### Pattern 1: Verb Correction

```
User: "Set up custody for Apex Fund"
Claude: [verb_search returns cbu.add-product]
Claude: "I'll add custody as a product..."
User: "No, I need to configure the custody account settings"

Claude: [calls intent_feedback]
{
  "feedback_type": "verb_correction",
  "original_input": "set up custody",
  "system_choice": "cbu.add-product",
  "correct_choice": "custody.configure-account",
  "user_explanation": "User wanted account configuration, not product addition"
}
```

### Pattern 2: Entity Disambiguation

```
User: "Add Sarah Chen as signatory"
Claude: [resolves to Sarah Chen from Singapore]
User: "The one in our London office"

Claude: [calls intent_feedback]
{
  "feedback_type": "entity_correction",
  "original_input": "Sarah Chen",
  "system_choice": "uuid-singapore-sarah",
  "correct_choice": "uuid-london-sarah"
}
```

### Pattern 3: Explicit Phrase Mapping

```
User: "FYI, when I say 'spin up a fund' I always mean create a new CBU"

Claude: [calls intent_feedback]
{
  "feedback_type": "phrase_mapping",
  "original_input": "spin up a fund",
  "correct_choice": "cbu.create",
  "user_explanation": "User's preferred terminology"
}
```

---

## The Flywheel

```
Day 1: YAML invocation_phrases + semantic embeddings (cold start)
       ↓
       verb_search gets ~70-80% hit rate
       ↓
User corrects: "No, I meant X"
       ↓
Claude calls intent_feedback
       ↓
Learning candidate created in agent.learning_candidates
       ↓
Entity aliases: Applied IMMEDIATELY (low risk)
Phrase mappings: Applied after 3 occurrences (medium risk)
       ↓
warmup loads into LearnedData at next restart
       (or hot-reloaded via intent_reload)
       ↓
verb_search checks learned FIRST (exact match, score 1.0)
       ↓
Day 30: 90%+ hit rate, corrections rare
```

**Key insight**: Learned phrases bypass semantic similarity entirely. They're exact matches from real user vocabulary → your verbs. The embeddings are just the fallback for novel phrases.

---

## Integration with Semantic Pipeline

In `dsl_generate` response, hint about feedback option:

```rust
Ok(json!({
    // ... existing fields ...
    "feedback_hint": if verb_candidates.len() > 1 && verb_candidates[0].score < 0.9 {
        Some("If this isn't the right verb, let me know and I'll learn your preference.")
    } else {
        None
    }
}))
```

---

## Testing Checklist

### Unit Tests

- [ ] `intent_feedback` persists to `agent.learning_candidates` table
- [ ] `intent_feedback` increments `occurrence_count` on duplicate fingerprint
- [ ] Low-risk entity corrections apply immediately to `agent.entity_aliases`
- [ ] Medium-risk phrase mappings stay pending until threshold (3)
- [ ] Threshold application promotes pending → applied

### Integration Tests

- [ ] Full flow: feedback → persist → hot-reload → verb_search returns learned
- [ ] Concurrent feedback doesn't corrupt data (fingerprint uniqueness)
- [ ] Memory matches database after hot-reload

### End-to-End Tests

- [ ] Claude correctly identifies when to call `intent_feedback`
- [ ] Learned corrections improve subsequent `verb_search` results
- [ ] Entity disambiguations persist across sessions

---

## Files Modified Summary

| File | Action |
|------|--------|
| `rust/src/mcp/tools.rs` | ADD `intent_feedback` tool definition |
| `rust/src/mcp/handlers/core.rs` | ADD `intent_feedback` handler |

**No new tables needed** — uses existing `agent.*` schema from migration 032.

---

## Success Criteria

1. **Explicit corrections** recorded with full context
2. **Entity aliases** auto-apply immediately (low risk)
3. **Phrase mappings** apply after 3 occurrences (configurable threshold)
4. **Hot reload** — corrections take effect without restart
5. **Audit trail** — all learnings traceable via `agent.learning_candidates`
6. **Deduplication** — fingerprint prevents duplicate candidates

---

## Future Enhancements

1. **Negative feedback** — "never use X for Y" blocklist
2. **Confidence decay** — reduce learned confidence if later corrected
3. **User-specific learning** — per-user phrase preferences
4. **Bulk import** — load phrase mappings from CSV/YAML
5. **Learning dashboard** — UI to review/approve pending learnings via MCP tools (`intent_list`, `intent_approve`, `intent_reject`)
