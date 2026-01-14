# TODO — Investor Register + UBO Control vs Economic Exposure (Look-through without Edge Explosion)

This TODO is based on the contents of `ob-poc-ownership-ubo.tar.gz` (investor/ownership DSL + Rust + SQL migrations).

## Objective
Refactor/extend the current investor + holdings + UBO sync model so that:

1) **UBO** remains a *control* question (board/GP/appointment/voting) and avoids mis-identifying pooled/intermediary funds as “UBOs”.
2) **Economic exposure** remains a *holdings / NAV* question and supports *look-through* **without** materializing cartesian implied edges.
3) The DSL verb YAML **matches the DB schema** (so agent-generated DSL executes correctly).

Deliverables should be implementable by Claude Code (Opus) in staged PRs.

---

## Background — Why this work exists (institutional SME context)

### The institutional problem this solves
Allianz-style private markets structures (and similar insurers/asset managers) commonly use multi-tier fund structures:

- **Feeder / Fund-of-Funds (FoF)** vehicles for different investor channels (internal group capital, external institutional, ELTIF-like wrappers).
- A **master pooling vehicle** (“asset bank”) such as a Luxembourg **SCSp** or **SICAV-RAIF**, which manages calls/distributions and holds portfolio assets via holdcos/SPVs.
- Optional **umbrella + compartment** patterns where compartments represent sleeves/strategies even when not separate legal entities.

This creates two distinct but overlapping questions:

1) **UBO / control** (regulatory/KYC): “Who ultimately controls the vehicle?”  
   Practically this is often **GP/ManCo/board appointment rights**, not “who holds units”.

2) **Economic exposure** (investment/operations): “Who has the economic interest / NAV exposure?”  
   This is often **multi-level** (end-investor → FoF → master pool → holdco → project SPV).

### Why naïve models fail
A naïve “holdings ≥ 25% ⇒ UBO edge” approach breaks in pooled environments because:
- Intermediary funds, nominees, omnibus holders, and master pools can exceed thresholds but are **not** ultimate beneficial owners.
- Control is often held via **GP/ManCo/board rights** independent of economic holdings.

A naïve “materialize look-through edges” approach breaks at scale because it creates **cartesian explosions**:
- If 1,000 investors hold a FoF and the FoF holds 200 SPVs, materializing implied edges creates 200,000 edges (and gets worse with more tiers).

### The design principle
Maintain **two overlay graphs** with explicit metadata and bounded look-through:

- **Control graph (UBO):** driven by control edges, special rights, board controller computation.
- **Economic graph:** store only direct holdings edges; compute look-through on-demand into bounded “exposure slices”.

Role metadata (issuer-scoped) is the critical control knob that:
- prevents pooled/intermediary holders from being misclassified as UBO,
- drives whether look-through is allowed/available,
- and supports hybrid programmes (intra-group + external institutional investors).


---

## 1) What exists today (quick map)

### 1.1 KYC/TA investor register (schema: `kyc`)
- `kyc.investors` — lifecycle + KYC status; keyed by `entity_id` plus `owning_cbu_id` (unique constraint).
- `kyc.holdings` — positions in share classes (`share_class_id`, `investor_entity_id`, `units`) plus `usage_type` (TA vs UBO intent).
- `kyc.movements` — subscription/redemption/transfer/capital_call etc.
- Views: `kyc.v_investor_register`, `kyc.v_ubo_holdings`, `kyc.v_share_class_summary`, `kyc.v_investor_portfolio`.

### 1.2 UBO proxy sync (migration 011)
- Trigger `kyc.sync_holding_to_ubo_relationship()` writes `ob-poc.entity_relationships(relationship_type='ownership')` when a holding ≥ 25%.

### 1.3 Capital/ownership framework (migration 013)
- `kyc.ownership_snapshots` (basis: UNITS/VOTES/ECONOMIC/CAPITAL/DECLARED) derived from REGISTER/BODS/GLEIF etc.
- `kyc.special_rights` to model board/veto etc.

### 1.4 Control edge framework (migration 022)
- `ob-poc.control_edges`, `cbu_board_controller`, `board_control_evidence`, `cbu_control_anchors`.

### 1.5 Economic investment registry (schema: `ob-poc`)
- `ob-poc.fund_investments` (investor_entity_id → investee_entity_id, %NAV/%AUM)
- `ob-poc.fund_investors` (fund_cbu_id → investor_entity_id, investor_type, kyc status...)

---

## 2) Issues found (must fix)

### 2.1 DSL YAML verb mappings do not match DB schema (critical)
Files:
- `rust/config/verbs/registry/investor.yaml`
- `rust/config/verbs/registry/holding.yaml`

Examples:
- `investor.yaml` uses `cbu_id` but the table column is `owning_cbu_id`.
- `investor.yaml` maps to `investor_name` / `tax_residence`, but `kyc.investors` does not have these columns.
- `holding.yaml` uses `usage-type: ta_kyc | ubo_tracking`, while schema comments/defaults indicate `'TA' | 'UBO'` (and views use uppercase strings).
- status enums are mixed case; SQL uses `'active'` checks in views/triggers via `COALESCE(holding_status, status) = 'active'`.

Impact: agent-generated DSL will fail or silently write wrong columns.

### 2.2 Trigger generates misleading “UBO ownership” edges for pooled vehicles
FoF/master pool/nominee holders can hit ≥25% but they are not “ultimate beneficial owners” in the governance/control sense. UBO should be driven by control edges / board controller computation.

### 2.3 Missing “holder role” + “look-through policy”
`investor_type` is not enough to decide:
- whether a holder is UBO-eligible
- whether look-through is allowed/available
- whether the holder is a pooled intermediary node that should be collapsed by default

### 2.4 Cartesian explosion risk
If you ever materialize implied edges like:
`ultimate investor -> every underlying SPV` (derived via all fund holdings paths),
you get explosive edge growth. Look-through must be computed on-demand and returned as *exposure slices*, not stored as graph edges.

### 2.5 Missing fund vehicle / umbrella representation + holding instrument types
To fully load Allianz Group (including FoF / umbrella / master pooling vehicles) we need explicit metadata for:

- **Fund vehicle types** (e.g., SCSp, SICAV-RAIF, SICAV-SIF, SIF, etc.) and umbrella/compartment structure.
- **Holding instrument types** (e.g., UNITS, SHARES, LP_INTEREST, PARTNERSHIP_INTEREST, NOMINEE_POSITION) so economic edges can represent *fund-of-fund* ownership cleanly.
- **Investor base affiliation** (INTRA_GROUP vs EXTERNAL vs MIXED) and **look-through availability** (do we have BO data?).

Without these, the system either:
- misclassifies FoF/master pools as “UBO”, or
- cannot represent “hybrid” pools that have both intra-group and third-party investors, or
- forces everything into a single generic “holding” shape that loses meaning.



---

## 3) Proposed direction (minimal but durable)

### 3.1 Separate **Control** from **Economic**
- Control (UBO): `ob-poc.control_edges` + `kyc.special_rights` + `cbu_board_controller`
- Economic: `kyc.holdings` / `kyc.ownership_snapshots(basis='ECONOMIC')` + `ob-poc.fund_investments`

### 3.2 Add explicit holder role metadata table
A small table answering: “for this issuer, who is this holder and how do we treat it?”

### 3.3 Compute look-through exposure views (don’t store implied edges)
- Store only direct edges (holdings / fund_investments)
- Derive look-through results into a *query response* (and optionally cache a limited, bounded table)

### 3.4 Add fund vehicle taxonomy + investor affiliation flags (supports Allianz umbrella/FoF)
Add explicit, schema-backed metadata so the same mechanics can represent:
- intra-group master pools,
- umbrella funds + compartments,
- feeder FoFs,
- and third-party end-investors (look-through when available).

Key principles:
- **Direct holdings edges only** (holder → issuer/share-class), enriched with *instrument type* and *rights profile*.
- Treat umbrella/compartments as first-class “fund structure” records even if they are not separate legal entities (you can still attach them to `entities` via synthetic UUIDs if needed).



---

## 4) Concrete TODO (Claude Code implementable)

## Phase A — Fix DSL verb YAML to match schema (DO FIRST)

### A1) Patch `rust/config/verbs/registry/investor.yaml`
Align to `kyc.investors` columns:

- Rename `cbu-id` → `owning-cbu-id` and map to `owning_cbu_id`.
- Remove `investor-name` (display name is `entities.name`).
- Add/align: `investor_type`, `investor_category`, `tax_status`, `tax_jurisdiction`, `fatca_status`, `crs_status`, `kyc_status`, etc. (start minimal: type/category/tax_jurisdiction/provider/provider_reference).
- Align enum values to your SQL docstrings: `RETAIL, PROFESSIONAL, INSTITUTIONAL, NOMINEE, INTRA_GROUP`.

### A2) Patch `rust/config/verbs/registry/holding.yaml`
Align to `kyc.holdings`:

- `usage-type` enum must be `'TA' | 'UBO'` (or whatever you actually store; standardize).
- Standardize holding state on `holding_status` only; treat legacy `status` as deprecated.
- Align `holding_status` enum to what you want: `PENDING, ACTIVE, SUSPENDED, CLOSED` (and update SQL views/triggers accordingly).
- Ensure upserts use the right conflict keys (share_class_id + investor_entity_id is fine for economic holdings; share_class_id + investor_id for TA investor holdings).

### A3) Add a regression harness
Add a small DSL test file under `rust/tests/fixtures/` that:
- creates entity
- creates investor record
- creates a holding (TA and UBO variants)
and executes end-to-end against a migrated DB.

**Acceptance:** test executes without SQL errors and rows appear in intended columns.

---

## Phase B — Add holder role profiles (stop treating pooled funds as “UBO”)

### B1) New migration: `migrations/0XX_investor_role_profiles.sql`
Create:

```sql
CREATE TABLE IF NOT EXISTS kyc.investor_role_profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  share_class_id UUID NULL REFERENCES kyc.share_classes(id),

  role_type VARCHAR(50) NOT NULL,
  lookthrough_policy VARCHAR(30) NOT NULL DEFAULT 'NONE',

  -- Hybrid pool support (Allianz FoF/umbrella): affiliation + look-through availability
  holder_affiliation VARCHAR(20) NOT NULL DEFAULT 'UNKNOWN',
  beneficial_owner_data_available BOOLEAN NOT NULL DEFAULT false,

  group_container_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id),
  group_label TEXT NULL,

  is_ubo_eligible BOOLEAN NOT NULL DEFAULT true,

  source VARCHAR(50) DEFAULT 'MANUAL',
  source_reference TEXT NULL,
  notes TEXT NULL,

  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),

  CONSTRAINT chk_role_type CHECK (role_type IN (
    'END_INVESTOR',
    'NOMINEE',
    'OMNIBUS',
    'INTERMEDIARY_FOF',
    'MASTER_POOL',
    'INTRA_GROUP_POOL',
    'TREASURY',
    'OTHER'
  )),
  CONSTRAINT chk_lookthrough CHECK (lookthrough_policy IN (
    'NONE',
    'ON_DEMAND',
    'AUTO_IF_DATA',
    'ALWAYS'
  )),
  CONSTRAINT chk_holder_affiliation CHECK (holder_affiliation IN (
    'INTRA_GROUP',
    'EXTERNAL',
    'MIXED',
    'UNKNOWN'
  )),

  CONSTRAINT uq_role_profile UNIQUE (
    issuer_entity_id, holder_entity_id, COALESCE(share_class_id, '00000000-0000-0000-0000-000000000000'::uuid)
  )
);

CREATE INDEX IF NOT EXISTS idx_role_profiles_issuer ON kyc.investor_role_profiles(issuer_entity_id);
CREATE INDEX IF NOT EXISTS idx_role_profiles_holder ON kyc.investor_role_profiles(holder_entity_id);
CREATE INDEX IF NOT EXISTS idx_role_profiles_group  ON kyc.investor_role_profiles(group_container_entity_id);
```

### B2) DSL verbs for role profiles
Add a new domain YAML, e.g. `rust/config/verbs/registry/investor-role.yaml`:
- `investor-role.set` (upsert by issuer + holder + optional share_class)
- `investor-role.read/list` (by issuer, by holder)

Use EntityRef lookups for issuer/holder and share class.

### B3) Update UBO sync trigger to respect role profiles and usage_type
Patch `kyc.sync_holding_to_ubo_relationship()` in `migrations/011_investor_register.sql`:

Rules:
- Only run when `NEW.usage_type = 'UBO'`
- If a role profile exists and `is_ubo_eligible = false`, do nothing
- (Optional) default-deny for pooled vehicles if `role_type in (INTERMEDIARY_FOF, MASTER_POOL, INTRA_GROUP_POOL)` unless explicitly overridden.

### B4) Add fund vehicle + umbrella/compartment metadata (Allianz group load)
Create minimal schema to represent fund structures and the “units/shares” instruments used between FoF → master pool → holdco layers.

**New tables (migration recommended):**
- `kyc.fund_vehicles` — one row per fund/legal vehicle (often LEI-backed)
- `kyc.fund_compartments` — optional compartments/sleeves under umbrella vehicles
- (Optional) extend `kyc.share_classes` with `instrument_type` / `share_class_type`

Suggested minimal DDL:

```sql
CREATE TABLE IF NOT EXISTS kyc.fund_vehicles (
  fund_entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id),
  vehicle_type VARCHAR(30) NOT NULL,          -- SCSP, SICAV_RAIF, SICAV_SIF, SIF, OTHER
  umbrella_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id),
  domicile_country CHAR(2) NULL,
  manager_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id),
  is_umbrella BOOLEAN NOT NULL DEFAULT false,
  meta JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_vehicle_type CHECK (vehicle_type IN (
    'SCSP','SICAV_RAIF','SICAV_SIF','SIF','OTHER'
  ))
);

CREATE TABLE IF NOT EXISTS kyc.fund_compartments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  umbrella_fund_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  compartment_code TEXT NOT NULL,
  compartment_name TEXT NULL,
  meta JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT uq_compartment UNIQUE (umbrella_fund_entity_id, compartment_code)
);
```

**Holdings / share-class enrichment:**
- Ensure your holdings model can represent:
  - FoF units/LP interests in master pools
  - nominee/omnibus positions
  - share classes with different rights (economic vs voting)

Recommended enum (store on share class or holding):
- `instrument_type IN ('UNITS','SHARES','LP_INTEREST','PARTNERSHIP_INTEREST','NOMINEE_POSITION')`

**DSL verbs to add:**
- `fund-vehicle.upsert` (entity-ref + vehicle_type + optional umbrella link)
- `fund-compartment.upsert` (umbrella + compartment_code/name)
- extend `share-class.upsert` to accept `instrument_type` and optional `compartment_id`

Acceptance:
- You can load Allianz vehicles (FoF/master/umbrella) with correct types.
- You can represent “hybrid” investor base via `holder_affiliation` and `beneficial_owner_data_available`.



---

## DSL verb specs (copy/paste guidance for engineering)

These verbs exist to make the model operable through your composite DSL (agent authored runbooks). They must:
- embed EntityRef search keys (LEI/name/etc),
- be schema-correct (column names + enums),
- and preserve the separation of **control** vs **economic** to prevent edge explosions.

### 1) `fund-vehicle.upsert`
Purpose: Load fund vehicles (FoF/master pool/umbrella) with a stable taxonomy so discovery, UI grouping, and look-through policies work.

**S-expression example**
```lisp
(fund-vehicle.upsert
  (fund (entity-ref legal-entity (k lei "529900KRX8A6KQV2XK98")))
  (vehicle-type SCSP)
  (is-umbrella false)
  (domicile "LU")
  (umbrella (entity-ref legal-entity (k lei "222100384GKS6R7YQS49"))) ; optional
  (manager (entity-ref legal-entity (k name "Allianz Capital Partners"))) ; optional
)
```

**YAML skeleton (verbs/config)**
- verb: `fund-vehicle.upsert`
- args:
  - `fund` (EntityRef; search keys: lei|name|id)
  - `vehicle-type` (enum: SCSP|SICAV_RAIF|SICAV_SIF|SIF|OTHER)
  - `umbrella` (EntityRef optional)
  - `domicile` (ISO2 optional)
  - `manager` (EntityRef optional)
  - `is-umbrella` (bool)
  - `meta` (json optional)

### 2) `fund-compartment.upsert`
Purpose: Represent compartments/sleeves under an umbrella so you can model allocations without inventing fake legal entities.

**S-expression example**
```lisp
(fund-compartment.upsert
  (umbrella (entity-ref legal-entity (k lei "222100384GKS6R7YQS49")))
  (compartment-code "RENEWABLES_01")
  (compartment-name "Renewables Sleeve")
)
```

**YAML skeleton**
- verb: `fund-compartment.upsert`
- args:
  - `umbrella` (EntityRef)
  - `compartment-code` (string)
  - `compartment-name` (string optional)
  - `meta` (json optional)

### 3) `share-class.upsert` (extend)
Purpose: Ensure holdings can represent FoF/master pool instruments cleanly (units vs LP interests etc).

**S-expression example**
```lisp
(share-class.upsert
  (issuer (entity-ref legal-entity (k lei "529900KRX8A6KQV2XK98")))
  (share-class-code "A")
  (instrument-type LP_INTEREST)
  (compartment-code "RENEWABLES_01") ; optional, maps to fund_compartments
  (votes-per-unit 1)
)
```

**YAML extension**
- add `instrument-type` enum:
  - UNITS|SHARES|LP_INTEREST|PARTNERSHIP_INTEREST|NOMINEE_POSITION
- add optional `compartment-code` or `compartment-id`

### 4) `investor-role.set` / `investor-role.list`
Purpose: Issuer-scoped holder profile describing what the holder *is* (end investor, FoF, nominee, intra-group pool) and what policies apply.

**S-expression example**
```lisp
(investor-role.set
  (issuer (entity-ref legal-entity (k lei "529900KRX8A6KQV2XK98")))
  (holder (entity-ref legal-entity (k name "Allianz European Infrastructure Fund")))
  (share-class (share-class-ref (issuer (entity-ref legal-entity (k lei "529900KRX8A6KQV2XK98"))) (code "A"))) ; optional
  (role-type INTERMEDIARY_FOF)
  (holder-affiliation MIXED)
  (beneficial-owner-data-available false)
  (lookthrough-policy ON_DEMAND)
  (is-ubo-eligible false)
  (notes "FoF feeder into master pool; do not treat as UBO; allow look-through only when BO data present.")
)
```

**YAML skeleton**
- verb: `investor-role.set`
- args:
  - `issuer` (EntityRef)
  - `holder` (EntityRef)
  - `share-class` (optional)
  - `role-type` enum (END_INVESTOR|NOMINEE|OMNIBUS|INTERMEDIARY_FOF|MASTER_POOL|INTRA_GROUP_POOL|TREASURY|OTHER)
  - `lookthrough-policy` enum (NONE|ON_DEMAND|AUTO_IF_DATA|ALWAYS)
  - `holder-affiliation` enum (INTRA_GROUP|EXTERNAL|MIXED|UNKNOWN)
  - `beneficial-owner-data-available` (bool)
  - `is-ubo-eligible` (bool)
  - `group-container` (EntityRef optional)
  - `notes` (string optional)

### 5) `economic.compute-exposure`
Purpose: Provide on-demand look-through results as a bounded set of exposure slices (table-friendly), **without materializing implied graph edges**.

**S-expression example**
```lisp
(economic.compute-exposure
  (root (entity-ref legal-entity (k lei "529900KRX8A6KQV2XK98")))
  (as-of "2026-01-01")
  (max-depth 6)
  (min-pct 0.0001)
  (max-rows 200)
)
```

Expected behavior:
- traverses direct edges only
- multiplies percentages along the path
- stops at pooled/intermediary nodes when lookthrough_policy=NONE or BO data unavailable
- returns (root, leaf, cumulative_pct, depth, path)



---

## Phase C — Economic look-through without exploding edges

### C1) Define a canonical “economic edge” view (direct only)
Create a VIEW (or SQL function) that yields direct, composable edges:

`kyc.v_economic_edges_direct` with columns:
- `from_entity_id` (holder)
- `to_entity_id` (issuer/investee)
- `pct_of_to` (numeric) — percentage ownership/exposure of *investee*
- `instrument_type` (UNITS | SHARES | LP_INTEREST | PARTNERSHIP_INTEREST | NOMINEE_POSITION)
- `share_class_id` (nullable) — ties to TA share-class when applicable
- `vehicle_type` (nullable) — SCSP/SICAV_RAIF/etc from `kyc.fund_vehicles` if available
- `basis` (ECONOMIC | UNITS | NAV)
- `source` (REGISTER | FUND_INVESTMENTS | MANUAL)
- `as_of_date`

Populate via:
- `kyc.ownership_snapshots` where `basis='ECONOMIC'` and `is_direct=true` and `superseded_at is null`
- UNION `ob-poc.fund_investments` (map `percentage_of_investee_aum` if present else NULL; keep `percentage_of_investor_nav` in metadata)

### C2) Implement a bounded look-through function (recursive, on-demand)

**Why this is on-demand (late calc):**
This function exists specifically to avoid cartesian joins/edge explosions. We never store derived edges like
`end_investor -> every underlying SPV`. We compute exposure slices when requested (UI drilldown / report), with
hard limits (depth, min_pct, max_rows) and stop conditions based on holder role profiles.
Create SQL function:

`kyc.fn_compute_economic_exposure(root_entity_id, as_of_date, max_depth, min_pct, max_rows)`

Behavior:
- recursively traverse `v_economic_edges_direct`
- multiply percentages along the path
- stop recursion when:
  - depth limit hit
  - pct falls below min_pct
  - holder role profile says `lookthrough_policy = NONE` (treat as leaf)
- output rows:
  - `root_entity_id`, `leaf_entity_id`, `cumulative_pct`, `depth`, `path_entities[]`, `path_edges[]`

Important: **do not materialize implied edges** (no investor→every-SPV edge table). This function is the engine.

### C3) Optional cache table (bounded)
If you need speed:
- `kyc.economic_exposure_cache` keyed by `(root_entity_id, as_of_date, params_hash)`
- store top-N results + explain payload
- TTL-based invalidation (recompute on demand)

### C4) Rust API + DSL integration
Add a custom op:
- domain: `economic`
- verb: `compute-exposure`
- calls `fn_compute_economic_exposure(...)`
- returns a record_set suitable for the UI inspector table

Add endpoints:
- `GET /economic/exposure?root_entity_id=...&as_of=...`
- `GET /economic/exposure/expand?issuer=...&holder=...` (optional UI drilldown)

---

## Phase D — Investor register visualization wiring (collapse pooled nodes)

You already have DTO structs in `rust/src/graph/investor_register.rs`. Wire them to real queries:

### D1) Implement query builder service
Create `rust/src/services/investor_register_service.rs` that:
- reads thresholds (issuer_control_config or defaults)
- loads holders from `kyc.ownership_snapshots` (basis controlled by query)
- applies role profiles:
  - holders with role_type in (INTERMEDIARY_FOF, MASTER_POOL, INTRA_GROUP_POOL, NOMINEE) can be collapsed by default
- returns:
  - `control_holders` (above threshold or has special rights)
  - `aggregate` (collapsed remainder; by_type/by_kyc_status/by_jurisdiction summaries)

### D2) Add API endpoints
- `GET /issuer/{entity_id}/investor-register` → returns `InvestorRegisterView`
- `GET /issuer/{entity_id}/investor-list` → paginated list (drilldown)

---


## Phase D.5 — Bulk load Allianz group + fund programme (LEI-first ingest)

### D5.1 xtask loader
Add an xtask command to ingest an Allianz group programme dataset (LEI list + vehicle type + optional umbrella/compartment):

- `cargo xtask load-allianz-group --input ./data/allianz_funds.csv`

Input columns (suggested):
- `lei`
- `entity_name`
- `vehicle_type` (SCSP/SICAV_RAIF/...)
- `umbrella_lei` (optional)
- `compartment_code` (optional)
- `holder_affiliation_default` (INTRA_GROUP|EXTERNAL|MIXED|UNKNOWN)
- `bo_data_available_default` (true/false)

Loader behavior:
- Upsert `entities` (by LEI)
- Upsert `kyc.fund_vehicles`
- Upsert `kyc.fund_compartments` when present
- Seed `kyc.investor_role_profiles` defaults for issuer vehicles when provided

Acceptance:
- You can load a full Allianz programme (FoF/master/umbrella) without manual hand-entry.
- Subsequent DSL operations (holdings, role profiles, exposure compute) work against the loaded dataset.


## Phase E — Tests & safety rails

### E1) SQL tests
- When a holding is inserted with `usage_type='TA'`, trigger must not write UBO edges.
- When role profile exists with `is_ubo_eligible=false`, trigger must not write UBO edges even if ≥25%.
- When `usage_type='UBO'` and eligible and ≥25%, edge is written/updated.

### E2) Look-through boundedness
- Ensure `fn_compute_economic_exposure` respects `max_depth`, `min_pct`, and `lookthrough_policy=NONE`.
- Ensure it never returns more than `max_rows`.
- Ensure look-through traversal treats `beneficial_owner_data_available=false` as a stop condition unless user explicitly overrides.

### E3) End-to-end DSL regression
- A single DSL fixture runs:
  - create entities + share classes + holdings
  - set role profile for pooled holder
  - verify UBO sync behavior
  - compute exposure view

---

## Implementation notes (keep it pragmatic)
- Keep role profiles **issuer-scoped** (issuer_entity_id) so that “same holder” can be treated differently across issuers.
- Don’t fight the immediate-mode UI: expose exposure results as a table (paged) and only expand look-through when user requests it.
- Prefer schema-driven enums; keep value casing consistent across SQL + YAML + Rust.

