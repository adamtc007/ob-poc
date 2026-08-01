# Use Case: Onboard a Luxembourg SICAV Sub-Fund to Custody

> **Audience:** Custody product, onboarding, and operations teams.
> This document describes the business outcome, process, and data output. It
> deliberately avoids prescribing a particular system implementation.

## 1. Summary

| Field | Value |
|---|---|
| Use-case ID | `UC-CTS-CUSTODY-DATA-001` |
| Objective | Identify and collect the data needed to deliver Custody to a client trading structure |
| Example | `Allianz Dynamic Commodities`, a sub-fund of the `Allianz Global Investors Fund` SICAV |
| Primary actor | Product onboarding analyst |
| Supporting actors | Onboarding requestor, product steward, service owner, resource owner, client data provider, operations approver |
| Main output | A versioned JSON record of the services, resources, data requirements, values, and unresolved gaps for the Custody onboarding |

The use case follows this business chain:

```text
Client trading structure + confirmed instrument matrix + Custody product
                                  ↓
                         Selected services
                                  ↓
                         Required resources
                                  ↓
                    Resource data requirements
                                  ↓
                   Populated onboarding data set
```

## 2. Purpose and business context

The purpose is to determine, obtain, validate, and preserve all data needed to
deliver Custody to a specific Luxembourg SICAV sub-fund.

### The client trading structure problem

BNY's Client Master is entity-centric. It records clients, legal entities, and
relationships between them, but it does not hold a first-class representation
of the **trading business created when several entities act together in defined
roles**.

A Luxembourg SICAV illustrates the gap:

- The commercial client may operate hundreds of funds and mandates.
- The umbrella SICAV may be the legal entity, while each sub-fund has its own
  assets, mandate, markets, accounts, and operating arrangements.
- A management company and investment manager act for the sub-fund in specific
  roles, and the same entities may act for many other sub-funds.
- Custody is delivered to that combined operating arrangement, not to the
  commercial client as a whole and not to any one participating entity alone.

Attaching onboarding data to the commercial client is therefore too broad.
Attaching it to one legal entity is too narrow and can be misleading. Neither
approach captures the particular combination of fund, actors, roles, trading
scope, instructions, accounts, counterparties, and readiness that makes up one
trading business.

The missing concept creates practical problems. There is no single subject to
which BNY can reliably attach the product, its service composition, the
instrument matrix, the required accounts and settlement instructions, or the
decision that the structure is operationally ready. The gap is not missing
entity data; it is the absence of the structure that gives those entities an
operational meaning together.

### The CTS construct

A **client trading structure (CTS)** is the purpose-built construct that fills
this gap. It represents one trading business and gives it a stable identity.
It is not a new legal entity and does not duplicate the legal-entity master.
Instead, it references the participating entities and records the role each
one performs for this particular structure.

Roles are contextual to the CTS. For example, an entity may be the investment
manager for one CTS, hold a different role in another, and have no role in a
third. A commercial client can therefore have many CTSs, and the same legal
entity can participate in several CTSs.

At a high level, the construct is:

```text
Commercial client / client group
└── Client trading structure — one fund, sub-fund, or mandate
    ├── Participating entities, bound by CTS-specific roles
    │   ├── Asset owner / sub-fund
    │   ├── Umbrella SICAV
    │   ├── Management company
    │   ├── Investment manager
    │   └── Other required actors
    ├── Instrument matrix — what and how the CTS trades
    ├── Product subscriptions
    │   └── Services → required resources → resource data
    └── CTS-level onboarding and operational-readiness state
```

The CTS is consequently the subject at which:

- Commercial products and service options are attached.
- The trading universe and operating model are described.
- Accounts, settlement instructions, connectivity, and other resources are
  discovered and provisioned.
- Product-level data dependencies are consolidated.
- Onboarding progress and operational readiness are assessed.

In the current platform, this construct may be persisted under the label
`CBU`. That storage label must not be taken to mean a generic business unit:
the business concept described here is specifically the CTS.

### Instrument matrix

The **instrument matrix** is the confirmed, versioned description of what the
CTS trades and how those trades operate. It covers, where applicable:

- Traded instrument types, including OTC instruments that are not held as
  safekept positions.
- Execution venues, settlement locations, depositories, currencies, and
  settlement methods.
- Who sends instructions, from which BIC, and through which channel.
- OTC counterparties and their governing ISDA and CSA agreements.
- Pricing and valuation preferences needed by selected services.

Execution venue and settlement location are different concepts. The execution
venue describes where a trade is executed; the settlement location or
depository determines where assets settle and are held. Custody resources are
usually driven by the settlement dimensions.

The matrix and service selection depend on each other:

- The matrix drives resource discovery and fan-out, such as accounts by
  settlement market and cash accounts by currency.
- Selected services can require additional matrix content. For example,
  collateral management requires the relevant OTC agreements and margin terms.

Each compilation uses one confirmed matrix version. For data held in that
matrix, its confirmation establishes the authoritative value and supporting
evidence.

## 3. Scope

### In scope

- Confirm the target CTS, Custody product, onboarding request, and instrument
  matrix version.
- Determine which services will deliver Custody for the CTS.
- Discover the resources needed for those services.
- Combine the resource data requirements into one product-level view while
  preserving their service and resource context.
- Populate known values and request missing values.
- Validate the resulting data and record any blockers.
- Save a frozen, versioned JSON record set.

### Out of scope

- Creating or validating the legal/KYC structure.
- Commercial negotiation, contracting, pricing, or billing.
- Defining missing catalogue services, resources, or attributes within the
  onboarding request.
- Provisioning or activating the resources.
- Declaring the CTS good to transact; that decision may depend on other legal,
  KYC, credit, and operational controls.

## 4. Key terms

| Term | Meaning |
|---|---|
| CTS | A purpose-built representation of one trading business, formed by legal entities acting together in CTS-specific roles. It is the subject of product onboarding and operational readiness. |
| Commercial product | The client-facing offering, here `Custody`. |
| Service | A business capability that contributes to Custody, such as settlement, safekeeping, corporate actions, or income collection. |
| Resource definition | A governed description of a resource needed to deliver a service, including when it is required and what data it needs. A resource definition may also be called an SRDEF. |
| Resource slice | One CTS-specific occurrence of a resource definition, such as a securities account for one settlement market or a cash account for one currency. |
| Attribute | A data item required by a resource, such as settlement currency, account structure, or agent BIC. |
| Product data set | The consolidated view of all applicable resource attributes, values, lineage, and blockers for this CTS and product. |

## 5. Preconditions and inputs

The process can start when:

1. The CTS exists, has passed structural validation, and is confirmed as the
   onboarding target.
2. Custody is an active, agreed product for the CTS.
3. A specific instrument matrix is attached and confirmed. An unconfirmed
   matrix may be used only for a clearly labelled preview.
4. The matrix contains the dimensions needed by the proposed services and can
   be used for resource discovery.
5. Product-to-service mappings, resource definitions, and attribute
   definitions are governed and available.
6. The onboarding request identifies the relevant deal or commercial handoff,
   product options, requestor, and target date.

The principal inputs are:

- CTS identity, SICAV/sub-fund context, and acting parties.
- Confirmed instrument matrix version and content hash.
- Custody product and service catalogue versions.
- Resource and attribute definition versions.
- Existing CTS, entity, reference, document, and derived data.
- Onboarding request and approved product/service options.

## 6. Process

### A. Confirm the onboarding scope

1. Resolve the CTS and confirm that it is the intended Luxembourg SICAV
   sub-fund.
2. Select and freeze the confirmed instrument matrix version.
3. Confirm the active Custody product and the onboarding request that
   authorises the work.

### B. Agree the service composition

4. Use the product catalogue to produce a proposed list of mandatory, default,
   and optional Custody services.
5. Evaluate any eligibility conditions using the CTS, matrix, and selected
   product options.
6. Ask the onboarding requestor to confirm the proposed service composition.
7. Record each selected service and the reason for its inclusion or exclusion.

### C. Discover the required resources

8. For each selected service, identify the applicable resource definitions.
9. Use the matrix to create the required resource slices. Examples include:
   - Securities accounts by settlement market or depository.
   - Cash accounts by settlement currency.
   - Settlement instructions by settlement location and currency.
   - Messaging relationships by instructing BIC and channel.
   - Collateral arrangements by OTC counterparty where relevant.
10. Combine identical discoveries while preserving every service that caused
    the resource to be required.
11. Record why each resource was selected and which matrix data drove it.

### D. Build the product data set

12. Load the attributes required by each resource slice.
13. Consolidate repeated attributes into a product-level view, but retain each
    resource-specific occurrence. The same attribute may have different values
    for different markets, currencies, or resources.
14. Apply required, optional, and conditional rules.
15. Record incompatible definitions or constraints as blockers rather than
    choosing one silently.

### E. Obtain and validate values

16. Populate values from approved sources, beginning with the confirmed matrix
    and existing governed data.
17. Request only the remaining applicable values from the appropriate client
    contact or data owner.
18. Avoid duplicate requests only where the attribute, subject, and resource
    scope are the same.
19. Record the value, source, date, and supporting evidence where required.
20. Validate each value against its type, conditions, constraints, and evidence
    rules.
21. Repeat until all required values are valid or the onboarding is blocked.

### F. Freeze the result

22. Save the selected services, resource discoveries, attribute requirements,
    values, validation results, and blockers as one versioned JSON record set.
23. Bind the record to the frozen matrix and catalogue versions and calculate a
    deterministic content hash.
24. Mark the data set `complete` when all applicable required data is valid;
    otherwise mark it `blocked` and include the reasons.
25. Make a complete data set available to the separate resource-provisioning
    process.

`Complete` describes the data set only. It does not mean that the resources
have been provisioned or that the CTS is ready to trade.

## 7. Business rules

1. The record-set scope is one CTS, one product, one onboarding request, and one
   confirmed instrument matrix version.
2. Stable identifiers and version references control identity; names are for
   display only.
3. A service may require several resources, and one resource may support
   several services.
4. Values remain scoped to their resource occurrence. A value for one market
   or currency cannot satisfy a different market or currency.
5. `Not applicable` is an evaluated result and is different from `missing`.
6. Defaults and derived values retain their source and are not presented as
   client-supplied values.
7. A value is complete only after its applicable validation rules pass.
8. A frozen request is not changed when its source definitions change. A
   material change creates a new version.
9. Repeating the same compilation is idempotent and must not create duplicate
   resource or attribute records.
10. No resource is released for provisioning while a required value, owner, or
    required system binding remains unresolved.

## 8. Exceptions

| Condition | Outcome |
|---|---|
| CTS is missing, ambiguous, or not validated | Stop and correct the onboarding target. |
| Custody is not active or agreed | Stop; catalogue availability does not establish commercial entitlement. |
| Matrix is missing or unconfirmed | Allow preview only; block formal collection and provisioning. |
| Selected service needs matrix data that is absent | Return to matrix authoring and confirm a new version. |
| Product has no valid service composition | Route to the product steward. |
| A selected service has no complete resource definition | Route to the service or resource owner. |
| Resource fan-out inputs are missing | Keep the discovery pending and identify the missing matrix data. |
| Attribute definitions or constraints conflict | Record a blocker and route it to the responsible steward. |
| A required value is missing or invalid | Preserve the partial data set and identify the responsible data owner. |
| Resource owner or required system binding is unresolved | Compile the data slice, but do not release it for provisioning. |
| Matrix or catalogue changes after compilation | Keep the original snapshot; create a new version if the change affects resources or data requirements. |
| Onboarding is cancelled | Cancel open work while retaining the record and audit history. |

## 9. Outcomes

### Successful outcome

- The CTS, Custody product, selected services, and source versions are clearly
  identified.
- All applicable resources and their data requirements have been discovered.
- Every required value is present and valid, with its source and resource
  context preserved.
- The JSON data set is saved with no unresolved blockers and can be consumed by
  provisioning.

### Blocked outcome

- The partial data set and all valid values are preserved.
- Each blocker identifies the affected service, resource, or attribute and the
  responsible owner where known.
- Nothing incomplete is released for provisioning.
- Work can resume without requesting still-valid data again.

## 10. JSON output

The JSON is a portable business record. A platform may store the underlying
data in normalized tables, provided it can reproduce this record consistently.

Attributes are consolidated by canonical identity, while values remain attached
to their individual resource occurrences.

```json
{
  "schema_version": "1.0",
  "record_type": "cts_product_data_dependency",
  "record_id": "<uuid>",
  "taxonomy_status": "complete",
  "compiled_at": "<timestamp>",
  "content_hash": "sha256:<digest>",
  "scope": {
    "onboarding_request_id": "<uuid>",
    "cts": {
      "id": "<uuid>",
      "name": "Allianz Dynamic Commodities",
      "jurisdiction": "LU"
    },
    "product": {
      "id": "<uuid>",
      "code": "CUSTODY"
    },
    "instrument_matrix": {
      "id": "<uuid>",
      "version": 1,
      "status": "confirmed",
      "confirmed_by": "<actor-or-role>",
      "confirmed_at": "<timestamp>",
      "content_hash": "sha256:<digest>"
    }
  },
  "services": [
    {
      "code": "SETTLEMENT",
      "version": "<version>",
      "selection_reason": "mandatory Custody service"
    }
  ],
  "resource_slices": [
    {
      "slice_key": "custody_cash|currency=EUR",
      "resource_definition_id": "SRDEF::CUSTODY::Account::custody_cash",
      "owner": "CUSTODY",
      "parameters": {
        "currency": "EUR"
      },
      "triggered_by_services": ["SETTLEMENT"]
    }
  ],
  "attributes": [
    {
      "attribute_id": "<uuid>",
      "code": "settlement_currency",
      "data_type": "string",
      "occurrences": [
        {
          "slice_key": "custody_cash|currency=EUR",
          "requirement": "required",
          "value": {
            "status": "present",
            "value": "EUR",
            "source": "instrument_matrix"
          },
          "validation": "valid"
        }
      ]
    }
  ],
  "blockers": [],
  "source_snapshots": {
    "product_service_catalogue": "<snapshot-id-or-hash>",
    "resource_catalogue": "<snapshot-id-or-hash>",
    "attribute_catalogue": "<snapshot-id-or-hash>"
  }
}
```

## 11. Acceptance criteria

1. The onboarding requestor confirms the service composition before resource
   discovery is finalized.
2. Resource fan-out uses settlement location, currency, counterparty, and other
   applicable matrix dimensions rather than relying on execution venue alone.
3. The same canonical attribute can hold different values for different
   resource occurrences without losing lineage.
4. Missing values and definition conflicts produce explicit blockers and do
   not result in guessed values.
5. Repeating compilation against unchanged inputs returns the same logical
   result without duplicate records.
6. A source change does not alter the frozen record; a material change produces
   a new version.
7. A complete JSON record can be traced from product to service, resource,
   attribute, and value.

## 12. Agreed policy decisions

1. **Matrix confirmation:** Formal data collection requires a confirmed matrix.
   `Confirmed` is the business state meaning that the version has passed the
   required review and is authorised for onboarding. The confirming actor and
   time must be recorded.
2. **Service selection:** The catalogue proposes mandatory, default, and
   optional services; the onboarding requestor confirms the final composition.
3. **Matrix authority:** The confirmed matrix is authoritative for data within
   its scope. Conflicts involving other data sources require an explicit
   decision.
4. **Freshness and evidence:** Matrix confirmation establishes currency and
   evidence for matrix-held data. Data obtained elsewhere follows the rules of
   its governed attribute or resource definition.
5. **Overrides:** The onboarding requestor approves product and service options.
   Definition or constraint overrides require the responsible product,
   catalogue, or resource steward.
6. **Material change:** A change is material when it alters service selection,
   resource discovery, fan-out, or an attribute requirement or value.
7. **Storage:** The storage design is implementation-specific, but it must
   preserve and reproduce the JSON business record described above.
