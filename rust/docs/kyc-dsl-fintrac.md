# Onboarding & KYC DSL — FINTRAC-Focused Brief

## Audience
- Senior engineering and product leaders at Canadian reporting entities (banks, MSBs, securities dealers) subject to PCMLTFA and associated regulations/guidance.

## What This Delivers
- Deterministic, auditable onboarding/KYC policy with versioned configuration (DSL) and replayable outcomes.
- Safe agentic operations: autonomy bounded by DSL guardrails; idempotent integrations; instant audit packs for FINTRAC exams.

## Regulatory Alignment
- Risk-Based Approach (RBA)
  - Declarative risk scoring with thresholds for entity/product/geography; maker-checker approvals and historical replays.
- CDD/Beneficial Ownership
  - Entity schemas include prescribed identification information; beneficial ownership capture with thresholds (e.g., 25%) and verification steps.
  - Non-face-to-face and high-risk categories trigger EDD predicates and escalations.
- PEPs/HIOs
  - Screening modules encode Canadian definitions of PEPs and Heads of International Organizations; deterministic EDD workflows and approvals.
- Sanctions
  - Screening against applicable Canadian sanctions lists (Global Affairs/UN) via provider adapters; rule versions + payload hashes logged.
- Ongoing Monitoring
  - Event-driven and periodic re-assessment; explainable diffs logged for changes in risk profile.
- Reporting
  - Deterministic triggers for STRs, LCTRs (≥ CAD 10,000), and EFT reporting; adapters with idempotency and full payload traces.
- Record Keeping
  - Input snapshots, rule versions, outcomes retained for ≥ 5 years; PIPEDA-aligned redaction settings.

## Controls & Audit
- Versioning and Checksums: embed artifact versions in every decision trace.
- Impact Analysis: golden-run replays quantify outcome changes before promotion.
- Maker/Checker: enforced approvals for policy changes with audit logs aligned to compliance program expectations.

## Tech Stack (Why Rust/Go)
- Rust engine: predictable performance, strong typing for the DSL, compact binaries.
- Go orchestration: IO-friendly for vendor calls and data stores; clean observability.
- Avoid heavyweight frameworks that complicate determinism, increase jitter, and raise TCO.

## UI Strategy
- DSL-driven form schemas; canonical validation server-side; no policy logic in channels (web/mobile/partner).

## Audit Pack (FINTRAC Examiner-Ready)
- Policy module version, changelog, approvals.
- Decision trace: input snapshot, rule versions, provider payload hashes, idempotency keys, outcome.
- Replay results: pre/post-change outcome diffs with KPI impacts (approvals, EDD rate, alerts).

## Example Mapping
- LCTR trigger → evidence of cash amount, parties, and method → report payload hash → adapter submission with idempotency key → confirmation and trace.

## KPIs
- 70–90% faster policy change cycles; 30–60% fewer manual reviews; lower infra cost via latency efficiency and fewer retries.

---

### Appendix: Representative DSL Snippets (Illustrative)

```yaml
policy:
  bo_threshold: 0.25
  edd_triggers:
    any:
      - "pep.ca_hit == true"
      - "risk_score.bucket == 'HIGH'"
      - "onboarding.method == 'non_face_to_face'"
  reporting:
    lctr:
      when: "txn.cash_amount_cad >= 10000"
      action: adapters.fintrac.lctr_submit
      idempotency_key: "${txn_id}:lctr:${payload_hash}"
trace:
  snapshot_inputs: true
  include_rule_versions: true
  retention: 5y
```

