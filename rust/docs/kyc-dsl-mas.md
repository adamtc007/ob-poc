# Onboarding & KYC DSL — MAS-Focused Brief

## Audience
- Senior engineering and product leaders at Singapore-regulated institutions subject to relevant MAS AML/CFT Notices (e.g., 626 for banks, PSN02 for payment services) and PDPA.

## What This Delivers
- Deterministic, auditable policy-as-configuration (DSL) replacing scattered code and UI logic.
- Safe agentic operations with strict guardrails, idempotent integrations, and instant audit packs for MAS/STRO inquiries.

## Regulatory Alignment
- Risk-Based Approach (RBA)
  - Declarative risk models for customer, product, delivery channel, and geography; staged promotion with replays and approvals.
- CDD/ECDD
  - Entity schemas for required identification information; document contracts for PoI/PoA with acceptance criteria and liveness checks.
  - ECDD triggers (PEP, high-risk countries, complex structures, non-face-to-face) encoded as predicates with deterministic escalation paths.
- Name Screening & Sanctions
  - Screening against UN Consolidated List and Singapore lists via provider adapters; versions and payload hashes preserved.
- Ongoing Monitoring
  - Continuous evaluation rules for transactional/activity monitoring with explainable diffs and alert routing.
- Reporting (STR to STRO)
  - Deterministic STR triggers; adapter with idempotency and full payload trace to STRO.
- Record Keeping
  - Decision traces retained ≥ 5 years after relationship end/transaction; PDPA-aligned redactions.
- Travel Rule (where applicable)
  - Deterministic data lineage, validation, and payload construction to meet FATF-aligned requirements (EFT/DPT contexts).

## Controls & Audit
- Versioned artifacts with checksums embedded in decision traces.
- Golden-run replays for impact analysis prior to promotion; four-eyes approvals enforced in CI.
- Clear RACI for policy authorship vs. runtime ownership; observability for latency and drift SLOs.

## Tech Stack (Why Rust/Go)
- Rust decision engine: predictable latency, memory safety, strong type system; ideal for high-throughput, low-jitter evaluation.
- Go orchestration: efficient vendor IO, simple concurrency, strong HTTP/gRPC ecosystem.
- Avoid heavy JVM stacks that introduce GC jitter and framework complexity that hinders determinism.

## UI Strategy
- DSL-driven schemas render forms across channels; canonical validation in engine; zero policy logic at the edge.

## Audit Pack (MAS Examiner-Ready)
- Policy module version, change history, approvals.
- Decision trace with input snapshot, rule versions, provider payload hashes, idempotency keys, and outcome.
- Replay results on representative cohorts with KPI impact analysis (approval rate, ECDD rate, alert volume).

## Example Mapping
- High-risk geography + complex ownership → ECDD state → required docs (e.g., UBO declarations, SOF) → analyst decision → Approve/Reject; all pinned to versions.

## KPIs
- 70–90% faster policy changes; 30–60% fewer manual reviews; lower infra cost via efficiency and fewer vendor retries.

---

### Appendix: Representative DSL Snippets (Illustrative)

```yaml
policy:
  ecdd_triggers:
    any:
      - "pep.sg_hit == true"
      - "risk_score.bucket == 'HIGH'"
      - "applicant.geography in HRG_SG"
  reporting:
    str:
      when: "suspicion == true"
      action: adapters.stro.submit
      idempotency_key: "${case_id}:stro:${payload_hash}"
trace:
  snapshot_inputs: true
  include_rule_versions: true
  retention: 5y
```

