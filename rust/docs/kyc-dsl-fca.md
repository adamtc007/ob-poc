# Onboarding & KYC DSL — FCA-Focused Brief

## Audience
- Senior engineering and agile product leaders at UK-regulated firms (banks, EMI, PI, brokers) operating under MLR 2017 (as amended), FCA Handbook (SYSC), and JMLSG guidance.

## What This Delivers
- Deterministic, auditable policy enforcement with versioned configuration (DSL) instead of scattered service/UI code.
- Faster, safer policy iteration with replayable outcomes, idempotent side effects, and instant audit packs for FCA/NCA inquiries.

## Regulatory Alignment
- Risk-Based Approach (RBA)
  - DSL encodes risk models and thresholds as declarative rules; promotion requires 4-eyes review with replay on historical datasets.
  - Supports segment- and product-level granularity per JMLSG Part I and sectoral guidance.
- CDD/EDD (MLR Reg 28, 33)
  - Entity schemas capture required attributes; document contracts for PoI/PoA with acceptance criteria and liveness/forgery checks.
  - EDD triggers modelled as predicates (PEP, adverse media, high-risk geography, non-face-to-face) with deterministic escalations.
- PEPs & Sanctions
  - Provider-agnostic screening; OFSI/HMT list alignment via adapter configuration; rule version and payload hashes stored in trace.
  - Re-screening schedules and event-driven rechecks represented in DSL workflow, with idempotency keys for safe retries.
- Ongoing Monitoring
  - Continuous evaluation rules for transaction/event streams; risk re-bucketing produces explainable diffs and audit logs.
- Record Keeping (Reg 40)
  - Input snapshots, rule versions, outcomes retained for 5 years; redaction settings for UK GDPR.
- SARs to NCA (UKFIU)
  - Deterministic SAR triggering predicates; action adapters for NCA submission with idempotency and full payload audit trail.

## Controls & Audit
- Versioned Everything: rules, schemas, provider configs, and scoring models with checksums embedded in decision traces.
- Impact Analysis: automatic golden-run replays before promotion; diffs show customer segments affected and rationale.
- Maker/Checker: enforced in CI; policy changes require approvals mapped to SYSC 6.1 responsibilities.

## Tech Stack (Why Rust/Go)
- Rust engine: predictable latency, strong typing for DSL compilation, small footprint — ideal for peak-period KYC.
- Go orchestration: simple IO concurrency for vendors, storage, and messaging; easy ops and observability.
- Avoid heavy ORM stacks that increase latency variance and complicate determinism/idempotency.

## UI Strategy
- Zero business logic at the edge: UI renders DSL-emitted form schemas; canonical validation in engine ensures channel consistency.

## Audit Pack (FCA Examiner-Ready)
- Policy artifacts: DSL module version, checksums, change log, approvals.
- Evidence: input snapshot, evaluation trace, provider payload hashes, idempotency keys, outcome.
- Replays: pre/post-change diffs on representative cohorts with KPI impact (approval rate, EDD rate, false positives).

## Example Mapping
- PEP Detected → EDD State → Required docs (e.g., SOW/SOF) → Analyst decision → Approve/Reject; all steps, predicates, and artifacts pinned to versions.

## KPIs
- 70–90% reduction in policy change lead time; 30–60% manual review reduction; improved p99 latency and lower infra cost.

---

### Appendix: Representative DSL Snippets (Illustrative)

```yaml
policy:
  pep:
    run: providers.pep_screen
    on_success: set("pep.hit", response.hit)
  edd_triggers:
    any:
      - "pep.hit == true"
      - "risk_score.bucket == 'HIGH'"
      - "applicant.geography in HRG"
  decision:
    approve_when: ["risk_score.bucket == 'LOW' && pep.hit == false && sanctions.hit == false"]
    reject_when: ["sanctions.hit == true"]
    else: "REVIEW"
trace:
  snapshot_inputs: true
  include_rule_versions: true
  retention: 5y
```

