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

---

## 9. Positioning Line

> "This isn't a CRUD app and it isn't a generic workflow engine. It's a **deterministic onboarding orchestration system** where business intent must be auditable, replayable, and continuously adaptable across products and regimes. DSL-as-state makes that intent explicit; Rust + SQLx makes it safe; LLM assistance becomes reliable because the system has sharp guardrails and deterministic replay."

---

## 10. Architecture Principles Summary

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
