# Disambiguation Feedback Loop Gap

**Status**: Gap identified, implementation required  
**Date**: 2026-01-27  
**Priority**: Medium (affects learning loop effectiveness)

## Problem Summary

When verb search returns `Ambiguous`, the agent chat presents multiple verb options to the user. However, when the user selects one of those options, the selection is **not captured** for learning purposes.

The rule should be:
> Ambiguity threshold crossed → always show verb options.  
> User selection = ground truth for learning.

## Current State

### What's Working ✅

1. **Ambiguity Detection** (`verb_search.rs`)
   - `normalize_candidates()` dedupes and ranks results
   - `check_ambiguity_with_margin()` detects close scores
   - `AMBIGUITY_MARGIN = 0.05` triggers disambiguation when top-2 are within 5%

2. **User-Friendly Response** (`agent_service.rs`)
   - When `VerbSearchOutcome::Ambiguous` returned, agent shows verb options
   - Response includes "Did you mean X or Y?" style message

3. **Execution Outcome Capture** (`feedback/service.rs`)
   - `FeedbackService.record_outcome_with_dsl()` captures successful executions
   - `Outcome::Executed` triggers learning signals for strong positive signals

### What's Missing ❌

1. **No Callback on Disambiguation Selection**
   - When user picks from options, there's no endpoint to capture the selection
   - The chat just continues with whatever the user types next

2. **`Outcome::SelectedAlt` Never Triggered**
   - The `Outcome` enum has `SelectedAlt` variant but it's never used
   - This was intended for capturing "user corrected our guess"

3. **No Immediate Learning from User's Pick**
   - Even if user confirms "yes, I meant X", that signal isn't recorded
   - The (original_input, selected_verb) pair never reaches `intent_feedback` or `user_learned_phrases`

## Proposed Fix

### Option A: Minimal - Inline Selection Capture

Add a selection capture to the existing chat flow:

```rust
// POST /api/session/:id/disambiguation-selected
pub struct DisambiguationSelected {
    original_input: String,
    selected_verb: String,
    candidates: Vec<String>, // All options that were shown
}

// Handler triggers:
// 1. FeedbackService.record_outcome_with_dsl(..., Outcome::SelectedAlt)
// 2. Learning signal: (original_input, selected_verb) → learning_candidates table
```

### Option B: Full - Structured Disambiguation UI

Add first-class disambiguation support:

1. Response includes structured `disambiguation_options` field
2. UI renders clickable buttons for each option
3. Button click calls dedicated endpoint with selection
4. Selection triggers learning signal immediately

## Files to Modify

| File | Change |
|------|--------|
| `rust/src/api/agent_routes.rs` | Add `/disambiguation-selected` endpoint |
| `rust/src/api/agent_service.rs` | Wire disambiguation selection to feedback service |
| `rust/crates/ob-semantic-matcher/src/feedback/service.rs` | Handle `SelectedAlt` outcome type |
| `rust/crates/ob-poc-ui/src/panels/repl_panel.rs` | Optional: structured disambiguation UI |

## Verification

After implementation, verify with:

1. Trigger an ambiguous query (e.g., "create something")
2. System shows multiple options
3. User selects one
4. Check `agent.learning_candidates` table for new entry
5. Run `populate_embeddings` and verify new pattern appears
6. Retry original query - should now match unambiguously

## Notes

- The `Outcome::SelectedAlt` enum variant already exists in the feedback module
- The `agent.record_learning_signal()` SQL function is ready to accept this signal type
- This is a relatively small change with high impact on learning loop closure
