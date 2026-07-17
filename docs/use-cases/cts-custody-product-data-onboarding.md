# Use Case: Onboard a Luxembourg SICAV Sub-Fund to the Custody Product

> **Audience:** Custody product, onboarding, and operations teams.
> This document describes the business process and its data contract. It is
> deliberately implementation-neutral: it specifies *what* must happen and
> *what* must be true, not how any particular platform executes it.

## 1. Use-case summary

| Field | Value |
|---|---|
| Use-case ID | `UC-CTS-CUSTODY-DATA-001` |
| Name | Compile and populate the data dependencies for onboarding a client trading structure to Custody |
| Primary actor | Product onboarding analyst |
| Supporting actors | Product catalogue steward, service owner, resource owner, client data provider, operations approver |
| Subject client trading structure | `Allianz Dynamic Commodities` (illustrative example) |
| Fund structure | A Luxembourg sub-fund (compartment) of the `Allianz Global Investors Fund` SICAV umbrella |
| Commercial product | `Custody` |
| Primary output | A versioned, populated, product-level data-dependency taxonomy persisted as a JSON record set |

## 2. Purpose

The purpose of this use case is to determine, obtain, validate, and preserve all data required to deliver the commercial Custody product to a specific Luxembourg SICAV sub-fund.

### The client trading structure

A client trading structure (CTS) is not a legal entity, and it is not the commercial client. It is the street-facing trading apparatus — the unit through which the client actually goes to market. One trading business is realised by a **constellation of legal entities bound together by roles**: the asset owner, the umbrella SICAV, the management company, the investment manager, and a general partner or trustee where relevant, together with the counterparty and settlement relationships that surround them. The commercial client sits above it; the individual legal entities sit inside it; the CTS is the middle grain where the trading business actually lives — a solar system within the client group's universe.

The construct exists because neither of the two natural grains works. "Client" is too coarse: a large asset manager operates hundreds of these structures, each with its own mandate, market footprint, and readiness state. "Legal entity" is too fine: no single entity *is* the trading business — the fund, its management company, and its investment manager are all necessary parts of one apparatus. The CTS is therefore the load-bearing grain for onboarding: products attach to it, accounts and SSIs fan out from it, the instrument matrix describes what *it* trades, and "good to transact" is decided about *it*.

### The instrument matrix

The instrument matrix is the operational statement of the CTS's trading universe: the complete, versioned declaration of what this structure trades and how that trading actually operates, so that custody, trade order management and routing, SWIFT setup, collateral arrangements, and fund accounting can all be provisioned from one frozen source. It is **self-contained** — it does not reference back to mandate or prospectus documents; it *is* the single source of truth for the trading universe as agreed for onboarding.

The construct exists to close a real gap: onboarding traditionally captures only the *custodied* world, leaving no path to capture the investment manager and front-office side of the business — who instructs, how orders and instructions flow, and from which BICs. Setting up the operational plant against that incomplete picture means SWIFT relationships, order routing, OTC collateral, and valuation configuration have no governed source. The matrix provides one.

Its dimensions:

1. **The full traded universe — not the custodied universe.** Every instrument type in scope, explicitly including types that never appear as safekept positions: OTC derivatives (swaps, CDS, FX forwards) exist as agreements and collateral flows rather than holdings. A custody-only lens misses them entirely.
2. **Instruction and routing topology.** Per instrument type: who instructs (the investment manager's front office / order management), from which BIC, over which channel and message types. This is the direct input for SWIFT setup (RMA relationships, message flows per instructing BIC) and trade routing configuration.
3. **Execution venues, settlement markets and depositories, settlement currencies, settlement types, and counterparties.** Execution venue (where trades are executed) and settlement market/CSD (where positions settle and are safekept) are distinct dimensions and must not be conflated: the execution venue belongs with the routing dimension above, while the settlement dimensions drive custody fan-out — securities account per settlement market, cash account per settlement currency, SSI per settlement market and currency. The settlement dimensions may be derived from execution venue, instrument type, and settlement rules.
4. **Legal agreement and collateral scope for OTC instruments.** For each OTC instrument type, the counterparty set and the ISDA master and CSA governing each counterparty, because the CSA drives collateral accounts, eligible-collateral schedules, and margin setup. Recording *which* agreement governs each counterparty is always required. The deeper margin mechanics (initial/variation margin terms, bilateral vs triparty, eligible collateral) are **conditionally required**: they become mandatory matrix content when collateral-management or margin services are part of the selected service composition for this CTS.
5. **Pricing and valuation preferences.** Per instrument class: pricing source hierarchy, fair-value and stale-price policy, and OTC valuation approach. Fund accounting consumes these to strike a NAV; they are part of the same "for this instrument class, here is how we operate" declaration as the rest of the matrix.

The word "matrix" reflects its dimensional shape: a sparse set of valid operational tuples across instrument class × execution venue × settlement market × currency × counterparty × instructing BIC/channel — only the combinations the CTS actually operates are populated, never the full cross-product. Every populated tuple drives provisioning fan-out somewhere — a custody account, an SSI, a SWIFT relationship, an ISDA/CSA and collateral account, a pricing-source configuration. That is why the matrix must be frozen and versioned before compilation: the entire downstream resource discovery hangs off it.

The matrix is a **living document**: it changes throughout the life of the CTS as it is serviced. Change is governed by versioning, not mutation — each change passes the matrix's own QA/validation gates and is **confirmed** as a new version, and every compilation binds to one specific confirmed version. Confirmation is what makes the matrix definitive: for any data it covers, the confirmed matrix is the single authoritative source, and its confirmation *is* the evidence — no separate evidence collection applies to matrix-sourced values. "Confirmed" is a business state, not a platform state: it means the version has passed those gates and is authorised as the operational statement of the trading universe. Any implementing platform must nominate which of its own lifecycle states satisfies this definition, and the confirmation itself must be auditable — who confirmed, when, and under what confirmation identity.

The dependency between the matrix and the service composition runs in **both directions**. Downward, the matrix drives resource fan-out (the examples above). Upward, service selection mandates matrix coverage: selecting OTC, variation-margin, or collateral-management services requires the confirmed matrix to cover the corresponding instrument types, governing agreements, and margin terms. Compilation blocks when a selected service demands a matrix dimension the confirmed matrix does not cover.

### Process overview

The process starts with the CTS, its instrument matrix, and the commercial product. It traverses the governed catalogue from product to services, from services to service-resource definitions, and from each resource definition to its attribute dictionary. It then consolidates the resource dictionaries into one CTS-specific product dependency taxonomy, populates the applicable attributes, and freezes the result as a reproducible JSON record set.

The output answers five questions:

1. Which business services collectively deliver Custody for this CTS?
2. Which resources are needed to deliver each selected service?
3. Which data attributes are required to configure or provision those resources?
4. What value, evidence, and provenance have been obtained for each applicable attribute?
5. Which gaps or conflicts still prevent the Custody onboarding from progressing?

## 3. Scope and boundaries

### In scope

- Selecting an existing, validated Luxembourg SICAV sub-fund CTS.
- Attaching and freezing a specific instrument-matrix version for dependency evaluation.
- Attaching the Custody commercial product to the CTS.
- Resolving the eligible, mandatory, default, and explicitly selected services that deliver Custody.
- Discovering service-resource definitions (SRDEFs), including per-market, per-currency, and per-counterparty fan-out.
- Resolving each resource's governed attribute dictionary.
- Consolidating and de-duplicating the attribute requirements without losing service and resource lineage.
- Auto-populating values from permitted sources.
- Soliciting unresolved values and supporting evidence.
- Validating values, conditions, constraints, evidence, and source freshness.
- Freezing and saving the populated dependency taxonomy as JSON.

### Out of scope

- Creating or validating the CTS's legal/KYC structure.
- Commercial negotiation, pricing, contracting, or fee billing.
- Authoring missing product, service, SRDEF, or attribute definitions during the onboarding transaction.
- Provisioning or activating the resources themselves. Provisioning is the next use case and consumes this use case's completed data set.
- Declaring the CTS "good to transact"; the broader readiness decision may depend on KYC, legal, credit, operational, and other controls.

## 4. Terminology

| Term | Meaning in this use case |
|---|---|
| CTS (client trading structure) | The street-facing trading apparatus being onboarded: a constellation of legal entities bound together by roles around one trading business. Here, one Luxembourg sub-fund with its surrounding structure. Products, accounts, and readiness decisions attach at this grain, not at the client-group level and not at any single legal entity. See "The client trading structure" in section 2. |
| SICAV | *Société d'investissement à capital variable* — an open-ended Luxembourg investment company, typically an umbrella structure containing multiple sub-funds (compartments). Each sub-fund has its own portfolio and is onboarded individually. |
| Commercial product | The client-facing offering, here `Custody`. |
| Service | A governed business capability or lifecycle service that contributes to the product, such as trade settlement, safekeeping, corporate actions, or income processing. |
| SRDEF | A service-resource definition describing a required resource type, its provisioning strategy, fan-out rules, dependencies, owner, and attribute requirements. |
| Resource slice | One CTS-specific occurrence of an SRDEF for a parameter set, such as one securities custody account for a market or one cash account for a currency. |
| Instrument matrix | The self-contained, versioned single source of truth for the CTS's trading universe and how that trading operates: the full *traded* instrument scope (including non-custodied OTC types), markets, settlement currencies and types, counterparties and their governing agreements, instruction and routing topology, and valuation preferences. It drives resource fan-out. See "The instrument matrix" in section 2. |
| Execution venue | The market (identified by MIC, e.g. XETR) on which trades are executed. Part of the instruction/routing dimension of the matrix; it does not by itself determine where assets settle or are safekept. |
| Settlement market / CSD | The market and central securities depository in which positions settle and are safekept (for example Germany via Clearstream Banking Frankfurt, the UK via CREST, the US via DTC). Custody account and SSI fan-out key off these settlement dimensions, which may be derived from execution venue, instrument type, and settlement rules. |
| SSI | Standing settlement instruction — the pre-agreed settlement routing details exchanged with counterparties per settlement market and currency. |
| Attribute definition | The canonical semantic definition of a data item, including type, meaning, constraints, and governance identity. |
| Attribute requirement | A resource-specific use of an attribute, including requiredness, condition, source policy, evidence policy, and any constraint override. |
| Product data-dependency taxonomy | The consolidated CTS-and-product-specific view of all applicable resource attribute requirements, with complete product → service → resource → attribute lineage. |
| Frozen data request | An immutable onboarding snapshot of discoveries, resource slices, requirements, values, and blockers at a recorded set of source versions. |

"Product-level" means that consumers can work with one consolidated dependency set for the Custody onboarding. It does not mean discarding the resource slice, market, currency, service, or source-definition lineage needed for provisioning and audit.

## 5. Trigger

An authorised onboarding actor requests that the Custody product be onboarded to the selected Luxembourg SICAV sub-fund, normally following an approved commercial handoff or contracted deal onboarding request.

## 6. Preconditions

### Business preconditions

1. The CTS exists, is the intended contracting/onboarding target, and has passed structural validation, so it is in a state that permits product attachment.
2. The CTS's Luxembourg SICAV context and relevant parties have been resolved sufficiently for product-data derivation.
3. The Custody product is active and approved for use.
4. The commercial or deal scope identifies Custody as an agreed product for this CTS.
5. An accountable onboarding owner and target completion date are recorded.

### Catalogue and data preconditions

1. A specific instrument-matrix version is attached to the CTS.
2. The matrix version is **confirmed** — it has passed the matrix's own QA/validation gates. Formal data solicitation, production compilation, and provisioning all require a confirmed version; a non-confirmed matrix supports only a clearly labelled preview. The version is immutable for the duration of compilation.
3. The matrix is materialized sufficiently to expose the applicable instruments (including non-custodied OTC types), execution venues, settlement markets and depositories, currencies, settlement types, counterparties and their governing agreements, instructing BICs and channels, valuation preferences, SSIs, and other discovery inputs.
4. Product-to-service mappings are effective-dated, governed, and resolvable for Custody.
5. Every selected service is published/active and has a stable version.
6. Every discoverable SRDEF is governed, complete, and free of unresolved attribute-definition gaps or conflicts.
7. All referenced attributes resolve to canonical attribute definitions.
8. Resource fan-out rules and resource dependencies are valid and acyclic.
9. Required resource owners—and any required live application/capability bindings—are resolvable.
10. A deal onboarding request exists when a frozen operational data request is to be compiled.

## 7. Inputs

| Input | Required content |
|---|---|
| CTS reference | CTS identity, name, jurisdiction, lifecycle state, SICAV/sub-fund context, and relevant entity roles |
| Instrument-matrix snapshot | Profile identity, version, status, confirmation record, content hash, traded instrument classes (custodied and OTC), execution venues, settlement markets and depositories, currencies, settlement types, counterparties and governing agreements (ISDA/CSA), instructing BICs and channels, and valuation/pricing preferences |
| Product snapshot | Product identity, code, version/effective date, status, and product configuration |
| Product-service catalogue | Service identities/codes, versions, mandatory/default flags, eligibility conditions, option definitions, and product overrides |
| Resource catalogue | SRDEF identities, versions/hashes, service triggers, owners, provisioning strategies, fan-out rules, dependencies, and capability bindings |
| Attribute catalogue | Canonical identities/codes, definitions, types, constraints, permitted sources, evidence rules, and derivations |
| Existing CTS data | CTS, entity, document, reference, derived, and previously supplied attribute values with timestamps and provenance |
| Onboarding context | Deal, contract, onboarding request, requested-by actor, target date, and approved product options |

## 8. Main success process

### Phase A — Establish and freeze the onboarding scope

1. Resolve the target CTS by its stable identifier and confirm it has passed structural validation.
2. Confirm that the CTS is a Luxembourg sub-fund within a SICAV umbrella structure.
3. Attach an instrument matrix if none exists. Select the permitted matrix version, validate its status, materialize its operational projections, and record its content hash.
4. Resolve the active Custody product and freeze its catalogue identity and effective version.
5. Record the deal/contract/onboarding-request context that authorises the product onboarding.

### Phase B — Attach Custody and resolve its service composition

6. Attach Custody to the CTS.
7. Read the governed product-to-service mappings for Custody.
8. Evaluate each mapping against the CTS, instrument matrix, product options, effective date, and any eligibility predicate.
9. Resolve a *proposed* service composition:
   - every mandatory applicable service;
   - every applicable default service unless explicitly opted out under product policy; and
   - every optional service explicitly agreed in the onboarding scope.
10. Present the proposed composition to the onboarding requestor; on the requestor's confirmation, create or reconcile one active service intent per confirmed CTS/product/service/options tuple.
11. Preserve an explanation for each inclusion, exclusion, default, override, and eligibility decision.

### Phase C — Discover the service resources

12. For each selected service intent, locate every SRDEF triggered by that service and its selected options.
13. Evaluate resource eligibility, service option constraints, and dependencies.
14. Apply fan-out rules using the frozen instrument matrix. Examples include:
   - securities custody account per applicable settlement market;
   - cash custody account per applicable settlement currency;
   - SSI instruction set per applicable settlement market and currency;
   - SWIFT relationship per instructing BIC and channel;
   - ISDA/CSA reference and, where collateral or margin services are selected, collateral account setup per OTC counterparty; and
   - pricing-source configuration per instrument class where fund accounting services are selected.
15. De-duplicate discoveries that represent the same SRDEF and parameter set, while retaining every triggering service intent.
16. Topologically order dependent resource slices and reject dependency cycles.
17. Record a discovery explanation containing the triggering services, rule, matrix inputs, parameters, SRDEF snapshot identity/hash, and decision timestamp.

### Phase D — Resolve and consolidate resource dictionaries

18. Load the governed attribute requirements for every discovered resource slice.
19. Resolve each requirement to its canonical attribute definition.
20. Build one product-level dependency taxonomy keyed by canonical attribute identity and scoped to the CTS, product, and frozen matrix version.
21. Preserve a **requirement occurrence** on every consolidated attribute for each service, SRDEF, and resource slice that requires it, carrying the slice key and parameters, the effective constraints and policies for that occurrence, and — once populated — the occurrence's own value and validation result. Consolidation is keyed by canonical attribute identity, but values bind at occurrence grain: one canonical attribute may legitimately carry different values across slices (a settlement currency of EUR, GBP, and USD across three cash accounts is three occurrences, not one value).
22. Merge requirements according to the following rules:
   - If any applicable source requires an attribute unconditionally, its effective product-level strength is `required`.
   - Otherwise, conditional requirements remain conditional and retain their expressions; they are not flattened into unconditional requirements.
   - An attribute is `optional` only when every applicable source treats it as optional.
   - Compatible constraints are combined to the strictest satisfiable set.
   - Incompatible types, enumerations, patterns, ranges, defaults, or evidence rules create an explicit blocking conflict; the compiler must not silently choose one.
   - Source and evidence policies retain their per-resource lineage even when a product-level preferred acquisition order is calculated.
23. Evaluate applicability conditions against already-known values and classify each requirement as unconditional, pending, satisfied, or not applicable.
24. Produce a gap list for unresolved attribute definitions, unresolved conditions, conflicting requirements, and missing applicable values.

### Phase E — Obtain and validate attribute values

25. Attempt automatic population in the order permitted by each requirement's source policy. Supported sources may include governed derivation, existing CTS data, related entity data, documents, reference data, defaults, and manual/client supply.
26. Reuse an existing value only when its subject, semantic identity, effective date, freshness, evidence, and permitted-use policy satisfy the current requirement.
27. Where multiple sources disagree, retain all candidates and route the conflict for resolution; do not apply an unrecorded precedence choice.
28. Group remaining requests into coherent solicitations by data owner or client contact. Deduplicate a request only when the canonical attribute, the subject, and the parameter scope are all identical; occurrences with different parameters (a different settlement market or currency) are distinct requests even for the same attribute.
29. For every supplied value, record the source, supplier, observation/effective time, evidence references, and any transformation or derivation used.
30. Validate the value against its canonical type, merged constraints, applicability condition, evidence policy, and freshness policy.
31. Re-run dependent conditions and derivations whenever an input value changes.
32. Continue until every applicable required attribute is present and valid, or until an authorised actor records a blocking exception.

### Phase F — Freeze and save the result

33. Compile the onboarding data request from the active discoveries.
34. Freeze one discovery snapshot per unique SRDEF/parameter set.
35. Freeze one owner-addressable resource slice per discovery.
36. Freeze the resolved attribute requirements, populated values, evidence status, validation results, and blockers for each slice.
37. Create the consolidated JSON projection described in section 12.
38. Compute a deterministic content hash over a canonical serialization of the record set (stable key ordering and value formats, with the `content_hash` field itself excluded from the hashed payload) and persist the JSON with its schema version and all source snapshot identifiers.
39. Mark the record set `complete` only when all applicable required values and evidence are valid and all blocking conflicts are resolved. Otherwise save it as `blocked` with machine-readable reasons. Completeness is a statement about the data only: it makes the record set eligible for provisioning; it is never a claim that provisioning has occurred.
40. Make the completed record set available to the separate resource-provisioning and onboarding-readiness processes.

## 9. Business rules and invariants

1. The compilation grain is `CTS × product × effective onboarding request × frozen instrument-matrix version`.
2. Catalogue definitions are design-time governed objects; CTS discoveries, values, and resource slices are operational onboarding instances.
3. Stable identifiers and version/hash references—not display names—control identity and reproducibility.
4. A service may trigger several resources, and a resource may support several services. The resulting graph is many-to-many.
5. Resource fan-out is part of the data dependency. A requirement scoped to one settlement market or currency cannot be satisfied by a value recorded for a different one; values bind at requirement-occurrence grain, not at canonical-attribute grain.
6. Consolidation must remove duplicate solicitation without removing resource-specific applicability, constraints, or lineage.
7. "Not applicable" is an evaluated state with a recorded reason; it is not equivalent to a missing value.
8. Defaulted and derived values retain their origin and are never represented as client-supplied values.
9. A value is not complete merely because it is non-null; type, constraints, evidence, condition, and freshness must also pass.
10. Definition changes do not mutate an already-frozen request. A material catalogue or matrix change requires impact analysis and a new compilation/version.
11. Repeating compilation for the same onboarding request is idempotent and returns the existing frozen request unless an explicit recompile/version operation is authorised.
12. No resource may be dispatched for provisioning while its slice has a missing required value, failed validation, missing required evidence, unresolved owner, or missing required live capability binding.
13. Service selection mandates matrix coverage. Selecting OTC, variation-margin, or collateral-management services requires the confirmed matrix to cover the corresponding instrument types, ISDA/CSA agreements, and margin terms; compilation blocks when a selected service demands a matrix dimension the confirmed matrix does not cover.
14. The confirmed instrument matrix is the authoritative source for every data item it covers. Precedence questions between sources can only arise for data outside the matrix's scope; those conflicts are routed for authorised resolution, never resolved by silent precedence.

## 10. Exceptions and alternate flows

| Condition | Required handling |
|---|---|
| CTS is absent, ambiguous, or not yet validated | Stop before product attachment and return the CTS gate failure. |
| SICAV context cannot be established | Stop and request structural correction or explicit scenario override. |
| Instrument matrix is absent | Attach/bootstrap a matrix, populate it, and resume only after it reaches confirmed state. |
| Matrix is not confirmed or not materialized | Permit a clearly labelled preview only; block formal solicitation, production compilation, and provisioning until a confirmed version exists. |
| Selected service requires matrix coverage the confirmed matrix lacks (e.g. collateral management selected but no CSA/margin terms recorded) | Block compilation and route back to matrix authoring for a new confirmed version; do not infer the missing dimensions. |
| Custody product is inactive or not contracted | Stop; do not infer commercial entitlement from catalogue availability. |
| Product has no service mappings | Block as a catalogue defect and route to the product catalogue steward. |
| Selected service is ungoverned, unpublished, deprecated, or retired | Exclude only where policy permits; otherwise block and route to the service steward. Preserve the reason. |
| Conditional service eligibility cannot be evaluated | Mark the service decision pending and solicit the missing decision inputs. |
| Selected service has no SRDEF | Block as an incomplete service-delivery definition. |
| SRDEF has missing attribute definitions or conflicts | Block compilation for production use and route to catalogue/service-resource stewardship. Do not author definitions inside this onboarding request. |
| Resource dependency cycle exists | Reject the affected discoveries and report the full cycle. |
| Fan-out inputs are missing | Preserve a pending discovery with the missing matrix dimension; do not create a falsely global resource slice. |
| Duplicate resources are discovered | Coalesce only identical SRDEF/parameter sets and union their triggering-service lineage. |
| Attribute requirements conflict | Record each source requirement and a blocking conflict; require steward resolution or a governed product-specific override. |
| Required value is unavailable | Save the request as `blocked`, identify the responsible source/owner, and retain an actionable gap. |
| Candidate sources disagree | Preserve candidates and provenance, then request an authorised resolution. |
| Value fails constraint, evidence, or freshness checks | Retain the submitted value as evidence of the attempt, mark it invalid, and solicit a corrected value. |
| Resource owner or required live capability binding is unresolved | The data slice may be compiled but cannot become dispatch-ready. |
| Catalogue or matrix changes during collection | Continue against the frozen snapshot, run impact analysis, and create a new request version if the change is material. |
| Existing request is compiled again | Return the existing request and indicate that it already existed; do not duplicate slices. |
| Onboarding is cancelled | Cancel open slices and downstream requests while preserving the frozen record and audit trail. |

## 11. Postconditions

### Success postconditions

1. The CTS, Custody product, and frozen instrument-matrix snapshot are unambiguously identified.
2. The selected service composition is recorded with inclusion/exclusion rationale and versions.
3. Every applicable service has complete resource discovery, including required fan-out and dependencies.
4. The consolidated product data-dependency taxonomy contains every applicable resource attribute and complete lineage back to its services and SRDEFs.
5. Every applicable required attribute has a valid value, adequate evidence where required, and recorded provenance.
6. There are no unresolved definition, requirement, owner, capability-binding, validation, or evidence blockers.
7. A deterministic, schema-versioned JSON record set is persisted with a content hash and frozen source references.
8. The completed record set is available for resource provisioning and broader readiness evaluation.

### Minimum postconditions on blocked completion

1. The partial taxonomy and all successfully obtained values are preserved.
2. Every blocker is machine-readable, attributable to a service/resource/attribute lineage, and assigned to an accountable owner where possible.
3. No non-ready resource slice is dispatched.
4. Re-entry can resume without re-soliciting still-valid values.

## 12. JSON record-set contract

The persisted operational model may remain normalized across request, discovery, slice, and attribute records. The JSON below is the portable consolidated projection. All identifiers and hashes are illustrative placeholders.

Two contract rules to note. First, consolidation is keyed by canonical attribute, but values and validation bind at **requirement-occurrence** grain — one attribute may carry different values across resource slices. Second, `taxonomy_status` describes **data completeness only**; provisioning readiness and completion belong to the consuming provisioning process, never to this record set.

```json
{
  "schema_version": "1.0",
  "record_type": "cts_product_data_dependency_taxonomy",
  "record_id": "<uuid>",
  "taxonomy_status": "complete",
  "compiled_at": "2026-07-17T00:00:00Z",
  "content_hash": "sha256:<digest>",
  "scope": {
    "onboarding_request_id": "<uuid>",
    "cts": {
      "id": "<uuid>",
      "name": "Allianz Dynamic Commodities",
      "jurisdiction": "LU",
      "structure_context": "Allianz Global Investors Fund / SICAV"
    },
    "product": {
      "id": "<uuid>",
      "code": "CUSTODY",
      "name": "Custody",
      "snapshot_version": "<version-or-effective-date>"
    },
    "instrument_matrix": {
      "profile_id": "<uuid>",
      "version": 1,
      "status": "confirmed",
      "confirmed_by": "<actor-or-role>",
      "confirmed_at": "<timestamp>",
      "confirmation_id": "<uuid>",
      "content_hash": "sha256:<digest>"
    }
  },
  "services": [
    {
      "service_id": "<uuid>",
      "service_code": "SETTLEMENT",
      "name": "Trade Settlement",
      "selection": "mandatory_default",
      "selection_reason": "Governed Custody product-service mapping",
      "service_version": "<published-version>"
    }
  ],
  "resource_slices": [
    {
      "slice_key": "SRDEF::CUSTODY::Account::custody_securities|settlement_market=DE",
      "srdef_id": "SRDEF::CUSTODY::Account::custody_securities",
      "srdef_snapshot_hash": "<digest>",
      "resource_type": "Account",
      "resource_name": "Securities Custody Account",
      "owner": "CUSTODY",
      "provisioning_strategy": "request",
      "parameters": {
        "settlement_market": "DE",
        "csd_or_depository": "Clearstream Banking Frankfurt",
        "derived_from_execution_venues": ["XETR"]
      },
      "triggered_by_services": [
        "SETTLEMENT"
      ],
      "depends_on": []
    }
  ],
  "attributes": [
    {
      "attribute_id": "<canonical-uuid>",
      "attribute_code": "settlement_currency",
      "definition_version": "<version>",
      "data_type": "string",
      "effective_requirement": "required",
      "conditions": [],
      "constraints": {
        "pattern": "^[A-Z]{3}$"
      },
      "source_policy": [
        "derived",
        "cts",
        "manual"
      ],
      "evidence_policy": {},
      "requirement_occurrences": [
        {
          "service_code": "SETTLEMENT",
          "srdef_id": "SRDEF::CUSTODY::Account::custody_cash",
          "slice_key": "SRDEF::CUSTODY::Account::custody_cash|currency=EUR",
          "parameters": {
            "settlement_currency": "EUR"
          },
          "effective_constraints": {
            "pattern": "^[A-Z]{3}$"
          },
          "value": {
            "status": "present",
            "value": "EUR",
            "source": "instrument_matrix_derivation",
            "observed_at": "2026-07-17T00:00:00Z",
            "evidence_refs": []
          },
          "validation": {
            "constraint_status": "valid",
            "evidence_status": "not_required",
            "blocking_reasons": []
          }
        },
        {
          "service_code": "SETTLEMENT",
          "srdef_id": "SRDEF::CUSTODY::Account::custody_cash",
          "slice_key": "SRDEF::CUSTODY::Account::custody_cash|currency=GBP",
          "parameters": {
            "settlement_currency": "GBP"
          },
          "effective_constraints": {
            "pattern": "^[A-Z]{3}$"
          },
          "value": {
            "status": "present",
            "value": "GBP",
            "source": "instrument_matrix_derivation",
            "observed_at": "2026-07-17T00:00:00Z",
            "evidence_refs": []
          },
          "validation": {
            "constraint_status": "valid",
            "evidence_status": "not_required",
            "blocking_reasons": []
          }
        }
      ]
    }
  ],
  "blockers": [],
  "source_snapshots": {
    "product_service_catalogue": "<snapshot-id-or-hash>",
    "srdef_catalogue": "<snapshot-id-or-hash>",
    "attribute_registry": "<snapshot-id-or-hash>"
  }
}
```

## 13. Acceptance scenarios

### A. Successful compilation

**Given** the sub-fund CTS is validated, its confirmed instrument matrix is frozen and materialized, Custody is contracted and active, and all selected services and SRDEFs are governed and complete  
**When** Custody is attached, service/resource discovery runs, all applicable required values are obtained and validated, and the data request is compiled  
**Then** one complete, hashed JSON dependency taxonomy is saved with full product-to-attribute lineage and no blockers.

### B. Market and currency fan-out

**Given** the matrix includes equities executed on XETR settling in Germany (Clearstream Banking Frankfurt) in EUR, on XLON settling in CREST in GBP and EUR, and on XNYS settling at DTC in USD  
**When** Custody resource discovery runs  
**Then** settlement-market-scoped and currency-scoped resource slices are created only for the applicable distinct parameter sets — keyed off the settlement dimensions, not the execution venues — and their shared attributes are consolidated at requirement-occurrence grain without losing slice lineage.

### C. Conflicting attribute requirements

**Given** two selected resources reference the same canonical attribute with incompatible constraints  
**When** their dictionaries are consolidated  
**Then** the compiler records both source requirements, marks the attribute conflict as blocking, and does not silently select a constraint.

### D. Missing client-supplied value

**Given** an applicable required attribute cannot be populated from an allowed internal source  
**When** the population phase finishes  
**Then** one non-duplicated solicitation is created for the responsible party, the affected slices remain blocked, and the partial taxonomy is saved.

### E. Idempotent recompilation

**Given** a frozen data request already exists for the onboarding request  
**When** compilation is invoked again without an authorised new version  
**Then** the existing request is returned, no duplicate discoveries/slices/attributes are created, and the response indicates that it already existed.

### F. Snapshot change during collection

**Given** the instrument matrix or governed catalogue changes after compilation  
**When** collection continues  
**Then** the original request remains tied to its frozen sources, an impact assessment identifies affected services/resources/attributes, and material changes require a new request version.

## 14. Policy decisions (resolved)

The following policy questions were raised during drafting and have been decided:

1. **Permitted instrument-matrix states.** The matrix must be in **confirmed** state. Formal data solicitation, production compilation, and provisioning all require a confirmed version; anything less supports only a clearly labelled preview. "Confirmed" is a business state — the version has passed the matrix's QA/validation gates and is authorised as the operational statement of the trading universe; an implementing platform must nominate which of its own lifecycle states satisfies this definition.
2. **Service option selection.** Product and service options are explicitly approved by the **onboarding requestor** — they are not inferred from the contracted scope. Catalogue resolution therefore produces a *proposed* composition, which the requestor confirms before any service intent becomes active. Service selection in turn switches matrix obligations: selecting OTC, variation-margin, or collateral-management services mandates that the confirmed matrix covers the corresponding instrument types, ISDA/CSA agreements, and margin terms.
3. **Source precedence.** There is no general precedence policy, because none is needed: the standalone confirmed matrix is the **definitive** source for every data item it covers — its confirmation is what makes it authoritative. Conflicts can only arise for data outside the matrix's scope, and those are routed for authorised resolution.
4. **Freshness.** No per-attribute freshness periods apply to matrix-sourced values. The matrix is a **living document** that changes throughout the life of the CTS under servicing; its currency is managed by versioned confirmation, not expiry clocks. Each compilation binds to one confirmed matrix version, and a subsequent matrix change triggers impact analysis and, where material, a new request version. Values obtained outside the matrix retain the freshness policies declared by their governed attribute requirements.
5. **Evidence.** No separate evidence classes are mandated for anything the matrix covers: the matrix's own QA/validation states, culminating in confirmation, are the evidence, and the confirmation itself is auditable (who confirmed, when, under what confirmation identity). Values obtained outside the matrix retain the evidence policies declared by their governed attribute and resource definitions.
6. **Override approval.** The onboarding requestor approves product and service options for the CTS. Requirement-definition conflicts and constraint overrides are outside the requestor's authority: they are resolved by the product/catalogue steward or the owning resource-definition owner.
7. **Material change.** A catalogue or matrix change is material when it would change **any resource configuration** — i.e., it alters the discovery, fan-out, or attribute outcome of a compilation. Material changes require a new frozen request version.
8. **Storage form of the JSON projection.** Implementation detail, deliberately out of scope. The only requirement this use case imposes is that the result is persisted as a JSON record set conforming to section 12.
