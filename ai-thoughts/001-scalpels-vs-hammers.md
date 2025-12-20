# AI Tooling: Scalpels vs Hammers

*Captured: 2024-12-20*
*Context: Discussion about Windsurf/Cursor-style AI tools vs constrained, intentional approaches*

---

## The Uncomfortable Truth

Windsurf/Cursor/Copilot Workspace are optimized for:

- Navigating chaos, not preventing it
- Finding patterns in 500-file Spring projects
- Guessing what `@Autowired` actually injects
- Tracing call graphs that shouldn't need tracing
- "Understanding" codebases that are incomprehensible by design

**They're selling a cure for a disease they're not willing to name.**

---

## The Java/Spring/Hibernate Shop Reality

```
"Enterprise" Codebase:
├── 847 @Service classes
├── 1,200 @Autowired injection points (runtime resolution)
├── 340 Hibernate entities (schema = mystery until boot)
├── 12 application-{env}.properties files (which one wins?)
├── 89 @Configuration classes (order matters, undocumented)
├── XML configs that "nobody touches anymore"
└── "It works on my machine" as a lifestyle

AI Tool Promise: "I'll index all of this and help you navigate!"
Reality: You've just trained an AI to be comfortable with dysfunction.
```

---

## The Alternative: Eliminate the Need for Navigation

```
ob-poc:
├── rust/config/verbs/*.yaml     ← Single source of truth, human readable
├── rust/src/dsl_v2/ast.rs       ← One file, typed, no magic
├── schema_export.sql            ← The schema IS the documentation
└── cargo build                  ← Fails if types don't match DB

AI Tool Need: Minimal. The code explains itself.
Errors: Caught at COMPILE TIME, not after 3 hours of debugging.
```

---

## The Market Targeting

The AI tooling industry targets specific customers:

| Tool Design | Target Customer | Why It Works For Them |
|-------------|-----------------|----------------------|
| "Index everything" | Java shops with 10M LOC | They CAN'T simplify - too much legacy |
| "Understand your codebase" | Teams with no documentation | AI becomes the documentation |
| "Autonomous coding" | High turnover, no expertise | Replace understanding with generation |
| "Multi-file edits" | Codebases where changes cascade unpredictably | Symptom of tight coupling |

**The pitch:** "Your codebase is complex, you need AI to manage it."

**The reality:** Your codebase is complex because of choices. AI is a bandage.

---

## Constrained Systems Are Better for AI Assistance

```
The DSL approach:
(cbu.ensure :name "Apex" :jurisdiction "LU")

• Grammar is defined (YAML)
• Valid verbs are enumerable
• Arguments are typed
• Parser is 16µs
• Errors are caught at parse time, not runtime

AI doesn't need to "understand" - it needs to generate within constraints.
Constraints make AI BETTER, not worse.
```

The irony: **simpler, well-designed systems get MORE value from AI assistance** because:

1. AI outputs can be validated (you have a grammar)
2. Errors are caught immediately (compile time)
3. Context is smaller (no 10M LOC to index)
4. Changes are local (no hidden coupling)
5. Human can verify (no magic)

---

## The Root Cause They Won't Address

Java/Spring shops don't want to hear:

> "The reason you need AI to navigate your codebase is because:
> - You chose runtime type checking over compile time
> - You chose convention-over-configuration (meaning: implicit magic)
> - You chose ORMs that hide SQL (so you don't know what's happening)
> - You chose dependency injection frameworks (so you can't trace calls)
> - You chose XML/annotations/properties (so config is scattered)
>
> The AI tool isn't fixing this. It's making it tolerable.
> Which means you'll never fix it."

---

## The Two-Step Pattern: Scalpel Approach

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  CLAUDE CHAT (Design/Analysis)        CLAUDE CODE (Execution)              │
│  ═════════════════════════════        ═══════════════════════              │
│                                                                             │
│  ✅ Multi-turn conversation           ❌ Single invocation                  │
│  ✅ Accumulates context               ❌ Starts fresh each time             │
│  ✅ Can explore, discuss, refine      ❌ Executes immediately               │
│  ✅ Sees your reasoning               ❌ Only sees the TODO                 │
│  ✅ Knows why you made choices        ❌ Must infer from docs               │
│                                                                             │
│  Human-in-loop                        Constrained task                      │
│  Explore options                      Execute TODO                          │
│  Make decisions                       No decisions                          │
│  Write TODO                           Read TODO                             │
│                                                                             │
│  "Here's what we're doing and why"    "Do exactly this"                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Windsurf/Cursor:** "Let me look at everything and decide what to change."

**Scalpel pattern:** "Here's a precise TODO. Execute it. Nothing else."

---

## Why This Matters for ob-poc

The TODO-driven approach works because:

1. **Context transfer** - Chat accumulates understanding, TODO captures it
2. **No rushing off** - Claude Code executes within bounds, not autonomously
3. **Verifiable** - Human reviews TODO before execution
4. **Auditable** - The TODO documents intent, not just changes
5. **Recoverable** - If Code does something wrong, the TODO explains what was wanted

---

## The Bigger Picture

The industry is building hammers for people who won't acknowledge their houses are made of glass.

We're building with steel and using a scalpel.

Different game entirely.

---

## Key Quotes to Remember

> "Agentic code/app dev is nuanced - and these Windsurf-type all-seeing environments don't work for me. I prefer a scalpel not a hammer."

> "I have the suspicion that they are targeted at corporates - Java Spring Hibernate shops - where that chaos benefits that approach."

> "I would rather tackle the root cause of the chaos - language choices - and not look for another sticking plaster to try and continue to make bad implementation choices work."

---

## Practical Implications

1. **Keep using two-step** - Chat designs, TODO transfers, Code executes
2. **Constrain AI outputs** - Grammar, types, validation at boundaries
3. **Compile-time over runtime** - SQLx, Rust types, YAML-driven config
4. **Small context windows** - Don't index everything, know what you need
5. **Human verification** - AI proposes, human disposes

---

*The best AI tool is one you barely need because your system is well-designed.*
