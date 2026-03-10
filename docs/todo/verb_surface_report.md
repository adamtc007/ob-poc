# SemTaxonomy Verb Surface Report

## Scope

This report captures the reuse surface for the SemTaxonomy replacement path. It is intentionally narrow: only the existing components that can support the new `discovery.*` verbs and the replacement session/composition path are listed.

## Discovery Verb Reuse Map

### discovery.search-entities

Primary reuse points:
- `rust/src/api/entity_routes.rs`
- `rust/src/mcp/handlers/session_tools.rs::entity_search(...)`
- `rust/crates/entity-gateway/src/search_engine.rs`

Use:
- fuzzy entity lookup
- entity-type narrowing
- score-bearing hit lists

New code needed:
- normalize returned match payload into the `EntitySearchHit` shape defined in the replacement TODO
- expose `include_inactive` and `entity_types` controls through a stable DSL verb wrapper

### discovery.entity-context

Primary reuse points:
- `rust/src/mcp/handlers/session_tools.rs`
- `rust/src/sem_reg/agent/mcp_tools.rs`
- `rust/src/session/unified.rs`

Use:
- session-scoped entity context
- SemReg context resolution
- activity and scope synthesis

New code needed:
- compose a stable `EntityContext` envelope that is independent of the current Sage/Coder structs

### discovery.entity-relationships

Primary reuse points:
- `rust/src/domain_ops/client_group_ops.rs`
- `rust/src/domain_ops/ubo_analysis.rs`
- `rust/src/graph/query_engine.rs`

Use:
- ownership/control graph traversal
- group/entity relationship expansion
- recursive lineage summaries

New code needed:
- normalize graph output into a single `RelationshipGraph` contract
- cap traversal depth deterministically

### discovery.cascade-research

Primary reuse points:
- `rust/src/session/research_context.rs`
- `rust/crates/sem_os_core/src/affinity/discovery.rs`
- `rust/src/domain_ops/client_group_ops.rs`

Use:
- seeded discovery workflow
- likely-intent hints from existing research/discovery machinery

New code needed:
- orchestrate search + entity-context + relationships into one read-only research result

### discovery.available-actions

Primary reuse points:
- `rust/src/agent/verb_surface.rs`
- `rust/src/agent/orchestrator.rs::resolve_sem_reg_verbs(...)`
- `rust/src/sem_reg/agent/mcp_tools.rs`

Use:
- current governed verb surface
- phase and entity-kind filtering
- SemReg-backed action visibility

New code needed:
- convert `SessionVerbSurface` into grouped `ActionSurface` output
- support domain/entity_type/aspect filtering explicitly

### discovery.verb-detail

Primary reuse points:
- `rust/src/sem_reg/agent/mcp_tools.rs`
- `rust/src/domain_ops/sem_reg_registry_ops.rs`

Use:
- authoritative contract lookup
- parameters, governance, preconditions, postconditions

New code needed:
- normalize sem_reg tool output to the `VerbContract` return shape in the TODO

### discovery.inspect-data

Primary reuse points:
- `rust/src/domain_ops/sem_reg_schema_ops.rs`
- `rust/src/sem_reg/agent/mcp_tools.rs`

Use:
- schema/domain/entity inspection
- table/field/relationship summaries

New code needed:
- attach entity-scoped data snapshot semantics instead of only structural schema output

### discovery.search-data

Primary reuse points:
- `rust/src/sem_reg/agent/mcp_tools.rs`
- `rust/src/domain_ops/affinity_ops.rs`

Use:
- attribute/table/verb search
- sem_reg search

New code needed:
- entity-scoped data search contract with result hit normalization

## Replacement Path Principle

The replacement path should reuse the existing query surfaces and only replace the semantic planning chain. The intended path is:

1. utterance
2. discovery query/use of session context
3. composition request
4. composed runbook
5. existing DSL execution engine

## What This Unblocks

- a direct utterance-to-runbook path without `OutcomeIntent`
- session-backed entity/domain/aspect grounding before runbook composition
- removal of multi-stage lossy classification before DSL generation
