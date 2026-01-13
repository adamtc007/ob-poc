# Refactoring Strategy

For large files requiring structural changes, choose the right strategy.

## Decision Matrix

| Scenario | Strategy |
|----------|----------|
| Small fix (<50 lines changed) | Incremental edit |
| Adding new function/method | Incremental edit |
| Renaming/moving (<100 lines) | Incremental edit |
| **File >500 lines, >30% changing** | **Rip and replace** |
| **Architectural refactor** | **Rip and replace** |
| **Changing data flow patterns** | **Rip and replace** |
| **Unifying two parallel systems** | **Rip and replace** |

## Rip and Replace Process

1. **Document the contract** - What API/behavior must be preserved?
2. **Identify all consumers** - grep for imports/usages
3. **Write fresh implementation** - new file or clear old one completely
4. **DELETE old code** - no commenting out, actual deletion
5. **cargo check** - let compiler find all breakage
6. **Fix errors** - compiler guides cleanup

## Why LLMs Do Better With Rip and Replace

```
WEAK: "Edit lines 47, 123, 256, 312 of this 800-line file"
      → Context drift, partial states, subtle bugs

STRONG: "Write a 200-line file that implements this contract"
        → Clean slate, clear target, atomic change
```

## Anti-Pattern: Death by 1000 Edits

```
Edit line 50... done
Edit line 120... done  
Edit line 200... done
(context window filling up)
Edit line 350... wait, what was line 50 again?
```

## Correct Pattern

```
Here's the new 200-line file replacing the old 800-line file.
Same API contract. Delete the old file.
cargo check → fix errors → done.
```

## Rust Advantage

Rust's compiler makes rip-and-replace safe:
- Orphaned imports are compile errors
- Missing trait impls are compile errors
- Type mismatches are compile errors
- "Forgot to update callsite" is impossible

## When NOT to Rip and Replace

- Adding (not changing) functionality
- Bug fixes in isolated functions
- Well-structured files where only one section needs work
- Changes affecting <100 lines

## Example: Unifying Resolution Systems

**BAD approach:**
```
Edit resolution_service.rs line 50...
Edit resolution_service.rs line 200...
Edit resolution_routes.rs line 30...
(1500 lines of edits across files)
```

**GOOD approach:**
```
1. Add methods to ResolutionSubSession (target exists)
2. Write new resolution_routes.rs from scratch (200 lines)
3. DELETE resolution_service.rs entirely (1500 lines gone)
4. cargo check → fix 5 import errors → done
```

Net result: -1300 lines, cleaner architecture, 1 hour not 1 day.
