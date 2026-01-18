# Claude Integration Guide for ob-poc

This document explains how the DSL system works with Claude, particularly around verb discovery, intent resolution, and the learning loop.

---

## 📋 Key Architecture Decisions

> **📄 Full Documentation:**
> - [`/docs/ARCH-DECISION-CANDLE-EMBEDDINGS.md`](docs/ARCH-DECISION-CANDLE-EMBEDDINGS.md) — Enterprise architecture review (for ARB)
> - [`/docs/VECTOR-DATABASE-PORTABILITY.md`](docs/VECTOR-DATABASE-PORTABILITY.md) — PostgreSQL vs Oracle analysis

### Local ML Inference for Semantic Search

This system uses **local ML inference** rather than external APIs:

| Component | Choice | Rationale |
|-----------|--------|----------|
| **Framework** | [HuggingFace Candle](https://github.com/huggingface/candle) | Pure Rust, no Python, $4.5B company backing |
| **Model** | [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) | 142M downloads/month, Apache 2.0 |
| **Storage** | pgvector (PostgreSQL) | IVFFlat indexes, cosine similarity |

**Why this matters:**
- ✅ **No external API calls** — data never leaves BNY infrastructure
- ✅ **10-20x faster** — 5-15ms vs 100-300ms (OpenAI API)
- ✅ **$0 marginal cost** — no per-embedding charges
- ✅ **Air-gap capable** — works in isolated networks
- ✅ **No Python runtime** — static Rust binary

**Enterprise validation:** HuggingFace backed by Google, Amazon, Nvidia, Intel, IBM, Salesforce ($4.5B valuation, $396M funding).

### Database Portability

**PostgreSQL coupling is LOW.** Oracle 23ai AI Vector Search uses the **same `<=>` operator** for cosine distance. Migration effort: ~2-3 days. See [VECTOR-DATABASE-PORTABILITY.md](docs/VECTOR-DATABASE-PORTABILITY.md).

| Feature | pgvector | Oracle 23ai | Compatible? |
|---------|----------|-------------|-------------|
| Cosine operator | `<=>` | `<=>` | ✅ **Identical** |
| Vector type | `vector(384)` | `VECTOR(384)` | ✅ Minor syntax |
| Index DDL | `ivfflat` | `NEIGHBOR PARTITIONS` | ⚠️ Different |

---

## ⚠️ Active Migration: Candle Embeddings

**Status**: Migration planned — see [`/docs/TODO-CANDLE-PIPELINE-CONSOLIDATION.md`](docs/TODO-CANDLE-PIPELINE-CONSOLIDATION.md)

The system is migrating from OpenAI embeddings to local Candle embeddings:

| | Before | After |
|---|--------|-------|
| **Embedder** | OpenAI API | Candle (local) |
| **Model** | text-embedding-3-small | all-MiniLM-L6-v2 |
| **Dimensions** | 1536 | 384 |
| **Latency** | 100-300ms | 5-15ms |
| **Cost** | $0.00002/embed | $0 |
| **API Key** | Required | Not needed |

**Impact**: After migration, `verb_search` semantic matching will be ~10x faster with no external dependencies.

---

## Quick Reference

```
User says: "spin up a fund for Acme"
                    ↓
            verb_search tool
                    ↓
    ┌───────────────┴───────────────┐
    │     Search Priority Order      │
    │  1. Learned phrases (exact)    │  ← User taught us this
    │  2. YAML invocation_phrases    │  ← Author defined these  
    │  3. Semantic similarity        │  ← Candle embeddings + pgvector
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

### The Golden Rule

**ALL DSL generation MUST go through this pipeline.** No side doors, no bypass paths.

```
┌─────────────────────────────────────────────────────────────┐
│                    UNIFIED DSL PIPELINE                      │
│                                                              │
│   verb_search ──► dsl_generate ──► dsl_execute              │
│       │               │                                      │
│       ▼               ▼                                      │
│   Candle embed    LLM extracts JSON only                    │
│   (384-dim)       Deterministic assembly                    │
│                                                              │
│   ❌ No direct DSL construction by LLM                      │
│   ❌ No IntentExtractor (legacy - removed)                  │
│   ❌ No FeedbackLoop.generate_valid_dsl() (legacy)          │
└─────────────────────────────────────────────────────────────┘
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

| Priority | Source | Confidence | Latency |
|----------|--------|------------|---------|
| 1 | **User learned exact** | 1.0 | <1ms |
| 2 | **Global learned exact** | 1.0 | <1ms |
| 3 | **User learned semantic** | 0.7-0.95 | 10-20ms |
| 4 | **Global learned semantic** | 0.6-0.9 | 10-20ms |
| 5 | **Blocklist check** | — | 5-10ms |
| 6 | **YAML exact/substring** | 0.7-1.0 | <1ms |
| 7 | **Cold start semantic** | 0.5-0.8 | 10-20ms |

**Fast path**: Tiers 1, 2, 6 are in-memory — sub-millisecond.
**Semantic path**: Tiers 3, 4, 5, 7 use Candle embeddings + pgvector — 10-20ms total.

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

### Step 4: Sync Embeddings

After adding verbs with `invocation_phrases`:

```
Claude: verbs_embed_sync domain="trading-profile"

This generates Candle embeddings (384-dim) for semantic matching.
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

**Alternative**: `intent_block` tool explicitly blocks a verb for a phrase pattern.

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

### 5. Semantic Search Not Working

**Symptom**: Only exact phrase matches work, similar phrases don't match.

**Causes**:
- Embedder not loaded (check startup logs)
- Embeddings not generated for phrases
- Embedding dimension mismatch (must be 384)

**Debug**:
```bash
# Check embedder loaded
grep "Candle" /var/log/dsl_mcp.log

# Check embeddings exist
psql -c "SELECT COUNT(*) FROM agent.invocation_phrases WHERE embedding IS NOT NULL"
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

### Core DSL Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `verb_search` | Find verbs matching natural language | **Always first** — before `dsl_generate` |
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

### Verb Lifecycle Tools (Future)

| Tool | Purpose | Status |
|------|---------|--------|
| `verbs_reload` | Hot reload VerbPhraseIndex | Planned |
| `verbs_coverage` | Report verbs missing invocation_phrases | Planned |
| `verbs_embed_sync` | Generate embeddings for phrases | Planned |

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

### Embedding System

**Model**: `all-MiniLM-L6-v2` (via Candle, local inference)
**Dimensions**: 384
**Index**: IVFFlat with cosine distance
**Latency**: 5-15ms per embedding

The embedder is loaded once at startup and cached. First startup downloads the model (~22MB) from HuggingFace to `~/.cache/huggingface/`.

### Database Schema (Learning)

```sql
agent.invocation_phrases    -- Learned phrase→verb mappings
  - phrase TEXT
  - verb TEXT  
  - embedding vector(384)   -- Candle embedding
  - embedding_model TEXT    -- 'all-MiniLM-L6-v2'

agent.entity_aliases        -- Learned entity name→canonical
  - alias TEXT
  - canonical_name TEXT
  - embedding vector(384)

agent.user_learned_phrases  -- User-specific learned phrases
  - user_id UUID
  - phrase TEXT
  - verb TEXT
  - confidence REAL
  - embedding vector(384)

agent.phrase_blocklist      -- Blocked verb+phrase combinations
  - phrase TEXT
  - blocked_verb TEXT
  - embedding vector(384)

agent.learning_candidates   -- Pending learnings awaiting threshold
agent.events               -- Full interaction log for analysis
```

### Hot Path vs Learning Path

```
HOT PATH (sync, <50ms total):
  verb_search 
    → in-memory LearnedData (exact match)
    → VerbPhraseIndex (YAML phrases)
    → Candle embed + pgvector (semantic)
    → return results

LEARNING PATH (async, background):
  intent_feedback 
    → INSERT agent.learning_candidates
    → (threshold reached) → promote to agent.invocation_phrases
    → (next warmup) → load into LearnedData
```

---

## Startup Sequence

```
dsl_mcp startup:
  1. Connect to database
  2. Load Candle embedder (all-MiniLM-L6-v2)     ← ~1-3s first time
  3. LearningWarmup loads from DB:
     - agent.invocation_phrases → LearnedData
     - agent.entity_aliases → LearnedData
  4. VerbPhraseIndex.load_from_verbs_dir()      ← Scans YAML files
  5. McpServer starts with learned_data + embedder
  6. Ready for requests
```

**No API keys required.** Semantic search works out of the box.

---

## Debugging Checklist

### Verb discovery not working:

- [ ] Does the verb YAML have `invocation_phrases`?
- [ ] Was MCP server restarted after YAML changes?
- [ ] Does `verb_search` return results for similar queries?
- [ ] Is there a learned phrase overriding? (Check `agent.invocation_phrases`)
- [ ] Is the verb marked `internal: true`? (Internal verbs are excluded)

### DSL generation fails:

- [ ] Did `verb_search` return the correct verb?
- [ ] Does the verb exist in RuntimeRegistry?
- [ ] Are required arguments being extracted?
- [ ] Does `dsl_validate` pass on the output?

### Execution fails:

- [ ] Are all entity references resolved to UUIDs?
- [ ] Do referenced entities exist in database?
- [ ] Does user have permission for this operation?
- [ ] Check `dsl_plan` output for execution strategy

### Semantic search not working:

- [ ] Check startup logs for "Candle embedder loaded"
- [ ] Verify embeddings exist: `SELECT COUNT(*) FROM agent.invocation_phrases WHERE embedding IS NOT NULL`
- [ ] Check embedding dimension is 384
- [ ] Try `verbs_embed_sync` to regenerate embeddings

---

## Related Documentation

| Document | Purpose |
|----------|---------|
| `/docs/VERB-AUTHORING-GUIDE.md` | How to write verb YAML files |
| `/docs/TODO-CANDLE-PIPELINE-CONSOLIDATION.md` | Migration plan (Candle + cleanup) |
| `/docs/TODO-LEARNING-ENHANCEMENTS-PGVECTOR.md` | Future learning system enhancements |
| `/docs/PERFORMANCE-ANALYSIS-VERB-SEARCH.md` | Performance analysis and optimizations |
| `/docs/CANDLE-EMBEDDER-GUIDE.md` | Deep dive on Candle embeddings |
