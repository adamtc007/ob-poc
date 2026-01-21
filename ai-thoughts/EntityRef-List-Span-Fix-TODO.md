# CRITICAL: EntityRef List Item Unique ref_id Fix

**Created**: 2025-01-21  
**Priority**: **CRITICAL** — Lists of entities are the norm in KYC workflows  
**Blocking**: Issue K (commit-by-ref_id) broken for list items

---

## Status

| Item | Status |
|------|--------|
| Code changes to `enrichment.rs` | ✅ DONE (already in repo) |
| Test `test_list_items_have_unique_ref_ids` | ❌ TODO |

---

## Claude Code Prompt Header

> **Copy this section when starting Claude Code session:**

The code fix for list item unique ref_ids is **already applied** to `rust/src/dsl_v2/enrichment.rs`. Your task is to:

1. **Verify the code compiles**: `cargo check --lib -p ob-poc`
2. **Add the missing test** (provided below)
3. **Run tests**: `cargo test --lib enrichment`

**Hard constraints:**
- Do NOT modify the existing code changes — they are correct
- Only add the test
- All existing tests must pass

**At the end, provide:**
- `cargo test` output for enrichment tests
- Confirmation that list items have unique ref_ids

---

## Problem (Already Fixed)

When parsing DSL with a list of entity references:

```clojure
(batch.create :clients ["Allianz" "BlackRock" "Vanguard"])
;;                      ^-------- span 20-55 --------^
```

The enricher was creating 3 `EntityRef` nodes that ALL got `ref_id: "0:20-55"` (the list's span).

**Fix applied**: List items now get index suffix: `"0:20-55:0"`, `"0:20-55:1"`, `"0:20-55:2"`

---

## Code Changes (Already in Repo)

The following changes are already applied to `rust/src/dsl_v2/enrichment.rs`:

1. ✅ Added `list_index: Option<usize>` parameter to `enrich_node()`
2. ✅ Updated ref_id generation for String literals to include list index
3. ✅ Updated ref_id generation for UUID literals to include list index
4. ✅ Updated List processing to enumerate and pass index
5. ✅ Updated Map processing to pass `None` for list_index
6. ✅ Updated call site in `enrich_argument()` to pass `None`

---

## Test to Add

Add this test to `rust/src/dsl_v2/enrichment.rs` in the `#[cfg(test)]` module, after `test_enrich_unknown_verb`:

```rust
#[test]
fn test_list_items_have_unique_ref_ids() {
    let registry = test_registry();

    // Raw AST with list of 3 items that should become EntityRefs
    let raw = Program {
        statements: vec![Statement::VerbCall(VerbCall {
            domain: "cbu".to_string(),
            verb: "assign-role".to_string(),
            arguments: vec![
                Argument {
                    key: "cbu-id".to_string(),
                    value: AstNode::SymbolRef {
                        name: "fund".to_string(),
                        span: Span::default(),
                    },
                    span: Span::default(),
                },
                // entity-id as a list of 3 items
                Argument {
                    key: "entity-id".to_string(),
                    value: AstNode::List {
                        items: vec![
                            AstNode::Literal(Literal::String("Alice".to_string())),
                            AstNode::Literal(Literal::String("Bob".to_string())),
                            AstNode::Literal(Literal::String("Charlie".to_string())),
                        ],
                        span: Span { start: 20, end: 55 }, // Simulate real span
                    },
                    span: Span { start: 10, end: 56 },
                },
                Argument {
                    key: "role".to_string(),
                    value: AstNode::Literal(Literal::String("DIRECTOR".to_string())),
                    span: Span::default(),
                },
            ],
            binding: None,
            span: Span::default(),
        })],
    };

    let result = enrich_program(raw, &registry);

    if let Statement::VerbCall(vc) = &result.program.statements[0] {
        let entity_arg = vc.get_arg("entity-id").unwrap();
        if let AstNode::List { items, .. } = &entity_arg.value {
            assert_eq!(items.len(), 3);

            // Extract ref_ids
            let ref_ids: Vec<Option<String>> = items
                .iter()
                .map(|item| {
                    if let AstNode::EntityRef { ref_id, .. } = item {
                        ref_id.clone()
                    } else {
                        panic!("Expected EntityRef, got {:?}", item);
                    }
                })
                .collect();

            // All should have ref_ids
            assert!(
                ref_ids.iter().all(|r| r.is_some()),
                "All items should have ref_id"
            );

            // All should be UNIQUE
            let unique: std::collections::HashSet<_> = ref_ids.iter().collect();
            assert_eq!(
                unique.len(),
                3,
                "ref_ids should be unique: {:?}",
                ref_ids
            );

            // Verify format includes index suffix
            let r0 = ref_ids[0].as_ref().unwrap();
            let r1 = ref_ids[1].as_ref().unwrap();
            let r2 = ref_ids[2].as_ref().unwrap();

            assert!(
                r0.ends_with(":0"),
                "First item should end with :0, got {}",
                r0
            );
            assert!(
                r1.ends_with(":1"),
                "Second item should end with :1, got {}",
                r1
            );
            assert!(
                r2.ends_with(":2"),
                "Third item should end with :2, got {}",
                r2
            );
        } else {
            panic!("Expected List");
        }
    }
}
```

---

## Verification

After adding the test, run:

```bash
cd rust
cargo test --lib enrichment
```

Expected output should show all enrichment tests passing, including the new `test_list_items_have_unique_ref_ids`.

---

## Checklist

- [x] Add `list_index: Option<usize>` parameter to `enrich_node()` — **DONE**
- [x] Update ref_id generation for String literals — **DONE**
- [x] Update ref_id generation for UUID literals — **DONE**
- [x] Update List processing to enumerate and pass index — **DONE**
- [x] Update Map processing to pass `None` — **DONE**
- [x] Update call site in `enrich_argument()` — **DONE**
- [ ] Add `test_list_items_have_unique_ref_ids` test — **TODO**
- [ ] Verify all existing enrichment tests pass — **TODO**
