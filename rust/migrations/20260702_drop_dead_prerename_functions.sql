-- Phase 6d (state-graph remediation, RW-5): three dead SQL functions,
-- pre-rename references (from before the kyc -> "ob-poc" schema
-- consolidation) with zero Rust callers and zero remaining DB dependents
-- (no triggers, no views) -- confirmed via pg_depend.
--
-- is_valid_cbu_transition() -- dead code, superseded by the Rust-side
-- is_valid_deal_status_transition-style guards; zero callers.
-- apply_case_decision() / evaluate_case_decision() -- reference the
-- pre-rename kyc.cases schema qualifier; dead since the schema rename.

BEGIN;

DROP FUNCTION IF EXISTS "ob-poc".is_valid_cbu_transition(character varying, character varying);
DROP FUNCTION IF EXISTS "ob-poc".apply_case_decision(uuid, character varying, character varying, text);
DROP FUNCTION IF EXISTS "ob-poc".evaluate_case_decision(uuid, character varying);

COMMIT;
