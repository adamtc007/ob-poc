# TODO: Client Entity Context — Human-to-Agent Semantic Bridge

> **Status:** ✅ COMPLETE — Core scope resolution implemented  
> **Date:** 2026-01-25  
> **Depends on:** migrations/047_client_group_tables.sql, migrations/048_client_group_seed.sql  
> **Reviewed by:** ChatGPT (2026-01-25)
> **Implemented:** 2026-01-25

## Implementation Summary

The core scope resolution system is complete with the following components:

### Stage 0 Hard Gate (IntentPipeline)
- Scope phrases ("allianz", "work on blackrock") are detected BEFORE Candle verb discovery
- `ScopeResolutionOutcome` enum provides deterministic UX contract:
  - `Resolved` → show "Client: X" chip, set session context
  - `Candidates` → show compact picker (not entity-search modal)
  - `Unresolved` → proceed silently to verb discovery
  - `NotScopePhrase` → continue to Candle

### Key Files Created/Modified
- `rust/src/mcp/scope_resolution.rs` — ScopeResolver, ScopeContext, search functions
- `rust/src/mcp/intent_pipeline.rs` — Stage 0 integration, scope context propagation
- `rust/tests/scope_resolution_integration.rs` — Full integration test suite

### Integration Tests Passing
- `test_scope_resolution_hard_gate` ✅ — "allianz" resolves without verb search
- `test_no_entity_search_modal` ✅ — No entity refs returned for client names
- `test_command_bypasses_scope_resolution` ✅ — Commands with verbs go to Candle
- `test_work_on_phrase` ✅ — "work on allianz" resolves correctly
- `test_scope_context_propagation` ✅ — Context preserved across commands
- `test_flywheel_records_selection` ✅ — User confirmations recorded

---

---

## Executive Summary

**The Problem:** Agents can't understand human shorthand.

```
Human says:  "Show me the Irish funds"
             "What's happening with the main ManCo"
             "Load the SICAV universe"

Agent needs: [entity_id_1, entity_id_2, entity_id_3]
```

**The Solution:** A Candle-assisted fuzzy search layer that maps informal human labels to formal entity references.

This is NOT:
- Formal entity taxonomy
- Legal ownership hierarchy  
- UBO analysis

This IS:
- How humans explain to agents what they're talking about
- A learned vocabulary of "what humans call things"
- Pre-seeded during discovery, enriched through interaction
- Persona-aware (KYC analyst vs trader use different shorthand)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 1: Client (Top-Level Brand)                                          │
│                                                                             │
│  Allianz | Aviva | BlackRock | Goldman Sachs | PIMCO                       │
│                                                                             │
│  "I'm working on Allianz" → sets client context                            │
│  (Already exists: client_group + client_group_alias)                       │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 2: Entity Membership (NEW)                                           │
│                                                                             │
│  "These 50+ entities belong to Allianz universe"                           │
│                                                                             │
│  Populated during discovery/onboarding:                                    │
│  - "Load all entities we think are Allianz Ireland"                        │
│  - membership_type: confirmed | suspected | historical                     │
│                                                                             │
│  This is the ASSEMBLY phase before formal analysis                         │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 3: Shorthand Tags (NEW) — THE KEY INNOVATION                         │
│                                                                             │
│  Human-think labels, NOT formal taxonomy:                                  │
│                                                                             │
│  Entity: "Allianz Global Investors GmbH" (LEI: 529900...)                  │
│                                                                             │
│  ┌─────────────┬─────────────────────┬────────────────────────┐            │
│  │ Persona     │ Tag                 │ Human Says             │            │
│  ├─────────────┼─────────────────────┼────────────────────────┤            │
│  │ kyc         │ main lux manco      │ "the ManCo for KYC"    │            │
│  │ trading     │ trading manco       │ "trading views ManCo"  │            │
│  │ (universal) │ agi manco           │ "the AGI ManCo"        │            │
│  │ (universal) │ lux management co   │ "Lux management"       │            │
│  └─────────────┴─────────────────────┴────────────────────────┘            │
│                                                                             │
│  Same entity_id, different discovery paths                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 4: Candle Semantic Search (NEW)                                      │
│                                                                             │
│  User: "Show me the Irish funds"                                           │
│         │                                                                   │
│         ▼                                                                   │
│  Candle: embed("irish funds") → search tag embeddings                      │
│         │                                                                   │
│         ▼                                                                   │
│  Returns: entities tagged with ~"irish fund" in current client context     │
│         │                                                                   │
│         ▼                                                                   │
│  DSL: entity.list entity-ids=[uuid1, uuid2, uuid3]                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## What Already Exists (047-048)

| Table | Purpose | Status |
|-------|---------|--------|
| `client_group` | Top-level brand (Allianz, Aviva) | ✅ Done |
| `client_group_alias` | Nicknames for the GROUP ("AGI", "Allianz GI") | ✅ Done |
| `client_group_alias_embedding` | Candle search for group aliases | ✅ Done |
| `client_group_anchor` | 2-3 KEY entities by role (ManCo, UBO apex) | ✅ Done |
| `client_group_anchor_role` | Role definitions | ✅ Done |

**Gap:** These only handle "resolve Allianz to its ManCo". They cannot:
- Track ALL entities belonging to Allianz (50+)
- Store informal shorthand tags per entity
- Support Candle search for "the Irish funds"

---

## What Needs to Be Added

### Schema Overview

```
client_group (existing)
    │
    ├── client_group_alias (existing) ──── GROUP-level nicknames
    │
    ├── client_group_anchor (existing) ─── KEY entities by formal role
    │
    └── client_group_entity (NEW) ──────── ALL entities in this client universe
            │
            └── client_group_entity_tag (NEW) ─── Shorthand tags per entity
                    │
                    └── client_group_entity_tag_embedding (NEW) ─── Candle search
```

---

## Phase 1: Database Schema

### 1.1 Migration: Entity Membership + Shorthand Tags

**File: `migrations/052_client_group_entity_context.sql`**

```sql
-- Migration 052: Client Group Entity Context
-- Human-to-agent semantic bridge for entity resolution
--
-- Purpose: Enable Candle-assisted fuzzy search from human shorthand to entity_ids
-- 
-- Layers:
-- 1. client_group_entity: Which entities BELONG to this client universe
-- 2. client_group_entity_tag: Informal shorthand tags (persona-scoped)
-- 3. client_group_entity_tag_embedding: Candle-searchable vectors
--
-- Prerequisites:
-- - pgcrypto extension enabled (gen_random_uuid) — see migration 001
-- - pgvector extension enabled — see migration 037

BEGIN;

-- ============================================================================
-- Entity Membership: "These entities belong to this client"
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_entity (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    
    -- Membership classification
    membership_type TEXT NOT NULL DEFAULT 'confirmed',  
        -- 'confirmed': verified as belonging to client
        -- 'suspected': discovered but unconfirmed (onboarding)
        -- 'historical': formerly belonged, now inactive
    
    -- Provenance
    added_by TEXT NOT NULL DEFAULT 'manual',
        -- 'manual': human added directly
        -- 'discovery': onboarding discovery process
        -- 'gleif': traced via GLEIF relationship
        -- 'ownership_trace': UBO/ownership analysis
        -- 'user_confirmed': human confirmed suspected
    
    -- Metadata
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(group_id, entity_id)
);

CREATE INDEX idx_cge_group ON "ob-poc".client_group_entity(group_id);
CREATE INDEX idx_cge_entity ON "ob-poc".client_group_entity(entity_id);
CREATE INDEX idx_cge_membership ON "ob-poc".client_group_entity(group_id, membership_type);

COMMENT ON TABLE "ob-poc".client_group_entity IS 
    'Entity membership in client groups. Tracks which entities belong to which client universe.';

-- ============================================================================
-- Shorthand Tags: "What humans call this entity"
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_entity_tag (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    
    -- The tag itself
    tag TEXT NOT NULL,              -- "main lux manco", "irish fund", "sicav"
    tag_norm TEXT NOT NULL,         -- normalized: lowercase, trimmed, collapsed spaces
    
    -- Persona scoping (same entity, different labels for different users)
    persona TEXT,                   -- NULL = universal, or: 'kyc' | 'trading' | 'ops' | 'onboarding'
    
    -- Provenance
    source TEXT NOT NULL DEFAULT 'manual',
        -- 'manual': human entered
        -- 'user_confirmed': human confirmed during interaction ("yes, I call it that")
        -- 'inferred': system guessed from usage patterns
        -- 'bootstrap': initial seed data
    
    confidence FLOAT DEFAULT 1.0,   -- 0.0-1.0, lower for inferred
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT,                -- user/session that created
    
    -- NOTE: Uniqueness enforced via index below (can't use COALESCE in table constraint)
);

-- Uniqueness: same tag can't exist twice for same entity+group+persona
-- Must use index because Postgres table constraints can't include expressions like COALESCE
CREATE UNIQUE INDEX uq_cget_tag 
    ON "ob-poc".client_group_entity_tag(group_id, entity_id, tag_norm, COALESCE(persona, ''));

-- Fast lookups
CREATE INDEX idx_cget_group_entity ON "ob-poc".client_group_entity_tag(group_id, entity_id);
CREATE INDEX idx_cget_tag_norm ON "ob-poc".client_group_entity_tag(tag_norm);
CREATE INDEX idx_cget_persona ON "ob-poc".client_group_entity_tag(group_id, persona) WHERE persona IS NOT NULL;

-- Trigram index for fuzzy text search
CREATE INDEX idx_cget_tag_norm_trgm ON "ob-poc".client_group_entity_tag 
    USING gin (tag_norm gin_trgm_ops);

COMMENT ON TABLE "ob-poc".client_group_entity_tag IS 
    'Informal shorthand tags for entities within a client context. Persona-scoped, human-think labels.';

COMMENT ON COLUMN "ob-poc".client_group_entity_tag.persona IS
    'NULL = universal tag. Otherwise scoped to persona: kyc, trading, ops, onboarding, etc.';

-- ============================================================================
-- Tag Embeddings: Candle-searchable vectors
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_entity_tag_embedding (
    tag_id UUID NOT NULL REFERENCES "ob-poc".client_group_entity_tag(id) ON DELETE CASCADE,
    embedder_id TEXT NOT NULL,           -- e.g., 'bge-small-en-v1.5'
    pooling TEXT NOT NULL,               -- e.g., 'cls', 'mean'
    normalize BOOLEAN NOT NULL,          -- should always be true for BGE
    dimension INT NOT NULL,              -- e.g., 384
    embedding vector(384) NOT NULL,      -- L2-normalized vector
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (tag_id, embedder_id)
);

-- ANN INDEX: DEFERRED until sufficient data volume
-- At low row counts (<2k tags), exact vector scan is fast enough.
-- Enable this when tag count exceeds ~5k rows:
--
-- CREATE INDEX idx_cgete_embedding ON "ob-poc".client_group_entity_tag_embedding
--     USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
--
-- Or use HNSW (if pgvector >= 0.5.0) for better performance without tuning:
-- CREATE INDEX idx_cgete_embedding ON "ob-poc".client_group_entity_tag_embedding
--     USING hnsw (embedding vector_cosine_ops);

COMMENT ON TABLE "ob-poc".client_group_entity_tag_embedding IS
    'Embeddings for shorthand tags. Enables Candle semantic search: "irish funds" → entity_ids';

-- ============================================================================
-- Helper View: Full tag context for search
-- ============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_client_entity_tags AS
SELECT
    cget.id AS tag_id,
    cget.tag,
    cget.tag_norm,
    cget.persona,
    cget.confidence,
    cget.source,
    cget.group_id,
    cg.canonical_name AS group_name,
    cget.entity_id,
    e.name AS entity_name,
    e.entity_type_id,
    cge.membership_type
FROM "ob-poc".client_group_entity_tag cget
JOIN "ob-poc".client_group cg ON cg.id = cget.group_id
JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
LEFT JOIN "ob-poc".client_group_entity cge 
    ON cge.group_id = cget.group_id AND cge.entity_id = cget.entity_id;

-- ============================================================================
-- Function: Search tags by text (exact + fuzzy + embedding)
-- Returns entities matching the human shorthand
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".search_entity_tags(
    p_group_id UUID,
    p_query TEXT,
    p_persona TEXT DEFAULT NULL,
    p_limit INT DEFAULT 10,
    p_include_historical BOOLEAN DEFAULT FALSE  -- exclude historical by default
) RETURNS TABLE (
    entity_id UUID,
    entity_name TEXT,
    tag TEXT,
    confidence FLOAT,
    match_type TEXT
) AS $
DECLARE
    v_query_norm TEXT;
BEGIN
    -- Normalize query
    v_query_norm := lower(trim(regexp_replace(p_query, '\s+', ' ', 'g')));
    
    RETURN QUERY
    WITH matches AS (
        -- Exact match (highest priority)
        SELECT 
            cget.entity_id,
            e.name AS entity_name,
            cget.tag,
            cget.confidence,
            'exact'::TEXT AS match_type,
            1 AS priority
        FROM "ob-poc".client_group_entity_tag cget
        -- MEMBERSHIP GATE: only return entities that are members of the group
        JOIN "ob-poc".client_group_entity cge 
            ON cge.group_id = cget.group_id AND cge.entity_id = cget.entity_id
        JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
        WHERE cget.group_id = p_group_id
          AND cget.tag_norm = v_query_norm
          AND (p_persona IS NULL OR cget.persona IS NULL OR cget.persona = p_persona)
          -- Exclude historical unless explicitly requested
          AND (p_include_historical OR cge.membership_type != 'historical')
        
        UNION ALL
        
        -- Trigram fuzzy match
        SELECT 
            cget.entity_id,
            e.name AS entity_name,
            cget.tag,
            (similarity(cget.tag_norm, v_query_norm) * cget.confidence)::FLOAT,
            'fuzzy'::TEXT AS match_type,
            2 AS priority
        FROM "ob-poc".client_group_entity_tag cget
        -- MEMBERSHIP GATE: only return entities that are members of the group
        JOIN "ob-poc".client_group_entity cge 
            ON cge.group_id = cget.group_id AND cge.entity_id = cget.entity_id
        JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
        WHERE cget.group_id = p_group_id
          AND cget.tag_norm % v_query_norm
          AND cget.tag_norm != v_query_norm  -- exclude exact matches
          AND (p_persona IS NULL OR cget.persona IS NULL OR cget.persona = p_persona)
          -- Exclude historical unless explicitly requested
          AND (p_include_historical OR cge.membership_type != 'historical')
    )
    SELECT DISTINCT ON (m.entity_id)
        m.entity_id,
        m.entity_name,
        m.tag,
        m.confidence,
        m.match_type
    FROM matches m
    ORDER BY m.entity_id, m.priority, m.confidence DESC
    LIMIT p_limit;
END;
$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".search_entity_tags IS
    'Search shorthand tags to resolve human language to entity_ids. Used by Candle intent pipeline.';

-- ============================================================================
-- Function: Search tags by embedding (semantic similarity)
-- NOTE: vector(384) is intentionally fixed — this system uses BGE-small-en-v1.5.
-- Changing embedding dimension requires schema + function migration.
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".search_entity_tags_semantic(
    p_group_id UUID,
    p_query_embedding vector(384),  -- 384-dim fixed (BGE-small-en-v1.5)
    p_persona TEXT DEFAULT NULL,
    p_limit INT DEFAULT 10,
    p_min_similarity FLOAT DEFAULT 0.5,
    p_include_historical BOOLEAN DEFAULT FALSE,
    p_embedder_id TEXT DEFAULT 'bge-small-en-v1.5'  -- must match embedding model
) RETURNS TABLE (
    entity_id UUID,
    entity_name TEXT,
    tag TEXT,
    similarity FLOAT,
    match_type TEXT
) AS $
BEGIN
    RETURN QUERY
    SELECT DISTINCT ON (cget.entity_id)
        cget.entity_id,
        e.name AS entity_name,
        cget.tag,
        (1.0 - (cgete.embedding <=> p_query_embedding))::FLOAT AS similarity,
        'semantic'::TEXT AS match_type
    FROM "ob-poc".client_group_entity_tag_embedding cgete
    JOIN "ob-poc".client_group_entity_tag cget ON cget.id = cgete.tag_id
    -- MEMBERSHIP GATE: only return entities that are members of the group
    JOIN "ob-poc".client_group_entity cge 
        ON cge.group_id = cget.group_id AND cge.entity_id = cget.entity_id
    JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
    WHERE cget.group_id = p_group_id
      -- Filter by embedder to avoid dimension/model mismatches
      AND cgete.embedder_id = p_embedder_id
      AND (p_persona IS NULL OR cget.persona IS NULL OR cget.persona = p_persona)
      AND (1.0 - (cgete.embedding <=> p_query_embedding)) >= p_min_similarity
      -- Exclude historical unless explicitly requested
      AND (p_include_historical OR cge.membership_type != 'historical')
    ORDER BY cget.entity_id, (cgete.embedding <=> p_query_embedding)
    LIMIT p_limit;
END;
$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".search_entity_tags_semantic IS
    'Semantic search using Candle embeddings. Fallback when text search returns nothing.';

COMMIT;
```

### 1.2 Seed Data

**File: `migrations/053_client_group_entity_seed.sql`**

```sql
-- Seed entity membership and shorthand tags for existing client groups
-- Based on Allianz entities already in database

BEGIN;

-- ============================================================================
-- Allianz Entity Membership
-- ============================================================================

-- Add known Allianz entities to group membership
INSERT INTO "ob-poc".client_group_entity (group_id, entity_id, membership_type, added_by, notes)
SELECT 
    '11111111-1111-1111-1111-111111111111'::UUID,  -- Allianz group
    e.entity_id,
    'confirmed',
    'bootstrap',
    'Initial seed from entity data'
FROM "ob-poc".entities e
WHERE e.name ILIKE '%allianz%'
   OR e.name ILIKE '%agi %'
   OR e.entity_id IN (
       -- Known Allianz entities from anchor table
       SELECT anchor_entity_id FROM "ob-poc".client_group_anchor 
       WHERE group_id = '11111111-1111-1111-1111-111111111111'
   )
ON CONFLICT (group_id, entity_id) DO NOTHING;

-- ============================================================================
-- Allianz Shorthand Tags
-- ============================================================================

-- Tag the main ManCo
INSERT INTO "ob-poc".client_group_entity_tag (group_id, entity_id, tag, tag_norm, persona, source, confidence)
SELECT 
    '11111111-1111-1111-1111-111111111111',
    '084d316f-fa4e-42f0-ac39-1b01a3fbdf27',  -- AGI Holdings GmbH
    tag,
    lower(trim(tag)),
    persona,
    'bootstrap',
    1.0
FROM (VALUES
    ('main manco', NULL),
    ('main lux manco', NULL),
    ('agi manco', NULL),
    ('the manco', NULL),
    ('management company', NULL),
    ('manco for kyc', 'kyc'),
    ('kyc manco', 'kyc'),
    ('trading manco', 'trading'),
    ('book manco', 'trading')
) AS t(tag, persona)
ON CONFLICT DO NOTHING;

-- Tag the ultimate parent
INSERT INTO "ob-poc".client_group_entity_tag (group_id, entity_id, tag, tag_norm, persona, source, confidence)
SELECT 
    '11111111-1111-1111-1111-111111111111',
    '7b6942b5-10e9-425f-b8c9-5a674a7d0701',  -- Allianz SE
    tag,
    lower(trim(tag)),
    NULL,
    'bootstrap',
    1.0
FROM (VALUES
    ('ultimate parent'),
    ('group parent'),
    ('allianz se'),
    ('the parent'),
    ('ubo apex'),
    ('top of house')
) AS t(tag)
ON CONFLICT DO NOTHING;

-- ============================================================================
-- Aviva Entity Membership (partial)
-- ============================================================================

INSERT INTO "ob-poc".client_group_entity (group_id, entity_id, membership_type, added_by, notes)
SELECT 
    '22222222-2222-2222-2222-222222222222'::UUID,  -- Aviva group
    e.entity_id,
    'confirmed',
    'bootstrap',
    'Initial seed from entity data'
FROM "ob-poc".entities e
WHERE e.name ILIKE '%aviva%'
ON CONFLICT (group_id, entity_id) DO NOTHING;

-- Aviva shorthand tags
INSERT INTO "ob-poc".client_group_entity_tag (group_id, entity_id, tag, tag_norm, persona, source, confidence)
SELECT 
    '22222222-2222-2222-2222-222222222222',
    cga.anchor_entity_id,
    tag,
    lower(trim(tag)),
    NULL,
    'bootstrap',
    1.0
FROM "ob-poc".client_group_anchor cga
CROSS JOIN (VALUES
    ('main manco'),
    ('aviva manco'),
    ('the manco')
) AS t(tag)
WHERE cga.group_id = '22222222-2222-2222-2222-222222222222'
  AND cga.anchor_role = 'governance_controller'
ON CONFLICT DO NOTHING;

-- ============================================================================
-- Verify seed
-- ============================================================================
DO $$
DECLARE
    entity_count INT;
    tag_count INT;
BEGIN
    SELECT COUNT(*) INTO entity_count FROM "ob-poc".client_group_entity;
    SELECT COUNT(*) INTO tag_count FROM "ob-poc".client_group_entity_tag;
    
    RAISE NOTICE 'Entity membership seeded: % entities, % tags', entity_count, tag_count;
END $$;

COMMIT;
```

### Acceptance Criteria (Phase 1)
- [ ] All tables created with proper constraints
- [ ] Indexes support text search (trigram) and embedding search (IVFFlat)
- [ ] Search functions work: exact → fuzzy → semantic fallback
- [ ] Seed data populates Allianz/Aviva memberships and tags
- [ ] Persona scoping works (same entity, different tags for KYC vs trading)

---

## Phase 2: CRUD Verbs

### 2.1 YAML Verb Definitions

**File: `rust/config/verbs/client-group.yaml`** (additions to existing)

```yaml
  # ==========================================================================
  # ENTITY MEMBERSHIP
  # ==========================================================================

  - verb: entity-add
    description: "Add an entity to this client group"
    behavior: plugin
    handler: ClientGroupEntityAddOp
    metadata:
      tier: intent
      source_of_truth: client_group_entity
      writes_operational: true
      internal: true
      noun: client_group_entity
      tags: [membership, crud]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
        lookup:
          table: entities
          schema: ob-poc
          search_key: name
          primary_key: entity_id
      - name: membership-type
        type: string
        required: false
        default: "confirmed"
        enum: ["confirmed", "suspected", "historical"]
      - name: notes
        type: string
        required: false
    returns:
      type: uuid

  - verb: entity-remove
    description: "Remove an entity from this client group"
    behavior: plugin
    handler: ClientGroupEntityRemoveOp
    metadata:
      tier: intent
      source_of_truth: client_group_entity
      writes_operational: true
      internal: true
      noun: client_group_entity
      tags: [membership, crud]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
    returns:
      type: boolean

  - verb: entity-list
    description: "List all entities in this client group"
    behavior: plugin
    handler: ClientGroupEntityListOp
    metadata:
      tier: diagnostics
      source_of_truth: client_group_entity
      internal: false
      noun: client_group_entity
      tags: [membership, read]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: membership-type
        type: string
        required: false
        description: "Filter by membership type"
      - name: limit
        type: integer
        required: false
        default: 100
    returns:
      type: record_set
      fields: [entity_id, entity_name, membership_type, added_by, tags]

  # ==========================================================================
  # SHORTHAND TAGS (Human-Think Labels)
  # ==========================================================================

  - verb: tag-add
    description: "Add a shorthand tag to an entity (within client context)"
    behavior: plugin
    handler: ClientGroupTagAddOp
    invocation_phrases:
      - "tag this as"
      - "call this"
      - "label this"
      - "this is the"
      - "mark as"
    metadata:
      tier: intent
      source_of_truth: client_group_entity_tag
      writes_operational: true
      internal: false
      noun: entity_tag
      tags: [tag, crud, learning]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
        lookup:
          table: entities
          schema: ob-poc
          search_key: name
          primary_key: entity_id
      - name: tag
        type: string
        required: true
        description: "The shorthand label (e.g., 'main lux manco', 'irish fund')"
      - name: persona
        type: string
        required: false
        description: "Scope to persona: kyc, trading, ops, onboarding (null = universal)"
    returns:
      type: uuid
    effects:
      - "Computes tag_norm (normalized)"
      - "Queues embedding computation"
      - "If from interaction: source='user_confirmed'"

  - verb: tag-remove
    description: "Remove a shorthand tag from an entity"
    behavior: plugin
    handler: ClientGroupTagRemoveOp
    metadata:
      tier: intent
      source_of_truth: client_group_entity_tag
      writes_operational: true
      internal: false
      noun: entity_tag
      tags: [tag, crud]
    args:
      - name: tag-id
        type: uuid
        required: true
    returns:
      type: boolean

  - verb: tag-list
    description: "List tags for an entity (within client context)"
    behavior: plugin
    handler: ClientGroupTagListOp
    metadata:
      tier: diagnostics
      source_of_truth: client_group_entity_tag
      internal: false
      noun: entity_tag
      tags: [tag, read]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: false
        description: "Filter to specific entity (omit for all tags in group)"
      - name: persona
        type: string
        required: false
        description: "Filter to persona"
    returns:
      type: record_set
      fields: [tag_id, entity_id, entity_name, tag, persona, source, confidence]

  # ==========================================================================
  # SEMANTIC SEARCH (The Core Capability)
  # ==========================================================================

  - verb: search
    description: "Search entities by shorthand tags (Candle-assisted fuzzy search)"
    behavior: plugin
    handler: ClientGroupSearchOp
    invocation_phrases:
      - "find"
      - "show me"
      - "where is"
      - "the"
      - "load"
      - "get"
    metadata:
      tier: intent
      source_of_truth: client_group_entity_tag
      internal: false
      noun: entity_search
      tags: [search, semantic, candle]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: query
        type: string
        required: true
        description: "Human shorthand (e.g., 'the Irish funds', 'main ManCo')"
      - name: persona
        type: string
        required: false
        description: "Scope search to persona"
      - name: limit
        type: integer
        required: false
        default: 10
    returns:
      type: record_set
      fields: [entity_id, entity_name, matched_tag, confidence, match_type]
    effects:
      - "Tries exact match first"
      - "Falls back to trigram fuzzy"
      - "Falls back to embedding similarity"
      - "Returns entity_ids for DSL pipeline"
```

### 2.2 Custom Ops

**File: `rust/src/domain_ops/client_group_ops.rs`** (additions)

```rust
// Entity membership
#[register_custom_op]
pub struct ClientGroupEntityAddOp;

#[register_custom_op]
pub struct ClientGroupEntityRemoveOp;

#[register_custom_op]
pub struct ClientGroupEntityListOp;

// Shorthand tags
#[register_custom_op]
pub struct ClientGroupTagAddOp;

#[register_custom_op]
pub struct ClientGroupTagRemoveOp;

#[register_custom_op]
pub struct ClientGroupTagListOp;

// Semantic search (THE KEY CAPABILITY)
#[register_custom_op]
pub struct ClientGroupSearchOp;
```

### Acceptance Criteria (Phase 2)
- [ ] All CRUD verbs functional
- [ ] `tag-add` normalizes and queues embedding
- [ ] `tag-add` from interaction sets `source='user_confirmed'`
- [ ] `search` implements exact → fuzzy → semantic fallback
- [ ] Persona filtering works

---

## Phase 3: Candle Integration

### 3.1 Entity Context Resolver

**File: `rust/src/session/entity_context_resolver.rs`**

> **SQLx + pgvector note:** Use `pgvector::Vector` or your existing `Vec<f32>` wrapper as the bind type. 
> See `ob-semantic-matcher` for the established pattern. The `$2::vector` cast requires proper type registration.

```rust
//! Candle-assisted fuzzy search from human shorthand to entity_ids
//! 
//! This is THE bridge between human intent and formal entity references.

use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;

/// Result of searching for entities by shorthand
#[derive(Debug, Clone)]
pub struct EntityContextMatch {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub matched_tag: String,
    pub confidence: f32,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, Copy)]
pub enum MatchType {
    Exact,      // tag_norm = query_norm
    Fuzzy,      // trigram similarity
    Semantic,   // embedding similarity
}

pub struct EntityContextResolver<'a> {
    pool: &'a PgPool,
    embedder: Option<&'a CandleEmbedder>,
}

impl<'a> EntityContextResolver<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool, embedder: None }
    }
    
    pub fn with_embedder(mut self, embedder: &'a CandleEmbedder) -> Self {
        self.embedder = Some(embedder);
        self
    }
    
    /// Search for entities matching human shorthand
    /// 
    /// This is called by Candle when it needs to resolve entity references.
    /// 
    /// Example:
    ///   query = "the Irish funds"
    ///   Returns: [(entity_id_1, "AGI Ireland Fund"), (entity_id_2, "AGI Dublin SICAV")]
    pub async fn search(
        &self,
        group_id: Uuid,
        query: &str,
        persona: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EntityContextMatch>> {
        // Step 1: Text search (exact + fuzzy)
        let text_matches = self.search_text(group_id, query, persona, limit).await?;
        
        if !text_matches.is_empty() {
            return Ok(text_matches);
        }
        
        // Step 2: Semantic search (embedding fallback)
        if let Some(embedder) = self.embedder {
            let semantic_matches = self.search_semantic(
                group_id, query, persona, limit, embedder
            ).await?;
            
            if !semantic_matches.is_empty() {
                return Ok(semantic_matches);
            }
        }
        
        Ok(vec![])
    }
    
    async fn search_text(
        &self,
        group_id: Uuid,
        query: &str,
        persona: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EntityContextMatch>> {
        let rows = sqlx::query!(r#"
            SELECT 
                entity_id,
                entity_name,
                tag,
                confidence::FLOAT4 as confidence,
                match_type
            FROM "ob-poc".search_entity_tags($1, $2, $3, $4)
        "#, group_id, query, persona, limit as i32)
            .fetch_all(self.pool)
            .await?;
        
        Ok(rows.into_iter().map(|r| EntityContextMatch {
            entity_id: r.entity_id,
            entity_name: r.entity_name,
            matched_tag: r.tag,
            confidence: r.confidence.unwrap_or(0.0),
            match_type: match r.match_type.as_deref() {
                Some("exact") => MatchType::Exact,
                Some("fuzzy") => MatchType::Fuzzy,
                _ => MatchType::Fuzzy,
            },
        }).collect())
    }
    
    async fn search_semantic(
        &self,
        group_id: Uuid,
        query: &str,
        persona: Option<&str>,
        limit: usize,
        embedder: &CandleEmbedder,
    ) -> Result<Vec<EntityContextMatch>> {
        // Embed the query
        let query_embedding = embedder.embed(query).await?;
        
        let rows = sqlx::query!(r#"
            SELECT 
                entity_id,
                entity_name,
                tag,
                similarity::FLOAT4 as similarity,
                match_type
            FROM "ob-poc".search_entity_tags_semantic($1, $2::vector, $3, $4, 0.5)
        "#, group_id, &query_embedding, persona, limit as i32)
            .fetch_all(self.pool)
            .await?;
        
        Ok(rows.into_iter().map(|r| EntityContextMatch {
            entity_id: r.entity_id,
            entity_name: r.entity_name,
            matched_tag: r.tag,
            confidence: r.similarity.unwrap_or(0.0),
            match_type: MatchType::Semantic,
        }).collect())
    }
}
```

### 3.2 Candle Intent Pipeline Integration

**Integration point in intent pipeline:**

```rust
// In Candle intent matching, when resolving entity arguments:

pub async fn resolve_entity_reference(
    phrase: &str,              // "the Irish funds"
    session: &Session,
    pool: &PgPool,
    embedder: &CandleEmbedder,
) -> Result<Vec<Uuid>> {
    // If session has a client context, search within it
    if let Some(group_id) = session.client_group_id() {
        let resolver = EntityContextResolver::new(pool)
            .with_embedder(embedder);
        
        let matches = resolver.search(
            group_id,
            phrase,
            session.persona(),  // kyc, trading, etc.
            10,
        ).await?;
        
        if !matches.is_empty() {
            // Return entity_ids for DSL argument
            return Ok(matches.into_iter().map(|m| m.entity_id).collect());
        }
    }
    
    // Fallback to global entity search
    global_entity_search(phrase, pool).await
}
```

### 3.3 Learning from Interactions

When a user confirms an entity reference during conversation:

```rust
// User: "Show me the Irish funds"
// Agent: "Do you mean AGI Ireland Fund and AGI Dublin SICAV?"
// User: "Yes, and also include the Cork vehicle"

// On confirmation, learn the tag:
async fn learn_entity_tag(
    pool: &PgPool,
    group_id: Uuid,
    entity_id: Uuid,
    tag: &str,
    persona: Option<&str>,
    session_id: &str,
) -> Result<()> {
    let tag_norm = normalize_tag(tag);
    
    sqlx::query!(r#"
        INSERT INTO "ob-poc".client_group_entity_tag 
            (group_id, entity_id, tag, tag_norm, persona, source, created_by)
        VALUES ($1, $2, $3, $4, $5, 'user_confirmed', $6)
        ON CONFLICT (group_id, entity_id, tag_norm, COALESCE(persona, '')) 
        DO UPDATE SET 
            confidence = GREATEST(client_group_entity_tag.confidence, 0.9),
            source = 'user_confirmed'
    "#, group_id, entity_id, tag, tag_norm, persona, session_id)
        .execute(pool)
        .await?;
    
    // Queue embedding computation
    queue_tag_embedding(pool, tag_id).await?;
    
    Ok(())
}
```

### Acceptance Criteria (Phase 3)
- [ ] `EntityContextResolver` searches exact → fuzzy → semantic
- [ ] Candle uses resolver when session has client context
- [ ] Persona scoping filters results appropriately
- [ ] User confirmations create `user_confirmed` tags
- [ ] Tags get embeddings computed asynchronously

---

## Phase 4: Discovery Workflow (Onboarding)

### 4.1 Discovery Verbs

**Add to `client-group.yaml`:**

```yaml
  # ==========================================================================
  # DISCOVERY (Onboarding New Clients)
  # ==========================================================================

  - verb: discover-entities
    description: "Discover entities that might belong to this client (first stage onboarding)"
    behavior: plugin
    handler: ClientGroupDiscoverEntitiesOp
    invocation_phrases:
      - "find all entities for"
      - "discover entities for"
      - "load all"
      - "what entities belong to"
    metadata:
      tier: intent
      source_of_truth: client_group_entity
      writes_operational: true
      internal: false
      noun: discovery
      tags: [onboarding, discovery, bulk]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: search-terms
        type: string_array
        required: false
        description: "Additional terms to search (e.g., ['ireland', 'dublin'])"
      - name: jurisdiction
        type: string
        required: false
        description: "Filter to jurisdiction (e.g., 'IE', 'LU')"
      - name: auto-add
        type: boolean
        required: false
        default: false
        description: "Automatically add as 'suspected' (vs just returning candidates)"
    returns:
      type: record_set
      fields: [entity_id, entity_name, entity_type, match_reason, already_member]
    effects:
      - "Searches entities by name patterns"
      - "Searches entities by GLEIF relationships"
      - "If auto-add: creates membership with type='suspected'"

  - verb: confirm-entity
    description: "Confirm a suspected entity as belonging to client"
    behavior: plugin
    handler: ClientGroupConfirmEntityOp
    metadata:
      tier: intent
      source_of_truth: client_group_entity
      writes_operational: true
      internal: false
      noun: membership
      tags: [onboarding, confirmation]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
      - name: tags
        type: string_array
        required: false
        description: "Initial shorthand tags to apply"
    returns:
      type: boolean
    effects:
      - "Updates membership_type to 'confirmed'"
      - "If tags provided: creates shorthand tags"

  - verb: reject-entity
    description: "Mark a suspected entity as NOT belonging to client"
    behavior: plugin
    handler: ClientGroupRejectEntityOp
    metadata:
      tier: intent
      source_of_truth: client_group_entity
      writes_operational: true
      internal: false
      noun: membership
      tags: [onboarding, rejection]
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
      - name: reason
        type: string
        required: false
    returns:
      type: boolean
    effects:
      - "Removes entity from group (or marks historical)"
```

### 4.2 Discovery Workflow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ONBOARDING DISCOVERY WORKFLOW                                              │
│                                                                             │
│  1. "Load all entities we think are Allianz Ireland"                       │
│      │                                                                      │
│      ▼                                                                      │
│  client-group.discover-entities group-id=... jurisdiction="IE" auto-add=t  │
│      │                                                                      │
│      ▼                                                                      │
│  Returns 15 suspected entities                                             │
│                                                                             │
│  2. Human reviews list:                                                    │
│      │                                                                      │
│      ├── "Yes, this is the main Irish ManCo"                               │
│      │       → client-group.confirm-entity ... tags=["irish manco"]        │
│      │                                                                      │
│      ├── "These are the Irish SICAVs"                                      │
│      │       → client-group.confirm-entity ... tags=["irish sicav"]        │
│      │                                                                      │
│      └── "No, this doesn't belong to Allianz"                              │
│              → client-group.reject-entity ...                              │
│                                                                             │
│  3. Tags are now searchable:                                               │
│      │                                                                      │
│      ▼                                                                      │
│  Future: "Show me the Irish SICAVs" → instant resolution                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Acceptance Criteria (Phase 4)
- [ ] `discover-entities` finds candidates by name pattern + GLEIF
- [ ] `discover-entities` with `auto-add=true` creates suspected memberships
- [ ] `confirm-entity` upgrades to confirmed + applies tags
- [ ] `reject-entity` removes or marks historical
- [ ] Discovery → confirm → tag flow works end-to-end

---

## Phase 5: Session Integration

### 5.1 Session Context

Add to session state:

```rust
pub struct SessionState {
    // ... existing fields
    
    /// Current client group context (for entity resolution)
    pub client_group_id: Option<Uuid>,
    pub client_group_name: Option<String>,
    
    /// Current persona (affects tag filtering)
    pub persona: Option<String>,  // "kyc", "trading", "ops", etc.
}
```

### 5.2 Session Verbs

**Add to `session.yaml`:**

```yaml
  set-client:
    description: "Set client group context for entity resolution"
    behavior: plugin
    handler: SessionSetClientOp
    invocation_phrases:
      - "work on"
      - "switch to"
      - "I'm working with"
      - "client is"
    metadata:
      tier: intent
      source_of_truth: session
      noun: session_context
      tags: [context, client]
    args:
      - name: client
        type: string
        required: true
        description: "Client nickname (resolved via client_group_alias)"
    returns:
      type: record
      fields: [group_id, group_name, entity_count, candidates]
    effects:
      - "If exact match: sets client_group_id immediately"
      - "If ambiguous (score gap < 0.10): returns top 3 candidates for user selection"
      - "If no match above threshold: returns empty with suggestions"

  set-persona:
    description: "Set persona context for tag filtering"
    behavior: plugin
    handler: SessionSetPersonaOp
    invocation_phrases:
      - "I'm doing kyc"
      - "trading view"
      - "for kyc"
      - "for trading"
    metadata:
      tier: intent
      source_of_truth: session
      noun: session_context
      tags: [context, persona]
    args:
      - name: persona
        type: string
        required: true
        enum: ["kyc", "trading", "ops", "onboarding"]
    returns:
      type: boolean
```

### Acceptance Criteria (Phase 5)
- [ ] Session tracks client_group_id and persona
- [ ] `set-client` resolves nickname to group
- [ ] `set-persona` affects tag search filtering
- [ ] Entity resolution uses session context

---

## Testing

### Integration Tests

```rust
#[tokio::test]
async fn test_entity_tag_search_exact() {
    let pool = test_pool().await;
    
    // Setup: Add tag "main manco" to entity
    add_test_tag(&pool, ALLIANZ_GROUP, ALLIANZ_MANCO, "main manco", None).await;
    
    // Search
    let resolver = EntityContextResolver::new(&pool);
    let matches = resolver.search(ALLIANZ_GROUP, "main manco", None, 10).await.unwrap();
    
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].entity_id, ALLIANZ_MANCO);
    assert!(matches!(matches[0].match_type, MatchType::Exact));
}

#[tokio::test]
async fn test_entity_tag_search_fuzzy() {
    let pool = test_pool().await;
    
    // Setup: Add tag "irish fund"
    add_test_tag(&pool, ALLIANZ_GROUP, IRISH_FUND_1, "irish fund", None).await;
    
    // Search with typo
    let resolver = EntityContextResolver::new(&pool);
    let matches = resolver.search(ALLIANZ_GROUP, "irish funds", None, 10).await.unwrap();
    
    assert!(!matches.is_empty());
    assert!(matches!(matches[0].match_type, MatchType::Fuzzy));
}

#[tokio::test]
async fn test_persona_scoping() {
    let pool = test_pool().await;
    
    // Setup: Same entity, different tags for different personas
    add_test_tag(&pool, ALLIANZ_GROUP, ALLIANZ_MANCO, "kyc manco", Some("kyc")).await;
    add_test_tag(&pool, ALLIANZ_GROUP, ALLIANZ_MANCO, "trading manco", Some("trading")).await;
    
    let resolver = EntityContextResolver::new(&pool);
    
    // KYC persona finds "kyc manco"
    let kyc_matches = resolver.search(ALLIANZ_GROUP, "manco", Some("kyc"), 10).await.unwrap();
    assert!(kyc_matches.iter().any(|m| m.matched_tag == "kyc manco"));
    
    // Trading persona finds "trading manco"
    let trading_matches = resolver.search(ALLIANZ_GROUP, "manco", Some("trading"), 10).await.unwrap();
    assert!(trading_matches.iter().any(|m| m.matched_tag == "trading manco"));
}

#[tokio::test]
async fn test_discovery_workflow() {
    let pool = test_pool().await;
    
    // 1. Discover entities
    let discovered = discover_entities(&pool, ALLIANZ_GROUP, Some("IE"), true).await.unwrap();
    assert!(!discovered.is_empty());
    
    // 2. Confirm one with tags
    let entity_id = discovered[0].entity_id;
    confirm_entity(&pool, ALLIANZ_GROUP, entity_id, vec!["irish manco"]).await.unwrap();
    
    // 3. Search by tag works
    let resolver = EntityContextResolver::new(&pool);
    let matches = resolver.search(ALLIANZ_GROUP, "irish manco", None, 10).await.unwrap();
    
    assert!(matches.iter().any(|m| m.entity_id == entity_id));
}
```

---

## Phase 6: egui Feedback Integration (Closing the Loop)

### 6.1 The Complete Feedback Loop

This TODO enables the full REPL-style learning cycle:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  INTENT → DSL → ACTION → DISPLAY → FEEDBACK → LEARN                        │
│                                                                             │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐             │
│  │  INTENT  │───▶│   DSL    │───▶│  ACTION  │───▶│ DISPLAY  │             │
│  │          │    │          │    │          │    │  (egui)  │             │
│  │ "Irish   │    │ entity   │    │ query DB │    │ render   │             │
│  │  funds"  │    │ .list    │    │ return   │    │ entities │             │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘             │
│       ▲                                               │                    │
│       │                                               ▼                    │
│       │                                         ┌──────────┐              │
│       │         ┌──────────┐                    │ FEEDBACK │              │
│       │         │  LEARN   │◀───────────────────│  (user)  │              │
│       │         │          │                    │          │              │
│       │         │ persist  │                    │ "also    │              │
│       │         │ tag      │                    │  Cork"   │              │
│       │         └──────────┘                    └──────────┘              │
│       │              │                                                     │
│       └──────────────┘                                                     │
│                                                                             │
│  NEXT ITERATION: "Irish funds" now includes Cork automatically            │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Feedback Types

**File: `rust/crates/ob-poc-ui/src/feedback.rs`**

```rust
use uuid::Uuid;
use serde::{Deserialize, Serialize};

/// User feedback on entity resolution results
/// Captured via egui interactions and persisted to learning tables
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityFeedback {
    /// "Yes, this entity belongs in the result" — reinforces existing tag
    Confirm {
        entity_id: Uuid,
        matched_tag: String,
    },
    
    /// "No, this entity doesn't belong" — negative signal
    Reject {
        entity_id: Uuid,
        matched_tag: String,
    },
    
    /// "Also include this entity" — user adds to result set
    Include {
        entity_id: Uuid,
        from_query: String,  // The original query that should now match this
    },
    
    /// "Call this entity X" — user provides new shorthand label
    Label {
        entity_id: Uuid,
        new_tag: String,
        persona: Option<String>,
    },
    
    /// "These entities are all X" — bulk tagging from selection
    BulkLabel {
        entity_ids: Vec<Uuid>,
        tag: String,
        persona: Option<String>,
    },
}

/// Result of processing feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResult {
    pub accepted: bool,
    pub tags_created: usize,
    pub tags_reinforced: usize,
    pub message: String,
}
```

### 6.3 Feedback Handler

**File: `rust/src/feedback/entity_feedback_handler.rs`**

```rust
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;

use crate::feedback::EntityFeedback;

pub struct EntityFeedbackHandler<'a> {
    pool: &'a PgPool,
    group_id: Uuid,
    session_id: String,
}

impl<'a> EntityFeedbackHandler<'a> {
    pub fn new(pool: &'a PgPool, group_id: Uuid, session_id: String) -> Self {
        Self { pool, group_id, session_id }
    }
    
    /// Process user feedback and update learning tables
    pub async fn handle(&self, feedback: EntityFeedback) -> Result<FeedbackResult> {
        match feedback {
            EntityFeedback::Confirm { entity_id, matched_tag } => {
                self.reinforce_tag(entity_id, &matched_tag).await
            }
            
            EntityFeedback::Reject { entity_id, matched_tag } => {
                self.record_negative_signal(entity_id, &matched_tag).await
            }
            
            EntityFeedback::Include { entity_id, from_query } => {
                self.add_tag_from_query(entity_id, &from_query).await
            }
            
            EntityFeedback::Label { entity_id, new_tag, persona } => {
                self.create_user_tag(entity_id, &new_tag, persona.as_deref()).await
            }
            
            EntityFeedback::BulkLabel { entity_ids, tag, persona } => {
                self.bulk_create_tags(&entity_ids, &tag, persona.as_deref()).await
            }
        }
    }
    
    /// Reinforce existing tag — increase confidence
    async fn reinforce_tag(&self, entity_id: Uuid, tag: &str) -> Result<FeedbackResult> {
        let tag_norm = normalize_tag(tag);
        
        let updated = sqlx::query!(r#"
            UPDATE "ob-poc".client_group_entity_tag
            SET confidence = LEAST(confidence + 0.1, 1.0),
                source = CASE WHEN source != 'user_confirmed' THEN 'user_confirmed' ELSE source END
            WHERE group_id = $1 
              AND entity_id = $2 
              AND tag_norm = $3
            RETURNING id
        "#, self.group_id, entity_id, tag_norm)
            .fetch_optional(self.pool)
            .await?;
        
        Ok(FeedbackResult {
            accepted: updated.is_some(),
            tags_created: 0,
            tags_reinforced: if updated.is_some() { 1 } else { 0 },
            message: "Tag reinforced".to_string(),
        })
    }
    
    /// Record negative signal — entity should NOT match this tag
    async fn record_negative_signal(&self, entity_id: Uuid, tag: &str) -> Result<FeedbackResult> {
        let tag_norm = normalize_tag(tag);
        
        // Option 1: Reduce confidence
        // Option 2: Add to negative_tags table (future)
        // For now, reduce confidence significantly
        sqlx::query!(r#"
            UPDATE "ob-poc".client_group_entity_tag
            SET confidence = GREATEST(confidence - 0.3, 0.0)
            WHERE group_id = $1 
              AND entity_id = $2 
              AND tag_norm = $3
        "#, self.group_id, entity_id, tag_norm)
            .execute(self.pool)
            .await?;
        
        Ok(FeedbackResult {
            accepted: true,
            tags_created: 0,
            tags_reinforced: 0,
            message: "Negative feedback recorded".to_string(),
        })
    }
    
    /// User said "also include this" — create tag from original query
    async fn add_tag_from_query(&self, entity_id: Uuid, query: &str) -> Result<FeedbackResult> {
        self.create_user_tag(entity_id, query, None).await
    }
    
    /// Create new tag from user input
    async fn create_user_tag(
        &self, 
        entity_id: Uuid, 
        tag: &str,
        persona: Option<&str>,
    ) -> Result<FeedbackResult> {
        let tag_norm = normalize_tag(tag);
        
        // Ensure entity is a member of the group
        sqlx::query!(r#"
            INSERT INTO "ob-poc".client_group_entity (group_id, entity_id, membership_type, added_by)
            VALUES ($1, $2, 'confirmed', 'user_confirmed')
            ON CONFLICT (group_id, entity_id) DO UPDATE SET
                membership_type = 'confirmed'
        "#, self.group_id, entity_id)
            .execute(self.pool)
            .await?;
        
        // Create the tag
        let tag_id = sqlx::query_scalar!(r#"
            INSERT INTO "ob-poc".client_group_entity_tag 
                (group_id, entity_id, tag, tag_norm, persona, source, confidence, created_by)
            VALUES ($1, $2, $3, $4, $5, 'user_confirmed', 1.0, $6)
            ON CONFLICT (group_id, entity_id, tag_norm, COALESCE(persona, '')) 
            DO UPDATE SET 
                confidence = GREATEST(client_group_entity_tag.confidence, 0.95),
                source = 'user_confirmed'
            RETURNING id
        "#, self.group_id, entity_id, tag, tag_norm, persona, self.session_id)
            .fetch_one(self.pool)
            .await?;
        
        // Queue embedding computation
        self.queue_embedding(tag_id).await?;
        
        Ok(FeedbackResult {
            accepted: true,
            tags_created: 1,
            tags_reinforced: 0,
            message: format!("Tag '{}' created for entity", tag),
        })
    }
    
    /// Bulk tag multiple entities
    async fn bulk_create_tags(
        &self,
        entity_ids: &[Uuid],
        tag: &str,
        persona: Option<&str>,
    ) -> Result<FeedbackResult> {
        let mut created = 0;
        
        for entity_id in entity_ids {
            let result = self.create_user_tag(*entity_id, tag, persona).await?;
            created += result.tags_created;
        }
        
        Ok(FeedbackResult {
            accepted: true,
            tags_created: created,
            tags_reinforced: 0,
            message: format!("Tagged {} entities with '{}'", entity_ids.len(), tag),
        })
    }
    
    /// Queue background job to compute embedding for tag
    async fn queue_embedding(&self, tag_id: Uuid) -> Result<()> {
        // Implementation depends on your job queue
        // Could be: 
        // - Direct call to embedder (if fast enough)
        // - Insert into job table
        // - Send to async channel
        Ok(())
    }
}
```

### 6.4 egui Integration Points

**In your egui viewport, capture these interactions:**

```rust
// In egui rendering code

impl EntityViewport {
    fn render_entity_result(&mut self, ui: &mut egui::Ui, entity: &EntityMatch, ctx: &mut FeedbackContext) {
        ui.horizontal(|ui| {
            // Entity info
            ui.label(&entity.entity_name);
            ui.label(format!("(matched: {})", entity.matched_tag));
            
            // Feedback buttons
            if ui.small_button("✓").on_hover_text("Correct match").clicked() {
                ctx.emit(EntityFeedback::Confirm {
                    entity_id: entity.entity_id,
                    matched_tag: entity.matched_tag.clone(),
                });
            }
            
            if ui.small_button("✗").on_hover_text("Wrong match").clicked() {
                ctx.emit(EntityFeedback::Reject {
                    entity_id: entity.entity_id,
                    matched_tag: entity.matched_tag.clone(),
                });
            }
        });
    }
    
    fn render_add_entity_button(&mut self, ui: &mut egui::Ui, ctx: &mut FeedbackContext) {
        if ui.button("+ Add entity to results").clicked() {
            // Open entity picker
            self.show_entity_picker = true;
        }
        
        if self.show_entity_picker {
            if let Some(selected_entity) = self.entity_picker.render(ui) {
                ctx.emit(EntityFeedback::Include {
                    entity_id: selected_entity,
                    from_query: ctx.current_query.clone(),
                });
                self.show_entity_picker = false;
            }
        }
    }
    
    fn render_bulk_tag_ui(&mut self, ui: &mut egui::Ui, ctx: &mut FeedbackContext) {
        if !self.selected_entities.is_empty() {
            ui.horizontal(|ui| {
                ui.label(format!("{} selected", self.selected_entities.len()));
                
                ui.text_edit_singleline(&mut self.bulk_tag_input);
                
                if ui.button("Tag all as").clicked() && !self.bulk_tag_input.is_empty() {
                    ctx.emit(EntityFeedback::BulkLabel {
                        entity_ids: self.selected_entities.clone(),
                        tag: self.bulk_tag_input.clone(),
                        persona: ctx.current_persona.clone(),
                    });
                    self.bulk_tag_input.clear();
                    self.selected_entities.clear();
                }
            });
        }
    }
}
```

### 6.5 The Learning Flywheel

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  SESSION 1: First interaction                                               │
│                                                                             │
│  User: "Show me the Irish funds"                                           │
│  Agent: Shows [AGI Ireland Fund, Dublin SICAV] (from seed tags)            │
│  User: Clicks "+ Add" → selects Cork Vehicle                               │
│  Agent: LEARNS tag "irish funds" → Cork Vehicle                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  SESSION 2: Next day                                                        │
│                                                                             │
│  User: "Show me the Irish funds"                                           │
│  Agent: Shows [AGI Ireland Fund, Dublin SICAV, Cork Vehicle] ← LEARNED     │
│  User: "Perfect" (no correction needed)                                    │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  SESSION 3: Another user (same client group)                                │
│                                                                             │
│  User: "Irish stuff"                                                       │
│  Agent: Shows [AGI Ireland Fund, Dublin SICAV, Cork Vehicle]               │
│         (semantic match to existing tags)                                  │
│  User: Tags all as "ireland ops" for ops persona                           │
│  Agent: LEARNS persona-specific tags                                       │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Acceptance Criteria (Phase 6)
- [ ] `EntityFeedback` enum covers all feedback types
- [ ] `EntityFeedbackHandler` persists feedback to learning tables
- [ ] Confirm action reinforces tag confidence
- [ ] Reject action reduces tag confidence  
- [ ] Include action creates new tag from query
- [ ] Label action creates user-provided tag
- [ ] BulkLabel works for multi-select
- [ ] egui captures feedback via buttons/interactions
- [ ] Embeddings queued for new tags
- [ ] Learning persists across sessions
- [ ] Learning shared within client group (all users benefit)

---

## Acceptance Criteria (Full TODO)

### Functional
- [ ] Human shorthand resolves to entity_ids
- [ ] Same entity can have different tags for different personas
- [ ] Discovery workflow populates suspected → confirmed → tagged
- [ ] Candle uses resolver when session has client context
- [ ] User confirmations create learned tags

### Search Quality
- [ ] Exact match: "main manco" → entity
- [ ] Fuzzy match: "main manco" ≈ "main management company" → entity  
- [ ] Semantic match: "the Luxembourg headquarters" ~ "lux manco" → entity

### Performance
- [ ] Text search (exact + fuzzy) < 50ms
- [ ] Semantic search < 100ms
- [ ] Tag embedding computation async (non-blocking)

### Learning
- [ ] User confirmations stored with `source='user_confirmed'`
- [ ] Confirmed tags have higher confidence
- [ ] Embeddings computed for all tags

---

## Summary

This TODO builds on the existing client_group schema (047-048) to add:

| Layer | Tables | Purpose |
|-------|--------|---------|
| **Entity Membership** | `client_group_entity` | Track ALL entities belonging to client |
| **Shorthand Tags** | `client_group_entity_tag` | Human-think labels, persona-scoped |
| **Embeddings** | `client_group_entity_tag_embedding` | Candle semantic search |

**The Core Capability:**

```
Human: "Show me the Irish funds"
         │
         ▼
Candle: embed("irish funds") → search tags → entity_ids
         │
         ▼
Agent:  Understands exactly which entities the human means
```

This is the **ultimate Candle-assisted fuzzy search** — a learned vocabulary of how humans refer to entities within a client context.

---

## Peer Review Notes (ChatGPT, 2026-01-25)

### What Was Excellent (kept as-is)
1. **4-layer model** — Group alias → membership universe → persona tags → Candle search
2. **Explicit separation from UBO/legal hierarchy** — This is a "human-language index", not a "truth engine"
3. **Persona-scoped tags** — Key innovation for different stakeholders
4. **Learning flywheel + egui feedback** — "Include → create tag from query" mechanic

### Critical Fixes Applied (P0)

| Issue | Fix |
|-------|-----|
| **UNIQUE constraint with COALESCE** | Postgres can't use expressions in table constraints. Changed to `CREATE UNIQUE INDEX uq_cget_tag ON ... COALESCE(persona, '')` |
| **Membership not enforced in search** | Added JOIN on `client_group_entity` + exclude `historical` by default via `p_include_historical` parameter |
| **ANN index baked in too early** | Commented out IVFFlat index. At <2k rows, exact scan is fine. Enable later with HNSW or tuned IVFFlat |
| **Embedder mismatch risk** | Added `p_embedder_id` parameter to semantic search function to filter by model |

### Minor Fixes Applied (Follow-up review 2026-01-25)

| Nit | Fix |
|-----|-----|
| **A: gen_random_uuid() extension** | Added prerequisites comment noting pgcrypto + pgvector dependencies |
| **B: vector(384) hard-coded** | Added inline comment explaining intentional 384-dim standard |
| **C: SQLx + pgvector wiring** | Added note pointing to `ob-semantic-matcher` for established pattern |
| **D: YAML schema consistency** | Verified `behavior: plugin` + `handler:` matches existing codebase (control.yaml) |
| **set-client candidates** | Added `effects:` with candidate fallback behavior for ambiguous resolution |

### Recommendations Noted (for future)
- **Tag concept normalization** — Consider `tag_concept` table with synonyms per persona to avoid near-duplicates
- **Discovery seeding** — `ILIKE '%allianz%'` is bootstrap only; production should use GLEIF + human confirmation
- **MCP contract** — Add `scope_resolution.candidates`, `scope_resolution.resolved`, `scope_resolution.none` event types

---

## References

- migrations/047_client_group_tables.sql — existing group/alias/anchor schema
- migrations/048_client_group_seed.sql — existing seed data
- ob-semantic-matcher — Candle BGE embedder
- rust/src/session — session state management
