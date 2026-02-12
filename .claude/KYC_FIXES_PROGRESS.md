# KYC Fixes Progress Tracker

## Status: COMPLETE

Last task completed: F-3e (ALL FIXES COMPLETE)
Timestamp: 2026-02-12
Tests: 1017/1017 passing, 0 clippy warnings
Files modified: rust/src/domain_ops/skeleton_build_ops.rs, rust/src/domain_ops/mod.rs, rust/config/verbs/kyc/skeleton-build.yaml
Next task: NONE — all 5 fixes (F-5, F-2, F-1, F-4, F-3a-e) complete
Blockers: none

## Session Log

<!-- Claude Code: append a line here after each task completion -->
- F-5: Decimal conversion cleanup — replaced `.to_string().parse::<f64>()` with `.to_f64()` at lines 224, 612. Added `use rust_decimal::prelude::ToPrimitive;`. 1017 tests pass.
- F-2: Coverage direction fix — scoped OWNERSHIP and CONTROL prongs to case entities via entity_workstreams subquery. Added case_id bind. 1017 tests pass.
- F-1: Transaction boundary — wrapped all 7 steps in pool.begin()/tx.commit(). Changed 5 run_* signatures from &PgPool to &mut sqlx::Transaction. 41→2 pool refs remaining. 1017 tests pass.
- F-4: Outreach cap configurable — added max-outreach-items YAML arg (integer, optional, default 8, clamped 1-50). Updated run_outreach_plan signature to accept cap + return (Option<Uuid>, i32). Replaced truncate(8) with configurable cap + tracing::warn. Added items_capped/total_gaps_before_cap to SkeletonBuildResult. 1017 tests pass.
- F-3a: Made run_graph_validate pub, Edge/GraphAnomaly pub(crate), re-exported from mod.rs. 1017 tests pass.
- F-3b: Made run_ubo_compute pub, re-exported from mod.rs. 1017 tests pass.
- F-3c: Made run_coverage_compute pub, extract_candidate_entity_ids/update_prong pub(crate), re-exported from mod.rs. 1017 tests pass.
- F-3d: Made run_outreach_plan pub, re-exported from mod.rs. 1017 tests pass.
- F-3e: Made run_tollgate_evaluate pub, re-exported from mod.rs. 1017 tests pass. 0 clippy warnings.
