-- 20260429_carrier_02_service_intent_comments.sql
-- Phase 2.2 of refactor-todo-2026-04-29.md (D-006).
-- Encodes the M-026 vs M-039 semantic distinction (Q2 (a)) as table
-- comments. service_intents is the intent layer (3 states); the new
-- cbu_service_consumption is the operational layer (6 states). They
-- coexist with distinct semantics, NOT one supersedes the other.

COMMENT ON TABLE "ob-poc".service_intents IS
    'Intent layer: per-(cbu, product/service) intent declarations. State machine M-026, 3 states (active, suspended, cancelled). Distinct from cbu_service_consumption (M-039) which models per-(cbu, service_kind) operational lifecycle.';

-- (cbu_service_consumption comment set in carrier_01.)

-- Materialises: M-026 · DAG: cbu_dag.yaml · Substrate audit: S-25
