# ob-poc Rust → Java 26 Port Approach

> **Version:** 0.1  
> **Date:** 28 February 2026  
> **Status:** Living document — captures decisions as they're made  
> **Authors:** Adam TC / Claude Opus 4.6  
> **Platform:** ob-poc (BNY Mellon Enterprise Onboarding)  
> **Source commit:** `e440bfd`

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 28 Feb 2026 | Adam TC / Claude Opus 4.6 | Initial capture: guardrails, pattern mapping, friction analysis, module strategy, governance type design |

---

## 1. Purpose

This document is the single reference for porting ob-poc from Rust to Java 26. It captures:

- Non-negotiable guardrails (what Java 26 features to use, what to reject)
- Pattern-by-pattern mapping from Rust idioms to Java 26 equivalents
- Friction zones where Rust patterns don't map cleanly
- Module structure and port ordering
- Decisions already made (with rationale so they don't get relitigated)

This is a living document. Every port decision gets recorded here before code is written.

---

## 2. Guardrails

These are non-negotiable. They are not preferences, they are constraints. Any code that violates them is rejected.

### 2.1 Java 26 Only

**Java 26 is the floor and the ceiling.** No fallbacks to Java 21, 17, or 11. No "compatible with earlier versions" compromises. If Java 26 has a better construct, use it. If a library doesn't support Java 26 features, find one that does or write the code by hand.

Java 26 features that are **mandatory** in the port:

| Feature | JEP | Use In Port |
|---------|-----|-------------|
| Records | 395 (Java 16, stable) | All value types, DTOs, events, snapshots |
| Sealed interfaces | 409 (Java 17, stable) | All ADTs (PruneReason, TocTouResult, SurfaceReason, VerbExecutor, etc.) |
| Pattern matching for switch | 441 (Java 21, stable) | Exhaustive dispatch on sealed hierarchies — **no `default` branches** |
| Virtual threads | 444 (Java 21, stable) | All async/concurrent operations — replaces tokio |
| Structured concurrency | 480 (Java 23 preview → 26 stable) | Fan-out operations (GLEIF + BODS + sanctions parallel calls) |
| Scoped values | 481 (Java 23 preview → 26 stable) | Request-scoped context (Principal, session state) — replaces thread-locals |
| String templates | 465 (Java 21 preview → 26 evolution) | Logging, error messages, JSON construction |
| Unnamed variables `_` | 456 (Java 22, stable) | Pattern matching, lambda parameters |
| Stream gatherers | 485 (Java 24 preview → 26 stable) | Complex collection transformations |
| Foreign Function & Memory API | 454 (Java 22, stable) | If needed for native lib interop (embedding models, etc.) |
| Module system (JPMS) | 261 (Java 9, stable) | Module boundaries match Rust crate boundaries |

### 2.2 Absolutely No Spring

**No Spring Boot. No Spring Framework. No Spring anything.** This is not a "keep it light" preference — Spring is architecturally incompatible with the port's goals.

Specifically banned:

| Banned | Why | Replacement |
|--------|-----|-------------|
| Spring DI (`@Autowired`, `@Component`, `@Bean`) | Runtime magic, hidden wiring, circular dependency risk | Constructor injection via plain Java. Factory methods. Module `provides` clauses. |
| Spring Web (`@RestController`, `@RequestMapping`) | Annotation-driven dispatch obscures control flow | [Javalin](https://javalin.io/) or plain `java.net.http.HttpServer` (Java 18+). Explicit route registration. |
| Spring Data JPA / Hibernate | Entity class explosion, N+1 queries, opaque SQL | Direct JDBC with `java.sql` or lightweight query builder. Governed queries as typed methods. |
| Spring Security | Filter chain complexity, magic authentication | Explicit `Principal` threading via type system (see §5). |
| Spring Boot auto-configuration | Opaque startup, classpath scanning, 5–15s startup | Explicit `main()` with hand-wired dependencies. Sub-second startup. |
| Spring AOP / `@Transactional` | Runtime proxy magic, silent behaviour changes | Explicit transaction boundaries: `try (var tx = pool.begin()) { ... tx.commit(); }` |
| Lombok | Hides code from the compiler, breaks IDE analysis | Java 26 records eliminate the need. |
| MapStruct | Code generation for something records already solve | Pattern matching + record constructors. |

**The principle:** If you can't trace every code path by reading the source top-to-bottom without consulting annotation documentation, it doesn't belong in this port.

### 2.3 POJO Java 26

The port produces Plain Old Java Objects using Java 26 language features. The dependency tree is:

```
java.base (JDK)
java.sql  (JDK)
java.net.http (JDK)
java.security (JDK — MessageDigest for SHA-256)
├── PostgreSQL JDBC driver (single external dep for database)
├── Javalin or equivalent (single external dep for HTTP — optional, can use JDK HttpServer)
├── SLF4J + backend (logging)
└── JUnit 5 (test only)
```

That's it. No framework. No annotation processor runtime. No classpath scanning. No reflection-based wiring. Every object is constructed by calling `new` or a factory method. Every dependency is visible in the constructor signature.

### 2.4 The Compiler Is The Framework

The entire point of porting to Java 26 (rather than Java 17 or 21) is to use the type system as the enforcement layer. Sealed interfaces + records + exhaustive pattern matching give Java the same "make illegal states unrepresentable" capability that Rust's enum system provides.

**Port mantra:** If Rust's compiler catches it, Java 26's compiler must catch it. If it can't, it becomes a startup-time validation (fail-fast before first request). Runtime errors for governance violations are bugs in the port.

---

## 3. Source Codebase Profile

### 3.1 Scale

| Metric | Count |
|--------|-------|
| Total Rust lines (src + crates) | ~324,000 |
| Crates | 21 |
| YAML verb definitions | 141 files, 642 verbs |
| SQL migrations | 41 |
| PostgreSQL tables | 92+ |
| React components | 30 (.tsx files) |

### 3.2 Dependency Hotspots (by grep count)

| Rust Dependency | Usage Count | Port Impact |
|----------------|-------------|-------------|
| serde / serde_json | 4,374 | High — every struct serialises. See §4.4. |
| sqlx (async Postgres) | 3,839 | High — every database call. See §4.5. |
| `#[cfg(feature = ...)]` | 1,257 | Medium — conditional compilation. See §4.6. |
| axum (HTTP framework) | 765 | Medium — route handlers. See §4.7. |
| tracing | 675 | Low — direct SLF4J mapping. |
| enums with data (ADTs) | 415 | High — core type modelling. See §4.1. |
| lifetime annotations | 273 | **Zero** — disappear entirely in Java. |
| tokio (async runtime) | 189 | Low — virtual threads replace. |
| `Arc<dyn Trait>` (trait objects) | 81 | Low — plain interfaces. |
| `#[governed_query]` proc macro | 5 (currently) | High architectural impact — see §5. |

### 3.3 Rust Crate → Java Module Mapping

| Rust Crate | LOC | Purpose | Java Module | Port Priority |
|-----------|-----|---------|-------------|---------------|
| `sem_os_core` | 15,212 | Pure governance logic: CCIR, gates, ABAC, types | `com.bnym.semos.core` | **P0** — no DB dependency, pure types + logic |
| `dsl-core` | 6,367 | Parser, AST, compiler, DAG | `com.bnym.dsl.core` | P1 — pure logic, nom → hand-rolled recursive descent |
| `ob-poc-types` | ~2,000 | Shared types (ChatResponse, VerbProfile, etc.) | `com.bnym.onboarding.types` | P0 — records, sealed interfaces |
| `governed_query_proc` | ~800 | Compile-time governance proc macro | `com.bnym.governance.validator` | P1 — becomes type system + startup validator. See §5. |
| `ob-poc-macros` | ~300 | `#[derive(IdType)]` for UUID newtypes | Eliminated — Java records with `Uuid` field | — |
| `sem_os_client` | ~500 | SemOsClient trait + types | `com.bnym.semos.client` | P0 — interface + record types |
| `sem_os_postgres` | ~3,000 | SemReg PostgreSQL implementation | `com.bnym.semos.postgres` | P2 — JDBC implementation |
| `sem_os_server` | ~2,000 | SemReg HTTP server | `com.bnym.semos.server` | P2 |
| `ob-execution-types` | ~1,500 | Verb execution types | `com.bnym.onboarding.execution` | P1 |
| `entity-gateway` | ~2,000 | Entity resolution, GLEIF | `com.bnym.onboarding.entity` | P2 |
| `ob-semantic-matcher` | ~1,500 | BGE embedding search | `com.bnym.semantic.matcher` | P3 — may keep as Rust sidecar |
| Main `ob-poc` binary | ~290,000 | Everything else: routes, orchestrator, agents, MCP, verb search | Split across multiple modules | Phased |

---

## 4. Pattern Mapping: Rust → Java 26

### 4.1 Tagged Enums (ADTs) → Sealed Interfaces + Records

**Rust pattern:** Enums with per-variant data, exhaustive `match`.

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PruneReason {
    AbacDenied { actor_role: String, required: String },
    EntityKindMismatch { verb_kinds: Vec<String>, subject_kind: String },
    TierExcluded { tier: String, reason: String },
    TaxonomyNoOverlap { verb_taxonomies: Vec<String> },
    PreconditionFailed { precondition: String },
    AgentModeBlocked { mode: String },
    PolicyDenied { policy_fqn: String, reason: String },
}
```

**Java 26 equivalent:**

```java
public sealed interface PruneReason {
    record AbacDenied(String actorRole, String required) implements PruneReason {}
    record EntityKindMismatch(List<String> verbKinds, String subjectKind) implements PruneReason {}
    record TierExcluded(String tier, String reason) implements PruneReason {}
    record TaxonomyNoOverlap(List<String> verbTaxonomies) implements PruneReason {}
    record PreconditionFailed(String precondition) implements PruneReason {}
    record AgentModeBlocked(String mode) implements PruneReason {}
    record PolicyDenied(String policyFqn, String reason) implements PruneReason {}
}
```

**Dispatch:**

```java
// Rust: match reason { PruneReason::AbacDenied { actor_role, required } => ... }
// Java 26:
String display = switch (reason) {
    case PruneReason.AbacDenied(var role, var req) ->
        "ABAC denied: role %s, required %s".formatted(role, req);
    case PruneReason.EntityKindMismatch(var kinds, var subject) ->
        "Entity kind mismatch: verb expects %s, got %s".formatted(kinds, subject);
    case PruneReason.TierExcluded(var tier, var r) ->
        "Tier excluded: %s (%s)".formatted(tier, r);
    // ... every variant — NO default branch
};
```

**Friction: LOW.** Direct mapping. Sealed interfaces provide the same exhaustiveness guarantee as Rust enums *if and only if* you never add a `default` branch.

**Project rule: `default` branches on sealed interface switches are forbidden.** Enforced by code review and a grep-based CI check: `grep -rn 'default\s*->' --include="*.java" | grep -v '// default-ok'` must return zero results (with an escape hatch comment for genuinely appropriate uses like `Map.getOrDefault`).

### 4.2 Structs → Records

**Rust:**
```rust
#[derive(Debug, Clone, Serialize)]
pub struct ContextEnvelope {
    pub allowed_verbs: HashSet<String>,
    pub pruned_verbs: Vec<PrunedVerb>,
    pub fingerprint: AllowedVerbSetFingerprint,
    pub computed_at: DateTime<Utc>,
    deny_all: bool,
}
```

**Java 26:**
```java
public record ContextEnvelope(
    Set<String> allowedVerbs,
    List<PrunedVerb> prunedVerbs,
    AllowedVerbSetFingerprint fingerprint,
    Instant computedAt,
    boolean denyAll
) {
    // Compact constructor for defensive copies
    public ContextEnvelope {
        allowedVerbs = Set.copyOf(allowedVerbs);
        prunedVerbs = List.copyOf(prunedVerbs);
        Objects.requireNonNull(fingerprint);
    }
}
```

**Friction: ZERO.** Records are a better fit than Rust structs for value types — they give you `equals()`, `hashCode()`, `toString()` for free, which Rust requires `#[derive(PartialEq, Hash, Debug)]` for.

**Key difference:** Rust structs are mutable-by-binding (`let mut`). Java records are always immutable. This is *better* for our use case — governance types should never be mutated after construction.

### 4.3 Newtype Pattern → Records with Single Field

**Rust:**
```rust
pub struct AllowedVerbSetFingerprint(pub String);
```

**Java 26:**
```java
public record AllowedVerbSetFingerprint(String value) {
    public AllowedVerbSetFingerprint {
        Objects.requireNonNull(value);
    }

    public static AllowedVerbSetFingerprint compute(Set<String> allowedFqns) {
        var sorted = allowedFqns.stream().sorted().toList();
        var digest = MessageDigest.getInstance("SHA-256");
        sorted.forEach(fqn -> {
            digest.update(fqn.getBytes(StandardCharsets.UTF_8));
            digest.update((byte) '\n');
        });
        return new AllowedVerbSetFingerprint(
            "v1:" + HexFormat.of().formatHex(digest.digest()));
    }
}
```

**Friction: ZERO.** The Rust `#[derive(IdType)]` proc macro in `ob-poc-macros` that generates UUID newtype boilerplate is eliminated entirely — a Java record with a `UUID` field does the same thing with zero code generation.

### 4.4 Serde → No Jackson, Minimal Serialisation

**Decision: No Jackson.** Jackson is the Spring ecosystem's default serialiser and carries 2.5MB of transitive dependencies, annotation-driven configuration, and runtime reflection.

**Strategy:** The port uses the **minimum viable serialiser** for each boundary:

| Boundary | Approach |
|----------|----------|
| HTTP API (JSON responses) | Hand-written `toJson()` methods on records using `StringTemplate` or a lightweight library (e.g., `com.google.code.gson:gson` at 280KB, or `jakarta.json` JSON-P at 60KB) |
| Database (JSONB columns) | Same serialiser as HTTP — PostgreSQL JDBC accepts raw JSON strings |
| Internal (between modules) | No serialisation — pass Java objects directly |
| Wire format (Rust ↔ Java interop during migration) | JSON, validated by round-trip tests |
| YAML verb definitions | `org.yaml.snakeyaml` (already minimal, 300KB) or `com.fasterxml.jackson.dataformat:jackson-dataformat-yaml` (only if Jackson is unavoidable for YAML — evaluate) |

**The serde `#[serde(tag = "kind")]` problem:**

Rust's serde produces flat tagged JSON: `{"kind": "abac_denied", "actor_role": "analyst"}`. Whatever serialiser we choose must produce the same wire format during the migration period when both Rust and Java services coexist.

**Port rule:** Before porting any type that crosses a wire boundary, write a `WireFormatTest` that:
1. Serialises the Java type to JSON
2. Asserts byte-identical output to the Rust serde output (captured as test fixtures)
3. Deserialises the Rust JSON into the Java type
4. Round-trips successfully

These tests are the migration safety net. They run in CI on every commit.

### 4.5 sqlx → JDBC + Governed Query Types

**Rust pattern:**
```rust
let row = sqlx::query_as!(MyRow, "SELECT id, name FROM entities WHERE id = $1", id)
    .fetch_one(pool)
    .await?;
```

`sqlx` checks SQL syntax and column types at compile time against a live database. This is a significant safety property.

**Java 26 equivalent:**

No compile-time SQL checking in Java. The port compensates with:

1. **Typed query methods** — one method per query, returning a record:

```java
public record EntityRow(UUID id, String name) {}

public static EntityRow fetchById(Connection conn, UUID id) throws SQLException {
    try (var stmt = conn.prepareStatement("SELECT id, name FROM entities WHERE id = ?")) {
        stmt.setObject(1, id);
        try (var rs = stmt.executeQuery()) {
            if (!rs.next()) throw new NotFoundException("Entity " + id);
            return new EntityRow(rs.getObject("id", UUID.class), rs.getString("name"));
        }
    }
}
```

2. **CI-time SQL validation** — a test suite that runs every query method against a real PostgreSQL instance (same as sqlx's `DATABASE_URL` approach, but as a test instead of a compiler plugin).

3. **GovernedQuery wrapper** — see §5 for how `@GovernedQuery` checks layer on top.

**Friction: MEDIUM.** Loss of compile-time SQL validation is the single biggest safety regression in the port. Compensated by mandatory CI tests against live Postgres. Non-negotiable: every query method must have a corresponding integration test.

### 4.6 `#[cfg(feature = "...")]` → Java Modules (JPMS)

**Rust pattern:** 1,257 conditional compilation directives. The `database` feature flag is the most significant — entire code paths are erased from the binary when building without it.

**Java 26 equivalent:** JPMS modules with explicit `requires` and `exports`.

```
com.bnym.semos.core          (no database dependency — pure types + logic)
com.bnym.semos.postgres       (requires java.sql — database implementation)
com.bnym.onboarding.types    (no database dependency)
com.bnym.onboarding.api      (requires com.bnym.semos.core, com.bnym.onboarding.types)
com.bnym.onboarding.database (requires java.sql, com.bnym.onboarding.types)
```

`com.bnym.semos.core` physically cannot reference database types — the module system prevents it at compile time. This replaces `#[cfg(feature = "database")]` with a stronger guarantee: not just "this code is excluded from the binary" but "this code cannot exist in this module."

**Friction: LOW.** JPMS is a better fit than Rust feature flags for this use case. Feature flags are additive (compile more code in); JPMS modules are restrictive (each module declares exactly what it can see). The port should be *stricter* than the Rust original.

### 4.7 Axum Routes → Explicit Handler Registration

**Rust pattern:**
```rust
Router::new()
    .route("/api/session/:id/chat", post(handle_chat))
    .route("/api/session/:id/commands", get(handle_commands))
```

**Java 26 (Javalin or JDK HttpServer):**
```java
// Option A: Javalin (~100KB, no reflection)
var app = Javalin.create();
app.post("/api/session/{id}/chat", ctx -> handleChat(ctx));
app.get("/api/session/{id}/commands", ctx -> handleCommands(ctx));

// Option B: JDK HttpServer (zero dependencies)
var server = HttpServer.create(new InetSocketAddress(8080), 0);
server.createContext("/api/session", exchange -> routeSession(exchange));
```

**Friction: ZERO.** Both options are explicit, readable, no annotations. Javalin is preferred for ergonomics; JDK HttpServer for zero-dependency purity.

### 4.8 Async/Await → Virtual Threads

**Rust pattern:**
```rust
async fn resolve_sem_reg_verbs(ctx: &OrchestratorContext) -> ContextEnvelope {
    let response = ctx.sem_os_client.resolve_context(&principal, request).await?;
    // ...
}
```

**Java 26:**
```java
// Virtual threads — no async/await syntax needed, blocking is free
ContextEnvelope resolveSemRegVerbs(OrchestratorContext ctx) {
    var response = ctx.semOsClient().resolveContext(principal, request);
    // blocking call, but on a virtual thread — no thread pool exhaustion
    return buildEnvelope(response);
}

// Fan-out with structured concurrency
ContextEnvelope resolveWithParallelChecks(OrchestratorContext ctx) {
    try (var scope = StructuredTaskScope.ShutdownOnFailure()) {
        var gleifTask = scope.fork(() -> gleifClient.lookup(entityId));
        var sanctionsTask = scope.fork(() -> sanctionsClient.screen(entityId));
        var bodsTask = scope.fork(() -> bodsClient.resolve(entityId));
        scope.join().throwIfFailed();
        return combine(gleifTask.get(), sanctionsTask.get(), bodsTask.get());
    }
}
```

**Friction: NEGATIVE (Java is simpler here).** Rust's async is viral (every caller must be async, every boundary must be `.await`'d). Java's virtual threads are transparent — blocking code just works. The port removes all `async`/`.await` machinery. 7,637 `.await` sites become normal method calls.

### 4.9 `Arc<dyn Trait>` → Interface References

**Rust:**
```rust
pub sem_os_client: Option<Arc<dyn SemOsClient>>,
pub type SharedLexicon = Arc<dyn LexiconService>;
```

**Java 26:**
```java
// No Arc, no dyn, no Send+Sync bounds. Just an interface.
private final SemOsClient semOsClient;  // nullable via Optional if needed
private final LexiconService lexicon;
```

**Friction: ZERO.** `Arc` is Rust's reference-counted pointer for shared ownership across threads. Java's GC handles this. `dyn Trait` is a trait object (virtual dispatch) — Java interfaces are virtual dispatch by default. The 81 `Arc<dyn ...>` sites become plain interface references.

### 4.10 `Vec::retain()` / Iterator Chains → Streams

**Rust:**
```rust
filtered_candidates.retain(|v| allowed.contains(&v.verb));
let profiles: Vec<VerbProfile> = candidates.iter()
    .filter(|c| !blocked.contains(&c.verb))
    .map(|c| build_profile(c))
    .collect();
```

**Java 26:**
```java
filteredCandidates.removeIf(v -> !allowed.contains(v.verb()));
var profiles = candidates.stream()
    .filter(c -> !blocked.contains(c.verb()))
    .map(this::buildProfile)
    .toList();
```

**Friction: ZERO.** 1:1 mapping.

### 4.11 Ownership & Borrowing → Disappears

Rust's 2,802 ownership-related patterns (`Arc<>`, `Box<dyn>`, `&mut`, `RefCell`, `Mutex<>`, `RwLock<>`) have no Java equivalent because Java has garbage collection. They simply disappear.

**The 273 lifetime annotations** (`<'a>`, `&'a`, `<'_>`) likewise disappear entirely.

This is the single largest reduction in code volume during the port. Roughly 10–15% of Rust code is ownership/lifetime machinery that has zero Java equivalent.

### 4.12 Tracing → SLF4J

**Rust:**
```rust
tracing::info!(verb = %verb_fqn, "TOCTOU recheck: still allowed");
tracing::warn!(
    pruned_count = pruned,
    remaining = results.len(),
    "VerbSearch: SemReg allowed_verbs filter removed candidates"
);
```

**Java 26:**
```java
logger.info("TOCTOU recheck: still allowed, verb={}", verbFqn);
logger.warn("VerbSearch: SemReg allowed_verbs filter removed candidates, pruned={}, remaining={}",
    pruned, results.size());
```

**Friction: LOW.** Structured logging (key=value pairs in tracing spans) is slightly more ergonomic in Rust's `tracing` crate than in SLF4J. For structured logging in Java, use SLF4J's `MDC` (Mapped Diagnostic Context) or a structured logging backend like `logback-logstash-encoder`.

---

## 5. Compile-Time Governance: The Type System Strategy

This is the highest-value section of the port. The Rust codebase uses `#[governed_query]` proc macros to enforce governance at compile time. The Java 26 port replaces these with a **type-level construction chain** that makes governance violations unrepresentable.

### 5.1 The Five Governance Checks

| # | Rust Proc Macro Check | Type-Enforceable? | Java 26 Mechanism |
|---|----------------------|-------------------|-------------------|
| 1 | Verb must be Active (not Deprecated) | No — data-dependent | Startup-time validator (§5.3) |
| 2 | Governed-tier verbs require `&Principal` | **Yes** | `GovernedContext` record requires `Principal` by construction |
| 3 | PII verbs require `allow_pii = true` | **Yes** | `PiiContext` only constructible from `GovernedContext.authorizePii()` |
| 4 | Proof trust requires Governed tier | **Yes** | `ProofContext` only constructible from `GovernedContext.elevateToProof()` |
| 5 | Referenced attributes must be Active | No — data-dependent | Startup-time validator (§5.3) |

Three of five become **compile-time type constraints**. Two remain as **startup-time validation** (fail-fast before first request). This is a better split than Rust's all-or-nothing proc macro approach — the type checks are faster, more IDE-visible, and don't require a governance cache binary.

### 5.2 The Context Construction Chain

The chain forms a **narrowing hierarchy** — each level is only constructible from the previous:

```
OperationalContext          (no Principal required)
    ↓ requires Principal
GovernedContext             (Principal required by construction)
    ├─ .authorizePii()  →  PiiContext     (PII access explicit)
    └─ .elevateToProof() → ProofContext   (Proof trust + Governed tier)
```

```java
// LAYER 1: Operational — session.*, view.*, agent.* verbs
public record OperationalContext(UUID sessionId, GovernanceTier tier, TrustClass trustClass) {
    public OperationalContext {
        if (tier == GovernanceTier.GOVERNED)
            throw new IllegalArgumentException("Use GovernedContext for GOVERNED tier");
    }
}

// LAYER 2: Governed — most business verbs. Principal REQUIRED by construction.
public record GovernedContext(UUID sessionId, Principal principal, TrustClass trustClass) {
    public GovernedContext { Objects.requireNonNull(principal); }

    public PiiContext authorizePii(String justification) {
        return new PiiContext(sessionId, principal, justification);
    }

    public ProofContext elevateToProof() {
        if (trustClass != TrustClass.PROOF)
            throw new IllegalStateException("Cannot elevate: trust class is " + trustClass);
        return new ProofContext(sessionId, principal);
    }
}

// LAYER 3a: PII — only reachable through GovernedContext.authorizePii()
public record PiiContext(UUID sessionId, Principal principal, String justification) {
    public PiiContext { Objects.requireNonNull(principal); Objects.requireNonNull(justification); }
}

// LAYER 3b: Proof — only reachable through GovernedContext.elevateToProof()
public record ProofContext(UUID sessionId, Principal principal) {
    public ProofContext { Objects.requireNonNull(principal); }
    public GovernanceTier tier() { return GovernanceTier.GOVERNED; }
    public TrustClass trustClass() { return TrustClass.PROOF; }
}
```

### 5.3 Verb Executor Hierarchy

```java
public sealed interface VerbExecutor
    permits OperationalVerb, GovernedVerb, PiiVerb, ProofVerb {
    String fqn();
    GovernanceTier requiredTier();
}

public non-sealed interface OperationalVerb extends VerbExecutor {
    Result execute(OperationalContext ctx, VerbArgs args);
}

public non-sealed interface GovernedVerb extends VerbExecutor {
    Result execute(GovernedContext ctx, VerbArgs args);
    //              ^^^^^^^^^^^^^^^^ — impossible to call without Principal
}

public non-sealed interface PiiVerb extends VerbExecutor {
    Result execute(PiiContext ctx, VerbArgs args);
    //              ^^^^^^^^^^ — impossible to reach without authorizePii()
}

public non-sealed interface ProofVerb extends VerbExecutor {
    Result execute(ProofContext ctx, VerbArgs args);
    //              ^^^^^^^^^^^^ — impossible to reach without elevateToProof()
}
```

### 5.4 Exhaustive Dispatch (No Default Branch)

```java
public final class VerbDispatcher {
    public Result dispatch(VerbExecutor verb, ExecutionContext execCtx, VerbArgs args) {
        return switch (verb) {
            case OperationalVerb op -> op.execute(execCtx.operational(), args);
            case GovernedVerb gov   -> gov.execute(execCtx.governed(), args);
            case PiiVerb pii        -> pii.execute(execCtx.governed().authorizePii(args.piiJustification()), args);
            case ProofVerb proof    -> proof.execute(execCtx.governed().elevateToProof(), args);
            // NO default. Adding a 5th permit forces compile error HERE.
        };
    }
}
```

### 5.5 Startup-Time Governance Validator (Checks 1 & 5)

```java
public final class GovernanceCacheValidator {
    public static void validateOnStartup(
            List<VerbExecutor> registeredVerbs,
            GovernanceCache cache) {

        var violations = new ArrayList<String>();

        for (var verb : registeredVerbs) {
            var entry = cache.lookup(verb.fqn());
            // Check 1: Verb lifecycle
            if (entry == null)
                violations.add("Verb %s not in governance cache".formatted(verb.fqn()));
            else if (entry.lifecycle() != Lifecycle.ACTIVE)
                violations.add("Verb %s is %s, expected ACTIVE".formatted(verb.fqn(), entry.lifecycle()));

            // Check 5: Attribute lifecycle
            if (verb instanceof AttributeConsumer ac) {
                for (var attr : ac.referencedAttributes()) {
                    var a = cache.lookupAttribute(attr);
                    if (a == null || a.lifecycle() != Lifecycle.ACTIVE)
                        violations.add("Verb %s references %s attribute %s".formatted(
                            verb.fqn(), attr, a == null ? "UNKNOWN" : a.lifecycle()));
                }
            }
        }

        if (!violations.isEmpty())
            throw new GovernanceCacheViolation(
                "Governance validation failed:\n  " + String.join("\n  ", violations));
    }
}
```

### 5.6 Comparison: Proc Macro vs Type Chain

| Property | Rust `#[governed_query]` | Java 26 Type Chain |
|----------|------------------------|--------------------|
| When checked | Compile time (reads binary cache) | Compile time (type system) + startup (cache) |
| IDE visibility | None (proc macros are opaque) | Full (IDE shows type errors inline) |
| Requires governance cache | Yes, always | Only for checks 1 & 5 |
| Adding a new governance tier | Edit proc macro code | Add new sealed interface variant → compiler finds all dispatch sites |
| Error messages | Custom `compile_error!()` | Java compiler's standard "type mismatch" / "missing case" |
| Test coverage | Proc macro tests + integration | Standard unit tests on record construction |
| Refactoring safety | High (compiler enforces) | Higher (IDE refactoring understands the types) |

---

## 6. Friction Zones: What Doesn't Port Cleanly

### 6.1 HIGH FRICTION: nom Parser Combinators → Recursive Descent

The `dsl-core` crate uses nom for S-expression parsing (~1,036 lines). Nom's combinator style (e.g., `delimited(char('('), many0(alt((parse_verb_call, parse_literal))), char(')'))`) has no Java equivalent.

**Port strategy:** Hand-written recursive descent parser. S-expressions are simple enough (balanced parens, atoms, strings) that a hand-written parser is arguably clearer than combinators. Estimate: 800–1,000 lines of Java, well-tested.

**Risk: LOW.** The grammar is small and well-defined. Recursive descent is the standard approach in Java and generates excellent error messages.

### 6.2 HIGH FRICTION: Feature Flags (1,257 `#[cfg]` sites)

Already addressed in §4.6. The strategy is JPMS modules, not runtime flags. But the migration requires auditing all 1,257 sites and deciding which module each belongs to.

**Port strategy:** Build a `feature_audit.csv` as part of the port planning. Each `#[cfg]` site gets classified: module boundary, dead code elimination, or test-only code.

### 6.3 MEDIUM FRICTION: Serde Wire Format Compatibility

Already addressed in §4.4. The 4,374 serde sites are the largest mechanical porting effort. Most are `#[derive(Serialize, Deserialize)]` on structs — these become `toJson()` / `fromJson()` methods on records.

**Risk: MEDIUM.** Wire format compatibility during Rust/Java coexistence. Mitigated by `WireFormatTest` suite (see §4.4).

### 6.4 MEDIUM FRICTION: sqlx Compile-Time SQL → Runtime Tests

Already addressed in §4.5. Loss of `query_as!` compile-time checking. Mitigated by mandatory CI integration tests.

### 6.5 LOW FRICTION: Embedding Search (BGE-small-en-v1.5)

The `ob-semantic-matcher` crate uses ONNX Runtime for running the BGE embedding model. Java options:

1. **Keep as Rust sidecar** — the embedding service runs as a separate process, Java calls it via HTTP/gRPC. Simplest migration path.
2. **ONNX Runtime Java bindings** — `ai.onnxruntime:onnxruntime` provides Java API. Works but adds a native library dependency.
3. **Java Foreign Function & Memory API** — call the ONNX C API directly via FFM. Maximum control, minimum dependencies, Java 26 stable feature.

**Recommendation:** Start with sidecar (option 1) during port. Evaluate migration to option 2 or 3 after core port is complete.

### 6.6 ZERO FRICTION: Ownership, Lifetimes, Borrowing

Disappear entirely. No port work needed. ~10–15% code volume reduction.

---

## 7. Port Ordering

### 7.1 Phase 0: Foundation Types (Week 1)

Port the types that everything else depends on. These are pure — no database, no HTTP, no I/O.

- `GovernanceTier`, `TrustClass`, `Lifecycle` enums → Java enums
- `Principal`, `ActorContext` → Java records
- `AllowedVerbSetFingerprint`, `SurfaceFingerprint` → Java records with `compute()` factory
- `PruneReason`, `TocTouResult`, `SurfaceReason`, `PruneLayer` → sealed interfaces
- `ContextEnvelope`, `SessionVerbSurface`, `SurfaceVerb`, `ExcludedVerb` → records
- Governance context chain: `OperationalContext` → `GovernedContext` → `PiiContext` → `ProofContext`
- `VerbExecutor` sealed interface hierarchy
- `VerbSurfaceFailPolicy` enum
- Wire format tests for every type that crosses Rust ↔ Java boundary

**Exit criterion:** All types compile, all wire format tests pass.

### 7.2 Phase 1: Governance Core (Weeks 2–3)

Port `sem_os_core` — the CCIR pipeline, ABAC evaluation, gates, context resolution.

- `evaluate_abac()` → Java method on typed context chain
- CCIR 9-step resolution pipeline → Java method chain
- Governance gates (4 technical gates) → Java validators
- `GovernanceCacheValidator` startup check
- Unit tests: mock inputs → expected ContextEnvelope

**Exit criterion:** CCIR pipeline produces identical ContextEnvelopes (verified by wire format comparison).

### 7.3 Phase 2: DSL Core (Weeks 3–4)

Port `dsl-core` — parser, AST, compiler, DAG.

- nom parser → hand-written recursive descent
- AST types → sealed interface hierarchy (Program, Statement, VerbCall, etc.)
- Compiler → Java method chain
- DAG builder + topological sort → standard graph algorithms

**Exit criterion:** Parse every verb YAML definition's example S-expressions. AST matches Rust output.

### 7.4 Phase 3: Session & Verb Surface (Weeks 4–5)

Port the `SessionVerbSurface` computation from the architecture paper (SESSION_VERB_SURFACE.md v1.1).

- `compute_session_verb_surface()` → Java method composing all governance layers
- `VerbDispatcher` type-safe dispatch
- Session state management
- Dual fingerprint computation

**Exit criterion:** Same inputs produce identical verb surfaces (compared by fingerprint).

### 7.5 Phase 4: Database Layer (Weeks 5–7)

Port the PostgreSQL access layer.

- `sem_os_postgres` → JDBC implementations of SemOsClient
- Governed query methods with typed returns
- Integration tests against live PostgreSQL
- Migration scripts (unchanged — SQL is SQL)

**Exit criterion:** All 41 migrations run. All query methods pass integration tests.

### 7.6 Phase 5: HTTP & API (Weeks 7–8)

Port the HTTP layer.

- Axum routes → Javalin (or JDK HttpServer) route registration
- Chat endpoint, commands endpoint, verb surface endpoint
- MCP tool handlers
- WebSocket verb_surface_changed events

**Exit criterion:** React UI works identically against Java backend.

### 7.7 Phase 6: Agent & Orchestrator (Weeks 8–10)

Port the agent orchestration layer.

- Orchestrator's `handle_utterance()` flow
- IntentPipeline with `with_allowed_verbs()` pre-filter
- HybridVerbSearcher (or sidecar to Rust embedding service)
- AgentMode gating

**Exit criterion:** End-to-end utterance → verb execution works with identical outcomes.

---

## 8. What NOT to Port

| Component | Decision | Rationale |
|-----------|----------|-----------|
| React UI | Keep as-is | TypeScript, talks to REST API. Backend language is irrelevant. |
| SQL migrations | Keep as-is | SQL is SQL. Same 41 migration files, same PostgreSQL schema. |
| YAML verb definitions | Keep as-is | Same 141 files, same format. Java reads them with SnakeYAML. |
| egui visualisation | Drop | No egui usage found in current codebase. If needed, replace with web UI. |
| dsl-lsp | Defer | Language Server Protocol for DSL editing. Port only if Zed/VSCode integration is needed for Java. |
| ob-templates | Defer | Template rendering. Port when needed. |
| Embedding model inference | Sidecar | Keep as Rust service. Java calls via HTTP. |

---

## 9. CI & Validation Strategy

### 9.1 Wire Format Compatibility

During the migration period, both Rust and Java services coexist. The CI pipeline runs:

```
1. Rust binary produces JSON fixtures for every serialisable type
2. Java tests deserialise Rust fixtures → verify all fields
3. Java tests serialise → compare byte-for-byte with Rust fixtures
4. Both run against same PostgreSQL instance with same migrations
```

### 9.2 Governance Invariant Tests

```
1. GovernanceCacheValidator runs on Java startup — any cache mismatch fails the build
2. Type chain tests: verify GovernedContext requires Principal (compile-time, but tested for regression)
3. Exhaustiveness tests: verify no default branches on sealed interface switches
4. VerbSurfaceFailPolicy tests: verify FailClosed never returns >30 verbs
```

### 9.3 No Default Branch Enforcement

```bash
# CI script: grep for default branches on sealed interface switches
# Exits non-zero if any found (except whitelisted patterns)
find src -name "*.java" -exec grep -Hn 'default\s*->' {} \; \
  | grep -v '// default-ok' \
  | grep -v 'Map.getOrDefault' \
  | grep -v 'test/' \
  && echo "FAIL: default branch on sealed interface switch" && exit 1 \
  || echo "PASS: no default branches"
```

---

## 10. Open Questions

| # | Question | Status | Notes |
|---|----------|--------|-------|
| 1 | HTTP framework: Javalin vs JDK HttpServer? | Open | Javalin is ~100KB, nice routing. JDK HttpServer is zero-dep but lower-level. |
| 2 | JSON library: Gson vs JSON-P vs hand-rolled? | Open | Depends on YAML strategy. If SnakeYAML suffices for YAML, Gson (280KB) for JSON. |
| 3 | Embedding service: sidecar vs Java ONNX? | Leaning sidecar | Port the critical path first. Embedding is a leaf dependency. |
| 4 | Build system: Maven vs Gradle? | Open | Gradle aligns with JPMS multi-module better. Maven is simpler. |
| 5 | Where does Go fit? | Open | Original architecture mentions Go for orchestration. Define boundary. |
| 6 | Structured concurrency availability in Java 26? | Verify | SC has been preview since Java 21. Confirm stable in Java 26 build. |
| 7 | YAML: SnakeYAML vs Jackson YAML? | Open | SnakeYAML is lighter. Jackson YAML would be the only Jackson we allow (YAML only, no Jackson Databind). |

---

## 11. Decision Log

Decisions recorded here are final unless explicitly revisited with rationale.

| # | Date | Decision | Rationale |
|---|------|----------|-----------|
| D1 | 28 Feb 2026 | Java 26 only, no fallbacks | Full language feature set required for type-level governance |
| D2 | 28 Feb 2026 | No Spring, no DI frameworks | Explicit wiring, no runtime magic, sub-second startup |
| D3 | 28 Feb 2026 | No Jackson for JSON serialisation | Spring ecosystem dependency, reflection-heavy. Use minimal alternative. |
| D4 | 28 Feb 2026 | No Lombok | Java 26 records eliminate the need |
| D5 | 28 Feb 2026 | Governance via type chain, not annotations | 3 of 5 checks become compile-time type errors. More IDE-visible than proc macros. |
| D6 | 28 Feb 2026 | Startup-time validator for data-dependent governance checks | Checks 1 (verb lifecycle) and 5 (attribute lifecycle) fail-fast on boot |
| D7 | 28 Feb 2026 | No `default` branches on sealed interface switches | Exhaustiveness is the Java equivalent of Rust's exhaustive match |
| D8 | 28 Feb 2026 | Wire format tests before porting any serialisable type | Safety net for Rust ↔ Java coexistence period |
| D9 | 28 Feb 2026 | JPMS modules replace `#[cfg(feature)]` | Stronger guarantees than feature flags — compile-time module boundaries |
| D10 | 28 Feb 2026 | Embedding search stays as Rust sidecar initially | Leaf dependency, port the critical path first |
| D11 | 28 Feb 2026 | Port order: types → governance → DSL → surface → DB → HTTP → agent | Dependencies flow downward; each phase is independently testable |
