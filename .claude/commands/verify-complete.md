# Verify Implementation Complete (No Hidden Stubs)

Before declaring any implementation "complete" or "done", you MUST run this verification.

## Mandatory Grep Patterns

Run these searches on the affected files/modules:

```bash
# TODOs and FIXMEs
rg -n "TODO|FIXME" --glob "*.rs" rust/src/

# Rust explicit incomplete markers
rg -n "unimplemented!|todo!" --glob "*.rs" rust/src/

# Panic placeholders
rg -n 'panic!\("not yet|panic!\("not implemented' --glob "*.rs" rust/src/

# Comment-based stubs
rg -n "// stub|// placeholder|// pending|// mock|// fake" -i --glob "*.rs" rust/src/

# Hardcoded fake data patterns
rg -n '"https://example\.com|example\.com|placeholder|dummy_|fake_|mock_' --glob "*.rs" rust/src/
```

## Hollow Implementation Detection

Look for functions that:
1. Have proper signatures and return types
2. But return hardcoded/fake data instead of real logic
3. Contain comments like "in production this would..." or "integration pending"

Example of a HOLLOW STUB (looks complete but isn't):
```rust
async fn execute_web_search(&self, query: &str) -> Result<Value> {
    // For now, return a placeholder...  <-- RED FLAG
    Ok(serde_json::json!({
        "results": [{ "note": "Web search API integration pending" }]  // <-- FAKE
    }))
}
```

## Declaration Requirements

Before saying "complete", you MUST:

1. Run all grep patterns above
2. List any findings explicitly
3. For each finding, state:
   - Is it blocking? (stub in critical path)
   - Is it known technical debt? (document in TODO.md)
   - Is it out of scope? (explain why)

## Never Say "Complete" If:
- Any `unimplemented!()` or `todo!()` in the code path
- Placeholder data being returned as real results
- Comments containing "pending", "stub", or "placeholder"
- Hardcoded example.com URLs or dummy values

## What To Say Instead:
- "Implementation compiles and core logic is in place. Known gaps: [list]"
- "Handlers are wired up but X feature uses a stub for Y reason"
- "Ready for review with the following TODO items documented: [list]"

Explicit disclosure > silent victory declarations.
