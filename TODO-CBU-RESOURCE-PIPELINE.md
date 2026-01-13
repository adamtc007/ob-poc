# TODO: CBU Service → Resource Discovery → Unified Dictionary → Provisioning

> **Status:** ✅ **IMPLEMENTED** (2026-01-13)
> 
> All phases completed:
> - Migrations 024-027 applied
> - Rust types in `rust/src/service_resources/`
> - SRDEF YAML configs in `rust/config/srdefs/`
> - Discovery + Rollup + Population engines
> - Provisioning orchestrator + Readiness engine
> - REST API endpoints at `/api/cbu/{id}/...`

---

## Goal
Implement an end-to-end pipeline where:

1) **CBU subscribes to Products** and configures Service options (markets, SSI, channels, etc.)  
2) A **Resource Discovery** phase derives required **ServiceResourceDefinitions (SRDEFs)** from ServiceIntent  
3) All SRDEF **Attribute Profiles** are rolled up to a **CBU Unified Attribute Dictionary** (de-duped)  
4) The unified dictionary is populated (from CBU/entity/doc/derived/manual)  
5) When SRDEF-required attributes are satisfied, run **resource provisioning** (create/discover/bind) deterministically

Deliver a minimal working vertical slice: **1–2 products, 2–3 services, 3–6 SRDEFs**.

---

## Non-negotiable invariants
- **SRDEF defines required attributes** via Resource Attribute Profile (subset of global Attribute Dictionary)
- **CBU Unified Dictionary is derived** (not hand-authored) from SRDEF Attribute Profiles
- **De-dupe key = AttributeId**
- **Provisioning gate**: SRDEF cannot provision until all required attributes satisfied + validations pass
- All derived artifacts must be **recomputable idempotently**

---

## Phase 0 — Add core data types (Rust structs) + DB tables

### 0.1 Add Rust types
Create module: `rust/src/service_resources/mod.rs` (or your preferred domain folder).

Define:

```rust
// Identifiers
type SrdefId = String;     // "SRDEF::APP::Kind::Purpose"
type Srid = String;        // "SR::APP::Kind::NativeKey"
type ProductId = String;
type ServiceId = String;

#[derive(Clone)]
struct ServiceIntent {
  cbu_id: uuid::Uuid,
  product_id: ProductId,
  service_id: ServiceId,
  options: serde_json::Value,     // minimal now; typed later
}

#[derive(Clone)]
struct Srdef {
  srdef_id: SrdefId,
  app_mnemonic: String,
  resource_kind: String,          // Account | InstructionSet | Connectivity | Entitlement | DataObject | DocumentArtifact
  resource_purpose: String,
  provisioning_strategy: String,  // create | request | discover
  dependencies: Vec<SrdefId>,
}

#[derive(Clone)]
struct SrdefAttributeRequirement {
  srdef_id: SrdefId,
  attr_id: String,                // AttributeId
  requirement: String,            // required | optional | conditional
  source_policy: Vec<String>,     // derived/entity/cbu/document/manual/external
  constraints: serde_json::Value, // type/range/regex etc.
  evidence_policy: serde_json::Value,
}

#[derive(Clone)]
struct CbuUnifiedAttrRequirement {
  cbu_id: uuid::Uuid,
  attr_id: String,
  requirement_strength: String,   // required|optional
  merged_constraints: serde_json::Value,
  preferred_source: String,
  required_by_srdefs: Vec<SrdefId>,
  conflict: Option<serde_json::Value>,
}

#[derive(Clone)]
struct CbuAttrValue {
  cbu_id: uuid::Uuid,
  attr_id: String,
  value: serde_json::Value,
  source: String,
  evidence_refs: Vec<String>,
  explain_refs: Vec<String>,
  as_of: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
struct ServiceResourceInstance {
  cbu_id: uuid::Uuid,
  srdef_id: SrdefId,
  srid: Option<Srid>,             // known after provision/discover
  native_key: Option<String>,
  state: String,                  // requested|provisioning|active|failed|...
  bind_to: serde_json::Value,      // entity-ref resolved pk, etc.
}
```

### 0.2 Add DB tables (goose migration)
Create new migrations:

- `service_intents`
- `srdefs`
- `srdef_attribute_requirements`
- `cbu_unified_attr_requirements`
- `cbu_attr_values`
- `service_resource_instances`

Make `cbu_unified_attr_requirements` and `service_resource_instances` **derived/materialized** tables (rebuildable).

---

## Phase 1 — SRDEF registry + attribute profile loader

### 1.1 SRDEF registry source
Pick one:
- YAML files under `config/srdefs/*.yaml`, loaded at startup
- or seed into DB via xtask

Implement loader:
- parse SRDEF
- parse attribute requirements
- register in memory (and optionally persist to DB)

### 1.2 Provide minimal seed definitions
Create **3–6 SRDEFs** to prove flow:

Example set:
- `SRDEF::CUSTODY::Account::custody_securities`
- `SRDEF::CUSTODY::Account::custody_cash`
- `SRDEF::SWIFT::Connectivity::swift_sender_receiver`
- `SRDEF::IAM::Entitlement::custody_ops_role`
- `SRDEF::TA::DataObject::fund_register_profile` (optional)

Each SRDEF must list required attrs (AttributeIds) such as:
- `market_scope`, `settlement_currency`, `bic_sender`, `bic_receiver`, `ssi_mode`, etc.

---

## Phase 2 — Service Intent capture (DSL verbs + API)

### 2.1 DSL verbs (minimal)
Add verbs:
- `(product.subscribe (cbu ...) (product "..."))`
- `(service.configure (cbu ...) (product "...") (service "...") (options ...))`

Parser → AST → store into `service_intents`.

### 2.2 API endpoints
- `POST /cbu/{id}/service-intents` (optional if DSL-only)
- `GET /cbu/{id}/service-intents`

---

## Phase 3 — Resource discovery engine (ServiceIntent → SRDEFs)

### 3.1 Implement discovery rules
Create `ResourceDiscoveryEngine`:

```rust
fn discover(service_intents: &[ServiceIntent]) -> Vec<(SrdefId, serde_json::Value /* reason */)>;
```

Rules can be simple mapping tables initially:
- if `Custody + Settlement` and markets include `XNAS` → require custody_securities + swift connectivity + entitlements
- if SSI mode = "standing" → require InstructionSet SRDEF, etc.

Persist outputs into `service_resource_instances` as `requested` with `srdef_id`, no srid yet.

### 3.2 API endpoint
- `POST /cbu/{id}/resource-discover` → recompute instances deterministically (delete+rebuild)

---

## Phase 4 — Unified dictionary roll-up + de-dupe

### 4.1 Implement roll-up
Function:

```rust
fn build_cbu_unified_requirements(
  cbu_id: Uuid,
  srdefs: &[SrdefId],
  profile_index: &HashMap<SrdefId, Vec<SrdefAttributeRequirement>>,
) -> Vec<CbuUnifiedAttrRequirement>;
```

Merge rules:
- required dominates optional
- merge constraints (best-effort)
- record `required_by_srdefs`
- if constraint merge impossible → set `conflict`

Write results into `cbu_unified_attr_requirements`.

### 4.2 API endpoint
- `POST /cbu/{id}/attributes/rollup` (or auto-run after discovery)
- `GET /cbu/{id}/attributes/requirements`

---

## Phase 5 — Population engine (fill values)

### 5.1 Implement population sources (minimal)
Implement population order:
1) Derived (computed)
2) Entity/CBU tables
3) Document extraction stub (later)
4) Manual (via API)

Implement:
- `populate_missing(cbu_id)` which attempts to fill any required attribute values.
- For now, implement only:
  - CBU/entity sourced fields
  - manual set via API

### 5.2 API endpoints
- `GET /cbu/{id}/attributes/values`
- `POST /cbu/{id}/attributes/values` to set manual values (with evidence refs)

---

## Phase 6 — Provisioning gate + provisioning execution

### 6.1 Readiness check per SRDEF
Function:

```rust
fn srdef_ready_to_provision(cbu_id, srdef_id) -> Result<bool, MissingInputsReport>;
```

`MissingInputsReport` must list:
- missing attr_ids
- conflicts
- missing evidence/gates (stub)

### 6.2 Provisioning orchestrator
Implement:
- topo-sort SRDEF dependencies
- for each srdef:
  - if ready: call provisioner
  - else: leave in `requested` and attach "missing inputs report"

### 6.3 Provisioner interface (stub)
Define trait:

```rust
trait ResourceProvisioner {
  fn provision(&self, cbu_id: Uuid, srdef: &Srdef, attrs: &HashMap<AttrId, Value>)
    -> Result<(Srid, String /*native_key*/), ProvisionError>;
}
```

Implement a dummy provisioner that synthesizes:
- srid = `SR::<APP>::<Kind>::<FAKEKEY>` (until real integration)
- transitions state to `active`

### 6.4 API endpoints
- `POST /cbu/{id}/resources/provision`
- `GET /cbu/{id}/resources`

---


---

## Phase 6.5 — Provisioning requests + owner responses (append-only ledger)

### 6.5.1 Add new tables
Add two append-only tables to capture the "last mile" loop closure from resource owners/platforms:

- `provisioning_requests` (append-only)
- `provisioning_events` (append-only)

**provisioning_requests** (one row per request)
- `request_id` (UUID, idempotency key; deterministic if possible)
- `cbu_id` (UUID)
- `srdef_id` (text)
- `requested_by` (text: agent|user|system)
- `requested_at` (timestamptz)
- `request_payload` (jsonb) — includes attrs snapshot, bind_to, evidence refs
- `status` (text: queued|sent|ack|completed|failed|cancelled)
- `owner_system` (text) — app mnemonic or owner team
- `owner_ticket_id` (text, nullable)

**provisioning_events** (many rows per request; append-only)
- `event_id` (UUID)
- `request_id` (UUID, FK)
- `occurred_at` (timestamptz)
- `direction` (text: OUT|IN)
- `kind` (text: REQUEST_SENT|ACK|RESULT|ERROR|STATUS)
- `payload` (jsonb) — the canonical ProvisioningResult or status update
- `hash` (text) — optional content hash for dedupe

### 6.5.2 Canonical ProvisioningResult payload (owner response)
Define and persist this payload format in `provisioning_events.payload` when `kind=RESULT`:

```json
{
  "srdef_id": "SRDEF::CUSTODY::Account::custody_securities",
  "request_id": "uuid",
  "status": "active | pending | rejected | failed",
  "srid": "SR::CUSTODY_APP::Account::ACCT-12345678",
  "native_key": "ACCT-12345678",
  "native_key_type": "AccountNo",
  "resource_url": "https://<platform>/accounts/ACCT-12345678",
  "owner_ticket_id": "INC12345",
  "explain": {
    "message": "optional failure or rejection reason",
    "codes": ["..."]
  },
  "timestamp": "2026-01-13T12:00:00Z"
}
```

### 6.5.3 Materialize into service_resource_instances
Update `service_resource_instances` when a RESULT arrives:
- set `srid`, `native_key`, `state`
- store `resource_url` (new column) and `owner_ticket_id` (new column)
- keep the append-only ledger as the audit trail

Add columns to `service_resource_instances`:
- `resource_url` (text, nullable)
- `owner_ticket_id` (text, nullable)
- `last_request_id` (uuid, nullable)
- `last_event_at` (timestamptz, nullable)

---

## Phase 6.6 — Service readiness (“good-to-transact”) computation

### 6.6.1 Add table
Add derived/materialized table:

- `cbu_service_readiness`

Columns:
- `cbu_id` (UUID)
- `product_id` (text)
- `service_id` (text)
- `status` (text: ready|blocked|partial)
- `blocking_reasons` (jsonb) — missing srdefs, missing attrs, conflicts, pending provisioning, failed provisioning
- `required_srdefs` (jsonb array of srdef_ids)
- `active_srids` (jsonb array of srids)
- `as_of` (timestamptz)

### 6.6.2 Implement readiness algorithm
For each ServiceIntent:
- compute required SRDEFs (from discovery reasons)
- check for each SRDEF:
  - instance exists in `service_resource_instances`
  - `state == active`
  - `resource_url` present if policy demands it
- mark READY only if all required SRDEFs are active
- else BLOCKED with concrete reasons (missing attrs, conflicts, pending owner response, failed provisioning)

### 6.6.3 API endpoints
- `POST /cbu/{id}/readiness/recompute`
- `GET  /cbu/{id}/readiness`

---

## Phase 7.5 — Observability: link explain + provenance across the loop
Extend explain payloads to include:
- `request_id` for provisioning events tied to each SRDEF instance
- `resource_url` when active
- `required_by` lists for unified attributes (which srdefs/services caused it)

This enables a single drill-down:
**ServiceIntent → SRDEF(s) → missing attrs / provisioning request → owner response → SRID + URL**


## Phase 7 — Observability + explainability (must)
For each derived artifact, store explain:

- discovery explain (which intent caused srdef)
- roll-up explain (why attribute is required, which srdefs)
- readiness explain (what’s missing)
- provisioning explain (what created srid)

At minimum, store these as JSON columns in the derived tables.

---

## Phase 8 — Tests (fast, deterministic)
Add tests covering:

1) Discovery idempotency: run twice → same srdef set / instances  
2) Roll-up correctness: union of srdef required attrs = cbu required attrs  
3) Conflict detection: conflicting constraints yields conflict  
4) Provisioning gate blocks if missing required value  
5) Provisioning topo order respects dependencies  

---

## Deliverables checklist
- migrations + models
- SRDEF registry loader + sample SRDEF YAMLs
- DSL verbs for subscribe/configure + persistence
- discovery + roll-up + population + provision pipeline
- endpoints to drive pipeline
- tests

---

## Notes (explicitly OK to refine later)
- `options` JSON for service intent can be typed later
- discriminator/gate policies can be stubbed now
- real provisioning adapters to BNY apps come later; keep interface stable
