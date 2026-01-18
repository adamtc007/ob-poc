# Vector Database Portability: PostgreSQL pgvector vs Oracle 23ai

**Document**: Technical Analysis  
**Related ADR**: `/docs/ARCH-DECISION-CANDLE-EMBEDDINGS.md`  
**Date**: 2025-01-18

---

## Executive Summary

**Good news**: The vector search layer has **low coupling** to PostgreSQL. Oracle 23ai AI Vector Search supports the same `<=>` cosine distance operator, making migration straightforward if required.

**Estimated migration effort**: 2-3 days (mostly DDL changes).

---

## 1. Current PostgreSQL/pgvector Implementation

### 1.1 Schema Elements

Location: `/migrations/033_learning_embeddings.sql`, `/migrations/034_candle_embeddings.sql`

```sql
-- Vector column definition
embedding vector(384)

-- IVFFlat index
CREATE INDEX idx_phrases_embedding 
ON agent.invocation_phrases 
USING ivfflat (embedding vector_cosine_ops) 
WITH (lists = 100);
```

### 1.2 Query Patterns

Location: `/rust/src/mcp/verb_search.rs`

```sql
-- Cosine similarity search (current implementation)
SELECT phrase, verb, 
       1 - (embedding <=> $1::vector) as similarity
FROM agent.invocation_phrases
WHERE embedding IS NOT NULL
ORDER BY embedding <=> $1::vector
LIMIT 5
```

### 1.3 Coupling Points

| File | pgvector-Specific Elements |
|------|---------------------------|
| `migrations/033_*.sql` | `vector(1536)`, `ivfflat`, `vector_cosine_ops` |
| `migrations/034_*.sql` | `vector(384)`, `ivfflat`, `vector_cosine_ops` |
| `rust/src/mcp/verb_search.rs` | `<=>` operator, `::vector` cast |
| `rust/src/mcp/handlers/core.rs` | `::vector` cast in INSERT/UPDATE |

---

## 2. Oracle 23ai AI Vector Search Compatibility

### 2.1 Syntax Comparison

| Feature | pgvector (PostgreSQL) | Oracle 23ai | Compatible? |
|---------|----------------------|-------------|-------------|
| Vector type | `vector(384)` | `VECTOR(384, FLOAT32)` | ✅ Minor syntax |
| Cosine operator | `<=>` | `<=>` | ✅ **Identical** |
| Distance function | N/A | `VECTOR_DISTANCE(a, b, COSINE)` | ✅ Alternative |
| IVF Index | `USING ivfflat` | `ORGANIZATION NEIGHBOR PARTITIONS` | ⚠️ Different DDL |
| HNSW Index | `USING hnsw` | `ORGANIZATION INMEMORY NEIGHBOR GRAPH` | ⚠️ Different DDL |
| Approximate search | `LIMIT n` | `FETCH APPROX FIRST n ROWS ONLY` | ⚠️ Different syntax |
| Embedding function | External (Candle) | Built-in `VECTOR_EMBEDDING()` | ✅ Both work |

### 2.2 Key Finding: `<=>` Operator Compatibility

**Oracle 23ai supports the `<=>` operator for cosine distance:**

```sql
-- Oracle 23ai documentation confirms:
-- <=> is the cosine distance operator
-- expr1 <=> expr2 is equivalent to VECTOR_DISTANCE(expr1, expr2, COSINE)
```

This means our core query pattern works unchanged:

```sql
-- This query works in BOTH pgvector AND Oracle 23ai:
SELECT phrase, verb, 1 - (embedding <=> query_vector) as similarity
FROM agent.invocation_phrases
ORDER BY embedding <=> query_vector
```

---

## 3. Architecture Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                    ARCHITECTURE COUPLING                         │
│                                                                  │
│  ┌────────────────────┐                                         │
│  │   Candle Embedder  │  ← Database-agnostic                    │
│  │   (Vec<f32>)       │     Returns [f32; 384]                  │
│  └─────────┬──────────┘                                         │
│            │                                                     │
│            ▼                                                     │
│  ┌────────────────────┐                                         │
│  │   Embedder Trait   │  ← Interface abstraction                │
│  │   embed(&str)      │     No DB knowledge                     │
│  └─────────┬──────────┘                                         │
│            │                                                     │
│            ▼                                                     │
│  ┌────────────────────┐                                         │
│  │   verb_search.rs   │  ← DATABASE-SPECIFIC                    │
│  │   (SQL queries)    │     Contains <=> operator               │
│  └─────────┬──────────┘                                         │
│            │                                                     │
│            ▼                                                     │
│  ┌──────────────┐ OR ┌──────────────┐                           │
│  │   pgvector   │    │ Oracle 23ai  │                           │
│  │ (PostgreSQL) │    │ (AI Vector)  │                           │
│  └──────────────┘    └──────────────┘                           │
└─────────────────────────────────────────────────────────────────┘

Legend:
  ✅ Database-agnostic (no changes needed)
  ⚠️ Database-specific (changes needed for Oracle)
```

---

## 4. Migration Effort Assessment

### 4.1 Components by Effort Level

| Component | Current State | Oracle Change | Effort |
|-----------|--------------|---------------|--------|
| **Candle Embedder** | Pure Rust, `Vec<f32>` | None | ✅ Zero |
| **Embedder Trait** | Database-agnostic | None | ✅ Zero |
| **verb_search.rs queries** | Uses `<=>` | Mostly compatible | ⚠️ Low |
| **handlers/core.rs** | Uses `::vector` cast | Different cast syntax | ⚠️ Low |
| **Migration DDL** | pgvector syntax | Oracle syntax | ⚠️ Medium |
| **Index creation** | `ivfflat` | `NEIGHBOR PARTITIONS` | ⚠️ Medium |
| **Connection pool** | sqlx + tokio-postgres | sqlx-oracle | ⚠️ Medium |

### 4.2 Total Effort Estimate

| Task | Estimate |
|------|----------|
| Schema DDL rewrite | 4 hours |
| Query adjustments (LIMIT → FETCH) | 2 hours |
| Vector cast syntax | 2 hours |
| Connection pool setup | 4 hours |
| Testing | 8 hours |
| **Total** | **~2-3 days** |

---

## 5. Migration Path (If Oracle Required)

### 5.1 Schema Migration

```sql
-- PostgreSQL/pgvector (current)
CREATE TABLE agent.invocation_phrases (
    id SERIAL PRIMARY KEY,
    phrase VARCHAR(500) NOT NULL,
    verb VARCHAR(100) NOT NULL,
    embedding vector(384),
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_phrases_embedding 
ON agent.invocation_phrases 
USING ivfflat (embedding vector_cosine_ops) 
WITH (lists = 100);
```

```sql
-- Oracle 23ai (equivalent)
CREATE TABLE agent.invocation_phrases (
    id NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    phrase VARCHAR2(500) NOT NULL,
    verb VARCHAR2(100) NOT NULL,
    embedding VECTOR(384, FLOAT32),
    created_at TIMESTAMP DEFAULT SYSTIMESTAMP
);

CREATE VECTOR INDEX idx_phrases_embedding 
ON agent.invocation_phrases(embedding)
ORGANIZATION NEIGHBOR PARTITIONS
DISTANCE COSINE
WITH TARGET ACCURACY 95;
```

### 5.2 Query Migration

```sql
-- PostgreSQL (current)
SELECT phrase, verb, 
       1 - (embedding <=> $1::vector) as similarity
FROM agent.invocation_phrases
WHERE embedding IS NOT NULL
ORDER BY embedding <=> $1::vector
LIMIT 5;

-- Oracle 23ai (equivalent)
SELECT phrase, verb, 
       1 - (embedding <=> :1) as similarity
FROM agent.invocation_phrases
WHERE embedding IS NOT NULL
ORDER BY embedding <=> :1
FETCH APPROX FIRST 5 ROWS ONLY WITH TARGET ACCURACY 90;
```

Key changes:
- `$1::vector` → `:1` (Oracle bind parameter syntax)
- `LIMIT 5` → `FETCH APPROX FIRST 5 ROWS ONLY`
- `<=>` operator **unchanged**

### 5.3 Rust Code Changes

```rust
// Current (PostgreSQL)
let rows = sqlx::query(r#"
    SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
    FROM agent.invocation_phrases
    ORDER BY embedding <=> $1::vector
    LIMIT $2
"#)
.bind(&embedding_vec)
.bind(limit)
.fetch_all(&pool).await?;

// Oracle (with feature flag)
#[cfg(feature = "oracle")]
let rows = sqlx::query(r#"
    SELECT phrase, verb, 1 - (embedding <=> :1) as similarity
    FROM agent.invocation_phrases
    ORDER BY embedding <=> :1
    FETCH APPROX FIRST :2 ROWS ONLY
"#)
.bind(&embedding_vec)
.bind(limit)
.fetch_all(&pool).await?;
```

---

## 6. Oracle 23ai Native Model Support

**Bonus**: Oracle 23ai can load `all-MiniLM-L6-v2` directly via ONNX:

```sql
-- Load model into Oracle
BEGIN
    DBMS_VECTOR.LOAD_ONNX_MODEL(
        'all_minilm_l6_v2',
        '/path/to/all-MiniLM-L6-v2.onnx'
    );
END;
/

-- Generate embeddings in-database
SELECT VECTOR_EMBEDDING(all_minilm_l6_v2 USING 'create a fund' AS data)
FROM DUAL;
```

This could eliminate Candle entirely for Oracle deployments, though we'd lose the Rust performance benefits.

---

## 7. Recommended Architecture (Future-Proof)

If Oracle migration is likely, abstract the vector storage:

```rust
/// Database-agnostic vector store interface
#[async_trait]
pub trait VectorStore {
    /// Store embedding for a phrase
    async fn upsert_phrase(&self, phrase: &str, verb: &str, embedding: &[f32]) -> Result<()>;
    
    /// Search for similar phrases
    async fn search_similar(&self, embedding: &[f32], limit: usize, threshold: f32) 
        -> Result<Vec<PhraseMatch>>;
}

/// PostgreSQL implementation
pub struct PgVectorStore { pool: PgPool }

/// Oracle implementation  
pub struct OracleVectorStore { pool: OraclePool }
```

This adds ~1 day effort but provides clean database portability.

---

## 8. Recommendation

### 8.1 For ob-poc Development

**Proceed with PostgreSQL/pgvector.** Reasons:
- Faster development velocity
- pgvector is mature and well-documented
- PostgreSQL is approved at BNY
- Migration path to Oracle is clear and low-effort

### 8.2 For Production

**Evaluate based on enterprise requirements:**

| If... | Then... |
|-------|---------|
| PostgreSQL approved for production | Stay with pgvector |
| Oracle mandatory | Budget 2-3 days for migration |
| Hedge both options | Implement `VectorStore` trait now (+1 day) |

### 8.3 Risk Mitigation

- **Model weights are portable**: Candle uses same SafeTensors as Oracle ONNX import
- **Embeddings are portable**: 384-dim float vectors work in any database
- **Core query syntax compatible**: `<=>` operator works in both
- **Only infrastructure changes**: No application logic changes needed

---

## 9. Summary

| Question | Answer |
|----------|--------|
| Is Candle coupled to PostgreSQL? | **No** — pure Rust, database-agnostic |
| Is all-MiniLM-L6-v2 coupled to PostgreSQL? | **No** — standard SafeTensors |
| Are embeddings coupled to pgvector? | **No** — 384-dim floats work anywhere |
| Are queries coupled to pgvector? | **Low** — `<=>` works in Oracle 23ai |
| Can we migrate to Oracle? | **Yes** — 2-3 days effort |
| Should we abstract now? | **Optional** — add VectorStore trait if Oracle likely |

**Bottom line**: The semantic search architecture is **not tightly coupled** to PostgreSQL. The Candle embedder and model weights are completely portable; only the SQL layer has database-specific syntax, and Oracle 23ai is remarkably compatible.
