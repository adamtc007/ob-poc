# Semantic OS — Standalone Service Architecture

**Standalone Service · Rust Reference Implementation · Java 21 Production Target**

Version: 2.0 — February 2026
Author: Adam / Lead Solution Architect
Status: Draft — architectural branch from ob-poc Semantic OS v1.1
Lineage: v1.1 ob-poc embedded Semantic OS → v2.0 standalone service extraction with dual-implementation strategy

---

## Document Scope & Branch Notice

This document is a **separate architectural branch** from `semantic-os-v1.1.md`. The v1.1 document describes the Semantic OS as embedded within ob-poc. This document describes the Semantic OS extracted as a **standalone, deployable service** with:

- A stable wire contract (OpenAPI + JSON schema) as the primary artifact
- A Rust reference implementation that validates semantics, invariants, and produces golden fixtures
- A Java 21 production implementation ported from the Rust reference, running against Oracle or MongoDB backends
- No dependency on ob-poc crate structure, DSL parser, CLI, or BPMN engine

The **domain model, invariants, security framework, governance model, and snapshot architecture remain identical to v1.1.** This document does not re-specify them — it specifies how they are delivered as a standalone service and how the dual-implementation strategy works.

---

## 1. Vision: Standalone Semantic OS Service

### 1.1 What changes from v1.1

The Semantic OS in v1.1 was designed as an embedded module within ob-poc: it lived in `src/sem_reg/`, depended on the ob-poc crate structure, was accessed via CLI commands (`ob reg attr describe ...`), and was consumed directly by the DSL parser, BPMN engine, and MCP agent layer.

The standalone service extracts the Semantic OS into an independent, network-addressable service with these properties:

- **Independently deployable.** The Semantic OS runs as its own process with its own database. It does not import or link against ob-poc code.
- **Wire contract is the product.** The OpenAPI specification + JSON schema defines the complete behavioural contract. Any implementation that satisfies the contract and passes the golden fixture suite is a valid Semantic OS.
- **Database-agnostic.** The service defines storage operations semantically (load snapshot, publish draft, resolve at point-in-time). Backend can be PostgreSQL (Rust reference), Oracle (Java production), or MongoDB (Java alternative).
- **Consumer-agnostic.** ob-poc becomes one consumer among potentially many. Other consumers (governance dashboards, audit systems, reporting, other platform services) interact through the same API.

### 1.2 What does NOT change

All of the following carry over from v1.1 without modification:

- Immutable snapshot architecture (§3)
- Governed vs. Operational semantics and the Proof Rule (§4)
- Security framework: SecurityLabel, ABAC, residency, purpose limitation, handling controls (§5)
- Governance framework: stewardship, lifecycle, change control, tier-aware posture (§6)
- Component model: Attribute Dictionary, Entity & Relationship Model, Verb Dictionary, Taxonomy Registry, View Definitions, Policy & Controls Registry, Source & Evidence Registry, Derived Attributes (§7–§9)
- Context Resolution contract (§8)
- Agent control plane model (§10)
- Publish-time gate model (§12)
- All object shapes and their fields

The v1.1 document remains the authoritative specification for domain semantics. This document specifies delivery architecture and implementation strategy.

---

## 2. Dual-Implementation Strategy

### 2.1 The approach

The Semantic OS will be built twice:

1. **Rust reference implementation** — the specification-as-code. Validates every invariant, produces golden fixtures, generates the OpenAPI spec, and serves as the mechanical source of truth for all behavioural expectations.

2. **Java 21 production implementation** — the deployment target. Ported from the Rust reference (LLM-assisted), implementing the same OpenAPI contract, passing the same fixture suite, running against Oracle or MongoDB.

The Rust implementation is **not a prototype**. It is a complete, tested, runnable service that defines correct behaviour. The Java implementation is not a rewrite — it is a **mechanical port** guided by working code, a stable API specification, and a deterministic test suite.

### 2.2 Why Rust first

Rust is used as an intermediate specification language — a "compilable spec" — for several reasons:

**Type system as correctness oracle.** Rust's `enum` with data, `Option<T>`, `Result<T, E>`, exhaustive `match`, and ownership model force every design decision to be explicit. By the time the Rust code compiles and tests pass, null handling, error paths, exhaustive case coverage, and mutation boundaries are all resolved. The Java port inherits these decisions rather than discovering them through runtime failures.

**Golden fixtures with known-correct outputs.** The Rust implementation produces the conformance fixture suite: request/response pairs, gate evaluation results, ABAC decisions, derivation outputs with inherited security labels. The Java port's acceptance criterion is: pass every fixture. This is a dramatically tighter specification than a prose architecture document.

**Rust-to-Java type mapping is clean for this domain.** The domain is dominated by immutable data structures, discriminated unions, and pure functions — exactly the shapes that map well:

| Rust | Java 21 |
|------|---------|
| `struct` (all fields) | `record` |
| `enum` with variants | `sealed interface` + `record` subtypes |
| `Option<T>` | `@Nullable` / `Optional<T>` |
| `Result<T, E>` | Result type or checked exception |
| `trait` (for store layer) | `interface` |
| `impl` block (pure functions) | Static methods or service class methods |
| `#[serde(tag="kind")]` | Jackson `@JsonTypeInfo(property="kind")` |

**LLM port quality is higher from working code than from prose.** An LLM generating Java from a 1100-line architecture spec will produce code that looks plausible. An LLM porting a 200-line Rust module with tests and fixtures will produce code that is mechanically verifiable. The difference is not marginal.

### 2.3 What the Rust phase produces

Three artifacts matter. The Rust code is useful but secondary to these:

1. **OpenAPI specification** — generated from wire DTO types, stable, versioned. This is the contract that both implementations satisfy.

2. **Golden fixture suite** — normalised request/response pairs covering all core operations: publish, resolve, ABAC decide, derivation evaluate, gate check. Normalisation removes volatility (generated IDs → placeholders, timestamps → epoch, arrays → sorted by documented key).

3. **Tested domain logic** — pure functions for gate evaluation, ABAC decisions, security inheritance, derivation computation, Proof Rule enforcement, snapshot resolution. These are the behavioural specifications that the Java port must replicate exactly.

### 2.4 Port strategy

The port proceeds crate-by-crate, each an LLM-sized unit of work:

| Rust crate | Java package | Port notes |
|------------|-------------|------------|
| `semantic_os_wire` | `com.semanticos.wire` | Records + sealed interfaces. Mechanical. Port first. |
| `semantic_os_domain` | `com.semanticos.domain` | Pure logic. Most important to get right. Test against fixtures. |
| `semantic_os_store` | `com.semanticos.store` | Port the **interface** only. Java implementation writes its own Oracle/Mongo persistence. |
| `semantic_os_service` | `com.semanticos.service` | HTTP layer. Rust uses axum/actix; Java uses plain `HttpServer` or Javalin (no Spring). Thinnest layer. |
| `semantic_os_tests` | `com.semanticos.conformance` | Fixture runner. Same fixtures, Java test harness. |

### 2.5 What "no Spring" means

The Java implementation uses:

- Java 21 records, sealed interfaces, pattern matching
- Virtual threads for concurrency (no reactive/CompletableFuture chains)
- A lightweight HTTP framework (Javalin, Helidon SE, or raw `com.sun.net.httpserver`) — request routing, JSON serialisation, that's it
- Jackson for JSON (mirrors serde behaviour)
- JDBC (Oracle) or MongoDB Java driver directly — no JPA, no Hibernate, no repository abstractions
- JUnit 5 for the conformance runner

No dependency injection framework. No annotation scanning. No Spring Boot auto-configuration. The service is a `main()` that wires dependencies explicitly and starts an HTTP server. This keeps the port mechanical — a Rust `fn main()` that builds an `AppState` and starts axum maps directly to a Java `main()` that builds a `ServiceContext` and starts Javalin.

---

## 3. Wire Contract Architecture

### 3.1 Wire contract is the product

The most important artifact of the entire project is the OpenAPI specification + JSON schema + golden fixtures. Both implementations exist to produce correct behaviour under this contract. If the contract is ambiguous, fix the contract — don't let implementations diverge.

### 3.2 Wire DTO design rules

These rules constrain the API surface for portability. They apply to both implementations.

**Flat, explicit fields.** DTOs use named fields, not maps-of-maps or deeply nested generics. Prefer `Vec<Item>` with explicit `id` fields over `HashMap<Id, Item>`.

**Discriminator-based polymorphism.** Any union type uses a `kind` string discriminator:

```json
{ "kind": "Regex", "pattern": "..." }
{ "kind": "Range", "min": 0, "max": 100 }
```

Variants are shallow — no nested discriminated unions in request/response bodies.

**String IDs and timestamps.** IDs are UUID text (never binary). Timestamps are ISO-8601 strings. No floats where an integer or string is safer.

**Deterministic ordering.** Every response array has a documented sort key. Default: `(type, id)` or `(score desc, id asc)` for ranked lists. Never rely on map iteration order.

**Structured errors.** All failures return:

```json
{
  "code": "PROOF_RULE_VIOLATION",
  "message": "Operational attribute cannot satisfy evidence requirement",
  "details": [...],
  "remediation": [...],
  "snapshot_context": { "snapshot_set_id": "..." }
}
```

### 3.3 Two-model rule

- **Wire DTOs** cross the HTTP boundary. They are boring, stable, and Java-shaped.
- **Domain types** may be idiomatic to the implementation language. They convert deterministically to/from wire DTOs.

Internal types never leak into request/response payloads.

---

## 4. API Surface

### 4.1 Core endpoints

The API is small and high-value. Every endpoint corresponds to a capability that a consumer (agent, governance dashboard, audit system, workflow engine) depends on.

**Draft & Publish lifecycle:**

- `POST /drafts` — create a draft containing one or more registry object changes
- `GET /drafts/{draft_id}` — inspect a draft
- `POST /drafts/{draft_id}/validate` — run publish-time gates without committing
- `POST /publish` — atomically publish a draft → new snapshot set

**Context Resolution:**

- `POST /context/resolve` — the universal query contract (v1.1 §8)

**Policy & Security:**

- `POST /policy/evaluate` — evaluate policy rules against a subject in context
- `POST /abac/decide` — compute access decision for actor × subject × purpose

**Registry queries:**

- `GET /snapshots/{snapshot_id}` — retrieve any snapshot by ID
- `GET /objects/{object_type}/{object_id}` — resolve current active snapshot
- `GET /objects/{object_type}/{object_id}/history` — all snapshots for an object identity
- `GET /objects/{object_type}/{object_id}/at?time={iso8601}` — point-in-time resolution

**Search (MVP):**

- `POST /search` — keyword search across registry objects (full-text on name, description, aliases)

**Derivations:**

- `POST /derive` — evaluate a derivation spec against current inputs, return output with inherited security labels

**Governance metrics:**

- `GET /governance/coverage` — classification %, stewardship %, policy %, freshness %, security %, Proof Rule compliance

### 4.2 Protocol

Standard request/response JSON over HTTP. No streaming, no WebSockets, no gRPC for MVP. Both Rust and Java implementations serve the same REST contract.

Content type: `application/json`
Compatibility header: `X-SemanticOS-Compat: v1` — locks behaviour for conformance testing.

---

## 5. Storage Architecture

### 5.1 Storage is an internal detail

The HTTP layer reveals nothing about storage: no table names, no join logic, no DB-specific constraints. The store interface is defined in terms of semantic operations.

### 5.2 Store trait / interface

```
trait SemanticStore {
    // Snapshot resolution
    fn load_snapshot(snapshot_id) -> Snapshot
    fn resolve_active(object_type, object_id, as_of) -> Snapshot
    fn resolve_active_set(object_type, object_ids[], as_of) -> Snapshot[]
    fn load_history(object_type, object_id) -> Snapshot[]

    // Draft & publish
    fn create_draft(actor, artifacts[]) -> DraftId
    fn load_draft(draft_id) -> Draft
    fn publish_draft(draft_id) -> SnapshotSetId

    // Queries
    fn query_memberships(taxonomy_id, filters) -> Membership[]
    fn search(query, object_types[], limit) -> SearchResult[]
    fn load_policy_bundle(subject, jurisdiction) -> PolicyRule[]

    // Derived projections (rebuildable)
    fn write_projection(projection_type, data)
    fn load_projection(projection_type, key) -> ProjectionData
}
```

### 5.3 Implementation targets

| Implementation | Backend | Notes |
|---------------|---------|-------|
| Rust reference | PostgreSQL | SQLx compile-time checked queries. Reference schema. |
| Java production (option A) | Oracle | JDBC. Schema mirrors Rust PostgreSQL with Oracle-specific types. |
| Java production (option B) | MongoDB | Document store. Each snapshot is a document. Natural fit for full-definition snapshots. |

### 5.4 Derived projections are rebuildable

Lineage graphs, search indices, coverage metrics, and embedding vectors are derived from snapshots. They can be materialised for performance but must be rebuildable from the snapshot store. If a projection diverges from snapshot state, the projection is wrong.

---

## 6. Project Structure

### 6.1 Rust reference

```
semantic_os/
  Cargo.toml                    # workspace
  crates/
    semantic_os_wire/           # Wire DTOs, serde, OpenAPI generation
      src/
        lib.rs
        attributes.rs           # AttributeDef wire types
        entities.rs             # EntityTypeDef, RelationshipTypeDef wire
        verbs.rs                # VerbContract wire types
        taxonomies.rs           # Taxonomy, Node, Membership wire
        views.rs                # ViewDef wire
        policies.rs             # PolicyRule, EvidenceRequirement wire
        evidence.rs             # DocumentTypeDef, Observation wire
        derivations.rs          # DerivationSpec wire
        security.rs             # SecurityLabel, ActorContext, AccessDecision wire
        governance.rs           # GovernanceTier, TrustClass wire enums
        snapshot.rs             # SnapshotMeta, SnapshotStatus wire
        context.rs              # ContextResolutionRequest/Response wire
        errors.rs               # Structured error wire type
    semantic_os_domain/         # Domain types, pure logic, invariant enforcement
      src/
        lib.rs
        gates.rs                # Publish-time gates, Proof Rule checks
        abac.rs                 # ABAC evaluation (pure function)
        resolve.rs              # Context Resolution logic
        derivation.rs           # Derivation evaluation + security inheritance
        security_inherit.rs     # Security label combination rules
        snapshot_logic.rs       # Snapshot lifecycle transitions
        validation.rs           # Cross-object validation
    semantic_os_store/          # Storage trait + PostgreSQL adapter
      src/
        lib.rs
        traits.rs               # SemanticStore trait
        pg/                     # PostgreSQL implementation
          mod.rs
          queries.rs
          migrations.rs
    semantic_os_service/        # HTTP server, routing, JSON handling
      src/
        lib.rs
        main.rs
        routes/
          drafts.rs
          publish.rs
          context.rs
          policy.rs
          abac.rs
          snapshots.rs
          search.rs
          derive.rs
          governance.rs
        middleware.rs            # Logging, auth hooks, compat header
    semantic_os_tests/          # Golden fixtures + conformance runner
      fixtures/
        publish/                # publish scenarios + expected GateReports
        resolve/                # context resolution scenarios + expected responses
        abac/                   # ABAC decision scenarios
        derive/                 # derivation scenarios + inherited labels
        gates/                  # gate evaluation scenarios
      src/
        runner.rs               # Normalise + diff + report
        normalise.rs            # Strip volatile fields, sort arrays
```

### 6.2 Java 21 production (target structure)

```
semantic-os/
  src/main/java/com/semanticos/
    wire/                       # Records + sealed interfaces (from wire crate)
    domain/                     # Pure logic (from domain crate)
    store/                      # Interface + Oracle or Mongo implementation
    service/                    # HTTP routes + main
  src/test/java/com/semanticos/
    conformance/                # Same fixtures, JUnit runner
  src/test/resources/fixtures/  # Copied from Rust fixture suite
```

---

## 7. Foundational Invariants

Carried over from v1.1 and the implementation plan. These are non-negotiable in both implementations.

1. **No in-place updates for registry snapshots.** Every change produces a new immutable snapshot. INSERT only. Predecessor gets `effective_until` set.

2. **Execution, decision, and derivation records pin snapshot IDs.** The `snapshot_manifest` on DecisionRecord is mandatory. Every derivation evaluation captures DerivationSpec snapshot + input snapshots.

3. **The Proof Rule is mechanically enforced.** `governance_tier_minimum` + `trust_class_minimum` on evidence requirements. `predicate_trust_minimum` on policy predicates. Gate checks tier and trust_class of every referenced attribute against minimums — rejects on mismatch. No interpretive enforcement.

4. **ABAC / security labels apply to both tiers.** Governance tier affects workflow rigour, not security posture. An operational PII field is masked identically to a governed PII field.

5. **Operational-tier snapshots do not require governed approval.** Auto-approve semantics: `approved_by = "auto"`, still recorded. No human approval gate blocks operational iteration.

6. **Derived attributes require a DerivationSpec.** No ad-hoc derived values. Security inheritance computed from inputs. `evidence_grade = Prohibited` for operational derivations.

---

## 8. Conformance & Portability

### 8.1 Golden fixtures are mandatory

The fixture suite covers:

| Operation | Fixture verifies |
|-----------|-----------------|
| Publish | GateReport: gates passed/failed, reasons, snapshot IDs assigned |
| ABAC decide | AccessDecision: verdict, reason codes, masking plan, applied rule snapshots |
| Context resolve | ContextResolutionResponse: ranked candidates, evidence, policy verdicts, all snapshot-pinned |
| Derivation | Output value + inherited SecurityLabel + input snapshot manifest |
| Point-in-time | Same operations with `as_of` parameter returning historical snapshots |
| Proof Rule | Rejection when operational attribute referenced by governed evidence requirement |
| Promotion | New snapshot with tier change, all governed gates applied |

### 8.2 Normalisation

The conformance runner normalises before diff:

- Generated UUIDs → deterministic placeholders (sorted, assigned sequentially)
- Timestamps → epoch or relative offsets
- Arrays → sorted by documented key before comparison
- Floating-point scores → rounded to documented precision

### 8.3 Definition of done: portability-ready

The Rust reference is portability-ready when:

- OpenAPI spec is generated and stable
- Golden fixture suite covers all core invariants (§8.1)
- A Java implementation can be built by implementing the same OpenAPI + passing the same fixtures
- No endpoint behaviour depends on Rust-specific assumptions (ordering, lifetime semantics, hidden defaults)

The Java production is acceptance-ready when:

- All golden fixtures pass
- Store layer backed by Oracle or MongoDB
- Performance meets deployment requirements
- `X-SemanticOS-Compat: v1` header locks v1 behaviour

---

## 9. What This Document Does Not Cover

- **v1.1 domain model details.** All object shapes, security framework semantics, governance rules, Context Resolution contract, gate logic, and derivation rules are specified in `semantic-os-v1.1.md`. This document does not duplicate them.
- **Implementation phases or TODO.** This is an architecture document, not a build plan. Implementation sequencing will be a separate document.
- **ob-poc integration.** How ob-poc (DSL parser, BPMN engine, MCP agent, CLI) consumes the standalone service API is an integration concern, not a Semantic OS concern.
- **Deployment topology.** Container orchestration, scaling, monitoring, and operational concerns are outside scope.

---

## 10. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Rust store layer too idiomatic to port | Store trait is narrow and semantic (§5.2). Java writes its own persistence — only the interface ports. |
| Async Rust patterns don't map to Java | Java 21 virtual threads replace tokio. Service layer is thin. No reactive chains. |
| Over-engineering Rust version | Portability guidelines: if a Rust construct can't be explained as one Java construct, simplify it. |
| LLM context window for port | Port crate-by-crate. Each crate is an LLM-sized unit. Wire DTOs first, domain second, store interface third, service last. |
| Fixture suite insufficient | Fixtures must cover Proof Rule, point-in-time, security inheritance, ABAC, and every gate. Add fixtures when bugs are found — in both implementations. |
| Wire contract drift between implementations | Single OpenAPI spec generated from Rust wire types. Java implements the same spec. Compat header locks behaviour. |

---

## Appendix A: Relationship to v1.1

| v1.1 Section | Standalone Service Mapping |
|-------------|---------------------------|
| §3 Immutable Snapshots | Store trait + snapshot resolution endpoints |
| §4 Governed/Operational | Domain logic in `semantic_os_domain` + gate checks |
| §5 Security Framework | ABAC endpoint + SecurityLabel on all wire DTOs |
| §6 Governance Framework | Governance coverage endpoint + publish-time gates |
| §7 Component Map | Registry query endpoints per object type |
| §8 Context Resolution | `POST /context/resolve` endpoint |
| §9 Capability Catalogue | Wire DTOs in `semantic_os_wire` |
| §10 Agent Control Plane | Decision records and plans as registry objects |
| §11 Derived Projections | Rebuildable from store; served via search + metrics endpoints |
| §12 Enforcement | Publish and runtime gates in domain logic |

## Appendix B: Version History

| Version | Date | Key Changes |
|---------|------|-------------|
| v1.1 | Feb 2026 | ob-poc embedded Semantic OS — authoritative domain specification |
| v2.0 | Feb 2026 | Standalone service extraction; dual-implementation strategy (Rust reference → Java 21 production); wire contract architecture; storage abstraction; conformance framework |
