# Architecture Brain Dump: Intent-Driven Onboarding

> **Created:** 2026-01-11
> **Context:** Why Rust + Deterministic DSL + LLM beats Java Spring + ORM for custody bank onboarding
> **Audience:** Solution architects, product owners, compliance/KYC ops, enterprise architects

---

## 1. The Core Problem: Intent Mismatch

### What Business Wants to Say

```
"If the fund is trading OTC derivatives under Prime X, 
 we need ISDA/CSA + LEIs + collateral setup."

"If securities lending is enabled, ensure borrower eligibility, 
 tax documentation, and SFTR reporting readiness."

"If a change occurs (new UBO, new manager), rerun the relevant 
 checks and re-provision only what changed."
```

That's **policy + workflow + evidence + dependencies**, not CRUD.

### What Java Spring/ORM Becomes

```
Controllers → DTOs → Validation Annotations
     ↓
Services → Transaction Boundaries → Implicit Semantics
     ↓
ORM Entities → Persistence Quirks → Lazy Loading
     ↓
Mapping Layers → Glue Code → Runtime Proxies
     ↓
Integration Logic → Non-Deterministic Replay
```

**Result:** "Stiffware" — business intent diluted into framework plumbing.

---

## 2. Why Custody Banking Amplifies This

Custody banks have a unique multiplier:

| Dimension | Complexity |
|-----------|------------|
| **Entities** | Funds, managers, SPVs, corporates |
| **Roles** | IM, prime broker, custodian, administrator, distributor |
| **Products** | Custody, collateral, sec lending, execution, prime, fund accounting, TA |
| **Regimes** | MiFIR/EMIR/SFTR/CFTC/ASIC/HKTR; LEI perimeters |
| **Evidence** | KYC packs, ISDA/CSA, mandates, resolutions, tax forms |
| **Lifecycle** | Client changes, periodic reviews, UBO refresh, remediation |

Each onboarding is a **bundle** of services, jurisdictions, and reporting perimeters.

A single change can affect multiple downstream activations.

"Done" = **operational readiness**, not one system's DB row existing.

---

## 3. The Alternative: DSL-as-State

### Architecture Principle

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   Business Intent  ═══════════════════════════════════════╗     │
│        │                                                  ║     │
│        ▼                                                  ║     │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  ║     │
│   │ DSL Verbs   │───▶│ Exec Plan   │───▶│ State Trans │  ║     │
│   │ (runbook)   │    │ (DAG)       │    │ (audit)     │  ║     │
│   └─────────────┘    └─────────────┘    └─────────────┘  ║     │
│        │                                       │         ║     │
│        │              SAME ARTIFACT            │         ║     │
│        ╚═══════════════════════════════════════╩═════════╝     │
│                                                                 │
│   • Product language                                            │
│   • Operational checklist                                       │
│   • Audit trail                                                 │
│   • Test oracle                                                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### What Changes

| Before (Spring/ORM) | After (DSL-as-State) |
|---------------------|----------------------|
| Meaning spread across layers | Meaning in versioned runbook |
| Correctness by convention | Correctness by replay |
| Audit via logging frameworks | Audit native to artifact |
| SMEs need engineering help | SMEs read runbook directly |

---

## 4. Concrete Examples

### Example A: "No LEI, No Trade" (MiFIR)

**Intent:** If execution/prime trading enabled under EU/UK, block unless LEI valid.

**Spring/ORM:**
- Scattered across validation, product activation, trade routing, exception handling
- Changes require multi-layer edits + integration testing

**DSL:**
```
lei.verify-status entity=$fund
execution.enable  # ← gate: requires lei.status == ACTIVE
```
- Replayable: run sheet, confirm typed auditable halt reason

---

### Example B: SFTR Securities Lending Readiness

**Intent:** If sec lending enabled → enforce issuer IDs, SFT reporting, evidence retention.

**Spring/ORM:**
- Integration with reporting services, data model changes
- Brittle cross-service coordination

**DSL:**
```
sec-lending.enable fund=$fund
  # Expands to dependency DAG:
  #   lei.verify (counterparties)
  #   issuer.identifier.check
  #   sftr.reporting.enable
  #   evidence.attach policy-docs
```
- Auditable "not ready because X"

---

### Example C: UBO Refresh on Investor Change

**Intent:** If major investor changes / threshold crossed → refresh UBO, re-run KYC.

**Spring/ORM:**
- Event bus + workflow engine + state reconcilers

**DSL:**
```
investor-register.snapshot fund=$fund
ownership.derive-from-register
ubo.refresh-from-snapshots
kyc.revalidate
```
- Runbook changes explicit, replayable, testable

---

## 5. Why Rust + SQLx (Not Just Performance)

### Rust = Guardrails

| Property | Benefit |
|----------|---------|
| Strong typing | Reduces ambiguous states |
| Explicitness | No hidden framework behavior |
| SQLx compile-time checks | No runtime schema surprises |
| No reflection/proxies | What you see is what runs |

### LLM Productivity Amplified

LLMs work best when:
- Surface area is constrained
- Correctness validated quickly (compile/test)
- Intent is explicit (schemas, verb dictionaries)
- Outputs are replayable (DSL scenario tests)

**Workflow:**
```
Add verb → Adjust schema → Implement handler → Add scenario test → Replay
```

In Spring/ORM, LLM operates in diffuse semantic environment → more boilerplate, subtle regressions.

---

## 6. Economic Fit for Custody Banking

Onboarding characteristics:

| Metric | Value |
|--------|-------|
| Transaction volume | Low-to-medium |
| Value per transaction | **High** |
| Cost of errors | **High** |
| Evidence requirements | **Heavy** |
| Policy churn | **Continuous** |

Optimize for:
- ✅ Accuracy
- ✅ Flexibility under change
- ✅ Replayability and audit
- ✅ Controlled evolution

**NOT** for:
- ❌ Raw throughput
- ❌ CRUD scaffolding speed

---

## 7. Comparison Table

| Dimension | Rust + Deterministic DSL | Java Spring + ORM |
|-----------|--------------------------|-------------------|
| Intent → implementation distance | **Low** (verb-level diffs) | High (multi-layer) |
| Auditability | **Native** (runbook replay) | Additional frameworks |
| Change velocity under policy churn | **High** | Medium/low |
| Runtime surprises | **Low** (typed, explicit) | Higher (ORM + reflection) |
| LLM productivity | **High** (bounded + replay) | Lower (diffuse semantics) |
| Best fit | **Regulated orchestration** | Stable CRUD domains |

---

## 8. Crisp Claims for Stakeholders

| # | Claim |
|---|-------|
| 1 | **Reduce intent mismatch** — Onboarding intent encoded directly as versioned runbooks |
| 2 | **Auditability by construction** — Runbook IS the audit trail AND state declaration |
| 3 | **Ship changes safely under churn** — Rust/SQLx guardrails + deterministic scenarios |
| 4 | **Reliable LLM productivity** — Bounded verb universe + schemas + replay = controlled workflow |
| 5 | **Aligned to custody economics** — Accuracy and flexibility trump throughput |
| 6 | **Refactoring is the real cost** — Compiler-guided changes vs. grep-and-hope; 10x TCO difference |

---

## 9. The Economics: Money Talks

### The Numbers They Can't Ignore

**Traditional Java Spring + ORM Onboarding Platform:**

| Item | Cost |
|------|------|
| Initial build (5-10 devs × 18 months) | $2.5M - $5M |
| Annual maintenance (3-5 devs) | $600K - $1M/year |
| Major regulatory change (EMIR Refit-scale) | $500K - $1M each |
| 5-year TCO | **$8M - $15M** |

**Rust + DSL + LLM (what we built):**

| Item | Cost |
|------|------|
| Initial build (1 architect + LLM × 4 months) | $80K - $150K |
| Annual maintenance (same person + LLM) | $50K - $100K/year |
| Major regulatory change | $20K - $50K each |
| 5-year TCO | **$400K - $800K** |

**Ratio: 10-20x cost difference over 5 years.**

### Why the Difference Is Real

**Java Spring + ORM cost drivers:**
- Coordination overhead (5-10 people = meetings, PRs, merge conflicts)
- Regression hunting (implicit behavior, runtime surprises)
- Integration testing (slow, expensive, incomplete)
- Knowledge silos ("ask Dave, he wrote that service")
- Framework churn (Spring Boot 2→3, Java 11→17→21)
- ORM migrations (Hibernate quirks, lazy loading bugs)

**Rust + DSL + LLM cost drivers:**
- One person who understands the whole system
- Compiler finds regressions instantly
- Deterministic replay = cheap verification
- LLM operates on bounded, schema-validated surface
- No framework churn (stable Rust, explicit SQLx)

### The Refactoring Multiplier

```
┌─────────────────────────────────────────────────────────────────┐
│           COST OF A "MEDIUM" CHANGE (e.g., add SFTR field)      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Java Spring + ORM:                                             │
│  ├── Impact analysis:           2-5 days    ($2K-$5K)          │
│  ├── Implementation:            2-4 weeks   ($10K-$20K)        │
│  ├── Code review / coordination: 1 week     ($3K-$5K)          │
│  ├── Integration testing:       2-3 weeks   ($8K-$15K)         │
│  ├── Regression fixes:          1-2 weeks   ($5K-$10K)         │
│  ├── UAT / sign-off:            1-2 weeks   ($3K-$5K)          │
│  └── TOTAL:                     8-16 weeks  ($31K-$60K)        │
│                                                                 │
│  Rust + DSL + LLM:                                              │
│  ├── Edit verb YAML + schema:   2-4 hours   ($200-$400)        │
│  ├── Compiler shows break sites: instant    ($0)               │
│  ├── Fix each site (guided):    1-2 days    ($500-$1K)         │
│  ├── Replay scenarios:          1-2 hours   ($100-$200)        │
│  ├── Review + ship:             2-4 hours   ($200-$400)        │
│  └── TOTAL:                     2-4 days    ($1K-$2K)          │
│                                                                 │
│  RATIO: 15-30x cheaper per change                               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Regulatory Change Frequency

In custody banking, "medium" changes happen **constantly**:

| Year | Example Changes |
|------|----------------|
| 2024 | EMIR Refit, DORA, MiCA transition |
| 2025 | T+1 settlement (US/Canada), SFTR amendments |
| 2026 | Basel III endgame, UK/EU divergence |
| 2027+ | More of the same, forever |

**At 10-20 regulatory changes per year:**
- Java: 10 × $40K = **$400K/year** just for changes
- Rust+DSL: 10 × $1.5K = **$15K/year** for same changes

### The Headcount Reality

**Java team for equivalent scope:**
```
2 × Senior Java devs        @ $180K = $360K
2 × Mid Java devs           @ $140K = $280K  
1 × Tech lead               @ $200K = $200K
1 × DevOps                  @ $160K = $160K
1 × QA                      @ $120K = $120K
0.5 × Architect oversight   @ $220K = $110K
─────────────────────────────────────────────
Annual team cost:                    $1.23M
```

**Rust + DSL + LLM:**
```
1 × Solution Architect + Claude subscription
                              @ $220K + $2K = $222K
```

**Ratio: 5.5x headcount cost.**

### What Management Actually Cares About

| Metric | Java Spring | Rust + DSL + LLM |
|--------|-------------|------------------|
| Time to first demo | 6-9 months | 2-3 months |
| Time to production | 18-24 months | 6-9 months |
| Cost per regulatory change | $30K-$60K | $1K-$2K |
| Annual run cost | $1M-$1.5M | $100K-$200K |
| 5-year TCO | $8M-$15M | $400K-$800K |
| Bus factor risk | Lower (team) | Higher (mitigated by LLM + docs) |

### The Uncomfortable Truth

> **The Java team isn't incompetent. The architecture is the problem.**

No amount of "best practices," "clean code," or "senior hires" fixes:
- Implicit ORM behavior
- Framework magic
- Distributed semantics across layers
- Runtime-only validation
- Non-deterministic replay

**These are architectural choices that compound costs over time.**

### The Pitch

> "We can build this for $150K and run it for $100K/year with one person.
> Or we can build it for $3M and run it for $1.2M/year with seven people.
> Both will demo the same. One will cost 10x more to change."

Pick one.

---

## 10. The Refactoring Problem: Technical Detail

### Management Sees the Vibe, Not the Value

Management watches a demo: "Blimey, you just built an onboarding system with AI!"

They see:
- Natural language → working code
- Fast initial prototype
- "AI makes development easy"

They don't see:
- **The refactoring problem**
- Preserving invariants across a large dependency graph
- The 10x cost difference when requirements change

### The Iceberg: Initial Build vs. Ongoing Change

```
┌─────────────────────────────────────────────────────────────────┐
│                     VISIBLE TO MANAGEMENT                      │
│                                                                 │
│     "Look, the AI wrote an onboarding system in 4 months!"      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ WATERLINE
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│                  INVISIBLE: THE REFACTORING COST                │
│                                                                 │
│  Year 1: "Add SFTR reporting fields"                           │
│  Year 2: "Change UBO threshold from 25% to 10%"                 │
│  Year 3: "New product bundle: Custody + Prime + Collateral"     │
│  Year 4: "EMIR Refit changes the whole trade lifecycle"         │
│  Year 5: "Migrate to new settlement platform"                   │
│                                                                 │
│  Each change touches 50+ tables, 200+ services, 1000+ tests     │
│                                                                 │
│  THIS IS WHERE 80% OF TOTAL COST LIVES                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Why Rust + DSL + LLM Wins at Refactoring

**The refactoring problem:** Preserve invariants across a large dependency graph while changing structure.

| Dimension | Java Spring + ORM | Rust + DSL + LLM |
|-----------|-------------------|------------------|
| **Find all affected code** | Grep + hope; runtime surprises | Compiler errors enumerate every site |
| **Verify invariants** | Integration tests (slow, incomplete) | Type system + compile-time checks |
| **LLM refactoring quality** | Hallucinates in diffuse semantic space | Bounded vocabulary, schema-validated |
| **Confidence to ship** | Low (need extensive manual review) | High (if it compiles, invariants hold) |
| **Regression detection** | Discovered in QA/prod | Discovered at compile time |

### The Compound Effect

**Java Spring + ORM refactoring cycle:**
```
Change request → Impact analysis (days) → Multi-layer edits (weeks)
     → Integration testing (weeks) → Regression fixes (weeks)
     → Repeat until stable
```

**Rust + DSL + LLM refactoring cycle:**
```
Change request → Edit verb/schema → Compiler shows all break sites
     → Fix each site (type-guided) → Replay scenarios → Done
```

### What One Person + LLM Can Do

This system was built by **one solution architect + Claude** in **4 months**:

- 824+ verbs across 105+ YAML files
- 55+ custom operation handlers
- 13 Rust crates
- 92+ database tables
- 16 schema migrations
- Full agent pipeline (MCP, REPL, research workflows)
- Graph visualization (egui, taxonomy navigation)
- Event infrastructure + feedback system

**The invisible part:** This architecture can be refactored safely. 

The same scope in Java Spring + ORM would require:
- 5-10 developers
- 12-18 months
- And **still be harder to change** going forward

### The Question for Management

> "Do you want a system that's easy to demo, or a system that's easy to change?"

The initial vibe is identical. The 5-year TCO is 10x different.

---

## 11. Positioning Line

> "This isn't a CRUD app and it isn't a generic workflow engine. It's a **deterministic onboarding orchestration system** where business intent must be auditable, replayable, and continuously adaptable across products and regimes. DSL-as-state makes that intent explicit; Rust + SQLx makes it safe; LLM assistance becomes reliable because the system has sharp guardrails and deterministic replay."

---

## 12. Architecture Principles Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                    OB-POC DESIGN PRINCIPLES                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. DSL-AS-STATE                                                │
│     The runbook is the source of truth, not DB rows             │
│                                                                 │
│  2. VERBS ARE FIRST-CLASS                                       │
│     Business operations = typed verbs with schemas              │
│                                                                 │
│  3. DETERMINISTIC REPLAY                                        │
│     Any state reproducible from runbook + snapshots             │
│                                                                 │
│  4. EXPLICIT DEPENDENCIES                                       │
│     DAG of preconditions, not implicit service coupling         │
│                                                                 │
│  5. EVIDENCE-FIRST                                              │
│     Artifacts (docs, checks, approvals) are first-class         │
│                                                                 │
│  6. POLICY-DRIVEN GATES                                         │
│     Rules expressed as verb preconditions, not code branches    │
│                                                                 │
│  7. LLM-FRIENDLY SURFACE                                        │
│     Bounded vocabulary + schemas + fast feedback loop           │
│                                                                 │
│  8. AUDIT BY CONSTRUCTION                                       │
│     Every state transition traced, every change versioned       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Related Documents

- `docs/strategy-patterns.md` — Data model philosophy, agent strategy
- `docs/dsl-verb-flow.md` — Parser/compiler/executor pipeline
- `ai-thoughts/020-research-workflows-external-sources.md` — Bounded non-determinism
- `CLAUDE.md` — Quick reference for Claude Code
