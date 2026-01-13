# TODO: Fix Stale Tantivy Index Returns Wrong Search Results

## Problem Summary

Entity disambiguation searches return stale/wrong results because Tantivy index readers aren't properly invalidated after refresh. The s-expression query system works correctly, but the underlying index serves outdated data.

## Root Cause

In `rust/crates/entity-gateway/src/index/tantivy_index.rs`, the `refresh()` method:
1. Deletes all documents
2. Adds new documents
3. Commits
4. Creates new reader

**But**: Tantivy readers are snapshot-based. Without proper reload policy and segment cleanup, old data persists.

---

## Fix 1: Update Reader Creation with Reload Policy

**File:** `rust/crates/entity-gateway/src/index/tantivy_index.rs`

**Location:** `refresh()` method, around line 480-520

**Current code:**
```rust
// Commit changes
writer
    .commit()
    .map_err(|e| IndexError::BuildFailed(e.to_string()))?;

// Create new reader
let new_reader = self
    .index
    .reader()
    .map_err(|e| IndexError::BuildFailed(e.to_string()))?;
```

**Replace with:**
```rust
// Commit changes
writer
    .commit()
    .map_err(|e| IndexError::BuildFailed(e.to_string()))?;

// Wait for merging threads to clean up deleted segments
// This ensures old documents are actually removed, not just marked deleted
if let Err(e) = writer.wait_merging_threads() {
    tracing::warn!(error = %e, "Merge threads warning (non-fatal)");
}

// Drop writer to release lock before creating reader
drop(writer);

// Create new reader with explicit reload policy
// OnCommitWithDelay reloads automatically after commits
let new_reader = self
    .index
    .reader_builder()
    .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
    .try_into()
    .map_err(|e: tantivy::TantivyError| IndexError::BuildFailed(e.to_string()))?;
```

**Add import at top of file:**
```rust
use tantivy::ReloadPolicy;
```

---

## Fix 2: Add Generation Tracking for Cache Validation

**File:** `rust/crates/entity-gateway/src/index/tantivy_index.rs`

**Add field to struct:**
```rust
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct TantivyIndex {
    // ... existing fields ...
    
    /// Generation counter - increments on each refresh
    generation: AtomicU64,
}
```

**Update `new()` constructor:**
```rust
Ok(Self {
    // ... existing fields ...
    generation: AtomicU64::new(0),
})
```

**Update `refresh()` to increment generation:**
```rust
// After updating the reader, increment generation
*self.reader.write().await = Some(new_reader);
self.generation.fetch_add(1, Ordering::SeqCst);
self.ready.store(true, Ordering::SeqCst);
```

**Add getter method:**
```rust
/// Get current index generation (increments on each refresh)
pub fn generation(&self) -> u64 {
    self.generation.load(Ordering::SeqCst)
}
```

---

## Fix 3: Add Force Reload Method

**File:** `rust/crates/entity-gateway/src/index/tantivy_index.rs`

**Add method:**
```rust
/// Force the reader to reload and see latest committed changes
/// Call this if you suspect stale results
pub async fn force_reload(&self) -> Result<(), IndexError> {
    let reader_guard = self.reader.read().await;
    if let Some(reader) = reader_guard.as_ref() {
        reader
            .reload()
            .map_err(|e| IndexError::BuildFailed(format!("Reload failed: {}", e)))?;
        tracing::info!(nickname = %self.config.nickname, "Forced reader reload");
    }
    Ok(())
}
```

**Add to `SearchIndex` trait in `rust/crates/entity-gateway/src/index/traits.rs`:**
```rust
#[async_trait]
pub trait SearchIndex: Send + Sync {
    // ... existing methods ...
    
    /// Force reload the index reader
    async fn force_reload(&self) -> Result<(), IndexError> {
        // Default no-op for indexes that don't support it
        Ok(())
    }
}
```

---

## Fix 4: Add Refresh API Endpoint

**File:** `rust/crates/entity-gateway/src/server/grpc.rs`

**Add new RPC method to proto first.**

**File:** `rust/crates/entity-gateway/proto/entity_gateway.proto`

Add to service definition:
```protobuf
service EntityGateway {
    rpc Search(SearchRequest) returns (SearchResponse);
    
    // New: Force refresh a specific entity index
    rpc RefreshIndex(RefreshIndexRequest) returns (RefreshIndexResponse);
}

message RefreshIndexRequest {
    string nickname = 1;
}

message RefreshIndexResponse {
    bool success = 1;
    uint64 generation = 2;
    string message = 3;
}
```

**Then implement in `grpc.rs`:**
```rust
async fn refresh_index(
    &self,
    request: Request<RefreshIndexRequest>,
) -> Result<Response<RefreshIndexResponse>, Status> {
    let req = request.into_inner();
    
    let index = self.registry.get(&req.nickname).await
        .ok_or_else(|| Status::not_found(format!("Unknown entity: {}", req.nickname)))?;
    
    index.force_reload().await
        .map_err(|e| Status::internal(format!("Reload failed: {}", e)))?;
    
    Ok(Response::new(RefreshIndexResponse {
        success: true,
        generation: 0, // TODO: get from index if generation tracking added
        message: format!("Index {} reloaded", req.nickname),
    }))
}
```

---

## Fix 5: Add Debug Logging to Search

**File:** `rust/crates/entity-gateway/src/index/tantivy_index.rs`

**In `search()` method, add at start:**
```rust
async fn search(&self, query: &SearchQuery) -> Vec<SearchMatch> {
    let generation = self.generation.load(Ordering::SeqCst);
    
    tracing::debug!(
        nickname = %self.config.nickname,
        search_key = %query.search_key,
        values = ?query.values,
        mode = ?query.mode,
        generation = generation,
        "Starting search"
    );
    
    let reader_guard = self.reader.read().await;
    let reader = match reader_guard.as_ref() {
        Some(r) => r,
        None => {
            tracing::warn!(nickname = %self.config.nickname, "No reader available");
            return vec![];
        }
    };

    let searcher = reader.searcher();
    
    // Add segment debugging
    tracing::debug!(
        nickname = %self.config.nickname,
        num_docs = searcher.num_docs(),
        num_segments = searcher.segment_readers().len(),
        generation = generation,
        "Searcher ready"
    );
    
    // ... rest of search logic
}
```

---

## Fix 6: Verify Disambiguation Flow Triggers

**File:** `rust/src/api/agent_service.rs`

Search for where `DisambiguationRequest` is created. Ensure that when EntityGateway returns multiple matches with similar scores, the agent service creates a disambiguation request.

**Look for pattern like:**
```rust
// When search returns multiple high-confidence matches
if matches.len() > 1 && matches[0].score - matches[1].score < 0.1 {
    // Should create DisambiguationRequest
    return AgentResponse {
        disambiguation: Some(DisambiguationRequest {
            request_id: Uuid::new_v4(),
            items: vec![DisambiguationItem::EntityMatch {
                param: param_name,
                search_text: query,
                matches: matches.into_iter().map(to_entity_match_option).collect(),
            }],
            prompt: format!("Multiple matches found for '{}'. Please select:", query),
        }),
        // ...
    };
}
```

**If this logic is missing, add it.**

---

## Testing Checklist

After implementing fixes:

1. [ ] Start entity-gateway with `RUST_LOG=entity_gateway=debug`
2. [ ] Add a new entity via the main app
3. [ ] Immediately search for it - should find it (tests refresh)
4. [ ] Check logs for generation number incrementing
5. [ ] Check segment count doesn't grow unbounded after multiple refreshes
6. [ ] Test disambiguation modal appears when searching ambiguous names

---

## Files Modified

| File | Changes |
|------|---------|
| `rust/crates/entity-gateway/src/index/tantivy_index.rs` | ReloadPolicy, generation tracking, force_reload, debug logging |
| `rust/crates/entity-gateway/src/index/traits.rs` | Add `force_reload()` to trait |
| `rust/crates/entity-gateway/proto/entity_gateway.proto` | Add RefreshIndex RPC |
| `rust/crates/entity-gateway/src/server/grpc.rs` | Implement RefreshIndex |
| `rust/src/api/agent_service.rs` | Verify disambiguation trigger logic |

---

## Quick Verification

Run this after fixes to verify reader is fresh:

```bash
# Terminal 1: Watch entity-gateway logs
RUST_LOG=entity_gateway=debug cargo run -p entity-gateway

# Terminal 2: Trigger a search, check generation number in logs
grpcurl -plaintext -d '{"nickname": "CBU", "values": ["test"], "mode": 0}' \
  localhost:50051 ob.gateway.v1.EntityGateway/Search
```

Generation should increment after each refresh cycle. Segment count should stay stable (not grow indefinitely).
