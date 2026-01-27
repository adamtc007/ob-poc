# Disambiguation Feedback Loop - Design Rationale

**Status**: Design requirements document  
**Date**: 2026-01-27  
**Related**: `rust/src/mcp/TODO_DISAMBIGUATION_FEEDBACK_LOOP.md` (implementation spec)

## Problem Summary

When verb search returns `Ambiguous`, the agent chat presents multiple verb options to the user. However, when the user selects one of those options, the selection is **not captured** for learning purposes.

The rule should be:
> Ambiguity threshold crossed → always show verb options.  
> User selection = ground truth for learning.

---

## Requirement 1: Ambiguity Must Trigger Concrete Options

When the user's request is ambiguous (e.g. it could match multiple actions/intents with similar confidence), the agent must respond by presenting a list of specific options for clarification. Instead of guessing or asking a vague follow-up, the agent should explicitly ask "Did you mean X, Y, or Z?" with each option corresponding to a concrete verb/intent. This approach defers the decision to the user and ensures the clarification is grounded in the system's known actions.

Most modern conversational systems follow this principle: if the top interpretations are too close in confidence, the system auto-triggers a disambiguation prompt listing the top candidates. By offering numbered verb options (as seen in the log for "list all cbus"), the agent gives the user a clear choice and avoids misunderstanding.

**Why this matters:** Presenting a menu of likely intents prevents the agent from taking the wrong action on an unclear request. It also feels more natural – human assistants, when unsure, will ask "Do you mean ___ or ___?" rather than leaving the clarification entirely open-ended.

**The rule:** Once the ambiguity threshold is crossed (e.g. top two intents within 5% confidence difference), the agent must use the disambiguation template. This was correctly applied when the user said "list all cbus": the agent responded "Did you mean one of these?" followed by three possible actions (like cbu.list, session.list-active-cbus, cbu.search). This is the desired behavior.

**Anti-pattern:** Asking an open-ended clarification question is discouraged when the system has a guess of possible intents. A generic response like "I'm not sure what you mean, could you clarify?" provides no guided help and puts the burden back on the user. Best practices suggest not to ask broad, open-ended questions in ambiguous cases, but rather to use quick replies or suggestions to narrow it down.

**Implementation guard:** The agent response template for ambiguity should always produce a concrete list of verbs/options (never a vague prompt) whenever the ambiguity logic is triggered. If any scenario where an ambiguous input does not result in the option list (and instead falls to a fallback message), that indicates a flaw in the ambiguity detection or template selection.

---

## Requirement 2: User Selection Must Be Captured

When the user selects one of the presented options (or otherwise clarifies which intent they meant), the system needs to capture that feedback pairing: linking the original utterance (the ambiguous phrase) with the user's chosen intent.

**Two purposes:**

1. **Fulfill the current request:** The agent now knows exactly which action to execute. The user picked option 1 for "list all cbus", which corresponded to the cbu.list command. The system notes this as an explicit correction – "User meant cbu.list when they said 'list all cbus'." The agent can then proceed to execute cbu.list.

2. **Learn from this clarification:** The captured pair ("list all cbus" => cbu.list) is valuable data. It should be recorded so that next time, the agent can recognize "list all cbus" as a direct trigger for the cbu.list action without needing to ask again. Many conversational AI systems treat the user's confirmed choice as a ground truth training example. The user has taught the system a new phrase for that intent.

**Critical:** Capturing the user's selection with the original input means storing the mapping of "ambiguous user phrase" → "intended verb" immediately during the conversation. If this step fails or is skipped, the agent treats each occurrence as new and never gets better at understanding that phrase.

---

## Requirement 3: Persist the Learned Pair

After capturing the user's selection in the live session, the next critical step is to persist that information into the system's knowledge base.

### Intent Feedback Log

A running log of user corrections/confirmations. Storing the event here provides a record for developers or automated training pipelines. It's essentially feedback that "User X used phrase Y which was resolved to intent Z (by disambiguation)."

### User Learned Phrases / Invocation Phrases

A more direct way the system improves next-turn accuracy. By adding the phrase to a "learned phrases" list for that intent (either specific to the user or globally), the next time that phrase appears, the search priority will find it immediately as a known exact match.

The engine's search priority places user-learned exact matches at the highest priority (score 1.0). This means once "list all cbus" is saved as a learned invocation for cbu.list, any future request "list all cbus" should hit that exact match and avoid ambiguity altogether. **The system remembers the correction for the future.**

**Critical:** The pairing should be stored with the original phrasing preserved. If the user said "show me the cbus" and selected an intent, that exact phrasing ("show me the cbus") should be saved under the chosen verb's synonyms. The system should not just record a generic event like "user confirmed intent Z" without context of the phrase used – it needs the exact phrase to recognize it next time.

---

## Failure Case Analysis

The user's second phrasing "show me the cbus" did not trigger the learned mapping. The agent produced a generic confusion or "retry" behavior. This implies:

1. The system did not learn from the first confirmation sufficiently, OR
2. The second phrasing was just different enough that it didn't match the learned entry, OR
3. The ambiguity detection didn't fire (perhaps only one candidate had a slightly higher score, leading the agent to incorrectly not offer the list and instead give a vague "I'm not sure" response)

**Root cause:** The system had "list all cbus" learned, but not "show me the cbus." After the second clarification, that phrase should also be learned. Over time, this makes the agent robust to various phrasings.

**Fix:** When storing learned phrases, also generate and store normalized variants:
- Plural normalization: "cbus" → "cbu"
- Common verb swaps: "list" ↔ "show"
- Article removal: "the", "all"

---

## The Complete Feedback Loop

Each of these stages must be present and functioning:

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. AMBIGUITY DETECTION                                          │
│    - Multiple intents close in score (within 5% margin)         │
│    - MUST trigger disambiguation template                       │
│    - NEVER fall back to open-ended "could you clarify?"         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. DISAMBIGUATION PROMPT                                        │
│    - Show concrete verb options as clickable buttons            │
│    - "Did you mean one of these?"                               │
│    - [cbu.list] [session.list-active-cbus] [cbu.search]         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. CAPTURE USER SELECTION                                       │
│    - User clicks button                                         │
│    - Capture: (original_input, selected_verb)                   │
│    - This is GOLD-STANDARD labeled data                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. EXECUTE AND PERSIST                                          │
│    - Execute the selected verb                                  │
│    - Write to intent_feedback log                               │
│    - Write to user_learned_phrases with variants                │
│    - Record negative signals for rejected alternatives          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. NEXT QUERY MATCHES INSTANTLY                                 │
│    - Same phrase → score 1.0 (LearnedExact)                     │
│    - Variant phrases → also match                               │
│    - No disambiguation needed                                   │
└─────────────────────────────────────────────────────────────────┘
```

If any link is missing (e.g., if the agent ever falls back to an unguided clarification, or if it fails to record the learning), the user experience degrades and the system remains "stuck" on the same misunderstandings.

---

## Implementation Checklist

- [ ] Adjust ambiguity detection so phrases like "show me the cbus" correctly trigger disambiguation rather than vague fallback (may need threshold tuning or input normalization)
- [ ] Ensure disambiguation response template (the "Did you mean one of these?" list) is used 100% of the time when ambiguity is detected
- [ ] When user picks an option, code paths for logging and learning must execute
- [ ] Original utterance and chosen intent must be saved to learning store
- [ ] Generate and store phrase variants (plural, verb swaps, article removal)
- [ ] Record negative signals for rejected alternatives

---

## Sources

- Cobus Greyling, "Your Chatbot Should Be Able To Disambiguate", on handling ambiguous inputs and using user feedback for training
- Sachin K. Singh, "Handling Ambiguous User Inputs in Kore.ai", noting that ambiguity should trigger a clarification prompt with options, and advising not to use open-ended questions
- Internal Audit Report (Semantic Intent Engine) – configuration details showing the ambiguity threshold (5% margin) and the existence of user learned phrase handling in the search priority
