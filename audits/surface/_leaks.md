# Layering-leak scan (Step 5)

HEAD=86031a08098ee6b86f2f5c3a07acf3ab929d9c3c

Method: for each `audits/surface/<crate>.txt`, filtered to declaration/signature lines
(`pub struct|enum|type|trait|const|static|fn|async fn`, plus inherent `impl` headers),
excluding blanket-impl boilerplate method names (`into`, `from`, `try_from`, `borrow`,
`as_any`, `vzip`, `deref`, `into_request`, etc. — the auto-derived noise `cargo public-api`
normally hides behind `-s`/`--omit`), then grepped for `sqlx::` / `tantivy::` / transport-tier
`tonic::` / `PgPool` / `PgConnection` / other lower-tier engine or connection types.
LIST ONLY — no fixes applied.

**No dedicated `-sss`-omitted re-scan was run**; the exclusion regex above is a manual
approximation. Treat this as a first pass, not a final signed-off leak inventory.

## Confirmed real leaks (charter violations — the crate's own stated anti-charter forbids these)

| crate | leaked symbol | suspected tier |
|---|---|---|
| `entity-gateway` | `pub struct entity_gateway::TantivyIndex` + inherent methods (`new`, `force_reload`, `generation`, `nickname`) | index-engine choice (Tantivy) leaking into gateway's public API — the seeded example from the charter, confirmed present verbatim |
| `ob-poc-authoring` | `FeedbackInspector::new(sqlx_postgres::PgPool, ...)` | §2 capability crate — DB accessor in public constructor |
| `ob-poc-derived-attributes` | `derived_attributes::repository::{get_current,get_direct_dependencies,get_entity_scoped_impact,get_latest,get_recompute_queue}(&sqlx_postgres::PgPool, ...)` | §1 pure-DTO crate — entire `repository` module is a raw DB accessor surface, not DTOs |
| `ob-poc-diagnostics` | `EventConfig::with_db_store`, `SessionLogQuery::new`, `SessionLogger::new`, `DbEventStore::new` — all take `sqlx_postgres::PgPool` | §1 pure-DTO/error-types crate — DB-backed constructors |
| `ob-poc-entity-linking` | `compile_entity_snapshot`, `lint_entity_data` (both `&sqlx_postgres::PgPool` param) | §1 pure-DTO crate |
| `ob-poc-sage` | `session_context::{create_session,list_active_client_groups,load_entity_states_for_group,load_session,post_verb_update}(&sqlx_postgres::PgPool, ...)` | §2 capability crate — Sage drafter is supposed to be the vocabulary/classifier surface, not a DB-access module |
| `ob-poc-taxonomy` | `TaxonomyContext::to_rules_from_config`, `MembershipRules::{from_view_config,from_view_config_with_overrides,get_edge_types_for_view}`, `TaxonomyBuilder::build` — all `&sqlx_postgres::PgPool` | §1 pure-DTO crate |
| `ob-poc-trading-profile` | `ast_db::{activate_profile,apply_and_save,clone_to_draft,create_draft,ensure_draft,...}` — full DB module, all `&sqlx_postgres::PgPool` | §1 pure-DTO crate (largest one — LOC 5,632) |
| `dsl-analysis` | `gateway_resolver::GatewayRefResolver::{client,new}` returning/taking `EntityGatewayClient<tonic::transport::channel::Channel>` | transport type (`tonic::transport::channel::Channel`) leaking through a DSL infra crate's public API, not just an entity-gateway-internal detail |

## Present but arguably by-design for the crate's own charter (flag for review, not clear-cut)

| crate | leaked symbol | note |
|---|---|---|
| `ob-semantic-matcher` | `PgClientGroupResolver::new`, `FeedbackAnalyzer/Repository/Service::new`, `PatternLearner::new` — all `sqlx_postgres::PgPool` | §3 infra/tooling; a learning/matching engine plausibly needs DB access, but raw `PgPool` in every public constructor means callers can't be decoupled from Postgres specifically |
| `ob-workflow` | `GuardEvaluator`, `RequirementEvaluator`, `WorkflowEngine`, `WorkflowLoader::load_and_sync`, `WorkflowRepository::new` — all `sqlx_postgres::PgPool` | §3 infra/tooling ("task queue + listener") — same shape as above |

## NOT leaks — DB/transport access is the crate's own explicit charter

| crate | why excluded |
|---|---|
| `sem_os_postgres` | name says it all — the Postgres store implementation |
| `ob-poc-kyc-store` | CLAUDE.md: "the Postgres membrane, §3 append protocol, projectors, drainers" — its entire purpose |
| `dsl-runtime` | owns the `TransactionScope` trait (`executor() -> &mut PgConnection`, `pool() -> &PgPool`) — this *is* the documented chokepoint (CLAUDE.md: "`scope.executor()` → `&mut PgConnection` inside the ambient txn") |
| `bpmn-runtime` | `PostgresJourneyStore` — an explicit, named Postgres-backed store impl, not incidental leakage |
| `bpmn-controller` | reconciliation doc §3 charter: "pool lifecycle + instance kick-off" — Postgres pool handling is the point |
| `ob-poc` | the application/integrator hub — expected to touch everything; not evidence of a *lower-tier* crate leaking upward |

## Scope note

This scan only covers the engine/transport/connection-type class of leak the task specified
(tantivy/sqlx/tonic/PgPool/PgConnection). It does **not** cover the separate boundary-specific
check flagged in the reconciliation doc §2 ("verify no draft/runbook state leaks into
`ob-poc-boundary`" — that belongs to `dsl-lsp` per plan §1.5) — that requires identifying
draft/runbook *domain* types, not engine types, and was out of scope for Step 5 as specified.
