# Client Group Resolver Implementation

## Overview

Implement two-stage resolution: `"Allianz" → client_group_id → anchor_entity_id`

**Design Decisions:**
- Bootstrap: Claude Code assists with initial data population
- Disambiguation: Return candidates to chat (not ESPER)
- Multi-jurisdiction: `Option<String>` - explicit param, no session inference
- Enrichment stays synchronous - resolution via existing dsl_lookup + ref_id commit
- Anchor mapping happens in plugin handlers, not enrichment

---

## Step 1: Database Migration

Create `rust/migrations/20250123000001_client_group_tables.sql`:

```sql
-- ============================================================================
-- Client Group Tables
-- Two-stage resolution: nickname → group_id → anchor_entity_id
-- ============================================================================

-- Client group (virtual entity for nicknames/brands)
CREATE TABLE client_group (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_name TEXT NOT NULL,
    short_code TEXT UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE client_group IS 'Virtual entity representing client brand/nickname groups';

-- Aliases (multiple per group, for fuzzy matching)
CREATE TABLE client_group_alias (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES client_group(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    alias_norm TEXT NOT NULL,  -- normalized: lowercase, trimmed
    source TEXT DEFAULT 'manual',
    confidence FLOAT DEFAULT 1.0,
    is_primary BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(group_id, alias_norm)
);

CREATE INDEX idx_cga_alias_norm ON client_group_alias(alias_norm);
CREATE INDEX idx_cga_group_id ON client_group_alias(group_id);

-- ============================================================================
-- Embeddings with versioning support
-- Composite PK allows multiple embeddings per alias (different models/pooling)
-- Contract: all embeddings are L2-normalized for proper cosine distance
-- ============================================================================
CREATE TABLE client_group_alias_embedding (
    alias_id UUID NOT NULL REFERENCES client_group_alias(id) ON DELETE CASCADE,
    embedder_id TEXT NOT NULL,           -- e.g., 'bge-small-en-v1.5'
    pooling TEXT NOT NULL,               -- e.g., 'cls', 'mean'
    normalize BOOLEAN NOT NULL,          -- should always be true for BGE
    dimension INT NOT NULL,              -- e.g., 384
    embedding vector(384) NOT NULL,      -- L2-normalized vector
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (alias_id, embedder_id)
);

COMMENT ON TABLE client_group_alias_embedding IS 
    'Embeddings must be L2-normalized. Query embeddings must also be normalized for correct cosine distance.';

-- IVFFlat index for approximate nearest neighbor search
-- Note: Run ANALYZE client_group_alias_embedding after bulk inserts for good recall
CREATE INDEX idx_cgae_embedding ON client_group_alias_embedding 
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- ============================================================================
-- Anchor mappings (group → real entities, role-based)
-- Jurisdiction uses empty string '' for "no jurisdiction" to enable unique constraint
-- ============================================================================
CREATE TABLE client_group_anchor (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES client_group(id) ON DELETE CASCADE,
    anchor_entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    anchor_role TEXT NOT NULL,           -- 'ultimate_parent', 'governance_controller', etc.
    jurisdiction TEXT NOT NULL DEFAULT '',  -- empty string = no jurisdiction filter
    confidence FLOAT DEFAULT 1.0,
    priority INTEGER DEFAULT 0,          -- higher = preferred
    valid_from DATE,
    valid_to DATE,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(group_id, anchor_role, anchor_entity_id, jurisdiction)
);

CREATE INDEX idx_cga_group_role ON client_group_anchor(group_id, anchor_role);
CREATE INDEX idx_cga_anchor_entity ON client_group_anchor(anchor_entity_id);

COMMENT ON COLUMN client_group_anchor.jurisdiction IS 
    'Empty string means "applies to all jurisdictions". Specific jurisdiction takes precedence over empty.';

-- ============================================================================
-- Anchor role reference (for documentation/validation)
-- ============================================================================
CREATE TABLE client_group_anchor_role (
    role_code TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    default_for_domains TEXT[]  -- which verb domains use this role by default
);

INSERT INTO client_group_anchor_role (role_code, description, default_for_domains) VALUES
    ('ultimate_parent', 'UBO top-level parent (ownership apex)', ARRAY['ubo', 'ownership']),
    ('governance_controller', 'Operational/board control entity (ManCo equivalent)', ARRAY['session', 'cbu', 'view']),
    ('book_controller', 'Regional book controller', ARRAY['view']),
    ('operating_controller', 'Day-to-day operations controller', ARRAY['contract', 'service']),
    ('regulatory_anchor', 'Primary regulated entity for compliance', ARRAY['kyc', 'screening']);
```

Run: `cd rust && sqlx migrate run`

**Post-migration:** After bulk inserting aliases, run:
```sql
ANALYZE client_group_alias_embedding;
```

---

## Step 2: Wire Rust Module

Edit `rust/crates/ob-semantic-matcher/src/lib.rs`:

```rust
pub mod client_group_resolver;
pub use client_group_resolver::*;
```

The resolver file already exists at `rust/crates/ob-semantic-matcher/src/client_group_resolver.rs`.

**Update the resolver** to handle versioned embeddings:

```rust
// In search_aliases(), query should filter by embedder_id
let rows = sqlx::query_as::<_, (Uuid, String, String, f32)>(
    r#"
    SELECT 
        cg.id,
        cg.canonical_name,
        cga.alias,
        1 - (cgae.embedding <=> $1::vector) as similarity
    FROM client_group_alias_embedding cgae
    JOIN client_group_alias cga ON cga.id = cgae.alias_id
    JOIN client_group cg ON cg.id = cga.group_id
    WHERE cgae.embedder_id = $3  -- filter by current embedder
    ORDER BY cgae.embedding <=> $1::vector
    LIMIT $2
    "#,
)
.bind(&embedding)
.bind(limit as i32)
.bind(&self.embedder_id)  // add embedder_id field to resolver
.fetch_all(&self.pool)
.await?;
```

**Update anchor resolution** with deterministic ordering:

```rust
// In resolve_anchor(), explicit deterministic ordering
let row = sqlx::query_as::<_, (Uuid, String, f32)>(
    r#"
    SELECT anchor_entity_id, jurisdiction, confidence
    FROM client_group_anchor
    WHERE group_id = $1 
      AND anchor_role = $2
      AND (valid_from IS NULL OR valid_from <= CURRENT_DATE)
      AND (valid_to IS NULL OR valid_to >= CURRENT_DATE)
      AND (
          jurisdiction = $3  -- exact match
          OR ($3 = '' AND jurisdiction = '')  -- no jurisdiction requested, match global
          OR ($3 != '' AND jurisdiction = '')  -- specific requested, fallback to global
      )
    ORDER BY 
        CASE WHEN jurisdiction = $3 THEN 0 ELSE 1 END,  -- exact jurisdiction first
        priority DESC,                                    -- then priority
        confidence DESC,                                  -- then confidence
        anchor_entity_id                                  -- stable tie-breaker
    LIMIT 1
    "#,
)
.bind(group_id)
.bind(role.as_str())
.bind(jurisdiction.unwrap_or(""))
.fetch_optional(&self.pool)
.await?;
```

Verify it compiles: `cargo check -p ob-semantic-matcher`

---

## Step 3: Add dsl_lookup Support for client_group

**This is the key architectural change:** Resolution happens via existing `dsl_lookup` flow, not async enrichment.

Edit `rust/crates/dsl-core/src/lookup/mod.rs` (or wherever dsl_lookup types live):

```rust
// Add client_group to LookupType enum
pub enum LookupType {
    Entity,
    Cbu,
    Document,
    ClientGroup,  // NEW
    // ...
}
```

Add lookup handler in the lookup resolution code:

```rust
// In the lookup resolver
LookupType::ClientGroup => {
    // 1. Try exact match on alias_norm
    let exact = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT cg.id, cg.canonical_name
        FROM client_group_alias cga
        JOIN client_group cg ON cg.id = cga.group_id
        WHERE cga.alias_norm = lower(trim($1))
        "#
    )
    .bind(&search_value)
    .fetch_optional(pool)
    .await?;
    
    if let Some((id, name)) = exact {
        return Ok(vec![LookupCandidate {
            id,
            display: name,
            score: 1.0,
            exact: true,
        }]);
    }
    
    // 2. Semantic search via embeddings
    let embedding = embedder.embed_query(&search_value).await?;
    let candidates = sqlx::query_as::<_, (Uuid, String, String, f32)>(
        r#"
        SELECT cg.id, cg.canonical_name, cga.alias,
               1 - (cgae.embedding <=> $1::vector) as score
        FROM client_group_alias_embedding cgae
        JOIN client_group_alias cga ON cga.id = cgae.alias_id
        JOIN client_group cg ON cg.id = cga.group_id
        WHERE cgae.embedder_id = $3
        ORDER BY cgae.embedding <=> $1::vector
        LIMIT $2
        "#
    )
    .bind(&embedding)
    .bind(top_k)
    .bind(embedder_id)
    .fetch_all(pool)
    .await?;
    
    // 3. Apply same ambiguity gating as verb matching
    // Return candidates for disambiguation if needed
}
```

---

## Step 4: Update Verb YAML - session.yaml

Edit `rust/config/verbs/session.yaml`, update `load-cluster`:

```yaml
      load-cluster:
        description: Load all CBUs under a client group or GROUP entity
        behavior: plugin
        handler: SessionLoadClusterOp
        invocation_phrases:
          - "work on"
          - "focus on"
          - "set scope to"
          - "load book for"
          - "load all funds under"
          - "show all CBUs for"
          - "load group"
          - "load cluster"
          - "load book"
          - "load manco"
        metadata:
          tier: intent
          source_of_truth: session
          noun: session_cbus
          tags: [load, cluster, book, client, bulk, scope]
        args:
          # Client group nickname - resolved via dsl_lookup
          - name: client
            type: string                    # <-- STRING not uuid
            required: false
            description: Client group nickname (e.g., "Allianz", "AGI")
            lookup:
              lookup_type: client_group     # <-- triggers client_group resolution
              search_key: alias
              primary_key: id
              anchor_role: governance_controller  # <-- used by plugin handler
          # Direct apex entity (fallback)
          - name: apex-entity-id
            type: uuid
            required: false
            description: UUID of GROUP apex entity (alternative to client)
            lookup:
              table: entities
              entity_type: group
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: jurisdiction
            type: string
            required: false
        validation:
          one_of_required: [client, apex-entity-id]
        returns:
          type: record
          fields: [apex_name, jurisdiction, count_added, total_loaded]
```

Remove hardcoded client names from `invocation_phrases`.

---

## Step 5: Update Verb YAML - ubo.yaml

Edit `rust/config/verbs/ubo.yaml`, update `trace-chains`:

```yaml
      trace-chains:
        description: Trace ownership chains for a CBU or client
        metadata:
          tier: intent
          source_of_truth: operational
          scope: cbu
          noun: ubo
        behavior: plugin
        args:
          - name: client
            type: string                    # <-- STRING not uuid
            required: false
            description: Client group nickname (resolves to ultimate_parent)
            lookup:
              lookup_type: client_group
              search_key: alias
              primary_key: id
              anchor_role: ultimate_parent  # <-- different role than session verbs
          - name: cbu-id
            type: uuid
            required: false
            description: CBU to trace ownership chains for
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: target-entity-id
            type: uuid
            required: false
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: threshold
            type: decimal
            required: false
            default: 25.0
          - name: as-of-date
            type: date
            required: false
        validation:
          one_of_required: [client, cbu-id, target-entity-id]
        returns:
          type: record
```

---

## Step 6: Plugin Handler - Anchor Mapping

**Key insight:** Anchor mapping (group_id → entity_id) happens in the plugin handler, not enrichment.

Edit `rust/crates/ob-workflow/src/ops/session_ops.rs` (or equivalent):

```rust
use ob_semantic_matcher::{ClientGroupAnchorResolver, AnchorRole};

impl SessionLoadClusterOp {
    pub async fn execute(&self, ctx: &ExecContext, args: &ResolvedArgs) -> Result<...> {
        // Get the root entity - either from direct arg or via client group
        let root_entity_id = if let Some(client_group_id) = args.get_uuid("client") {
            // Resolve client_group_id → anchor entity for this verb's role
            let anchor_role = AnchorRole::GovernanceController;  // from YAML config
            let jurisdiction = args.get_string("jurisdiction");
            
            ctx.anchor_resolver
                .resolve_anchor(client_group_id, anchor_role, jurisdiction.as_deref())
                .await?
                .anchor_entity_id
        } else if let Some(apex_id) = args.get_uuid("apex-entity-id") {
            apex_id
        } else {
            return Err(anyhow!("Either client or apex-entity-id required"));
        };
        
        // Now proceed with root_entity_id as before
        self.load_cluster_for_entity(ctx, root_entity_id).await
    }
}
```

Similarly for `UboTraceChainsOp`:

```rust
impl UboTraceChainsOp {
    pub async fn execute(&self, ctx: &ExecContext, args: &ResolvedArgs) -> Result<...> {
        let root_entity_id = if let Some(client_group_id) = args.get_uuid("client") {
            let anchor_role = AnchorRole::UltimateParent;  // UBO uses different role
            let jurisdiction = args.get_string("jurisdiction");
            
            ctx.anchor_resolver
                .resolve_anchor(client_group_id, anchor_role, jurisdiction.as_deref())
                .await?
                .anchor_entity_id
        } else if let Some(cbu_id) = args.get_uuid("cbu-id") {
            self.get_cbu_root_entity(ctx, cbu_id).await?
        } else if let Some(entity_id) = args.get_uuid("target-entity-id") {
            entity_id
        } else {
            return Err(anyhow!("client, cbu-id, or target-entity-id required"));
        };
        
        self.trace_chains_from(ctx, root_entity_id).await
    }
}
```

---

## Step 7: Create client.yaml

Create `rust/config/verbs/client.yaml`:

```yaml
domains:
  client:
    description: Manage client group nicknames and anchor mappings

    verbs:
      create-group:
        description: Create a new client group
        behavior: crud
        metadata:
          tier: admin
          source_of_truth: client_group
          scope: global
          noun: client_group
          tags: [create, admin]
        crud:
          operation: insert
          table: client_group
          schema: ob-poc
          returning: id
        args:
          - name: canonical-name
            type: string
            required: true
            maps_to: canonical_name
          - name: short-code
            type: string
            required: false
            maps_to: short_code
          - name: description
            type: string
            required: false
            maps_to: description
        returns:
          type: uuid
          name: group_id
          capture: true

      add-alias:
        description: Add an alias to a client group
        behavior: crud
        metadata:
          tier: admin
          source_of_truth: client_group
          scope: global
          noun: client_group_alias
          tags: [create, admin]
        crud:
          operation: upsert
          table: client_group_alias
          schema: ob-poc
          returning: id
          conflict_keys: [group_id, alias_norm]
        args:
          - name: group-id
            type: string
            required: true
            maps_to: group_id
            lookup:
              lookup_type: client_group
              search_key: canonical_name
              primary_key: id
          - name: alias
            type: string
            required: true
            maps_to: alias
          - name: is-primary
            type: boolean
            required: false
            maps_to: is_primary
            default: false
        transforms:
          alias_norm: "lower(trim(alias))"
        returns:
          type: uuid
          name: alias_id
          capture: true

      add-anchor:
        description: Link client group to anchor entity for a role
        behavior: crud
        metadata:
          tier: admin
          source_of_truth: client_group
          scope: global
          noun: client_group_anchor
          tags: [create, admin]
        crud:
          operation: upsert
          table: client_group_anchor
          schema: ob-poc
          returning: id
          conflict_keys: [group_id, anchor_role, anchor_entity_id, jurisdiction]
        args:
          - name: group-id
            type: string
            required: true
            maps_to: group_id
            lookup:
              lookup_type: client_group
              search_key: canonical_name
              primary_key: id
          - name: anchor-entity-id
            type: uuid
            required: true
            maps_to: anchor_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: anchor-role
            type: string
            required: true
            maps_to: anchor_role
            valid_values:
              - ultimate_parent
              - governance_controller
              - book_controller
              - operating_controller
              - regulatory_anchor
          - name: jurisdiction
            type: string
            required: false
            maps_to: jurisdiction
            default: ""   # empty string = global
          - name: priority
            type: integer
            required: false
            maps_to: priority
            default: 0
        returns:
          type: uuid
          name: anchor_id
          capture: true

      list-groups:
        description: List all client groups
        behavior: crud
        metadata:
          tier: diagnostics
          source_of_truth: client_group
          scope: global
          noun: client_group
          tags: [query, admin]
        crud:
          operation: select
          table: client_group
          schema: ob-poc
        args:
          - name: limit
            type: integer
            required: false
            default: 100
        returns:
          type: record_set

      resolve:
        description: Resolve client alias to anchor entity (diagnostic)
        behavior: plugin
        handler: ClientResolveOp
        metadata:
          tier: diagnostics
          source_of_truth: client_group
          scope: global
          noun: client_group
          tags: [query, resolution]
        args:
          - name: alias
            type: string
            required: true
          - name: anchor-role
            type: string
            required: false
            default: governance_controller
            valid_values:
              - ultimate_parent
              - governance_controller
              - book_controller
              - operating_controller
              - regulatory_anchor
          - name: jurisdiction
            type: string
            required: false
        returns:
          type: record
          fields: [group_id, group_name, anchor_entity_id, anchor_name, similarity_score]
```

---

## Step 8: Seed Test Data

Create `rust/migrations/20250123000002_client_group_seed.sql`:

```sql
-- Allianz test group
INSERT INTO client_group (id, canonical_name, short_code, description) VALUES
    ('11111111-1111-1111-1111-111111111111', 'Allianz Global Investors', 'AGI', 'Allianz asset management');

INSERT INTO client_group_alias (group_id, alias, alias_norm, is_primary) VALUES
    ('11111111-1111-1111-1111-111111111111', 'Allianz Global Investors', 'allianz global investors', true),
    ('11111111-1111-1111-1111-111111111111', 'Allianz', 'allianz', false),
    ('11111111-1111-1111-1111-111111111111', 'AGI', 'agi', false),
    ('11111111-1111-1111-1111-111111111111', 'AllianzGI', 'allianzgi', false);

-- Link to actual entities (update UUIDs after checking your entities table)
-- Run: SELECT entity_id, name FROM entities WHERE name ILIKE '%allianz%';
-- Then:
-- INSERT INTO client_group_anchor (group_id, anchor_entity_id, anchor_role, jurisdiction, priority) VALUES
--     ('11111111-1111-1111-1111-111111111111', '<allianz_se_id>', 'ultimate_parent', '', 10),
--     ('11111111-1111-1111-1111-111111111111', '<allianzgi_gmbh_id>', 'governance_controller', '', 10),
--     ('11111111-1111-1111-1111-111111111111', '<allianzgi_lux_id>', 'governance_controller', 'LU', 20);  -- jurisdiction-specific
```

---

## Step 9: Batch Embedding Job

Create `rust/crates/ob-semantic-matcher/src/jobs/embed_client_aliases.rs`:

```rust
use sqlx::PgPool;
use crate::embedder::Embedder;
use uuid::Uuid;

/// Batch embed all client aliases that don't have embeddings for the current embedder
pub async fn embed_client_aliases_batch(
    pool: &PgPool,
    embedder: &dyn Embedder,
    embedder_id: &str,
    pooling: &str,
    dimension: i32,
) -> anyhow::Result<usize> {
    // 1. Fetch all aliases missing embeddings for this embedder
    let aliases: Vec<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT cga.id, cga.alias
        FROM client_group_alias cga
        WHERE NOT EXISTS (
            SELECT 1 FROM client_group_alias_embedding cgae
            WHERE cgae.alias_id = cga.id AND cgae.embedder_id = $1
        )
        "#
    )
    .bind(embedder_id)
    .fetch_all(pool)
    .await?;

    if aliases.is_empty() {
        return Ok(0);
    }

    // 2. Batch embed all aliases (as targets, no query prefix)
    let texts: Vec<&str> = aliases.iter().map(|(_, alias)| alias.as_str()).collect();
    let embeddings = embedder.embed_batch_targets(&texts).await?;

    // 3. Insert all in a transaction
    let mut tx = pool.begin().await?;
    
    for ((alias_id, _), embedding) in aliases.iter().zip(embeddings.iter()) {
        sqlx::query(
            r#"
            INSERT INTO client_group_alias_embedding 
                (alias_id, embedder_id, pooling, normalize, dimension, embedding)
            VALUES ($1, $2, $3, true, $4, $5)
            ON CONFLICT (alias_id, embedder_id) DO UPDATE SET
                embedding = EXCLUDED.embedding,
                pooling = EXCLUDED.pooling,
                dimension = EXCLUDED.dimension
            "#
        )
        .bind(alias_id)
        .bind(embedder_id)
        .bind(pooling)
        .bind(dimension)
        .bind(embedding)
        .execute(&mut *tx)
        .await?;
    }
    
    tx.commit().await?;

    // 4. Refresh index statistics
    sqlx::query("ANALYZE client_group_alias_embedding")
        .execute(pool)
        .await?;

    Ok(aliases.len())
}
```

Add to `rust/crates/ob-semantic-matcher/src/jobs/mod.rs`:

```rust
pub mod embed_client_aliases;
pub use embed_client_aliases::*;
```

---

## Step 10: Tests

Add to `rust/crates/ob-semantic-matcher/src/client_group_resolver.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_role_roundtrip() {
        for role in [
            AnchorRole::UltimateParent,
            AnchorRole::GovernanceController,
            AnchorRole::BookController,
            AnchorRole::OperatingController,
            AnchorRole::RegulatoryAnchor,
        ] {
            let s = role.as_str();
            let parsed = AnchorRole::from_str(s).expect("should parse");
            assert_eq!(role, parsed);
        }
    }

    #[test]
    fn test_disambiguation_response() {
        let err = ClientGroupResolveError::Ambiguous {
            input: "AGI".to_string(),
            candidates: vec![
                ClientGroupMatch {
                    group_id: Uuid::new_v4(),
                    canonical_name: "Allianz Global Investors".to_string(),
                    matched_alias: "AGI".to_string(),
                    similarity_score: 0.95,
                },
                ClientGroupMatch {
                    group_id: Uuid::new_v4(),
                    canonical_name: "Aberdeen Global Infrastructure".to_string(),
                    matched_alias: "AGI".to_string(),
                    similarity_score: 0.92,
                },
            ],
        };

        let response = DisambiguationResponse::from_ambiguous_error(&err).unwrap();
        assert_eq!(response.candidates.len(), 2);
        assert!(response.message.contains("AGI"));
        
        // Test selection by index
        let first_id = response.candidates[0].group_id;
        assert_eq!(response.resolve_selection("1"), Some(first_id));
        
        // Test selection by name
        assert_eq!(response.resolve_selection("Allianz"), Some(first_id));
    }
}

// Integration tests (require DB)
#[cfg(test)]
mod integration_tests {
    // Test: exact match "allianz" resolves without embedding call
    // Test: semantic match "AllianzGI" resolves via embeddings
    // Test: ambiguity "AGI" returns multiple candidates
    // Test: anchor mapping with jurisdiction preference
}
```

---

## Step 11: Test Commands

```bash
# Compile
cargo check -p ob-semantic-matcher
cargo check -p dsl-core

# Run migrations
cd rust && sqlx migrate run

# Populate embeddings (via CLI or add to startup)
# embed_client_aliases_batch(pool, embedder, "bge-small-en-v1.5", "cls", 384)

# Test resolution in chat:
# "load cluster for Allianz"      → governance_controller anchor
# "trace UBO chains for AGI"      → ultimate_parent anchor
# "load cluster for AGI"          → disambiguation if multiple matches
```

---

## Files Changed Summary

| Action | File |
|--------|------|
| CREATE | `rust/migrations/20250123000001_client_group_tables.sql` |
| CREATE | `rust/migrations/20250123000002_client_group_seed.sql` |
| EDIT | `rust/crates/ob-semantic-matcher/src/lib.rs` |
| EDIT | `rust/crates/ob-semantic-matcher/src/client_group_resolver.rs` |
| CREATE | `rust/crates/ob-semantic-matcher/src/jobs/embed_client_aliases.rs` |
| EDIT | `rust/crates/dsl-core/src/lookup/mod.rs` (add ClientGroup lookup type) |
| EDIT | `rust/config/verbs/session.yaml` |
| EDIT | `rust/config/verbs/ubo.yaml` |
| CREATE | `rust/config/verbs/client.yaml` |
| EDIT | `rust/crates/ob-workflow/src/ops/session_ops.rs` (anchor mapping) |
| EDIT | `rust/crates/ob-workflow/src/ops/ubo_ops.rs` (anchor mapping) |

---

## Architecture Summary

```
User: "load cluster for Allianz"
        │
        ▼
[Intent → DSL] verb: session.load-cluster, args: {client: "Allianz"}
        │
        ▼
[Enrichment] EntityRef{entity_type: "client_group", value: "Allianz", resolved_key: None}
        │
        ▼ (synchronous, no async DB calls)
        │
[dsl_lookup] lookup_type=client_group
        │
        ├─► exact match on alias_norm? → return group_id
        │
        └─► semantic search via embeddings → return candidates
                │
                ├─► confident? → commit ref_id → resolved_key = group_id
                │
                └─► ambiguous? → disambiguation chat response
        │
        ▼
[Plugin Handler] SessionLoadClusterOp
        │
        ▼
[Anchor Mapping] group_id + anchor_role=governance_controller → anchor_entity_id
        │
        ▼
[Execute] load_cluster_for_entity(anchor_entity_id)
```

**Key principle:** Enrichment stays synchronous. Resolution uses existing dsl_lookup + disambiguation flow. Anchor mapping happens in plugin handlers where the role is known.
