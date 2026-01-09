# ob-poc Entity Model (ASCII)

> Portable entity model diagrams - works everywhere

---

## 1. Complete System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                    ob-poc SYSTEM                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│   CORE ENTITY MODEL              INVESTOR REGISTER              KYC DOMAIN              │
│   ─────────────────              ─────────────────              ──────────              │
│                                                                                          │
│   ┌─────────┐                    ┌──────────┐                   ┌──────────┐            │
│   │   CBU   │───represents──────▶│ INVESTOR │───kyc_case_id───▶│ KYC_CASE │            │
│   └────┬────┘                    └────┬─────┘                   └────┬─────┘            │
│        │                              │                              │                   │
│        │ entity_id                    │ holds                        │ contains          │
│        ▼                              ▼                              ▼                   │
│   ┌─────────┐                    ┌──────────┐                   ┌───────────┐           │
│   │ ENTITY  │◀──investor_id──────│ HOLDING  │                   │WORKSTREAM │           │
│   └────┬────┘                    └────┬─────┘                   └─────┬─────┘           │
│        │                              │                               │                  │
│        │ has                          │ movements                     │ checks           │
│        ▼                              ▼                               ▼                  │
│   ┌────────────┐                 ┌──────────┐                 ┌───────────────┐         │
│   │ IDENTIFIER │                 │ MOVEMENT │                 │SCREEN │ VERIF │         │
│   │(LEI,Tax,etc)│                │(sub/red) │                 └───────────────┘         │
│   └────────────┘                 └──────────┘                                           │
│        │                                                                                 │
│        │ LEI lookup                                                                      │
│        ▼                                                                                 │
│   ┌─────────────────────────────────────────────┐                                       │
│   │              GLEIF / BODS                    │                                       │
│   │  ┌────────────┐  ┌───────────┐  ┌────────┐  │                                       │
│   │  │GLEIF_ENTITY│─▶│ GLEIF_REL │─▶│  BODS  │  │                                       │
│   │  │ (LEI data) │  │(hierarchy)│  │(export)│  │                                       │
│   │  └────────────┘  └───────────┘  └────────┘  │                                       │
│   └─────────────────────────────────────────────┘                                       │
│                                                                                          │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Core Entity Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                        CORE ENTITY MODEL                            │
│                        (ob-poc schema)                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                         ┌───────────────┐                           │
│                         │      CBU      │                           │
│                         │───────────────│                           │
│                         │ cbu_id (PK)   │                           │
│                         │ name          │                           │
│                         │ jurisdiction  │                           │
│                         │ parent_cbu_id │──┐ (self-ref)             │
│                         │ entity_id (FK)│  │                        │
│                         │ category      │  │                        │
│                         │ client_type   │◀─┘                        │
│                         └───────┬───────┘                           │
│                                 │                                   │
│                                 │ 1:1 represents                    │
│                                 ▼                                   │
│                         ┌───────────────┐                           │
│                         │    ENTITY     │                           │
│                         │───────────────│                           │
│                         │ entity_id (PK)│                           │
│                         │ name          │                           │
│                         │ entity_type   │                           │
│                         │ country_code  │                           │
│                         │ jurisdiction  │                           │
│                         │ is_active     │                           │
│                         └───────┬───────┘                           │
│                                 │                                   │
│              ┌──────────────────┼──────────────────┐                │
│              │                  │                  │                │
│              ▼                  ▼                  ▼                │
│    ┌─────────────────┐ ┌───────────────┐ ┌─────────────────┐       │
│    │   IDENTIFIER    │ │  RELATIONSHIP │ │    INVESTOR     │       │
│    │─────────────────│ │───────────────│ │ (kyc schema)    │       │
│    │ identifier_id   │ │relationship_id│ │─────────────────│       │
│    │ entity_id (FK)  │ │ from_entity   │ │ investor_id     │       │
│    │ scheme          │ │ to_entity     │ │ entity_id (FK)  │       │
│    │ id (value)      │ │ rel_type      │ │ lifecycle_state │       │
│    │ lei_status      │ │ percentage    │ │ kyc_status      │       │
│    │ is_validated    │ │ effective_from│ │ owning_cbu_id   │       │
│    └─────────────────┘ └───────────────┘ └─────────────────┘       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘

Entity Types:
─────────────
  proper_person     - Natural person (individual)
  limited_company   - Ltd, GmbH, SA, etc.
  partnership       - LP, LLP, GP
  trust             - Express trust, unit trust
  fund              - Investment fund, SICAV
  nominee           - Nominee holder
  foundation        - Foundation, Stiftung

Identifier Schemes:
───────────────────
  LEI              - Legal Entity Identifier (GLEIF)
  CLEARSTREAM_KV   - Clearstream reference
  CLEARSTREAM_ACCT - Clearstream account
  ISIN             - Securities identifier
  company_register - National company number
  tax_id           - Tax identification
  SWIFT_BIC        - Bank identifier
  DUNS             - Dun & Bradstreet
  VAT              - VAT registration
  national_id      - Passport/national ID
```

---

## 3. Investor Register Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                      INVESTOR REGISTER                              │
│                       (kyc schema)                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌───────────────┐         issues          ┌───────────────┐      │
│   │      CBU      │────────────────────────▶│  SHARE_CLASS  │      │
│   │ (fund manager)│                         │───────────────│      │
│   └───────┬───────┘                         │ id (PK)       │      │
│           │                                 │ cbu_id (FK)   │      │
│           │ owning_cbu_id                   │ entity_id     │      │
│           │                                 │ name          │      │
│           ▼                                 │ isin          │      │
│   ┌───────────────┐                         │ currency      │      │
│   │   INVESTOR    │                         │ nav_per_share │      │
│   │───────────────│                         │ fund_type     │      │
│   │ investor_id   │                         │ status        │      │
│   │ entity_id(FK) │                         └───────┬───────┘      │
│   │ investor_type │                                 │              │
│   │ investor_cat  │                                 │              │
│   │ lifecycle_st  │                                 │              │
│   │ kyc_status    │         ┌───────────────┐       │              │
│   │ kyc_case_id   │         │    HOLDING    │       │              │
│   │ risk_rating   │         │───────────────│       │              │
│   │ provider      │────────▶│ id (PK)       │◀──────┘              │
│   └───────────────┘  holds  │ share_class_id│                      │
│                             │ investor_id   │                      │
│                             │ investor_ent  │                      │
│                             │ units         │                      │
│                             │ cost_basis    │                      │
│                             │ usage_type    │ ◀── TA or UBO        │
│                             │ holding_status│                      │
│                             │ provider      │                      │
│                             └───────┬───────┘                      │
│                                     │                              │
│                                     │ 1:N movements                │
│                                     ▼                              │
│                             ┌───────────────┐                      │
│                             │   MOVEMENT    │                      │
│                             │───────────────│                      │
│                             │ id (PK)       │                      │
│                             │ holding_id    │                      │
│                             │ movement_type │                      │
│                             │ units         │                      │
│                             │ price_per_unit│                      │
│                             │ trade_date    │                      │
│                             │ settle_date   │                      │
│                             │ reference     │                      │
│                             │ status        │                      │
│                             │ commitment_id │ ◀── PE capital calls │
│                             │ call_number   │                      │
│                             │ distrib_type  │                      │
│                             └───────────────┘                      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘

Movement Types:
───────────────
  Standard:      subscription, redemption, transfer_in, transfer_out
  PE/VC:         commitment, capital_call, distribution, recallable
  Lifecycle:     initial_subscription, additional_subscription,
                 partial_redemption, full_redemption
  Corporate:     dividend, stock_split, merger, spinoff, adjustment

Usage Types:
────────────
  TA  - Transfer Agency (client's end investors)
  UBO - Intra-group holdings (for beneficial ownership)
```

---

## 4. Investor Lifecycle State Machine

```
┌─────────────────────────────────────────────────────────────────────┐
│                    INVESTOR LIFECYCLE STATES                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌──────────┐                                                      │
│   │ ENQUIRY  │◀─────────────────────────────────────────────┐       │
│   └────┬─────┘                                              │       │
│        │ submit docs                                        │       │
│        ▼                                                    │       │
│   ┌───────────────────┐                                     │       │
│   │ PENDING_DOCUMENTS │◀──────────────────┐                 │       │
│   └─────────┬─────────┘                   │                 │       │
│             │ all docs received           │ retry           │       │
│             ▼                             │                 │       │
│   ┌───────────────────┐             ┌─────┴──────┐          │       │
│   │  KYC_IN_PROGRESS  │────────────▶│KYC_REJECTED│          │       │
│   └─────────┬─────────┘   rejected  └────────────┘          │       │
│             │                                               │       │
│             │ approved                                      │       │
│             ▼                                               │       │
│   ┌───────────────────┐                                     │       │
│   │   KYC_APPROVED    │                                     │       │
│   └─────────┬─────────┘                                     │       │
│             │ make eligible                                 │       │
│             ▼                                               │       │
│   ┌───────────────────┐                                     │       │
│   │ELIGIBLE_TO_SUBSCR │                                     │       │
│   └─────────┬─────────┘                                     │       │
│             │ first subscription                            │       │
│             ▼                                               │       │
│   ┌───────────────────┐                                     │       │
│   │    SUBSCRIBED     │                                     │       │
│   └─────────┬─────────┘                                     │       │
│             │ settlement                                    │       │
│             ▼                                               │       │
│   ┌───────────────────┐          ┌───────────┐              │       │
│   │  ACTIVE_HOLDER    │─────────▶│ SUSPENDED │              │       │
│   └────┬─────────┬────┘ suspend  └─────┬─────┘              │       │
│        │         │                     │ reinstate          │       │
│        │         │◀────────────────────┘                    │       │
│        │         │                                          │       │
│        │         └───────────────▶┌─────────┐               │       │
│        │              block       │ BLOCKED │               │       │
│        │                          └─────────┘               │       │
│        │ full redemption                                    │       │
│        ▼                                                    │       │
│   ┌───────────────────┐                                     │       │
│   │    REDEEMING      │                                     │       │
│   └─────────┬─────────┘                                     │       │
│             │ settlement complete                           │       │
│             ▼                                               │       │
│   ┌───────────────────┐                                     │       │
│   │   OFFBOARDED      │─────────────────────────────────────┘       │
│   └───────────────────┘   re-engage                                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 5. KYC Domain Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                         KYC DOMAIN                                  │
│                        (kyc schema)                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌───────────────┐                                                 │
│   │      CBU      │                                                 │
│   │  (subject)    │                                                 │
│   └───────┬───────┘                                                 │
│           │ 1:N                                                     │
│           ▼                                                         │
│   ┌───────────────┐          ┌───────────────┐                      │
│   │   KYC_CASE    │          │   INVESTOR    │                      │
│   │───────────────│◀─────────│ (kyc_case_id) │                      │
│   │ case_id (PK)  │          └───────────────┘                      │
│   │ cbu_id (FK)   │                                                 │
│   │ case_type     │  ONBOARDING, PERIODIC_REVIEW, TRIGGER_EVENT    │
│   │ status        │  OPEN, IN_PROGRESS, PENDING_APPROVAL, COMPLETE │
│   │ priority      │                                                 │
│   │ assigned_to   │                                                 │
│   │ due_date      │                                                 │
│   └───────┬───────┘                                                 │
│           │ 1:N                                                     │
│           ▼                                                         │
│   ┌───────────────────┐                                             │
│   │ ENTITY_WORKSTREAM │                                             │
│   │───────────────────│                                             │
│   │ workstream_id(PK) │                                             │
│   │ case_id (FK)      │                                             │
│   │ entity_id (FK)    │──────▶ ENTITY                               │
│   │ discovery_reason  │  SHAREHOLDER, DIRECTOR, UBO, SIGNATORY     │
│   │ is_ubo            │                                             │
│   │ ownership_pct     │                                             │
│   │ status            │  PENDING, IN_PROGRESS, COMPLETE, BLOCKED   │
│   │ risk_rating       │  LOW, MEDIUM, HIGH, PROHIBITED             │
│   └─────────┬─────────┘                                             │
│             │                                                       │
│      ┌──────┴──────┬──────────────┐                                 │
│      │             │              │                                 │
│      ▼             ▼              ▼                                 │
│ ┌──────────┐ ┌────────────┐ ┌──────────┐                            │
│ │SCREENING │ │VERIFICATION│ │ DOCUMENT │                            │
│ │──────────│ │────────────│ │──────────│                            │
│ │screen_id │ │ verif_id   │ │ doc_id   │                            │
│ │workstr_id│ │ workstr_id │ │workstr_id│                            │
│ │type      │ │ type       │ │ type     │                            │
│ │provider  │ │ status     │ │ file_path│                            │
│ │status    │ │ outcome    │ │ status   │                            │
│ │result    │ │ evidence   │ │ expiry   │                            │
│ │matches   │ └────────────┘ └──────────┘                            │
│ └──────────┘                                                        │
│                                                                     │
│ Screening Types:        Verification Types:    Document Types:      │
│ ─────────────────       ──────────────────     ───────────────      │
│ SANCTIONS               ID_VERIFICATION        PASSPORT             │
│ PEP                     ADDRESS_PROOF          UTILITY_BILL         │
│ ADVERSE_MEDIA           COMPANY_SEARCH         CERT_OF_INCORP       │
│ WATCHLIST               REGISTRY_CHECK         ANNUAL_RETURN        │
│                                                STRUCTURE_CHART      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 6. UBO Discovery Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                      UBO DISCOVERY FLOW                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   INVESTOR REGISTER                    ENTITY RELATIONSHIPS         │
│   ─────────────────                    ────────────────────         │
│                                                                     │
│   ┌──────────┐                                                      │
│   │ HOLDING  │                                                      │
│   │──────────│                                                      │
│   │ units    │                                                      │
│   └────┬─────┘                                                      │
│        │                                                            │
│        │ calculate                                                  │
│        ▼                                                            │
│   ┌──────────────┐                                                  │
│   │ ownership %  │                                                  │
│   │ = units /    │                                                  │
│   │   total_units│                                                  │
│   └──────┬───────┘                                                  │
│          │                                                          │
│          │                                                          │
│          ▼                                                          │
│   ┌──────────────┐     YES    ┌─────────────────────┐               │
│   │   >= 25% ?   │───────────▶│ Create/Update       │               │
│   └──────┬───────┘            │ ENTITY_RELATIONSHIP │               │
│          │                    │─────────────────────│               │
│          │ NO                 │ from: investor      │               │
│          │                    │ to: fund entity     │               │
│          ▼                    │ type: ownership     │               │
│   ┌──────────────┐            │ %: ownership_pct    │               │
│   │  No action   │            │ source: INVESTOR_   │               │
│   │  (not UBO)   │            │         REGISTER    │               │
│   └──────────────┘            └──────────┬──────────┘               │
│                                          │                          │
│                                          │                          │
│                                          ▼                          │
│                               ┌─────────────────────┐               │
│                               │ Is investor a       │               │
│                               │ natural person?     │               │
│                               └──────────┬──────────┘               │
│                                          │                          │
│                          ┌───────────────┴───────────────┐          │
│                          │                               │          │
│                          ▼ YES                           ▼ NO       │
│                   ┌─────────────┐              ┌─────────────────┐  │
│                   │ DIRECT UBO  │              │ TRACE via GLEIF │  │
│                   │ Mark is_ubo │              │ (corporate inv) │  │
│                   │ in workstr  │              │ Find ultimate   │  │
│                   └─────────────┘              │ natural person  │  │
│                                               └─────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘

Trigger: INSERT/UPDATE on kyc.holdings
Action:  Sync to "ob-poc".entity_relationships if >= 25%
```

---

## 7. GLEIF / BODS Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                      GLEIF / BODS INTEGRATION                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ENTITY                    GLEIF                    BODS           │
│   ──────                    ─────                    ────           │
│                                                                     │
│   ┌──────────────┐    LEI    ┌───────────────┐                      │
│   │   ENTITY     │◀─────────▶│ GLEIF_ENTITY  │                      │
│   │──────────────│   lookup  │───────────────│                      │
│   │ entity_id    │           │ lei (PK)      │                      │
│   └──────┬───────┘           │ legal_name    │                      │
│          │                   │ jurisdiction  │                      │
│          │                   │ entity_status │                      │
│   ┌──────┴───────┐           │ category      │                      │
│   │  IDENTIFIER  │           │ reg_address   │                      │
│   │──────────────│           │ hq_address    │                      │
│   │ scheme='LEI' │           └───────┬───────┘                      │
│   │ id=<lei>     │                   │                              │
│   │ lei_status   │                   │ parent/child                 │
│   │ lei_renewal  │                   ▼                              │
│   └──────────────┘           ┌───────────────┐                      │
│                              │  GLEIF_REL    │                      │
│                              │───────────────│                      │
│                              │ from_entity   │                      │
│                              │ to_entity     │                      │
│                              │ rel_type      │ IS_ULTIMATELY_CONSOL │
│                              │ rel_status    │ IS_DIRECTLY_CONSOL   │
│                              │ effective_from│ IS_INTERNATIONAL_BR  │
│                              └───────────────┘ IS_FUND_MANAGED_BY   │
│                                                                     │
│                                      │                              │
│                                      │ feeds into                   │
│                                      ▼                              │
│                              ┌─────────────────────────────────┐    │
│                              │      BODS EXPORT VIEW           │    │
│                              │─────────────────────────────────│    │
│                              │ v_bods_ownership_statements     │    │
│                              │─────────────────────────────────│    │
│                              │ Sources:                        │    │
│                              │  - Holdings (investor register) │    │
│                              │  - Entity relationships         │    │
│                              │  - GLEIF hierarchy              │    │
│                              │                                 │    │
│                              │ Output: BODS 0.4 JSON/NDJSON    │    │
│                              │  - ownershipOrControlStatement  │    │
│                              │  - entityStatement              │    │
│                              │  - personStatement              │    │
│                              └─────────────────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 8. Dual-Use Holding Pattern

```
┌─────────────────────────────────────────────────────────────────────┐
│                    DUAL-USE HOLDING PATTERN                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   USE CASE A: Transfer Agency                USE CASE B: UBO        │
│   ───────────────────────────                ──────────────         │
│                                                                     │
│   BNY Client (Fund Manager)                  Legal Structure        │
│        │                                          │                 │
│        │ issues shares                            │ owns            │
│        ▼                                          ▼                 │
│   ┌──────────┐                              ┌──────────┐            │
│   │SHARE_CLS │                              │SHARE_CLS │            │
│   └────┬─────┘                              └────┬─────┘            │
│        │                                         │                  │
│        │                                         │                  │
│   ┌────┴─────┐                              ┌────┴─────┐            │
│   │ HOLDING  │                              │ HOLDING  │            │
│   │──────────│                              │──────────│            │
│   │usage=TA  │                              │usage=UBO │            │
│   │investor  │                              │entity    │            │
│   │  _id set │                              │  only    │            │
│   └────┬─────┘                              └────┬─────┘            │
│        │                                         │                  │
│        │                                         │                  │
│        ▼                                         ▼                  │
│   ┌──────────────────┐                   ┌──────────────────┐       │
│   │    INVESTOR      │                   │ ENTITY_RELATION  │       │
│   │──────────────────│                   │──────────────────│       │
│   │ lifecycle_state  │                   │ from: entity     │       │
│   │ kyc_status       │                   │ to: fund entity  │       │
│   │ kyc_case_id      │                   │ %: ownership     │       │
│   │ eligible_funds   │                   │ feeds UBO disc   │       │
│   └──────────────────┘                   └──────────────────┘       │
│        │                                         │                  │
│        │                                         │                  │
│        ▼                                         ▼                  │
│   ┌──────────────────┐                   ┌──────────────────┐       │
│   │    KYC_CASE      │                   │  KYC WORKSTREAM  │       │
│   │ (investor onb)   │                   │  (UBO review)    │       │
│   └──────────────────┘                   └──────────────────┘       │
│                                                                     │
│   Both feed into:                                                   │
│   ─────────────────                                                 │
│   ┌───────────────────────────────────────────────────────────┐     │
│   │              v_bods_ownership_statements                  │     │
│   │           (unified BODS 0.4 regulatory export)            │     │
│   └───────────────────────────────────────────────────────────┘     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 9. Data Provider Pattern

```
┌─────────────────────────────────────────────────────────────────────┐
│                      DATA PROVIDER PATTERN                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   EXTERNAL SOURCES                                                  │
│   ────────────────                                                  │
│                                                                     │
│   ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐   │
│   │CLEARSTREAM │  │ EUROCLEAR  │  │  API FEED  │  │ CSV IMPORT │   │
│   │ CASCADE-RS │  │            │  │            │  │            │   │
│   └─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘   │
│         │               │               │               │          │
│         └───────────────┴───────┬───────┴───────────────┘          │
│                                 │                                   │
│                                 ▼                                   │
│                    ┌────────────────────────┐                       │
│                    │    INVESTOR RECORD     │                       │
│                    │────────────────────────│                       │
│                    │ provider = 'CLEARSTRM' │                       │
│                    │ provider_reference     │                       │
│                    │ provider_sync_at       │                       │
│                    └────────────────────────┘                       │
│                                 │                                   │
│                                 ▼                                   │
│                    ┌────────────────────────┐                       │
│                    │    HOLDING RECORD      │                       │
│                    │────────────────────────│                       │
│                    │ provider = 'CLEARSTRM' │                       │
│                    │ provider_reference     │                       │
│                    │ provider_sync_at       │                       │
│                    └────────────────────────┘                       │
│                                 │                                   │
│                                 ▼                                   │
│                    ┌────────────────────────┐                       │
│                    │   ENTITY_IDENTIFIER    │                       │
│                    │────────────────────────│                       │
│                    │ scheme='CLEARSTREAM_KV'│                       │
│                    │ id = provider_ref      │                       │
│                    └────────────────────────┘                       │
│                                                                     │
│   Provider Values:                                                  │
│   ───────────────                                                   │
│     CLEARSTREAM  - Clearstream CASCADE-RS/Vestima                   │
│     EUROCLEAR    - Euroclear FundSettle                             │
│     API_FEED     - Direct API integration                           │
│     CSV_IMPORT   - Bulk file import                                 │
│     MANUAL       - Manual entry (default)                           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 10. Key Relationships Summary

```
┌─────────────────────────────────────────────────────────────────────┐
│                    KEY RELATIONSHIPS                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   FROM              RELATIONSHIP         TO              CARD       │
│   ────              ────────────         ──              ────       │
│   CBU               represents           Entity          1:1        │
│   CBU               parent_of            CBU             1:N        │
│   CBU               issues               Share_Class     1:N        │
│   CBU               owns_investor_rel    Investor        1:N        │
│                                                                     │
│   Entity            has                  Identifier      1:N        │
│   Entity            participates_in      Relationship    N:M        │
│   Entity            has_profile          Investor        1:0..1     │
│                                                                     │
│   Investor          holds                Holding         1:N        │
│   Investor          has_kyc_case         KYC_Case        N:1        │
│                                                                     │
│   Share_Class       has_positions        Holding         1:N        │
│   Share_Class       belongs_to_entity    Entity          N:1        │
│                                                                     │
│   Holding           has_movements        Movement        1:N        │
│   Holding (>=25%)   syncs_to             Relationship    1:1        │
│                                                                     │
│   KYC_Case          contains             Workstream      1:N        │
│   Workstream        reviews              Entity          N:1        │
│   Workstream        has                  Screening       1:N        │
│   Workstream        has                  Verification    1:N        │
│   Workstream        has                  Document        1:N        │
│                                                                     │
│   Identifier(LEI)   lookup               GLEIF_Entity    1:1        │
│   GLEIF_Entity      hierarchy            GLEIF_Rel       1:N        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 11. Schema Distribution

```
┌─────────────────────────────────────────────────────────────────────┐
│                     SCHEMA DISTRIBUTION                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   "ob-poc" schema (core)           kyc schema (register + KYC)      │
│   ──────────────────────           ───────────────────────────      │
│                                                                     │
│   ├── entities                     ├── investors                    │
│   ├── entity_identifiers           ├── share_classes                │
│   ├── entity_relationships         ├── holdings                     │
│   ├── cbus                         ├── movements                    │
│   ├── gleif_entities               ├── kyc_cases                    │
│   ├── gleif_relationships          ├── entity_workstreams           │
│   ├── bods_interest_types          ├── screenings                   │
│   └── (reference tables)           ├── verifications                │
│                                    ├── documents                    │
│                                    ├── investor_lifecycle_history   │
│                                    └── investor_lifecycle_trans     │
│                                                                     │
│   teams schema (RBAC)              instruments schema (trading)     │
│   ───────────────────              ────────────────────────────     │
│                                                                     │
│   ├── teams                        ├── instruments                  │
│   ├── team_members                 ├── instrument_identifiers       │
│   ├── roles                        ├── instrument_relationships     │
│   ├── permissions                  └── (market data)                │
│   └── delegations                                                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

Generated: 2026-01-09
