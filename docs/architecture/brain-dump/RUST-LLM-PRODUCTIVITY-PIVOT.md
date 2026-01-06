# The Rust + LLM Productivity Pivot

## Date: 2025-01-06
## Context: Reflections on 9 months of agentic development with Claude

---

## The Shift

**9 months ago:**
```
You: [thinks] → [types code] → [compiler error] → [fix] → [error] → [fix] → ✓
     |___________________________|
            Your bottleneck
```

**Now:**
```
You: [thinks] → [describes intent]
Claude: [types code] → [compiler error] → [reads error] → [fixes] → [error] → [fixes] → ✓
        |_______________________________________________|
                   Claude's bottleneck (fast)
```

You've moved from **implementer** to **architect/director**. The typing and compiler wrestling - which used to consume 70% of your time - is now Claude's problem. And Claude doesn't get tired, doesn't fat-finger, doesn't lose context.

---

## Division of Labor

### What you bring that Claude can't:

- 15 years of BNY domain knowledge
- Understanding of what the business actually needs
- Judgment on trade-offs (speed vs correctness, now vs later)
- Recognition of when Claude is confidently wrong
- The "that feels off" instinct

### What Claude brings that you couldn't match:

- Types 1000 WPM with zero typos
- Holds entire codebase in context
- Never forgets the function signature from 500 lines ago
- Reads compiler errors without emotional frustration
- Tries 10 approaches in the time you'd try 1

---

## Why Rust + LLM is Disproportionately Powerful

The "very picky" Rust compiler used to be friction for *you*. Now it's quality control on *Claude's* output. Same strictness, but you're no longer the one paying the tax.

### The Feedback Loop

**Java cycle:**
```
LLM writes code
    ↓
Compiles ✓
    ↓
Run tests... maybe pass
    ↓
Deploy to dev
    ↓
Runtime exception at edge case
    ↓
Stack trace points to symptom, not cause
    ↓
Debug for 30 mins
    ↓
Find the actual bug
    ↓
Ask LLM to fix
    ↓
LLM introduces different bug
    ↓
Repeat
```

**Rust cycle:**
```
LLM writes code
    ↓
Compile error: "borrowed value does not live long enough"
    ↓
Error shows exact line, expected lifetime, actual lifetime
    ↓
hint: consider using `clone()` here
    ↓
LLM reads error, applies fix
    ↓
Compiles ✓
    ↓
If it compiles, it probably works
```

### Rust Errors are Structured Data, Not Prose

```
error[E0382]: borrow of moved value: `x`
 --> src/main.rs:4:20
  |
2 |     let x = vec![1, 2, 3];
  |         - move occurs because `x` has type `Vec<i32>`
3 |     let y = x;
  |             - value moved here
4 |     println!("{:?}", x);
  |                      ^ value borrowed here after move
  |
help: consider cloning the value
  |
3 |     let y = x.clone();
  |              ++++++++
```

LLM can parse that mechanically:
- **What failed**: borrow after move
- **Where**: line 4, column 20
- **Why**: moved on line 3
- **Fix**: add `.clone()`

No interpretation needed. No "maybe it's this, maybe it's that." Deterministic.

### Clippy Amplifies This

```
warning: this `if` has identical blocks
 --> src/main.rs:10:5
  |
  = help: for further information visit https://rust-lang.github.io/rust-clippy/...

warning: redundant clone
 --> src/main.rs:15:10
  |
  = note: this value is dropped without further use
  = help: remove this clone
```

LLM sees "redundant clone" + "remove this clone" → does exactly that. No ambiguity.

---

## The Productivity Multiplier

| Metric | Java + LLM | Rust + LLM |
|--------|------------|------------|
| Time to first compile | Fast | Slower |
| Time to correct code | Slow (runtime discovery) | Fast (compile-time discovery) |
| LLM iterations to fix | 3-5 (vague errors) | 1-2 (precise errors) |
| Confidence when it compiles | Low | High |

You pay upfront with Rust's strictness, but LLM + compiler iterate faster than LLM + human debugging.

---

## Why Java Struggles with LLMs

The conventional take is "LLMs are better at Python/Java because more training data." But that's measuring the wrong thing. The question isn't *can* the LLM write valid code - it's *will* the LLM write the code you actually want.

### Java's Problem is Variance

```
Java solution space for "store user in database":
├── Spring Data JPA
│   ├── Repository interface
│   ├── @Query annotation
│   └── Specification API
├── Hibernate direct
│   ├── Session API
│   ├── Criteria API
│   └── HQL
├── JDBC Template
├── jOOQ
├── MyBatis
└── Each with:
    ├── Java 8 style
    ├── Java 17 style
    ├── Java 21 style
    ├── Reactive variant
    └── Your team's conventions
```

LLM picks one. 80% chance it's not the one you use. Now you're debugging style mismatches instead of building features.

### Rust's Advantage is Constraint

```
Rust solution space for "store user in database":
├── sqlx (compile-time checked)
└── That's basically it for your use case
    └── One way to write it
        └── Compiler enforces it
```

LLM writes it. Compiler checks it. Either it works or you get a precise error. No "works but wrong pattern" failure mode.

---

## The Null Pointer Microcosm

| Language | Null handling |
|----------|---------------|
| Java | Every reference might be null. `Optional` is advisory. Runtime NPE. |
| Kotlin | `?` helps but interop with Java reintroduces nulls |
| Rust | `Option<T>` is the ONLY way. Compiler refuses to let you unwrap without handling `None`. |

When LLM writes Rust and forgets a null check, **it doesn't compile**. 

When LLM writes Java and forgets a null check, it compiles fine and explodes at 3am in production.

**The hidden uplift**: Rust's compiler is basically a free code reviewer that catches LLM mistakes before you even run the code. Java's compiler waves everything through and wishes you luck.

---

## Why DSL + Formal Grammars Work Well with LLMs

Constrained briefs produce good results. The failure modes with agentic coding are:

| Problem | Result |
|---------|--------|
| Vague brief | Claude explores, backtracks, invents requirements |
| Multiple valid approaches | Claude picks one, you wanted another |
| Implicit domain knowledge | Claude hallucinates plausible-sounding nonsense |
| Underspecified boundaries | Scope creep, touching files it shouldn't |

DSL with formal grammar hits the sweet spot:
- **Constrained grammar** - lookup tables define valid symbols
- **Clear data flow** - one direction, no ambiguity
- **Single source of truth** - the doc, nothing else
- **Well-defined schema** - types already exist
- **Explicit lifecycle** - states are enumerated

Claude can't really go wrong because the rails are laid.

---

## Summary

This is the difference between "build me something" and "wire these specific pieces together this specific way."

The pair programming model works when:
1. **You** provide domain knowledge, intent, and judgment
2. **Claude** provides typing speed, context retention, and iteration
3. **Rust compiler** provides correctness verification
4. **Clippy** provides style and optimization hints

The compiler's pickiness is a feature when someone else is doing the typing.
