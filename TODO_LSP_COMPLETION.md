# LSP Database Completion - Debug Session TODO

## Current Status
The LSP is working for:
- Syntax highlighting (via clojure grammar)
- Verb name completion
- Keyword completion
- Symbol reference completion (@symbol)
- Static value completion (roles, jurisdictions, etc.)

## NOT Working
- Database fuzzy lookup for `:cbu-id` and `:entity-id` values

## Problem
When typing `(cbu.assign-role :cbu-id "Ap` the context detection returns `None` instead of `KeywordValue { keyword: "cbu-id", prefix: "Ap", in_string: true }`.

**Unit tests pass** - the context detection works correctly in tests.
**But in Zed** - completion context is `None`.

## Debugging Done
1. Added file-based logging to `/tmp/dsl-lsp.log`
2. Database connection works and is verified
3. Context detection tests all pass
4. Added debug logging to see actual line/col/prefix being analyzed

## Next Steps

### 1. Restart servers and check debug logs
After restarting Zed's language servers, open `examples/completion_test.dsl` and check `/tmp/dsl-lsp.log` for:
```
Context detection: line=X, col=Y, prefix='...'
```
This will show what line content the LSP is actually seeing.

### 2. Possible issues to investigate
- **Incremental sync**: Document state might not be updating correctly with incremental changes
- **Position offset**: Zed might be sending different character positions than expected
- **Document not stored**: The document might not be in the `documents` HashMap when completion is triggered

### 3. Key files
- `rust/crates/dsl-lsp/src/analysis/context.rs` - Context detection logic
- `rust/crates/dsl-lsp/src/handlers/completion.rs` - Completion handler with DB lookups
- `rust/crates/dsl-lsp/src/server.rs` - LSP server with document state
- `rust/crates/dsl-lsp/src/analysis/document.rs` - DocumentState implementation

### 4. Test command
```bash
cd rust && cargo test -p dsl-lsp context -- --nocapture
```

### 5. Check logs
```bash
tail -f /tmp/dsl-lsp.log
```

## Requirements for DB completion
- Type at least 2 characters after `:cbu-id "` or `:entity-id "`
- Example: `(cbu.assign-role :cbu-id "Ap` should show "Apex Capital Partners"

## Relevant CBUs in database
```
Apex Capital Partners
Global Alpha Master Fund
Europa Equity UCITS
Global Asset Management
```
