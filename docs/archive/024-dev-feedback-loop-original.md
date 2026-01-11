# 024: Dev Feedback Loop (ARCHIVED)

> **Status:** ARCHIVED - Consolidated into 023-adaptive-feedback-system.md
> **Original Date:** 2026-01-10
> **Archived:** 2026-01-10
> **See:** `ai-thoughts/023-adaptive-feedback-system.md`

---

## Why This Document Is Preserved

This document contains the **design rationale** and **research findings** that led to the Adaptive Feedback System (023). While the implementation details are now in 023, this archive preserves the "why" for future reference.

---

## Original Research Findings

### The Gap That Nobody Had Bridged

```
PRODUCTION                    DEVELOPMENT
(runtime errors)              (code fixes)

┌─────────────┐    ???        ┌─────────────┐
│   AIOps     │ ────────────▶ │   SWE-agent │
│   detects   │               │   fixes     │
│   issues    │               │   code      │
└─────────────┘               └─────────────┘

Nobody bridges this gap automatically.
```

### Existing Systems Analyzed

1. **Runtime Self-Healing (2009-2015)** - ClearView, ASSURE, RCV
   - Patches running binaries, not source code
   - Dead end: doesn't scale, doesn't learn

2. **Automated Program Repair (2009-2023)** - GenProg, DeepFix
   - Requires failing test as oracle
   - Human must find the bug first

3. **LLM-Based APR (2023-2025)** - SWE-bench, SWE-agent
   - Input: GitHub issue (human-written)
   - Still offline, still requires human issue creation

4. **AIOps (2020-2025)** - Datadog, AWS DevOps Agent
   - Monitors and mitigates, doesn't fix code

### Why Our Architecture Enables This

**Three things we have that others don't:**

1. **Stable Reproduction Artifact** - DSL command + captured bindings
2. **Semantic Anchor** - Verb registry maps errors to handlers/files/schemas
3. **Deterministic Replay** - Re-run exact DSL as verification oracle

---

## Key Design Decisions Preserved in 023

### Decision: One Capture, Two Paths

Originally 023 (runtime learning) and 024 (code feedback) were separate. Consolidated because:
- Same capture point (executor failure)
- Classification determines routing
- Simpler mental model

### Decision: Verb as Primary Key

The verb provides:
- Handler name, file, line
- Input/output schemas
- Source list
- Stable fingerprint base

### Decision: Repro-First

From peer review: "The fix must make the test pass."
- Every captured issue → deterministic test
- Test fails before fix, passes after
- This is the differentiator vs "fancy Sentry"

### Decision: Policy-Based Redaction

From peer review: Generic redaction destroys reproducibility.
- EnumDrift: preserve offending value (it's the point)
- SchemaDrift: preserve structure, redact PII strings
- Everything else: full redaction

---

## References That Informed The Design

### Academic
- ClearView (2009): Binary patching
- GenProg (2009): Genetic programming for repair
- SWE-bench (2024): LLM code repair benchmarks
- RepairAgent (2024): Autonomous program repair

### Industry
- Sentry Autofix (2025)
- Datadog Bits AI Dev Agent (2025)
- AWS DevOps Agent (2025)

### The Insight

Commercial tools struggle because:
1. Stack trace isn't a test (no oracle)
2. Log lines have no semantic boundary
3. No standard handoff from observability to code agent

Our DSL provides the oracle. The verb registry provides the semantic boundary. The TODO document provides the handoff protocol.

---

## Peer Review Feedback (Preserved)

Key refinements from peer review that shaped final 023:

1. **SchemaDrift vs ParseError** - Distinguish provable drift from potential bugs
2. **RuntimeHandler architecture** - Return plans, not results; executor retries
3. **Occurrence context** - Capture session_id, http_status per occurrence
4. **generate_repro_test** - The killer differentiator tool
5. **Fingerprint collision handling** - Store key alongside hash

---

## See Also

- `ai-thoughts/023-adaptive-feedback-system.md` - The consolidated implementation
- This document preserved for "why did we do it this way?" questions
