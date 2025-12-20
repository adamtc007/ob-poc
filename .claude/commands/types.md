# Type System & API Boundaries

ob-poc-types crate is the SINGLE SOURCE OF TRUTH for all API types.

## Rules
1. All API types live in ob-poc-types - no inline structs in handlers
2. Server wins - UI types match server, not the other way around
3. Use #[derive(TS)] for TypeScript generation
4. Tagged enums only: #[serde(tag = "type")]
5. UUIDs as Strings in API types for TypeScript compatibility

## Workflow After Changing Types
```bash
cd rust/
cargo test --package ob-poc-types export_bindings
cp rust/crates/ob-poc-types/bindings/*.ts rust/crates/ob-poc-web/static/ts/generated/
cd rust/crates/ob-poc-web/static && npx eslint ts/generated/*.ts --fix
```

Or use xtask:
```bash
cargo x ts-bindings
```

## Key Files
- rust/crates/ob-poc-types/src/lib.rs - All shared types
- rust/crates/ob-poc-types/bindings/*.ts - Generated TypeScript

## Type Boundaries
```
Rust Server (Axum) ←JSON→ TypeScript (HTML panels)
       ↓
      JSON
       ↓
Rust WASM (Graph) ←CustomEvent→ TypeScript (just entity IDs)
```

Read CLAUDE.md section "Shared Types Crate (ob-poc-types)".
