# 030: Internal Pitch Strategy — Bank-Safe Positioning

> **Status:** Strategic planning
> **Context:** Internal BNY pitch framing, not a vendor bid
> **Related:** 003 (paradigm sell problem), 005 (Allianz play), 006 (custody flanking)

---

## The Core Insight

**Employment risk is about politics and framing, not technical merit.**

If you pitch "I'm replacing the Java shop" → antibodies triggered.
If you pitch "bank-safe way to remove friction with controlled pilot" → aligns with transformation narrative.

---

## The Sharp Argument (What to Say)

You're NOT arguing "Rust is better than Java." You're arguing:

### 1. The Artifact is the Product

A deterministic, auditable DSL runbook + matrix that encodes onboarding intent directly.

**Business value:** Reduces "intent drift" and rework. What was decided IS what gets executed.

### 2. Tooling Makes Change Cheap

Compile-time guardrails + deterministic replay mean policy changes are safer and faster.

**Business value:** Regulatory changes, new product variants, SSI updates — all lower risk.

### 3. LLM Productivity is Real Because Surface is Constrained

The verb universe + schemas + replay oracle is what makes "one architect + Opus" viable.

**Business value:** Not "AI magic" but constrained domain where AI actually works reliably.

---

## Pitch Structure (Avoids the Java-vs-Rust Fight)

### Part 1: Outcome-First

> "Reduce onboarding time and operational risk by making intent executable and auditable."

Don't lead with technology. Lead with the problem they already know they have.

### Part 2: Pilot-First

> "We'll prove it on one slice (Custody + SFTR/LEI gating + SSI + CA policy), with measurable metrics."

Scope is already defined in `trading-profile.*` + entity resolution work.

### Part 3: Coexistence-First

> "This is not a rewrite. It's an orchestration layer that can call existing services and materialize operational tables."

**The key sentence:**
> "The DSL materializes to PostgreSQL operational tables. Existing Java services read those tables unchanged. Zero integration risk — it's an authoring/orchestration layer, not a runtime replacement."

This lets the Java team keep their dignity and gives leadership a low-risk adoption path.

---

## Coalition Building (Kotter's Model)

You need a "guiding coalition" and visible wins quickly.

### Find 1-2 Senior Allies

| Role | Why They'd Care |
|------|-----------------|
| Product Owner | Faster time-to-market, fewer handoffs |
| Ops/Implementation | Less rework, clearer intent, audit trail |
| Compliance | Replayability, governance, audit logs |
| Tech Leadership | Innovation story, AI productivity proof point |

### Get a Narrow Pilot

Don't boil the ocean. Pick ONE slice:
- Single client (or single fund family)
- Single product type (Custody trading setup)
- Single regulatory gate (SFTR/LEI eligibility)

### Show a Win in Weeks, Not Quarters

| Short-Term Win | Timeline | Metric |
|----------------|----------|--------|
| "Set up a new fund with trading profile" | 20 minutes vs. 3 days | Time-to-active |
| "SSI assignment with full audit trail" | Same session | Replayable log exists |
| "CA policy configured once, materialized everywhere" | Minutes | No manual sync needed |

---

## Bank-Safe Guardrails (What Makes It Acceptable)

### Governance

| Guardrail | Implementation | Status |
|-----------|----------------|--------|
| DSL/verb versioning | Verb governance, mandatory metadata | ✅ Done (028/029) |
| Deprecation policy | Designed but not needed for POC | ✅ Designed |
| Audit logs | Event infrastructure + feedback inspector | ✅ Done (023a/023b) |
| Replayability | Deterministic DSL execution, session logging | ✅ Done |

### Control

| Guardrail | Implementation | Status |
|-----------|----------------|--------|
| Approval on materialize | Plan/apply separation (generate → review → apply) | ✅ Done (029) |
| Maker/checker flow | Can be added at apply step | 📝 Designed |
| Four-eyes on production | Session ownership + apply audit | 📝 Designed |

### Risk Containment

| Guardrail | Implementation | Status |
|-----------|----------------|--------|
| Feature-flagged execution | `internal: true` verbs, tier enforcement | ✅ Done |
| Read-only mode | Diagnostics tier verbs, generate-plan without apply | ✅ Done |
| Parallel run | Materialize to shadow tables, compare outputs | 📝 Designed |

### Metrics

| Metric | What It Shows |
|--------|---------------|
| Time-to-onboard | Before/after comparison |
| Exception rate | Fewer manual interventions |
| Rework loops | Intent captured correctly first time |
| Audit finding surface | Reduced exposure, full traceability |

---

## Why Other LLMs Feel Weak Here (And Why This Approach is Strong)

HBR's classic point: decision makers reject ideas when the pitch doesn't match how they evaluate risk/value.

**Losing pitch looks like:**
> "New tech stack! Rust is faster! AI wrote it!"

**Winning pitch looks like:**
> "Controlled change + measurable outcomes + low integration risk"

The difference:
- Losing: Technology push
- Winning: Business pull with technology as enabler

---

## Mapping to BNY Transformation Narrative

BNY's public posture includes:

| BNY Theme | How This Fits |
|-----------|---------------|
| "Simplify" | Single authoring surface, one source of truth |
| "Break silos" | Unified DSL across custody, SSI, CA, gateways |
| "Efficiency" | One architect + AI vs. 50-person team |
| "Culture of innovation" | Controlled AI adoption with governance |
| "Client experience" | Faster onboarding, fewer errors |

**The internal story:**
> "This is what 'run the company better' looks like in practice."

---

## Pilot Slice Definition

Already scoped in the codebase:

| Slice | Codebase Location | Verbs |
|-------|-------------------|-------|
| Trading matrix | `trading-profile.*` | ~30 verbs |
| SSI assignment | Materialized from matrix | Via materialize |
| CA policy | `trading-profile.ca.*` | ~5 verbs |
| LEI/GLEIF gating | Entity resolution + research | ~15 verbs |
| Custody setup | `cbu-custody.*` (read-only diagnostics) | ~10 verbs |

**Pilot deliverable:** "Configure a new custody client's trading profile end-to-end in one session, with full audit trail."

---

## Integration Strategy (Coexistence)

```
┌─────────────────────────────────────────────────────────────────┐
│                    DSL Orchestration Layer                      │
│  (ob-poc: Rust + Go, ~50 verb handlers, YAML-driven)           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ materialize
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                 PostgreSQL Operational Tables                    │
│  (custody.cbu_*, ssi_assignments, ca_preferences)               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ unchanged reads
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Existing Java Services                          │
│  (read from operational tables, no code changes)                │
└─────────────────────────────────────────────────────────────────┘
```

**Key message:** Existing services are unaffected. They read the same tables. Only the authoring/orchestration layer changes.

---

## Risk Mitigation Talking Points

| Objection | Response |
|-----------|----------|
| "What if you leave?" | DSL is declarative + self-documenting. Verb definitions are YAML. Any competent developer can maintain it. |
| "Rust is niche" | The runtime is Rust, but the interface is YAML + DSL. No one needs to write Rust to use or extend it. |
| "It's not enterprise-ready" | Show the governance framework (028/029), audit infrastructure (023a/b), and lint tiers. More rigorous than most internal tools. |
| "How does it scale?" | It's a compilation layer, not a runtime. Scales with PostgreSQL. No new infrastructure. |
| "What about DR/failover?" | Standard PostgreSQL HA. No special requirements. |

---

## Executive Summary (Draft)

> **One-Pager for Leadership**
>
> **Problem:** Onboarding takes weeks, involves multiple handoffs, intent gets lost, rework is common, audit trail is fragmented.
>
> **Solution:** A deterministic orchestration layer that captures onboarding intent as executable DSL, materializes to operational tables, and provides full audit trail.
>
> **Approach:**
> - Pilot on one product slice (Custody trading setup + SSI + CA)
> - Coexists with current systems (no rewrite, no migration)
> - Measurable: time-to-onboard, exception rate, audit coverage
>
> **Governance:**
> - Verb versioning and lifecycle management
> - Plan/apply separation with approval gates
> - Deterministic replay for audit and debugging
>
> **Ask:** Approve a 60-day pilot with one client segment. Success metric: 10x faster configuration with zero manual reconciliation.

---

## Next Steps

1. **Finalize 029 implementation** — Clean verb lexicon, idempotency tests
2. **Draft 2-page executive summary** — Non-technical, outcome-focused
3. **Identify pilot sponsor** — Product or Ops leader who owns onboarding pain
4. **Define success metrics** — Before/after comparison framework
5. **Prepare demo** — "Configure fund X in 20 minutes" walkthrough

---

## Appendix: Kotter's 8-Step Change Model

John Kotter, Harvard Business School, 1996. The canonical framework for organizational change. Banks love it because it's structured and risk-aware.

| Step | What It Means | Your Context |
|------|---------------|--------------|
| 1. Create urgency | People need to feel "we must change" | Allianz onboarding pain, competitive pressure, audit findings |
| 2. Build guiding coalition | Small group of influential people who champion it | 2-3 allies: Product + Ops + maybe Compliance |
| 3. Form strategic vision | Clear picture of the future state | "Intent-driven onboarding with full audit trail" |
| 4. Enlist volunteer army | Broader group who execute | Not needed yet — you're pre-pilot |
| 5. Enable action by removing barriers | Clear blockers (budget, politics, tech) | Coexistence pitch removes "but the Java team..." blocker |
| 6. Generate short-term wins | Visible, unambiguous successes early | "Fund configured in 20 minutes" demo |
| 7. Sustain acceleration | Build on wins, don't declare victory | Expand pilot scope after first win |
| 8. Institute change | Make it stick (process, culture, incentives) | Production rollout, team training |

---

## Appendix: Why Short-Term Wins Matter

Kotter's research: change efforts fail when early wins don't materialize within 12-24 months. For internal pitches, shrink that to **weeks**.

A short-term win must be:
- **Visible** — Leadership can see it without explanation
- **Unambiguous** — Clear success, not "it kinda worked"
- **Related to the change** — Proves the new approach works

**Your candidates:**

| Win | Timeline | Why It's Visible |
|-----|----------|------------------|
| Configure fund trading profile in one session | 1 hour demo | Contrast with current 3-day process |
| SSI assignment with replayable audit log | Same demo | Show the log, click replay |
| CA policy materialized to all systems | Minutes | No manual sync emails afterward |

---

## Appendix: Ally Roles (Guiding Coalition)

You need 2-3 people, not a committee. Each serves a different function:

| Role | What They Provide | Who to Look For |
|------|-------------------|------------------|
| **Power sponsor** | Air cover, budget sign-off, removes blockers | Someone at MD/Director level who owns onboarding outcomes |
| **Expert credibility** | Technical validation, "this is sound" | Could be you, or a respected architect peer |
| **Operational voice** | "This solves real pain" testimony | Implementation manager or ops lead who lives the current mess |
| **Political navigator** | Knows where the landmines are | Someone who's been there 10+ years, knows the players |

You don't need all four. **Power sponsor + Operational voice** is the minimum viable coalition.

---

## Appendix: The Pattern That Works

```
1. Find someone who OWNS THE PAIN (ops/implementation lead)
         │
         ▼
2. Show them a demo, get them saying "this would save us X"
         │
         ▼
3. Together, approach someone with BUDGET/AUTHORITY
         │
         ▼
4. Pitch: "Here's the pain, here's the proof, here's the ask (60-day pilot)"
         │
         ▼
5. Deliver a visible win in WEEK 2-3, not month 3
         │
         ▼
   Win creates momentum → Momentum creates permission → Permission creates scope
```

---

*Related documents:*
- `003-paradigm-sell-problem.md` — Why novel combinations are hard to explain
- `005-the-allianz-play.md` — Strategic entry point
- `006-custody-flanking-maneuver.md` — Product positioning
- `028-verb-lexicon-governance.md` — Governance framework
- `029-implement-verb-governance.md` — Implementation plan
