# Claude Integration Guide for ob-poc

This document explains how the DSL system works with Claude, particularly around verb discovery, intent resolution, and the learning loop.

## Quick Reference

```
User says: "spin up a fund for Acme"
                    ↓
            verb_search tool
                    ↓
    ┌───────────────┴───────────────┐
    │     Search Priority Order      │
    │  1. Learned phrases (DB)       │  ← User taught us this
    │  2. YAML invocation_phrases    │  ← Author defined these
    │  3. pgvector semantic          │  ← Embedding similarity
    └───────────────┬───────────────┘
                    ↓
            Top match: cbu.create
                    ↓
            dsl_generate tool
                    ↓
    LLM extracts args as JSON (NOT DSL)
                    ↓
    Deterministic DSL assembly
                    ↓
            (cbu.create :name "Acme")
```

---

## Verb Discovery System

### How Verbs Become Discoverable

A verb is only discoverable if it has **invocation_phrases** in its YAML definition:

```yaml
# config/verbs/cbu.yaml
domains:
  cbu:
    verbs:
      create:
        description: "Create a new Client Business Unit"
        invocation_phrases:           # ← REQUIRED for discovery
          - "create cbu"
          - "new client business unit"
          - "spin up a fund"
          - "onboard client"
          - "set up cbu"
        args:
          - name: name
            type: String
            required: true
```

**Without `invocation_phrases`**: The verb exists in the registry but `verb_search` cannot find it. The LLM may hallucinate or fail.

### Search Priority (7-Tier System)

| Priority | Source | Confidence | When Used |
|----------|--------|------------|-----------|
| 1 | **User learned exact** | 1.0 | User-specific learned phrases (exact match) |
| 2 | **Global learned exact** | 1.0 | Global learned phrases from corrections |
| 3 | **User learned semantic** | 0.7-0.95 | User-specific pgvector similarity |
| 4 | **Global learned semantic** | 0.6-0.9 | Global pgvector similarity |
| 5 | **Blocklist check** | — | Skip verbs blocked for this phrase pattern |
| 6 | **YAML exact/substring** | 0.7-1.0 | Phrase matches `invocation_phrases` |
| 7 | **Cold start semantic** | 0.5-0.8 | Fallback embedding similarity |

**User-specific learning**: Pass `user_id` to `verb_search` for personalized results.

**Blocklist**: Use `intent_block` to prevent a verb from matching certain phrases.

### The Learning Loop

When `verb_search` returns the wrong verb, use `intent_feedback`:

```
User: "add a signatory to the fund"
Claude: verb_search → entity.create (WRONG)
User: "no, I meant entity.assign-role"

Claude calls intent_feedback:
  feedback_type: "verb_correction"
  original_input: "add a signatory to the fund"
  system_choice: "entity.create"
  correct_choice: "entity.assign-role"

System records this. Next time:
  "add a signatory" → entity.assign-role (score 1.0, source: learned)
```

---

## Adding New Verbs

### Step 1: Define the Verb YAML

```yaml
# config/verbs/trading-profile.yaml
domains:
  trading-profile:
    invocation_hints:              # Domain-level hints
      - "trading"
      - "custody"
      - "settlement"
    verbs:
      add-custody-account:
        description: "Add a custody account to a trading profile"
        invocation_phrases:        # ← CRITICAL: Add these!
          - "add custody account"
          - "set up custody"
          - "link custody"
          - "add custodian"
          - "connect custody account"
        args:
          - name: trading_profile_id
            type: Uuid
            required: true
          - name: custodian
            type: String
            required: true
          - name: account_number
            type: String
            required: true
```

### Step 2: Restart MCP Server

Currently, verb discovery requires a restart:

```bash
# Restart dsl_mcp to pick up new verbs
pkill -f dsl_mcp
DATABASE_URL=postgresql://localhost/ob-poc ./target/debug/dsl_mcp
```

**Future**: `verbs_reload` tool will enable hot reload without restart.

### Step 3: Verify Discovery

```
Claude: verb_search query="add custody account"

Expected response:
{
  "results": [{
    "verb": "trading-profile.add-custody-account",
    "score": 1.0,
    "source": "phrase_exact",
    "matched_phrase": "add custody account"
  }]
}
```

### Step 4: Sync Embeddings (Future)

Once pgvector integration is complete:

```
Claude: verbs_embed_sync domain="trading-profile"

This generates embeddings for semantic fallback matching.
```

---

## Common Pitfalls

### 1. Verb Not Found

**Symptom**: `verb_search` returns empty results or wrong verbs.

**Causes**:
- Missing `invocation_phrases` in YAML
- MCP server not restarted after YAML changes
- Phrase doesn't match any defined patterns

**Fix**:
```yaml
# Add invocation_phrases to the verb
invocation_phrases:
  - "exact phrase users say"
  - "alternative phrasing"
  - "common abbreviation"
```

### 2. Wrong Verb Keeps Winning

**Symptom**: Same wrong verb selected repeatedly despite corrections.

**Cause**: High-scoring YAML phrase match overriding learned data.

**Fix**: Use `intent_feedback` repeatedly. After threshold (3+ occurrences), learned phrase takes priority.

**Future**: `intent_block` tool will explicitly block a verb for a phrase pattern.

### 3. LLM Writes Bad DSL

**Symptom**: `dsl_generate` produces invalid syntax.

**Cause**: NOT a discovery problem — the structured pipeline should prevent this.

**Debug**:
1. Check `verb_search` returned correct verb
2. Check verb signature in registry matches expectations
3. LLM should only extract JSON arguments, never DSL syntax

### 4. Entity Not Resolved

**Symptom**: DSL contains entity name instead of UUID.

**Expected behavior**: `dsl_generate` returns `unresolved_refs` array.

**Fix**: Use `dsl_lookup` to resolve entity names to UUIDs before execution:

```
Claude: dsl_lookup lookup_type="entity" search="Acme Corp"
→ Returns: { entity_id: "uuid-here", name: "Acme Corporation" }

Then update DSL with resolved UUID.
```

---

## Tool Usage Patterns

### Pattern 1: Simple Command

```
User: "create a cbu called Apex Fund"

1. verb_search query="create a cbu"
   → cbu.create (score 1.0)

2. dsl_generate instruction="create a cbu called Apex Fund"
   → (cbu.create :name "Apex Fund")
   → valid: true, unresolved_refs: []

3. dsl_execute source="(cbu.create :name \"Apex Fund\")"
```

### Pattern 2: With Entity Resolution

```
User: "add John Smith as signatory to Apex Fund"

1. verb_search query="add signatory"
   → entity.assign-role (score 0.85)

2. dsl_generate instruction="add John Smith as signatory to Apex Fund"
   → (entity.assign-role :entity "John Smith" :role "signatory" :cbu "Apex Fund")
   → unresolved_refs: [
       { param: "entity", search: "John Smith", type: "person" },
       { param: "cbu", search: "Apex Fund", type: "cbu" }
     ]

3. dsl_lookup lookup_type="person" search="John Smith"
   → { entity_id: "uuid-1", name: "John Smith" }

4. dsl_lookup lookup_type="cbu" search="Apex Fund"
   → { cbu_id: "uuid-2", name: "Apex Fund" }

5. dsl_execute source="(entity.assign-role :entity \"uuid-1\" :role \"signatory\" :cbu \"uuid-2\")"
```

### Pattern 3: Correction Flow

```
User: "set up an ISDA"
Claude: verb_search → isda.create ✓
Claude: dsl_generate → (isda.create ...)
User: "no, I wanted a CSA not ISDA"

Claude: intent_feedback
  feedback_type: "verb_correction"
  original_input: "set up an ISDA"
  correct_choice: "csa.create"

Claude: verb_search query="set up a CSA"
   → csa.create (now learned)
```

---

## MCP Tools Reference

### Core Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `verb_search` | Find verbs matching natural language | Before `dsl_generate` to understand options |
| `dsl_generate` | Convert instruction to DSL | After confirming verb with `verb_search` |
| `dsl_lookup` | Resolve entity names to UUIDs | When `dsl_generate` returns `unresolved_refs` |
| `dsl_execute` | Run DSL against database | After DSL is complete and valid |
| `dsl_validate` | Check DSL syntax without executing | For debugging or preview |
| `verbs_list` | List all available verbs | For exploration/debugging |
| `schema_info` | Get entity types, roles, etc. | When unsure of valid enum values |

### Learning Management Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `intent_feedback` | Record user corrections | When user says "no, I meant X" |
| `intent_block` | Block a verb for a phrase pattern | When a verb keeps winning incorrectly |
| `learning_list` | List learned phrases/aliases | Audit what the system has learned |
| `learning_approve` | Manually approve a learning candidate | Promote pending learning to active |
| `learning_reject` | Reject a learning candidate | Prevent bad learning from activating |
| `learning_import` | Bulk import phrase→verb mappings | Bootstrap from CSV/JSON |
| `learning_stats` | Get learning system statistics | Monitor learning health |

---

## Architecture Notes

### Why Structured Intent Extraction?

The old approach had LLM write DSL directly:
```
User input → LLM → "(cbu.create :name \"Apex\")"  ← LLM invented syntax
```

Problems:
- Inconsistent syntax
- Hallucinated verbs
- Can't learn from corrections (string is opaque)

The new approach separates concerns:
```
User input → verb_search → verb signature → LLM extracts JSON args → deterministic assembly
```

Benefits:
- LLM never sees DSL syntax
- Verb must exist in registry
- Arguments validated against signature
- Corrections create learnable mappings

### Database Schema (Learning)

```
agent.invocation_phrases    ← Learned phrase→verb mappings (+ embedding column)
agent.entity_aliases        ← Learned entity name→canonical (+ embedding column)
agent.learning_candidates   ← Pending learnings awaiting threshold
agent.events               ← Full interaction log for analysis
agent.phrase_blocklist      ← Blocked verb+phrase combinations (+ embedding)
agent.user_learned_phrases  ← User-specific learned phrases (+ embedding, confidence)
```

**pgvector**: Embeddings use `vector(1536)` from OpenAI text-embedding-3-small.
Semantic search uses IVFFlat indexes with cosine distance.

### Hot Path vs Learning Path

```
HOT PATH (sync, <100ms):
  verb_search → in-memory LearnedData → VerbPhraseIndex → return

LEARNING PATH (async, background):
  intent_feedback → INSERT agent.learning_candidates
  (Later) warmup → load into LearnedData
```

---

## Debugging Checklist

When verb discovery isn't working:

- [ ] Does the verb YAML have `invocation_phrases`?
- [ ] Was MCP server restarted after YAML changes?
- [ ] Does `verb_search` return results for similar queries?
- [ ] Is there a learned phrase overriding? (Check `agent.invocation_phrases`)
- [ ] Is the verb marked `internal: true`? (Internal verbs are excluded)

When DSL generation fails:

- [ ] Did `verb_search` return the correct verb?
- [ ] Does the verb exist in RuntimeRegistry?
- [ ] Are required arguments being extracted?
- [ ] Does `dsl_validate` pass on the output?

When execution fails:

- [ ] Are all entity references resolved to UUIDs?
- [ ] Do referenced entities exist in database?
- [ ] Does user have permission for this operation?
- [ ] Check `dsl_plan` output for execution strategy
