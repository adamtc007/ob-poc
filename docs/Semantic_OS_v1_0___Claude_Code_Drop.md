# Semantic OS v1.0 — Claude Code Implementation Drop

**Status:** ✅ IMPLEMENTED (2026-02-25) — All stages complete. Harness green in inprocess mode.
**Date:** Feb 2026
**Version:** v1.1 (standalone service + stewardship Phase 0-1 + schema visibility)
**Scope:** Full standalone Semantic OS: workspace crates → kernel extraction → API → outbox → projections → stewardship → schema visibility
**Original Spec:** v1.0 (production-ready drop from v3.4 peer review lineage)

> **Implementation Summary:** 6 workspace crates (`sem_os_core`, `sem_os_postgres`, `sem_os_server`, `sem_os_client`, `sem_os_harness`, `sem_os_obpoc_adapter`), 7 migrations (092-098), ~101 MCP tools total (~32 sem_reg + 23 stewardship + rest), 3 test suites (core, projections, permissions), stewardship agent Phase 0-1 (changeset workflow + Show Loop), `db_introspect` MCP tool, `AttributeSource` real (schema, table, column) triples. Adapter wires into ob-poc via `SemOsClient` trait in both inprocess and remote modes.

---

## CLAUDE CODE EXECUTION RULES — READ FIRST, ENFORCE ALWAYS

> These rules apply to every stage, every sub-task, every line of code. Non-negotiable.

1. **Never skip a TODO item.** If a TODO says "implement X", implement X completely. Do not write `// TODO: implement later` or `unimplemented!()` and move on. The only exception is explicitly labelled stubs in the port trait definitions (marked `/* stub */`) — those are intentional and documented.

2. **Never defer within a stage.** If a stage says "add migration 092", the migration file must be created, complete, and valid SQL before the stage is marked done. Partial migrations are worse than no migration.

3. **Never silently skip a stage.** After completing each stage, output the completion marker:
   ```
   ✅ STAGE [N.N] COMPLETE — [brief description of what was done]
   → IMMEDIATELY PROCEEDING TO STAGE [N+1]
   ```
   Then begin the next stage without waiting for further input.

4. **Rip and replace when structure conflicts.** If an existing file's structure conflicts with what this document requires, do not patch around it. Delete the file and rewrite it from scratch per the specification. Partial migrations of structure create hidden coupling bugs.

5. **`cargo check` after every stage.** Each stage ends with `cargo check --workspace`. If it fails, fix it before proceeding. Do not proceed to the next stage with a broken workspace.

6. **Every port trait method must be fully implemented in the Postgres adapter.** If a method cannot be implemented because a migration hasn't landed yet, the method returns `Err(SemOsError::Internal(anyhow!("migration pending — see stage X")))`. This is not the same as `unimplemented!()`.

7. **No `unwrap()` or `expect()` in production paths.** Only in tests, and only with a comment explaining what precondition is asserted.

8. **`#[async_trait]`** must be applied to every trait definition and every `impl` block that contains async methods. No exceptions. Use the `async-trait` crate.

---

## 1) Architecture overview

### 1.1 What this builds

Semantic OS is extracted from `ob_poc` as a standalone service with a hard API boundary. `ob_poc` communicates with Semantic OS exclusively via the `SemOsClient` trait — either in-process (for development/testing) or over HTTP REST (for integration and eventual production). The REST API is designed from day one against Protobuf types so that a future gRPC/HTTP2 migration requires adding one client impl, not restructuring the API.

### 1.2 Mode switching

Controlled by a single environment variable read at `ob_poc` startup:

```
SEM_OS_MODE=inprocess    # default; wraps core service directly
SEM_OS_MODE=remote       # calls sem_os_server over HTTP
```

This is **not** a Cargo feature. It is read in `ob_poc/src/main.rs` at process start. A `Arc<dyn SemOsClient>` is constructed accordingly and passed down via dependency injection. The rest of ob-poc never reads this env var.

### 1.3 API contract — Protobuf-first, REST now, gRPC later

**The Protobuf file is the source of truth for all API types.** Request/response types are `prost`-generated from `api/proto/sem_os/v1/service.proto`. The REST handlers in `sem_os_server` use these same types with `serde` JSON serialisation as a transitional measure. When gRPC is needed, add a `GrpcClient` impl to `sem_os_client` — the trait, the proto types, and the server logic are already correct.

```
api/proto/sem_os/v1/service.proto  ← source of truth
    ↓ prost-build (build.rs)
sem_os_core/src/proto/             ← generated types used by ALL crates
    ↓ serde JSON (now)             ↓ tonic (later, one afternoon)
sem_os_server axum REST            sem_os_server tonic gRPC
```

### 1.4 Workspace crate graph

```
[workspace]
members = [
  "sem_os_core",              # pure: Principal, types, ports, gates, ABAC, seeds, proto types
  "sem_os_postgres",          # SQLx impl of ports + outbox dispatcher + projection writer
  "sem_os_server",            # axum REST server + outbox worker
  "sem_os_client",            # SemOsClient trait + InProcessClient + HttpClient
  "sem_os_obpoc_adapter",     # scanner, seeds, YAML — depends on sem_os_core only
  "sem_os_harness",           # compatibility test harness — depends on sem_os_client only
  "ob_poc",                   # existing app — depends on sem_os_client only
]
```

**Cargo dependency constraints (build-time enforced):**

```
sem_os_core          → (no workspace deps)
sem_os_postgres      → sem_os_core
sem_os_server        → sem_os_core, sem_os_postgres
sem_os_client        → sem_os_core
sem_os_obpoc_adapter → sem_os_core
sem_os_harness       → sem_os_client
ob_poc               → sem_os_client   ← NEVER sem_os_postgres or sem_os_server
```

---

## 2) Error types — define before everything else

**File:** `sem_os_core/src/error.rs`

This must be created in Stage 1.1 before any port traits are written. All port traits and all `SemOsClient` methods return `Result<T, SemOsError>`.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SemOsError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("gate failed: {0} violations")]
    GateFailed(Vec<GateViolation>),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("migration pending — {0}")]
    MigrationPending(String),

    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct GateViolation {
    pub gate_id:     String,
    pub severity:    GateSeverity,
    pub message:     String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateSeverity {
    Error,
    Warning,
}
```

HTTP status mapping (used in `sem_os_server` error handler):

```rust
impl SemOsError {
    pub fn http_status(&self) -> u16 {
        match self {
            Self::NotFound(_)        => 404,
            Self::GateFailed(_)      => 422,
            Self::Unauthorized(_)    => 403,
            Self::Conflict(_)        => 409,
            Self::InvalidInput(_)    => 400,
            Self::MigrationPending(_)=> 503,
            Self::Internal(_)        => 500,
        }
    }
}
```

---

## 3) Principal type — define before ABAC, ports, or server

**File:** `sem_os_core/src/principal.rs`

```rust
use std::collections::HashMap;
use crate::error::SemOsError;

#[derive(Debug, Clone)]
pub struct Principal {
    pub actor_id: String,
    pub roles:    Vec<String>,
    pub claims:   HashMap<String, String>,
    pub tenancy:  Option<String>,
}

impl Principal {
    /// Construct from validated JWT claims at the server boundary (remote mode).
    /// The server middleware calls this; core logic never reads raw JWT tokens.
    pub fn from_jwt_claims(claims: &JwtClaims) -> Result<Self, SemOsError> {
        let actor_id = claims.sub.clone()
            .ok_or_else(|| SemOsError::Unauthorized("missing sub claim".into()))?;
        Ok(Self {
            actor_id,
            roles:   claims.roles.clone().unwrap_or_default(),
            claims:  claims.extra.clone().unwrap_or_default(),
            tenancy: claims.tenancy.clone(),
        })
    }

    /// Construct explicitly for in-process mode.
    /// Caller is responsible for populating roles correctly.
    /// There is no implicit or thread-local identity anywhere in the codebase.
    pub fn in_process(actor_id: impl Into<String>, roles: Vec<String>) -> Self {
        Self {
            actor_id: actor_id.into(),
            roles,
            claims:  HashMap::new(),
            tenancy: None,
        }
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn is_admin(&self) -> bool {
        self.has_role("admin")
    }

    pub fn require_admin(&self) -> Result<(), SemOsError> {
        if self.is_admin() { Ok(()) }
        else { Err(SemOsError::Unauthorized(format!("{} is not an admin", self.actor_id))) }
    }
}

/// JWT claims shape expected from the identity provider.
/// Deserialised by the server JWT middleware.
#[derive(Debug, serde::Deserialize)]
pub struct JwtClaims {
    pub sub:     Option<String>,
    pub roles:   Option<Vec<String>>,
    pub tenancy: Option<String>,
    #[serde(flatten)]
    pub extra:   Option<HashMap<String, String>>,
}
```

---

## 4) Protobuf API contract

**File:** `api/proto/sem_os/v1/service.proto`

Create this file. Add a `build.rs` to `sem_os_core` that runs `prost-build` over it. The generated types land in `sem_os_core/src/proto/mod.rs`.

```protobuf
syntax = "proto3";
package sem_os.v1;

// ── Core value types ─────────────────────────────────────────────────────────

message SnapshotSetId  { string value = 1; }
message SnapshotId     { string value = 1; }
message Fqn            { string value = 1; }

// ── Resolve context ───────────────────────────────────────────────────────────

message ResolveContextRequest {
    string  verb        = 1;
    string  entity_type = 2;
    string  context_key = 3;
    string  tenant_id   = 4;
    string  snapshot_set_id = 5;  // pin to specific set; empty = use latest active
}

message ResolveContextResponse {
    repeated Candidate candidates = 1;
    repeated GateViolation violations = 2;
    string resolved_snapshot_set_id = 3;
}

message Candidate {
    string fqn          = 1;
    double score        = 2;
    string verb_name    = 3;
    bytes  payload_json = 4;  // JSON-encoded verb contract payload
}

message GateViolation {
    string gate_id     = 1;
    string severity    = 2;  // "error" | "warning"
    string message     = 3;
    string remediation = 4;
}

// ── Manifest ──────────────────────────────────────────────────────────────────

message GetManifestRequest  { string snapshot_set_id = 1; }
message GetManifestResponse {
    string snapshot_set_id = 1;
    string published_at    = 2;  // RFC3339
    repeated ManifestEntry entries = 3;
}

message ManifestEntry {
    string snapshot_id  = 1;
    string object_type  = 2;
    string fqn          = 3;
    string content_hash = 4;
}

// ── Publish ───────────────────────────────────────────────────────────────────

message PublishRequest  { bytes payload_json = 1; }  // opaque publish payload
message PublishResponse { string snapshot_set_id = 1; }

// ── Snapshot export ───────────────────────────────────────────────────────────

message ExportSnapshotSetRequest  { string snapshot_set_id = 1; }
message ExportSnapshotSetResponse {
    string snapshot_set_id = 1;
    repeated ExportEntry entries = 2;
}
message ExportEntry {
    string snapshot_id  = 1;
    string fqn          = 2;
    string object_type  = 3;
    bytes  payload_json = 4;
}

// ── Bootstrap ─────────────────────────────────────────────────────────────────

message BootstrapSeedBundleRequest  { bytes bundle_json = 1; }
message BootstrapSeedBundleResponse { string snapshot_set_id = 1; }

// ── Service definition (for future tonic gRPC) ───────────────────────────────

service SemOs {
    rpc ResolveContext       (ResolveContextRequest)       returns (ResolveContextResponse);
    rpc GetManifest          (GetManifestRequest)          returns (GetManifestResponse);
    rpc Publish              (PublishRequest)               returns (PublishResponse);
    rpc ExportSnapshotSet    (ExportSnapshotSetRequest)     returns (ExportSnapshotSetResponse);
    rpc BootstrapSeedBundle  (BootstrapSeedBundleRequest)  returns (BootstrapSeedBundleResponse);
}
```

**`sem_os_core/build.rs`:**

```rust
fn main() {
    prost_build::compile_protos(
        &["../../api/proto/sem_os/v1/service.proto"],
        &["../../api/proto"],
    ).expect("prost-build failed");
}
```

**`sem_os_core/src/proto/mod.rs`:**

```rust
// Auto-generated by prost-build. Do not edit manually.
include!(concat!(env!("OUT_DIR"), "/sem_os.v1.rs"));
```

All request/response types used by `SemOsClient` and `sem_os_server` must come from `sem_os_core::proto::*`. No hand-rolled request/response structs anywhere.

---

## 5) Storage port traits

**File:** `sem_os_core/src/ports.rs`

```rust
use async_trait::async_trait;
use crate::{error::SemOsError, principal::Principal, types::*};

pub type Result<T> = std::result::Result<T, SemOsError>;

#[async_trait]
pub trait SnapshotStore: Send + Sync {
    async fn resolve(&self, fqn: &Fqn, as_of: Option<&SnapshotSetId>) -> Result<Snapshot>;
    async fn publish(&self, principal: &Principal, req: PublishInput) -> Result<SnapshotSetId>;
    async fn list_as_of(&self, as_of: &SnapshotSetId) -> Result<Vec<SnapshotSummary>>;
    async fn get_manifest(&self, id: &SnapshotSetId) -> Result<Manifest>;
    async fn export(&self, id: &SnapshotSetId) -> Result<Vec<SnapshotExport>>;
}

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn load_typed(&self, snapshot_id: &SnapshotId, fqn: &Fqn) -> Result<TypedObject>;
}

#[async_trait]
pub trait ChangesetStore: Send + Sync {
    /* stub — implemented in Stage 3 */
}

#[async_trait]
pub trait AuditStore: Send + Sync {
    async fn append(&self, principal: &Principal, entry: AuditEntry) -> Result<()>;
}

#[async_trait]
pub trait OutboxStore: Send + Sync {
    /// Must be called inside the publish transaction. Atomicity is the caller's responsibility.
    async fn enqueue(&self, event: OutboxEvent) -> Result<()>;
    async fn claim_next(&self, claimer_id: &str) -> Result<Option<OutboxEvent>>;
    async fn mark_processed(&self, event_id: &EventId) -> Result<()>;
    async fn mark_failed(&self, event_id: &EventId, error: &str) -> Result<()>;
}

#[async_trait]
pub trait EvidenceInstanceStore: Send + Sync {
    async fn record(&self, principal: &Principal, instance: EvidenceInstance) -> Result<()>;
}

#[async_trait]
pub trait ProjectionWriter: Send + Sync {
    /// Called by the outbox dispatcher ONLY. Never called by publish directly.
    async fn write_active_snapshot_set(&self, snapshot_set_id: &SnapshotSetId) -> Result<()>;
}
```

---

## 6) SeedBundle

**File:** `sem_os_core/src/seeds.rs`

```rust
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedBundle {
    /// SHA-256 of the canonical JSON serialisation of this bundle (fields sorted, deterministic).
    /// Computed by the adapter via `SeedBundle::compute_hash()`.
    /// Used as the idempotency key on POST /bootstrap/seed_bundle.
    /// Prefixed with "v1:" to allow future hash algorithm migration.
    pub bundle_hash:    String,
    pub verb_contracts: Vec<VerbContractSeed>,
    pub attributes:     Vec<AttributeSeed>,
    pub entity_types:   Vec<EntityTypeSeed>,
    pub taxonomies:     Vec<TaxonomySeed>,
    pub policies:       Vec<PolicySeed>,
    pub views:          Vec<ViewSeed>,
}

impl SeedBundle {
    /// Compute a stable, version-prefixed SHA-256 hash of the bundle contents.
    /// Sort all vecs by their FQN/name field before hashing to ensure determinism
    /// regardless of source ordering. This is the canonical form.
    pub fn compute_hash(
        verb_contracts: &[VerbContractSeed],
        attributes:     &[AttributeSeed],
        entity_types:   &[EntityTypeSeed],
        taxonomies:     &[TaxonomySeed],
        policies:       &[PolicySeed],
        views:          &[ViewSeed],
    ) -> String {
        // Construct a temporary struct without the hash field, sort all fields,
        // serialise to canonical JSON, then hash.
        #[derive(Serialize)]
        struct Canonical<'a> {
            verb_contracts: Vec<&'a VerbContractSeed>,
            attributes:     Vec<&'a AttributeSeed>,
            entity_types:   Vec<&'a EntityTypeSeed>,
            taxonomies:     Vec<&'a TaxonomySeed>,
            policies:       Vec<&'a PolicySeed>,
            views:          Vec<&'a ViewSeed>,
        }
        let mut vc = verb_contracts.iter().collect::<Vec<_>>();
        vc.sort_by_key(|s| &s.fqn);
        // ... (sort remaining vecs by their fqn/name field)

        let canonical = Canonical { verb_contracts: vc, /* ... */ };
        let json = serde_json::to_string(&canonical)
            .expect("canonical serialisation must not fail");
        let hash = Sha256::digest(json.as_bytes());
        format!("v1:{}", hex::encode(hash))
    }
}

// Seed DTOs — pure data, no SQLx derives, no ob-poc config types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbContractSeed { pub fqn: String, pub payload: serde_json::Value }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeSeed   { pub fqn: String, pub payload: serde_json::Value }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeSeed  { pub fqn: String, pub payload: serde_json::Value }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomySeed    { pub fqn: String, pub payload: serde_json::Value }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySeed      { pub fqn: String, pub payload: serde_json::Value }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSeed        { pub fqn: String, pub payload: serde_json::Value }
```

---

## 7) SemOsClient trait

**File:** `sem_os_client/src/lib.rs`

```rust
use async_trait::async_trait;
use sem_os_core::{error::SemOsError, principal::Principal, proto::*, seeds::SeedBundle};

pub type Result<T> = std::result::Result<T, SemOsError>;

#[async_trait]
pub trait SemOsClient: Send + Sync {
    async fn resolve_context(
        &self,
        principal: &Principal,
        req: ResolveContextRequest,
    ) -> Result<ResolveContextResponse>;

    async fn get_manifest(
        &self,
        snapshot_set_id: &str,
    ) -> Result<GetManifestResponse>;

    async fn export_snapshot_set(
        &self,
        snapshot_set_id: &str,
    ) -> Result<ExportSnapshotSetResponse>;

    async fn bootstrap_seed_bundle(
        &self,
        principal: &Principal,
        bundle: SeedBundle,
    ) -> Result<BootstrapSeedBundleResponse>;

    /// Test-only: synchronously drain and process all pending outbox events.
    /// Only implemented by InProcessClient. HttpClient returns Ok(()) immediately.
    /// Used by sem_os_harness to avoid timing-dependent test failures.
    #[cfg(test)]
    async fn drain_outbox_for_test(&self) -> Result<()>;
}
```

**`sem_os_client/src/inprocess.rs`** — wraps the core service directly:

```rust
pub struct InProcessClient {
    service: Arc<dyn CoreService>,  // defined in sem_os_core
    principal: Principal,            // stored for convenience; passed explicitly on each call
}

impl InProcessClient {
    pub fn new(service: Arc<dyn CoreService>, principal: Principal) -> Self {
        Self { service, principal }
    }
}
```

**`sem_os_client/src/http.rs`** — calls `sem_os_server` over HTTP:

```rust
pub struct HttpClient {
    base_url: String,
    http:     reqwest::Client,
    jwt:      String,  // Bearer token presented on every request
}

impl HttpClient {
    pub fn new(base_url: impl Into<String>, jwt: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http:     reqwest::Client::new(),
            jwt:      jwt.into(),
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.jwt)
    }
}
```

All HTTP calls send/receive `prost` types serialised as JSON. Response errors are deserialised to `SemOsError` via the HTTP status code + error body.

---

## 8) Delivery stages

---

### STAGE 1.1 — Workspace crate scaffold

**Do not move any logic yet. Create skeletons only. `cargo check --workspace` must pass before proceeding.**

**TODO — every item must be completed:**

- [ ] Create `api/proto/sem_os/v1/service.proto` — full content from §4 above.
- [ ] Create `sem_os_core/Cargo.toml`:
  ```toml
  [package]
  name = "sem_os_core"
  version = "0.1.0"
  edition = "2021"

  [dependencies]
  async-trait = "0.1"
  thiserror   = "1"
  anyhow      = "1"
  serde       = { version = "1", features = ["derive"] }
  serde_json  = "1"
  sha2        = "0.10"
  hex         = "0.4"
  prost       = "0.12"

  [build-dependencies]
  prost-build = "0.12"
  ```
- [ ] Create `sem_os_core/build.rs` — prost-build invocation from §4.
- [ ] Create `sem_os_core/src/lib.rs` — empty module declarations:
  ```rust
  pub mod error;
  pub mod principal;
  pub mod proto;
  pub mod ports;
  pub mod seeds;
  pub mod types;
  pub mod gates;
  pub mod abac;
  pub mod context_resolution;
  ```
- [ ] Create `sem_os_core/src/error.rs` — full content from §2.
- [ ] Create `sem_os_core/src/principal.rs` — full content from §3.
- [ ] Create `sem_os_core/src/proto/mod.rs` — prost include from §4.
- [ ] Create `sem_os_core/src/ports.rs` — full content from §5.
- [ ] Create `sem_os_core/src/seeds.rs` — full content from §6.
- [ ] Create `sem_os_core/src/types.rs` — stub (empty structs) for: `Fqn`, `SnapshotId`, `SnapshotSetId`, `Snapshot`, `SnapshotSummary`, `Manifest`, `SnapshotExport`, `TypedObject`, `PublishInput`, `AuditEntry`, `OutboxEvent`, `EventId`, `EvidenceInstance`. These will be filled in Stage 1.2.
- [ ] Create `sem_os_core/src/gates/mod.rs` — empty.
- [ ] Create `sem_os_core/src/abac.rs` — empty.
- [ ] Create `sem_os_core/src/context_resolution.rs` — empty.
- [ ] Create `sem_os_postgres/Cargo.toml`:
  ```toml
  [package]
  name = "sem_os_postgres"
  version = "0.1.0"
  edition = "2021"

  [dependencies]
  sem_os_core = { path = "../sem_os_core" }
  sqlx        = { version = "0.7", features = ["postgres","runtime-tokio","uuid","chrono","json"] }
  async-trait = "0.1"
  anyhow      = "1"
  uuid        = { version = "1", features = ["v4"] }
  chrono      = { version = "0.4", features = ["serde"] }
  ```
- [ ] Create `sem_os_postgres/src/lib.rs` — empty module stubs.
- [ ] Create `sem_os_server/Cargo.toml`:
  ```toml
  [package]
  name = "sem_os_server"
  version = "0.1.0"
  edition = "2021"

  [[bin]]
  name = "sem_os_server"
  path = "src/main.rs"

  [dependencies]
  sem_os_core     = { path = "../sem_os_core" }
  sem_os_postgres = { path = "../sem_os_postgres" }
  axum            = "0.7"
  tokio           = { version = "1", features = ["full"] }
  tower           = "0.4"
  jsonwebtoken    = "9"
  async-trait     = "0.1"
  anyhow          = "1"
  serde_json      = "1"
  tracing         = "0.1"
  tracing-subscriber = "0.3"
  ```
- [ ] Create `sem_os_server/src/main.rs` — empty `main()`.
- [ ] Create `sem_os_client/Cargo.toml`:
  ```toml
  [package]
  name = "sem_os_client"
  version = "0.1.0"
  edition = "2021"

  [dependencies]
  sem_os_core = { path = "../sem_os_core" }
  async-trait = "0.1"
  anyhow      = "1"
  reqwest     = { version = "0.11", features = ["json"] }
  serde_json  = "1"
  ```
- [ ] Create `sem_os_client/src/lib.rs` — full content from §7.
- [ ] Create `sem_os_client/src/inprocess.rs` — stub impl (all methods return `Err(SemOsError::MigrationPending("S1.2".into()))`).
- [ ] Create `sem_os_client/src/http.rs` — stub impl (all methods return `Err(SemOsError::MigrationPending("S2.1".into()))`).
- [ ] Create `sem_os_obpoc_adapter/Cargo.toml`:
  ```toml
  [package]
  name = "sem_os_obpoc_adapter"
  version = "0.1.0"
  edition = "2021"

  [dependencies]
  sem_os_core = { path = "../sem_os_core" }
  serde       = { version = "1", features = ["derive"] }
  serde_yaml  = "0.9"
  anyhow      = "1"
  ```
- [ ] Create `sem_os_obpoc_adapter/src/lib.rs` — empty stubs.
- [ ] Create `sem_os_harness/Cargo.toml`:
  ```toml
  [package]
  name = "sem_os_harness"
  version = "0.1.0"
  edition = "2021"

  [dev-dependencies]
  sem_os_client = { path = "../sem_os_client" }
  tokio         = { version = "1", features = ["full"] }
  ```
- [ ] Create `sem_os_harness/src/lib.rs` — stub `run_scenario_suite`.
- [ ] Update root `Cargo.toml`:
  ```toml
  [workspace]
  members = [
    "sem_os_core",
    "sem_os_postgres",
    "sem_os_server",
    "sem_os_client",
    "sem_os_obpoc_adapter",
    "sem_os_harness",
    "ob_poc",
  ]
  resolver = "2"
  ```
- [ ] Verify `ob_poc/Cargo.toml` does NOT list `sem_os_postgres` or `sem_os_server` as dependencies. If it does, remove them and replace with `sem_os_client`.
- [ ] Run `cargo check --workspace`. Fix all errors before proceeding.

**Completion marker:**
```
✅ STAGE 1.1 COMPLETE — workspace scaffold green
→ IMMEDIATELY PROCEEDING TO STAGE 1.2
```

---

### STAGE 1.2 — Extract `sem_os_core` (semantic kernel)

**Goal:** All pure logic moves to `sem_os_core`. `cargo check -p sem_os_core` must succeed with zero SQLx/web/MCP imports.

**TODO — every item must be completed:**

- [ ] Populate `sem_os_core/src/types.rs` by migrating from `rust/src/sem_reg/types.rs`:
  - Copy all pure types (no `sqlx::FromRow`, no `PgPool`).
  - Remove all `#[derive(sqlx::FromRow)]` — these move to `sem_os_postgres/src/sqlx_types.rs`.
  - Add `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]` where appropriate.
- [ ] Create `sem_os_postgres/src/sqlx_types.rs` — SQLx row types and `impl From<Row> for CoreType` mappings.
- [ ] Migrate gate logic:
  - `rust/src/sem_reg/gates.rs` → `sem_os_core/src/gates/mod.rs`
  - `rust/src/sem_reg/gates_governance.rs` → `sem_os_core/src/gates/governance.rs`
  - `rust/src/sem_reg/gates_technical.rs` → `sem_os_core/src/gates/technical.rs`
  - All gate functions must take `principal: &Principal` as first argument.
  - `evaluate_publish_gates(principal, ...)` must be pure (no DB calls, no async).
- [ ] Migrate ABAC:
  - `rust/src/sem_reg/abac.rs` → `sem_os_core/src/abac.rs`
  - All ABAC checks take `principal: &Principal`. No thread-local context. No implicit identity.
- [ ] Migrate security:
  - `rust/src/sem_reg/security.rs` → `sem_os_core/src/security.rs`
- [ ] Migrate context resolution:
  - `rust/src/sem_reg/context_resolution.rs` → `sem_os_core/src/context_resolution.rs`
  - Must be pure (no async, no DB). Takes snapshot data as input; returns candidates.
- [ ] Run `cargo check -p sem_os_core`. If SQLx appears anywhere in the dependency tree, find it and remove it. Do not proceed with SQLx in core.
- [ ] Run `cargo check --workspace`. Fix all errors.
- [ ] Update `InProcessClient` to wire up core service (partially — full wiring in S1.3).

**Completion marker:**
```
✅ STAGE 1.2 COMPLETE — sem_os_core clean, zero SQLx imports
→ IMMEDIATELY PROCEEDING TO STAGE 1.3
```

---

### STAGE 1.3 — Storage ports + Postgres adapter

**Goal:** `sem_os_postgres` implements all port traits. Existing tests still pass.

**TODO — every item must be completed:**

- [ ] Create `sem_os_postgres/src/store.rs`:
  - Implement `SnapshotStore` for `PgSnapshotStore(PgPool)`.
  - Implement `ObjectStore` for `PgObjectStore(PgPool)`.
  - Implement `AuditStore` for `PgAuditStore(PgPool)`.
  - Implement `OutboxStore` for `PgOutboxStore(PgPool)` — stub methods return `Err(SemOsError::MigrationPending("S2.2".into()))` until migration 092 lands.
  - Implement `EvidenceInstanceStore` for `PgEvidenceStore(PgPool)`.
  - Implement `ProjectionWriter` for `PgProjectionWriter(PgPool)` — stub until migration 093 lands.
- [ ] Migrate `rust/src/sem_reg/store.rs` logic into the above implementations. **Rip and replace:** do not patch the old file. Move the logic, then delete the old file.
- [ ] Refactor `rust/src/sem_reg/registry.rs` into a `CoreService` struct in `sem_os_core/src/service.rs` that takes ports via `Arc<dyn PortTrait>`. This is the object `InProcessClient` wraps.
- [ ] Wire `InProcessClient` to `CoreService` in `sem_os_client/src/inprocess.rs`. All methods now call through to the service.
- [ ] The publish path must atomically: (a) insert snapshot rows, (b) call `OutboxStore::enqueue()`. Both inside the same `PgPool` transaction. Do not commit the snapshot without committing the outbox event.
- [ ] Run existing integration tests: `cargo test -p sem_reg_integration` (or equivalent). All must pass. If tests reference old module paths, update the paths — do not delete the tests.
- [ ] Run `cargo check --workspace`.

**Completion marker:**
```
✅ STAGE 1.3 COMPLETE — ports implemented, atomic publish+outbox, existing tests pass
→ IMMEDIATELY PROCEEDING TO STAGE 1.4
```

---

### STAGE 1.4 — Move scanner/onboarding into `sem_os_obpoc_adapter`

**Goal:** `sem_os_core` has no dependency on ob-poc YAML config structures.

**TODO — every item must be completed:**

- [ ] Move:
  - `rust/src/sem_reg/scanner.rs` → `sem_os_obpoc_adapter/src/scanner.rs`
  - `rust/src/sem_reg/onboarding/*` → `sem_os_obpoc_adapter/src/onboarding/`
  - Seed reader files that read ob-poc YAML → `sem_os_obpoc_adapter/src/seeds/`
  - Pure seed DTO builders (no file I/O) → remain in or move to `sem_os_core/src/seeds.rs`
- [ ] Implement `SeedBundle::compute_hash()` fully (see §6 — sort all vecs by FQN, canonical JSON, SHA-256, "v1:" prefix).
- [ ] `sem_os_obpoc_adapter` produces a `SeedBundle` with a valid `bundle_hash`.
- [ ] Update `ob_poc` startup to call `sem_os_client::bootstrap_seed_bundle(principal, bundle)` rather than calling scanner directly.
- [ ] Run `cargo check --workspace`. `sem_os_core` must not reference any ob-poc YAML config types.

**Completion marker:**
```
✅ STAGE 1.4 COMPLETE — adapter isolated, SeedBundle with bundle_hash computable
→ IMMEDIATELY PROCEEDING TO STAGE 1.5
```

---

### STAGE 1.5 — Golden/invariant tests + compatibility harness

**Goal:** Harness in CI. This is the regression gate for all subsequent stages. Do not skip or defer any part of this stage.

**Test database isolation:** Each harness test run creates an isolated Postgres schema:

```rust
// sem_os_harness/src/db.rs
pub async fn isolated_pool(base_url: &str) -> (PgPool, String) {
    let schema = format!("test_{}", uuid::Uuid::new_v4().simple());
    let pool   = PgPool::connect(base_url).await.expect("test DB connect");
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&pool).await.expect("create test schema");
    // Run all migrations scoped to this schema
    sqlx::query(&format!("SET search_path = {schema}"))
        .execute(&pool).await.expect("set search_path");
    // Run migrations here
    (pool, schema)
}

pub async fn drop_schema(pool: &PgPool, schema: &str) {
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(pool).await.expect("drop test schema");
}
```

**Harness implementation — `sem_os_harness/src/lib.rs`:**

```rust
use sem_os_client::SemOsClient;
use sem_os_core::{principal::Principal, proto::*};

pub async fn run_scenario_suite(client: &dyn SemOsClient) {
    test_gate_suite_outcomes(client).await;
    test_publish_invariants(client).await;
    test_context_resolution_determinism(client).await;
    test_manifest_stability(client).await;
    test_projection_watermark_advances(client).await;
}

async fn test_gate_suite_outcomes(client: &dyn SemOsClient) {
    // Publish a known verb contract set.
    // Attempt publish of a set that violates a governance gate.
    // Assert GateFailed with the expected violation IDs.
    // Assert successful publish does not produce gate violations.
    todo!("implement: gate suite outcomes")  // ← MUST be implemented, not left as todo
}

// ... implement every scenario function completely
```

**Note to Claude Code:** Every `todo!()` above must be replaced with a complete implementation. The harness must execute real calls through the client and assert real outcomes. A harness that compiles but asserts nothing is not a harness.

**`test_projection_watermark_advances` polling strategy:**

```rust
async fn test_projection_watermark_advances(client: &dyn SemOsClient) {
    let principal = Principal::in_process("harness", vec!["admin".into()]);
    // Publish something
    // ...
    // Drain outbox (in-process) or poll with timeout (remote)
    #[cfg(test)]
    client.drain_outbox_for_test().await.expect("drain outbox");
    // Assert projection_watermark.last_outbox_seq > previous value
    // Assert sem_reg_pub.active_verb_contracts contains the published FQNs
}
```

**TODO — every item must be completed:**

- [ ] Implement `isolated_pool` + `drop_schema` in `sem_os_harness/src/db.rs`.
- [ ] Implement `run_scenario_suite` with all five scenario functions fully implemented (no `todo!()`).
- [ ] Implement `InProcessClient::drain_outbox_for_test()` — synchronously processes all pending outbox events by calling `ProjectionWriter::write_active_snapshot_set()` for each.
- [ ] Add CI job: `cargo test -p sem_os_harness --test harness_inprocess` on every PR.
- [ ] Strengthen existing invariant tests in `rust/tests/sem_reg_invariants.rs` to cover:
  - Append-only snapshot assertion (publish twice, assert row count doubles, no updates).
  - Successor chain assertion (new snapshot set references predecessor).
  - Context resolution determinism (same inputs, same FQN ordering, 10 successive calls).

**Completion marker:**
```
✅ STAGE 1.5 COMPLETE — harness green, CI gate active
→ IMMEDIATELY PROCEEDING TO STAGE 2.1
```

---

### STAGE 2.1 — Semantic OS server (REST-first, Protobuf types)

**Goal:** Running axum server with JWT middleware. Bootstrap endpoint live.

**TODO — every item must be completed:**

- [ ] Add migration `migrations/091_sem_reg_bootstrap_audit.sql`:
  ```sql
  CREATE TABLE sem_reg.bootstrap_audit (
      bundle_hash        TEXT PRIMARY KEY,
      origin_actor_id    TEXT NOT NULL,
      bundle_counts      JSONB NOT NULL,
      snapshot_set_id    UUID,
      status             TEXT NOT NULL CHECK (status IN ('in_progress','published','failed')),
      started_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
      completed_at       TIMESTAMPTZ,
      error              TEXT
  );
  ```
- [ ] Create `sem_os_server/src/middleware/jwt.rs`:
  - Extract `Authorization: Bearer <token>` header.
  - Validate JWT signature (configurable secret/JWKS URL via env var `SEM_OS_JWT_SECRET`).
  - Call `Principal::from_jwt_claims()`.
  - Inject `Principal` into request extensions.
  - Return 401 if token missing or invalid.
- [ ] Create `sem_os_server/src/handlers/`:
  - `health.rs` — `GET /health` → 200 OK `{"status":"ok"}`
  - `resolve_context.rs` — `POST /resolve_context` → deserialise `ResolveContextRequest` from JSON body → call core → return `ResolveContextResponse` as JSON.
  - `manifest.rs` — `GET /snapshot_sets/{id}/manifest`
  - `publish.rs` — `POST /publish` (admin required — call `principal.require_admin()?`)
  - `export.rs` — `GET /exports/snapshot_set/{id}`
  - `bootstrap.rs` — `POST /bootstrap/seed_bundle` (admin required):
    - Check `sem_reg.bootstrap_audit` for existing `bundle_hash`.
    - If `status = 'published'`: return 200 + existing `snapshot_set_id`.
    - If `status = 'in_progress'`: return 409.
    - Insert `status='in_progress'` row.
    - Call core publish.
    - Update to `status='published'` + `snapshot_set_id`.
    - Return `BootstrapSeedBundleResponse`.
- [ ] Create `sem_os_server/src/main.rs`:
  - Read config from env vars: `SEM_OS_DATABASE_URL`, `SEM_OS_JWT_SECRET`, `SEM_OS_BIND_ADDR`.
  - Build `PgPool`, run migrations, construct port implementations.
  - Build axum `Router` with all routes + JWT middleware on protected routes.
  - Start `OutboxDispatcher` as a background task (stub — real in S2.2).
  - Bind and serve.
- [ ] Implement `HttpClient` in `sem_os_client/src/http.rs`:
  - All methods call the corresponding server endpoints.
  - Deserialise error bodies to `SemOsError` based on HTTP status.
- [ ] Run `cargo check --workspace`.
- [ ] Test manually: `SEM_OS_MODE=remote cargo run -p sem_os_server` + `curl /health`.

**Completion marker:**
```
✅ STAGE 2.1 COMPLETE — server running, JWT middleware active, bootstrap endpoint live
→ IMMEDIATELY PROCEEDING TO STAGE 2.2
```

---

### STAGE 2.2 — Publish → Outbox event invariant

**Goal:** Every publish produces an outbox event. Dispatcher delivers projection updates. Watermark advances.

**TODO — every item must be completed:**

- [ ] Add migration `migrations/092_sem_reg_outbox.sql`:
  ```sql
  CREATE TABLE sem_reg.outbox_events (
      outbox_seq        BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
      event_id          UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
      event_type        TEXT NOT NULL,
      aggregate_version BIGINT,
      snapshot_set_id   UUID NOT NULL,
      correlation_id    UUID NOT NULL,
      payload           JSONB NOT NULL,
      created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
      claimed_at        TIMESTAMPTZ,
      claimer_id        TEXT,
      claim_timeout_at  TIMESTAMPTZ,
      processed_at      TIMESTAMPTZ,
      failed_at         TIMESTAMPTZ,
      attempt_count     INT NOT NULL DEFAULT 0,
      last_error        TEXT
  );

  CREATE INDEX idx_outbox_claimable ON sem_reg.outbox_events (outbox_seq)
      WHERE processed_at IS NULL
        AND (claimed_at IS NULL OR claim_timeout_at < now());
  ```
- [ ] Implement `PgOutboxStore` in `sem_os_postgres/src/store.rs` — replace the `MigrationPending` stubs with real SQL.
- [ ] Implement dispatcher claim SQL in `sem_os_postgres/src/dispatcher.rs`:
  ```sql
  UPDATE sem_reg.outbox_events
  SET    claimed_at       = now(),
         claimer_id       = $1,
         claim_timeout_at = now() + interval '30 seconds',
         attempt_count    = attempt_count + 1
  WHERE  outbox_seq = (
      SELECT outbox_seq FROM sem_reg.outbox_events
      WHERE  processed_at IS NULL
        AND  (claimed_at IS NULL OR claim_timeout_at < now())
      ORDER  BY outbox_seq
      LIMIT  1
      FOR UPDATE SKIP LOCKED
  )
  RETURNING *;
  ```
- [ ] Create `sem_os_server/src/dispatcher.rs` — `OutboxDispatcher`:
  ```rust
  pub struct OutboxDispatcher {
      outbox:    Arc<dyn OutboxStore>,
      projector: Arc<dyn ProjectionWriter>,
      interval:  Duration,
      max_fails: u32,
  }

  impl OutboxDispatcher {
      pub async fn run(&self) {
          loop {
              match self.outbox.claim_next("dispatcher-1").await {
                  Ok(Some(event)) => self.process(event).await,
                  Ok(None)        => tokio::time::sleep(self.interval).await,
                  Err(e)          => { tracing::error!("claim failed: {e}"); sleep(self.interval).await }
              }
          }
      }

      async fn process(&self, event: OutboxEvent) {
          match self.projector.write_active_snapshot_set(&event.snapshot_set_id).await {
              Ok(())  => { let _ = self.outbox.mark_processed(&event.event_id).await; }
              Err(e)  => {
                  let _ = self.outbox.mark_failed(&event.event_id, &e.to_string()).await;
                  if event.attempt_count >= self.max_fails {
                      tracing::error!("DEAD LETTER: outbox_seq={} error={e}", event.outbox_seq);
                  }
              }
          }
      }
  }
  ```
- [ ] Wire `OutboxDispatcher` into `sem_os_server/src/main.rs` as a `tokio::spawn` background task (replace the Stage 2.1 stub).
- [ ] Implement `InProcessClient::drain_outbox_for_test()` — runs dispatcher synchronously until no events remain.
- [ ] Run harness: `cargo test -p sem_os_harness`. `test_projection_watermark_advances` must pass.
- [ ] Run `cargo check --workspace`.

**Completion marker:**
```
✅ STAGE 2.2 COMPLETE — outbox live, dispatcher running, watermark advancing, harness green
→ IMMEDIATELY PROCEEDING TO STAGE 2.3
```

---

### STAGE 2.3 — Enforce DB boundary with roles/privileges

**TODO — every item must be completed:**

- [ ] Create `sql/sem_os_roles.sql`:
  ```sql
  DO $$
  BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'sem_os_owner') THEN
      CREATE ROLE sem_os_owner NOLOGIN;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'sem_os_app') THEN
      CREATE ROLE sem_os_app NOLOGIN;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'ob_app') THEN
      CREATE ROLE ob_app NOLOGIN;
    END IF;
  END
  $$;
  ```
- [ ] Create `sql/sem_os_privileges.sql`:
  ```sql
  -- sem_os_owner owns the schema
  ALTER SCHEMA sem_reg OWNER TO sem_os_owner;
  ALTER SCHEMA sem_reg_pub OWNER TO sem_os_owner;

  -- sem_os_app: full access to sem_reg; write access to sem_reg_pub
  GRANT USAGE ON SCHEMA sem_reg     TO sem_os_app;
  GRANT ALL   ON ALL TABLES IN SCHEMA sem_reg TO sem_os_app;
  GRANT USAGE ON SCHEMA sem_reg_pub TO sem_os_app;
  GRANT ALL   ON ALL TABLES IN SCHEMA sem_reg_pub TO sem_os_app;

  -- ob_app: NO access to sem_reg (not even SELECT)
  REVOKE ALL ON SCHEMA sem_reg FROM ob_app;
  REVOKE ALL ON ALL TABLES IN SCHEMA sem_reg FROM ob_app;

  -- ob_app: read-only on sem_reg_pub
  GRANT USAGE  ON SCHEMA sem_reg_pub TO ob_app;
  GRANT SELECT ON ALL TABLES IN SCHEMA sem_reg_pub TO ob_app;
  ```
- [ ] Add integration test: connect as `ob_app` role; assert `SELECT 1 FROM sem_reg.snapshots` raises a permission error.
- [ ] Add integration test: connect as `ob_app` role; assert `SELECT 1 FROM sem_reg_pub.active_verb_contracts` succeeds.

**Completion marker:**
```
✅ STAGE 2.3 COMPLETE — DB boundary enforced, permission tests passing
→ IMMEDIATELY PROCEEDING TO STAGE 2.4
```

---

### STAGE 2.4 — ob-poc client cutover (`remote` mode)

**TODO — every item must be completed:**

- [ ] In `ob_poc/src/main.rs`, read `SEM_OS_MODE` env var at startup:
  ```rust
  let client: Arc<dyn SemOsClient> = match std::env::var("SEM_OS_MODE")
      .as_deref()
      .unwrap_or("inprocess")
  {
      "remote" => Arc::new(HttpClient::new(
          std::env::var("SEM_OS_URL").expect("SEM_OS_URL required in remote mode"),
          std::env::var("SEM_OS_JWT").expect("SEM_OS_JWT required in remote mode"),
      )),
      _ => Arc::new(InProcessClient::new(
          Arc::new(build_core_service().await),
          Principal::in_process("ob_poc_inprocess", vec!["admin".into()]),
      )),
  };
  ```
- [ ] Pass `Arc<dyn SemOsClient>` via dependency injection to all ob-poc components that call the semantic registry. No global state.
- [ ] Remove all direct calls to `sem_reg::*` functions from ob-poc. If any remain after this stage, they are bugs — remove them.
- [ ] Update `rust/src/mcp/tools_sem_reg.rs` to call `sem_os_client` methods instead of `sem_reg::*` directly.
- [ ] Run `cargo check --workspace`.
- [ ] Run full harness in both modes:
  ```bash
  SEM_OS_MODE=inprocess cargo test -p sem_os_harness
  SEM_OS_MODE=remote    cargo test -p sem_os_harness  # requires running sem_os_server
  ```
  Both must be green before proceeding.

**Completion marker:**
```
✅ STAGE 2.4 COMPLETE — ob-poc uses client only, harness green in both modes
→ IMMEDIATELY PROCEEDING TO STAGE 2.5
```

---

### STAGE 2.5 — `sem_reg_pub.*` projection schema

**TODO — every item must be completed:**

- [ ] Add migration `migrations/093_sem_reg_pub.sql`:
  ```sql
  CREATE SCHEMA IF NOT EXISTS sem_reg_pub;

  CREATE TABLE sem_reg_pub.active_verb_contracts (
      snapshot_set_id  UUID NOT NULL,
      snapshot_id      UUID NOT NULL,
      fqn              TEXT NOT NULL,
      verb_name        TEXT NOT NULL,
      payload          JSONB NOT NULL,
      published_at     TIMESTAMPTZ NOT NULL,
      PRIMARY KEY (snapshot_set_id, fqn)
  );

  CREATE TABLE sem_reg_pub.active_entity_types (
      snapshot_set_id  UUID NOT NULL,
      snapshot_id      UUID NOT NULL,
      fqn              TEXT NOT NULL,
      payload          JSONB NOT NULL,
      published_at     TIMESTAMPTZ NOT NULL,
      PRIMARY KEY (snapshot_set_id, fqn)
  );

  CREATE TABLE sem_reg_pub.active_taxonomies (
      snapshot_set_id  UUID NOT NULL,
      snapshot_id      UUID NOT NULL,
      fqn              TEXT NOT NULL,
      payload          JSONB NOT NULL,
      published_at     TIMESTAMPTZ NOT NULL,
      PRIMARY KEY (snapshot_set_id, fqn)
  );

  CREATE TABLE sem_reg_pub.projection_watermark (
      projection_name  TEXT PRIMARY KEY,
      last_outbox_seq  BIGINT,
      updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
  );

  INSERT INTO sem_reg_pub.projection_watermark (projection_name, last_outbox_seq)
  VALUES ('active_snapshot_set', NULL)
  ON CONFLICT DO NOTHING;
  ```
- [ ] Implement `PgProjectionWriter::write_active_snapshot_set()` in `sem_os_postgres/src/projections/pub_writer.rs`:
  - Load all snapshot entries for `snapshot_set_id` from `sem_reg.*`.
  - Upsert into `sem_reg_pub.active_verb_contracts`, `active_entity_types`, `active_taxonomies`.
  - Update `projection_watermark` with the event's `outbox_seq`.
  - All in one Postgres transaction.
- [ ] Replace the `MigrationPending` stub in `PgProjectionWriter`. It must now fully execute.
- [ ] Run `cargo test -p sem_os_harness`. `test_projection_watermark_advances` must pass with real data.

**Completion marker:**
```
✅ STAGE 2.5 COMPLETE — sem_reg_pub populated, watermark advancing, harness green
→ IMMEDIATELY PROCEEDING TO STAGE 3.1
```

---

### STAGE 3.1 — Changesets + Draft entries + Review/Approval

**`sem_reg.draft_snapshots` does not exist. `changeset_entries` is the sole draft payload store. Do not create a `draft_snapshots` table.**

**TODO — every item must be completed:**

- [ ] Add migration `migrations/094_sem_reg_changesets.sql`:
  ```sql
  CREATE TABLE sem_reg.changesets (
      changeset_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      status          TEXT NOT NULL CHECK (status IN ('draft','in_review','approved','published','rejected')),
      owner_actor_id  TEXT NOT NULL,
      scope           TEXT NOT NULL,
      created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
      updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
  );

  CREATE TABLE sem_reg.changeset_entries (
      entry_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      changeset_id     UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
      object_fqn       TEXT NOT NULL,
      object_type      TEXT NOT NULL,
      change_kind      TEXT NOT NULL CHECK (change_kind IN ('add','modify','remove')),
      draft_payload    JSONB NOT NULL,
      base_snapshot_id UUID,
      created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
  );

  CREATE TABLE sem_reg.changeset_reviews (
      review_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      changeset_id    UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
      actor_id        TEXT NOT NULL,
      verdict         TEXT NOT NULL CHECK (verdict IN ('approved','rejected','requested_changes')),
      comment         TEXT,
      reviewed_at     TIMESTAMPTZ NOT NULL DEFAULT now()
  );
  ```
- [ ] Implement `ChangesetStore` port (replaces stub) in `sem_os_postgres`.
- [ ] Implement changeset promotion logic in core service:
  - Check `changeset.status = 'approved'` (return `SemOsError::Unauthorized` if not).
  - For each `changeset_entry`, compare `base_snapshot_id` to current active snapshot. If mismatch, return `SemOsError::Conflict("stale draft: FQN <x>")`.
  - Insert new rows into `sem_reg.snapshots` (insert-only, no updates).
  - Update `changeset.status = 'published'`.
  - Enqueue outbox event (atomic with the snapshot inserts).
- [ ] Run `cargo check --workspace`.

**Completion marker:**
```
✅ STAGE 3.1 COMPLETE — changesets live, promotion logic implemented, stale detection working
→ IMMEDIATELY PROCEEDING TO STAGE 3.2
```

---

### STAGE 3.2 — Workbench APIs

**TODO — every item must be completed:**

- [ ] Add endpoints to `sem_os_server`:
  - `GET /changesets` — list with `?status=`, `?owner=`, `?scope=` query params.
  - `GET /changesets/{id}/diff` — diff `changeset_entries` against current active snapshots, pinned to `base_snapshot_id`. Return added/modified/removed entries.
  - `GET /changesets/{id}/impact` — for each modified FQN, list downstream dependents (consumers of that verb contract, entity type, etc).
  - `POST /changesets/{id}/gate_preview` — run `evaluate_publish_gates()` against the draft entries. Return `GateViolation[]`. Must produce identical output to a real publish gate run.
  - `POST /changesets/{id}/publish` — trigger promotion logic from S3.1. Requires `changeset.status = 'approved'`.
- [ ] Add corresponding methods to `SemOsClient` trait and both client implementations.
- [ ] Run `cargo check --workspace`.

**Completion marker:**
```
✅ STAGE 3.2 COMPLETE — workbench APIs live
→ IMMEDIATELY PROCEEDING TO STAGE 3.3
```

---

### STAGE 3.3 — Stewardship agent guardrails

**TODO — every item must be completed:**

- [ ] Implement in `sem_os_core/src/stewardship.rs`:
  - `validate_role_constraints(principal, changeset_entries) -> Result<()>` — asserts that the actor's roles permit the change_kinds in the entries.
  - `check_proof_chain_compatibility(entries, snapshot_store) -> Result<()>` — validates that draft entries don't break existing proof chains.
  - `detect_stale_drafts(entries, snapshot_store) -> Result<Vec<StaleDraftConflict>>` — returns all entries where `base_snapshot_id` no longer matches current active.
- [ ] Stewardship checks must run as part of `POST /changesets/{id}/gate_preview` and `POST /changesets/{id}/publish`.
- [ ] Agent cannot call `POST /publish` (the raw publish endpoint) — that endpoint requires `has_role("admin")`. Agents have `has_role("steward")` at most.
- [ ] Run `cargo check --workspace`. Run harness.

**Completion marker:**
```
✅ STAGE 3.3 COMPLETE — stewardship guardrails active
→ IMMEDIATELY PROCEEDING TO STAGE 4
```

---

### STAGE 4 — Cutover validation + rollback verification

**TODO — every item must be completed:**

- [ ] Run full harness against `SEM_OS_MODE=inprocess`:
  ```bash
  SEM_OS_MODE=inprocess cargo test -p sem_os_harness --test harness -- --nocapture
  ```
  All scenarios must pass. Diff output must show zero gate outcome differences.
- [ ] Run full harness against `SEM_OS_MODE=remote` (server running):
  ```bash
  SEM_OS_MODE=remote SEM_OS_URL=http://localhost:9000 SEM_OS_JWT=<test-token> \
    cargo test -p sem_os_harness --test harness -- --nocapture
  ```
  All scenarios must pass.
- [ ] Verify rollback: stop `sem_os_server`, set `SEM_OS_MODE=inprocess`, run ob-poc. Must work without any DB changes.
- [ ] Document migration sequence confirmation: verify `091`–`094` do not conflict with any existing migration numbers in the repo. If conflicts exist, renumber and update all references in this document and in CLAUDE.md.
- [ ] Add post-cutover housekeeping notes to CLAUDE.md (do not execute yet):
  - D2 evidence correction: add `sem_reg.attribute_observations`, migrate MCP tools, drop `sem_reg.observations` in housekeeping migration.
  - Drop in-process ABAC fallback code once remote mode is stable in production.

**Completion marker:**
```
✅ STAGE 4 COMPLETE — harness green in both modes, rollback verified, cutover ready
```

---

## 9) Migration sequence

| Migration file | Contents | Stage |
|---|---|---|
| `091_sem_reg_bootstrap_audit.sql` | Bootstrap idempotency + audit table | S2.1 |
| `092_sem_reg_outbox.sql` | Outbox with `outbox_seq GENERATED ALWAYS AS IDENTITY` | S2.2 |
| `093_sem_reg_pub.sql` | `sem_reg_pub.*` projection schema + watermark | S2.5 |
| `094_sem_reg_changesets.sql` | Changesets + `changeset_entries` (no `draft_snapshots`) | S3.1 |

> **Before starting Stage 2.1:** cross-check these numbers against all files in `migrations/`. If `091`–`094` conflict with existing files, renumber to the next available block and update every reference in this document.

---

## 10) Environment variables reference

| Variable | Required | Default | Description |
|---|---|---|---|
| `SEM_OS_MODE` | no | `inprocess` | `inprocess` or `remote` |
| `SEM_OS_DATABASE_URL` | server only | — | Postgres connection string for `sem_os_server` |
| `SEM_OS_JWT_SECRET` | server only | — | JWT HMAC secret (or JWKS URL prefix `jwks:`) |
| `SEM_OS_BIND_ADDR` | server only | `0.0.0.0:9000` | Server bind address |
| `SEM_OS_URL` | remote mode | — | Base URL of running `sem_os_server` |
| `SEM_OS_JWT` | remote mode | — | Bearer JWT for `HttpClient` |
| `SEM_OS_DISPATCHER_INTERVAL_MS` | no | `500` | Outbox poll interval |
| `SEM_OS_DISPATCHER_MAX_FAILS` | no | `5` | Dead-letter threshold |

---

## 11) What is NOT in scope for this implementation drop

The following are explicitly deferred. Claude Code must not implement them during this drop — doing so risks scope creep that breaks the stage gates:

- **gRPC / tonic transport** — the proto file and prost types are laid down now. Adding a `GrpcClient` impl and a tonic server is a future one-afternoon task.
- **D2 evidence schema correction** — add `sem_reg.attribute_observations` in the post-cutover housekeeping phase.
- **UI for workbench** — APIs only in S3.2.
- **mTLS** — JWT is sufficient for now. mTLS is a deployment concern.
- **Multi-region or distributed outbox** — single-process dispatcher is correct for this deployment scale.
