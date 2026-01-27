# Claude Code Task: Disambiguation Feedback Loop

**Status**: Ready for implementation  
**Date**: 2026-01-27  
**Priority**: HIGH (closes the learning loop, required for 90%+ accuracy)

**Design Rationale**: See `docs/architecture/DISAMBIGUATION-FEEDBACK-LOOP-RATIONALE.md`

## Problem Summary

When verb search returns `Ambiguous`, the agent shows multiple verb options. When the user selects one, the selection is **not captured** for learning.

**The rule:**
> Ambiguity threshold crossed → always show verb options as clickable buttons  
> User selection = ground truth for learning (gold-standard label)

Without this, the feedback loop is broken. The system never learns from corrections.

---

## Current State

### Working ✅

1. **Ambiguity Detection** (`verb_search.rs`)
   - `normalize_candidates()` dedupes and ranks results
   - `check_ambiguity_with_margin()` detects close scores
   - `AMBIGUITY_MARGIN = 0.05` triggers disambiguation

2. **Execution Outcome Capture** (`feedback/service.rs`)
   - `FeedbackService.record_outcome_with_dsl()` captures successful executions
   - `Outcome::Executed` triggers learning signals

### Missing ❌

1. **No structured disambiguation UI** - Options shown as text, not clickable
2. **No callback on selection** - User's choice isn't captured
3. **`Outcome::SelectedAlt` never triggered** - Enum exists but unused
4. **Learning signal never recorded** - `(original_input, selected_verb)` pair lost

---

## Implementation: Option B (Structured UI + Endpoint)

### Why Option B

| Approach | Problem |
|----------|---------|
| Option A (user types selection) | Friction → users rephrase instead → signal lost |
| Option B (clickable buttons) | One click = instant capture, no lost signals |

### Step 1: Structured Disambiguation Response

Modify agent response to include structured data:

```rust
// In agent_service.rs or wherever ambiguity response is built

#[derive(Debug, Clone, Serialize)]
pub struct DisambiguationResponse {
    pub message: String,
    pub original_input: String,
    pub options: Vec<DisambiguationOption>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DisambiguationOption {
    pub verb_fqn: String,
    pub display_name: String,      // e.g., "List all CBUs in system"
    pub example: String,           // e.g., "(cbu.list)"
    pub score: f32,                // For transparency
}

// When VerbSearchOutcome::Ambiguous:
let response = DisambiguationResponse {
    message: "Which did you mean?".to_string(),
    original_input: user_input.clone(),
    options: candidates.iter().take(5).map(|c| DisambiguationOption {
        verb_fqn: c.verb_fqn.clone(),
        display_name: get_verb_description(&c.verb_fqn),
        example: format!("({})", c.verb_fqn),
        score: c.score,
    }).collect(),
};
```

### Step 2: Add Selection Endpoint

```rust
// In agent_routes.rs

/// POST /api/session/:id/select-verb
/// Called when user clicks a disambiguation option
pub async fn select_disambiguation_option(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<SelectVerbRequest>,
) -> Result<Json<SelectVerbResponse>, ApiError> {
    // 1. Record the learning signal (gold-standard label)
    state.feedback_service
        .record_disambiguation_selection(
            &payload.original_input,
            &payload.selected_verb,
            &payload.candidates,
        )
        .await?;
    
    // 2. Execute the selected verb
    let result = state.agent_service
        .execute_verb(&session_id, &payload.selected_verb, &payload.args)
        .await?;
    
    Ok(Json(SelectVerbResponse {
        executed: true,
        result,
    }))
}

#[derive(Debug, Deserialize)]
pub struct SelectVerbRequest {
    pub original_input: String,
    pub selected_verb: String,
    pub candidates: Vec<String>,  // All options that were shown
    pub args: Option<serde_json::Value>,  // Any args extracted from original input
}

#[derive(Debug, Serialize)]
pub struct SelectVerbResponse {
    pub executed: bool,
    pub result: DslExecutionResult,
}
```

### Step 3: Record Learning Signal with High Confidence

```rust
// In feedback/service.rs

impl FeedbackService {
    /// Record disambiguation selection as gold-standard learning signal
    /// 
    /// These are HIGH CONFIDENCE labels because user explicitly chose
    /// from alternatives. Weight higher than implicit signals.
    pub async fn record_disambiguation_selection(
        &self,
        original_input: &str,
        selected_verb: &str,
        candidates: &[String],
    ) -> Result<(), FeedbackError> {
        // Generate phrase variants to make learning more robust
        let variants = generate_phrase_variants(original_input);
        
        // Record primary phrase with SelectedAlt outcome
        let signal = LearningSignal {
            input_text: original_input.to_string(),
            matched_verb: selected_verb.to_string(),
            outcome: Outcome::SelectedAlt,
            confidence: 0.95,  // Gold-standard: user explicitly chose
            rejected_alternatives: candidates
                .iter()
                .filter(|c| *c != selected_verb)
                .cloned()
                .collect(),
            source: SignalSource::UserDisambiguation,
        };
        
        self.record_learning_signal(signal).await?;
        
        // Record variants with slightly lower confidence
        for variant in variants {
            if variant != original_input {
                let variant_signal = LearningSignal {
                    input_text: variant,
                    matched_verb: selected_verb.to_string(),
                    outcome: Outcome::SelectedAlt,
                    confidence: 0.85,  // Slightly lower for generated variants
                    rejected_alternatives: vec![],
                    source: SignalSource::GeneratedVariant,
                };
                self.record_learning_signal(variant_signal).await?;
            }
        }
        
        // Record negative signals for rejected alternatives
        for rejected in &signal.rejected_alternatives {
            self.record_negative_signal(original_input, rejected, 0.7).await?;
        }
        
        Ok(())
    }
}

/// Generate phrase variants for more robust learning
/// 
/// Addresses the failure case where "list all cbus" was learned
/// but "show me the cbus" wasn't recognized.
fn generate_phrase_variants(phrase: &str) -> Vec<String> {
    let mut variants = vec![phrase.to_string()];
    let lower = phrase.to_lowercase();
    
    // Plural normalization (cbus -> cbu, entities -> entity)
    if lower.contains("cbus") {
        variants.push(phrase.replace("cbus", "cbu").replace("CBUs", "CBU"));
    }
    if lower.contains("entities") {
        variants.push(phrase.replace("entities", "entity"));
    }
    
    // Common verb swaps
    let verb_swaps = [
        ("list", "show"),
        ("show", "list"),
        ("display", "show"),
        ("get", "list"),
        ("view", "show"),
        ("find", "search"),
    ];
    for (from, to) in verb_swaps {
        if lower.contains(from) {
            variants.push(lower.replace(from, to));
        }
    }
    
    // Article/quantifier removal
    let stripped = lower
        .replace(" the ", " ")
        .replace(" all ", " ")
        .replace(" my ", " ");
    if stripped != lower {
        variants.push(stripped);
    }
    
    // Dedupe and return
    variants.sort();
    variants.dedup();
    variants
}

#[derive(Debug, Clone)]
pub struct LearningSignal {
    pub input_text: String,
    pub matched_verb: String,
    pub outcome: Outcome,
    pub confidence: f32,           // 0.0-1.0, SelectedAlt = 0.95
    pub rejected_alternatives: Vec<String>,
    pub source: SignalSource,
}

#[derive(Debug, Clone)]
pub enum SignalSource {
    UserDisambiguation,  // Clicked from options (gold)
    GeneratedVariant,    // Auto-generated phrase variant
    SuccessfulExecution, // Executed without error
    ImplicitAcceptance,  // User continued without correction
    ExplicitCorrection,  // User said "no, I meant X"
}
```

### Step 4: Confidence-Weighted Learning

Update the learning candidate promotion logic:

```rust
// In feedback/learner.rs or promotion.rs

/// Confidence thresholds for auto-promotion to learned phrases
const AUTO_PROMOTE_THRESHOLD: f32 = 0.90;  // SelectedAlt qualifies immediately
const ACCUMULATE_THRESHOLD: f32 = 0.70;    // Need multiple signals

impl Learner {
    pub async fn process_signal(&self, signal: &LearningSignal) -> Result<()> {
        if signal.confidence >= AUTO_PROMOTE_THRESHOLD {
            // Gold-standard: promote immediately
            self.promote_to_learned_phrase(
                &signal.input_text,
                &signal.matched_verb,
                signal.confidence,
            ).await?;
        } else {
            // Accumulate in candidates table
            self.add_learning_candidate(signal).await?;
            
            // Check if accumulated confidence crosses threshold
            self.check_promotion_threshold(&signal.input_text).await?;
        }
        
        Ok(())
    }
}
```

### Step 5: UI Changes (REPL Panel)

```typescript
// In repl_panel.rs or equivalent TypeScript

interface DisambiguationResponse {
  message: string;
  original_input: string;
  options: DisambiguationOption[];
}

interface DisambiguationOption {
  verb_fqn: string;
  display_name: string;
  example: string;
  score: number;
}

// Render clickable buttons
function renderDisambiguation(response: DisambiguationResponse) {
  return (
    <div className="disambiguation">
      <p>{response.message}</p>
      <div className="options">
        {response.options.map(opt => (
          <button
            key={opt.verb_fqn}
            onClick={() => selectVerb(response.original_input, opt.verb_fqn, response.options)}
            className="disambiguation-option"
          >
            <code>{opt.example}</code>
            <span>{opt.display_name}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

async function selectVerb(originalInput: string, selectedVerb: string, options: DisambiguationOption[]) {
  const response = await fetch(`/api/session/${sessionId}/select-verb`, {
    method: 'POST',
    body: JSON.stringify({
      original_input: originalInput,
      selected_verb: selectedVerb,
      candidates: options.map(o => o.verb_fqn),
    }),
  });
  // Handle execution result...
}
```

---

## Files to Create/Modify

| File | Change |
|------|--------|
| `rust/src/api/agent_routes.rs` | Add `/select-verb` endpoint |
| `rust/src/api/agent_service.rs` | Return `DisambiguationResponse` struct |
| `rust/crates/ob-semantic-matcher/src/feedback/service.rs` | Add `record_disambiguation_selection()` with variant generation |
| `rust/crates/ob-semantic-matcher/src/feedback/learner.rs` | Add confidence-weighted promotion |
| `rust/crates/ob-semantic-matcher/src/feedback/variants.rs` | NEW: `generate_phrase_variants()` function |
| `rust/crates/ob-poc-ui/src/panels/repl_panel.rs` | Render clickable disambiguation buttons |

---

## Database: Learning Signal Table

Verify `agent.learning_candidates` has these columns (or add migration):

```sql
-- Check existing schema, add if missing:
ALTER TABLE agent.learning_candidates 
  ADD COLUMN IF NOT EXISTS confidence REAL DEFAULT 0.5,
  ADD COLUMN IF NOT EXISTS source TEXT DEFAULT 'implicit',
  ADD COLUMN IF NOT EXISTS rejected_alternatives TEXT[];
```

---

## Verification Test

After implementation, test the full loop:

```
1. Query: "list all cbus"
   → System returns Ambiguous with 3 options as buttons
   
2. User clicks [cbu.list] button
   → POST /api/session/:id/select-verb called
   → Learning signal recorded with confidence=0.95
   → Verb executes
   
3. Check database:
   SELECT * FROM agent.learning_candidates 
   WHERE input_text = 'list all cbus';
   → Should see entry with matched_verb='cbu.list', confidence=0.95
   
4. Run: cargo run --bin populate_embeddings
   → New phrase should be promoted to verb_pattern_embeddings
   
5. Query again: "list all cbus"
   → Should now match cbu.list unambiguously (no disambiguation)
```

---

## Success Criteria

- [ ] Disambiguation shows clickable buttons, not just text
- [ ] Clicking button calls `/select-verb` endpoint
- [ ] Selection recorded in `learning_candidates` with confidence=0.95
- [ ] Phrase variants generated and stored with confidence=0.85
- [ ] `Outcome::SelectedAlt` finally used
- [ ] After re-embedding, previously ambiguous query matches correctly
- [ ] Variant phrases also match (e.g., "show me the cbus" after learning "list all cbus")
- [ ] Rejected alternatives recorded as negative signals
- [ ] **Abandonment tracked** - when user bails without selecting, record negative signal for ALL candidates

---

## Abandonment Signal (User Bails)

When user sees disambiguation options but abandons (types something else, closes session, navigates away), that's a **strong negative signal** - the options were so bad they gave up.

### Implementation

```rust
// POST /api/session/:id/abandon-disambiguation
pub async fn abandon_disambiguation(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<AbandonDisambiguationRequest>,
) -> Result<Json<AbandonDisambiguationResponse>, ApiError> {
    // Record negative signals for ALL candidates - they were all wrong
    for candidate in &payload.candidates {
        record_negative_signal(
            &state.pool,
            &payload.original_input,
            candidate,
            0.3,  // Lower confidence - user didn't explicitly reject, just bailed
            "user_abandoned",
        ).await?;
    }
    
    // Also record to feedback log for analysis
    record_abandon_event(&state.pool, &payload).await?;
    
    Ok(Json(AbandonDisambiguationResponse { recorded: true }))
}

#[derive(Debug, Deserialize)]
pub struct AbandonDisambiguationRequest {
    pub request_id: String,
    pub original_input: String,
    pub candidates: Vec<String>,
    pub abandon_reason: Option<String>,  // "typed_new_input", "closed_session", "timeout"
}
```

### UI Triggers for Abandon

1. **User types new input** - Instead of clicking an option, they type something else
   → UI calls `/abandon-disambiguation` with previous request, then processes new input

2. **Session close/navigate away** - User leaves without choosing
   → UI calls `/abandon-disambiguation` on `beforeunload` or route change

3. **Timeout** - Options shown for >30 seconds with no interaction
   → UI auto-calls `/abandon-disambiguation` with reason="timeout"

### Why This Matters

Without abandon tracking:
- Bad options persist because we never learn they're wrong
- The same useless suggestions keep appearing
- No signal that the verb search results were off-target

With abandon tracking:
- ALL candidates get downweighted
- System learns "these verbs don't match this phrase"
- Next time, different (hopefully better) candidates surface

---

## Why This Matters

**Current:** 80% accuracy, no learning from corrections  
**After:** Each disambiguation selection improves the model  
**Target:** 90%+ accuracy through accumulated corrections

This is the difference between a demo and a product.
