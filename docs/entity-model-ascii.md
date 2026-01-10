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

## 12. Capital Structure & Ownership Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                   CAPITAL STRUCTURE MODEL                           │
│                      (kyc schema)                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ISSUER (Entity)                                                   │
│        │                                                            │
│        │ issues                                                     │
│        ▼                                                            │
│   ┌───────────────────┐        ┌───────────────────┐                │
│   │   SHARE_CLASS     │        │  SHARE_CLASS_ID   │                │
│   │───────────────────│        │───────────────────│                │
│   │ id (PK)           │───────▶│ identifier_id     │                │
│   │ cbu_id            │  1:N   │ share_class_id    │                │
│   │ issuer_entity_id  │        │ scheme (ISIN,etc) │                │
│   │ name              │        │ value             │                │
│   │ instrument_kind   │        │ is_primary        │                │
│   │ votes_per_unit    │        └───────────────────┘                │
│   │ economic_per_unit │                                             │
│   │ seniority_rank    │        ┌───────────────────┐                │
│   │ is_voting         │        │SHARE_CLASS_SUPPLY │                │
│   │ is_participating  │        │───────────────────│                │
│   │ status            │───────▶│ supply_id         │                │
│   └─────────┬─────────┘  1:N   │ share_class_id    │                │
│             │                  │ authorized_units  │                │
│             │                  │ issued_units      │                │
│             │                  │ outstanding_units │                │
│             │                  │ treasury_units    │                │
│             │                  │ reserved_units    │                │
│             │                  │ as_of_date        │                │
│             │                  └───────────────────┘                │
│             │                                                       │
│             │                  ┌───────────────────┐                │
│             │                  │  ISSUANCE_EVENT   │                │
│             └─────────────────▶│───────────────────│                │
│                          1:N   │ event_id          │                │
│                                │ share_class_id    │                │
│                                │ event_type        │                │
│                                │ units_delta       │                │
│                                │ ratio_from/to     │ ◀── splits     │
│                                │ price_per_unit    │                │
│                                │ effective_date    │                │
│                                │ status            │                │
│                                └───────────────────┘                │
│                                                                     │
│   Instrument Kinds:           Event Types:                          │
│   ─────────────────           ────────────                          │
│   ORDINARY_EQUITY             INITIAL_ISSUE, NEW_ISSUE              │
│   PREFERENCE_EQUITY           STOCK_SPLIT, CONSOLIDATION            │
│   RESTRICTED_STOCK            BONUS_ISSUE, CANCELLATION             │
│   NON_VOTING                  BUYBACK, TREASURY_RELEASE             │
│   CONVERTIBLE_PREF            MERGER_IN/OUT, SPINOFF                │
│                               CONVERSION                             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 13. Ownership & Control Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                   OWNERSHIP & CONTROL MODEL                         │
│                      (kyc schema)                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   SHARE_CLASS                    HOLDER (Entity)                    │
│        │                              │                             │
│        │ held_in                      │ owns                        │
│        ▼                              ▼                             │
│   ┌───────────────────────────────────────────────┐                 │
│   │                  HOLDINGS                     │                 │
│   │───────────────────────────────────────────────│                 │
│   │ holding_id (PK)                               │                 │
│   │ share_class_id (FK)                           │                 │
│   │ holder_entity_id (FK)                         │                 │
│   │ units                                         │                 │
│   │ acquisition_date                              │                 │
│   │ cost_basis                                    │                 │
│   │ status                                        │                 │
│   └───────────────────┬───────────────────────────┘                 │
│                       │                                             │
│                       │ derives                                     │
│                       ▼                                             │
│   ┌───────────────────────────────────────────────┐                 │
│   │             CONTROL_POSITIONS                 │                 │
│   │───────────────────────────────────────────────│                 │
│   │ position_id (PK)                              │                 │
│   │ issuer_entity_id                              │                 │
│   │ holder_entity_id                              │                 │
│   │ voting_pct           ◀── aggregate across     │                 │
│   │ economic_pct             all share classes    │                 │
│   │ has_control (>50%)                            │                 │
│   │ has_significant_influence (20-50%)            │                 │
│   │ derived_from (HOLDINGS | DECLARED | BOTH)     │                 │
│   │ as_of_date                                    │                 │
│   └───────────────────────────────────────────────┘                 │
│                                                                     │
│   ┌───────────────────────────────────────────────┐                 │
│   │              SPECIAL_RIGHTS                   │                 │
│   │───────────────────────────────────────────────│                 │
│   │ right_id (PK)                                 │                 │
│   │ issuer_entity_id                              │                 │
│   │ holder_entity_id (optional)                   │                 │
│   │ share_class_id (optional)                     │                 │
│   │ right_type                                    │                 │
│   │ notes                                         │                 │
│   └───────────────────────────────────────────────┘                 │
│                                                                     │
│   Right Types:                                                      │
│   ────────────                                                      │
│   BOARD_APPOINTMENT   - Right to appoint board member(s)            │
│   VETO_MA             - Veto over M&A transactions                  │
│   VETO_STRATEGY       - Veto over strategic decisions               │
│   ANTI_DILUTION       - Anti-dilution protection                    │
│   DRAG_ALONG          - Drag-along rights                           │
│   TAG_ALONG           - Tag-along rights                            │
│   PREEMPTION          - Pre-emption rights                          │
│   LIQUIDATION_PREF    - Liquidation preference                      │
│   INFORMATION         - Information/inspection rights               │
│   OTHER               - Other contractual rights                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 14. Dilution Instruments Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                   DILUTION INSTRUMENTS MODEL                        │
│                      (kyc schema)                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌───────────────────────────────────────────────┐                 │
│   │            DILUTION_INSTRUMENTS               │                 │
│   │───────────────────────────────────────────────│                 │
│   │ instrument_id (PK)                            │                 │
│   │ issuer_entity_id (FK)                         │                 │
│   │ holder_entity_id (FK)                         │                 │
│   │ converts_to_share_class_id (FK)               │                 │
│   │                                               │                 │
│   │ instrument_type    ◀── STOCK_OPTION, WARRANT  │                 │
│   │                        CONVERTIBLE_NOTE,      │                 │
│   │                        SAFE, RSU, PHANTOM     │                 │
│   │                                               │                 │
│   │ units_granted                                 │                 │
│   │ units_exercised                               │                 │
│   │ units_forfeited                               │                 │
│   │ units_outstanding  = granted-exercised-forf   │                 │
│   │                                               │                 │
│   │ conversion_ratio   ◀── adjusted on splits     │                 │
│   │ exercise_price     ◀── adjusted on splits     │                 │
│   │ is_exercisable                                │                 │
│   │ vesting_start_date                            │                 │
│   │ vesting_end_date                              │                 │
│   │ expiration_date                               │                 │
│   │ status (ACTIVE, EXERCISED, EXPIRED, etc)      │                 │
│   └───────────────────┬───────────────────────────┘                 │
│                       │                                             │
│                       │ on exercise                                 │
│                       ▼                                             │
│   ┌───────────────────────────────────────────────┐                 │
│   │         DILUTION_EXERCISE_EVENTS              │                 │
│   │───────────────────────────────────────────────│                 │
│   │ exercise_id (PK)                              │                 │
│   │ instrument_id (FK)                            │                 │
│   │ units_exercised                               │                 │
│   │ exercise_date                                 │                 │
│   │ exercise_price_paid                           │                 │
│   │ shares_issued                                 │                 │
│   │ resulting_holding_id (FK)                     │                 │
│   │ is_cashless                                   │                 │
│   │ shares_withheld_for_tax                       │                 │
│   └───────────────────────────────────────────────┘                 │
│                                                                     │
│                                                                     │
│   Fully Diluted Calculation:                                        │
│   ──────────────────────────                                        │
│   outstanding_shares                                                │
│   + SUM(units_outstanding * conversion_ratio) from dilution_inst    │
│   ─────────────────────────────────────────────────────────────     │
│   = fully_diluted_shares                                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 15. Ownership Snapshots & Reconciliation

```
┌─────────────────────────────────────────────────────────────────────┐
│               OWNERSHIP SNAPSHOTS & RECONCILIATION                  │
│                      (kyc schema)                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌───────────────────────────────────────────────┐                 │
│   │            OWNERSHIP_SNAPSHOTS                │                 │
│   │───────────────────────────────────────────────│                 │
│   │ snapshot_id (PK)                              │                 │
│   │ issuer_entity_id                              │                 │
│   │ snapshot_date                                 │                 │
│   │ snapshot_type (QUARTERLY, ANNUAL, AD_HOC)     │                 │
│   │ created_by                                    │                 │
│   │ status (DRAFT, FINAL, SUPERSEDED)             │                 │
│   └───────────────────┬───────────────────────────┘                 │
│                       │                                             │
│                       │ 1:N                                         │
│                       ▼                                             │
│   ┌───────────────────────────────────────────────┐                 │
│   │         OWNERSHIP_SNAPSHOT_LINES              │                 │
│   │───────────────────────────────────────────────│                 │
│   │ line_id (PK)                                  │                 │
│   │ snapshot_id (FK)                              │                 │
│   │ holder_entity_id                              │                 │
│   │ share_class_id                                │                 │
│   │ units                                         │                 │
│   │ voting_pct                                    │                 │
│   │ economic_pct                                  │                 │
│   │ source (DERIVED, DECLARED)                    │                 │
│   └───────────────────────────────────────────────┘                 │
│                                                                     │
│                                                                     │
│   ┌───────────────────────────────────────────────┐                 │
│   │          RECONCILIATION_RUNS                  │                 │
│   │───────────────────────────────────────────────│                 │
│   │ run_id (PK)                                   │                 │
│   │ issuer_entity_id                              │                 │
│   │ snapshot_a_id (FK)                            │                 │
│   │ snapshot_b_id (FK) or comparison_source       │                 │
│   │ run_date                                      │                 │
│   │ status (RUNNING, COMPLETE, FAILED)            │                 │
│   │ summary_json                                  │                 │
│   └───────────────────┬───────────────────────────┘                 │
│                       │                                             │
│                       │ 1:N                                         │
│                       ▼                                             │
│   ┌───────────────────────────────────────────────┐                 │
│   │         RECONCILIATION_FINDINGS               │                 │
│   │───────────────────────────────────────────────│                 │
│   │ finding_id (PK)                               │                 │
│   │ run_id (FK)                                   │                 │
│   │ finding_type (MISSING, EXTRA, MISMATCH, etc)  │                 │
│   │ holder_entity_id                              │                 │
│   │ share_class_id                                │                 │
│   │ expected_value                                │                 │
│   │ actual_value                                  │                 │
│   │ severity (INFO, WARNING, ERROR)               │                 │
│   │ resolution_status (OPEN, RESOLVED, IGNORED)   │                 │
│   │ resolution_notes                              │                 │
│   └───────────────────────────────────────────────┘                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 16. Capital Structure Graph Edges

```
┌─────────────────────────────────────────────────────────────────────┐
│                 CAPITAL STRUCTURE GRAPH EDGES                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Edge Type              From               To                      │
│   ─────────              ────               ──                      │
│   ISSUED_BY              Share_Class        Entity (issuer)         │
│   AFFECTS_SUPPLY         Issuance_Event     Share_Class             │
│   CONVERTS_TO            Dilution_Inst      Share_Class             │
│   GRANTED_TO             Dilution_Inst      Entity (holder)         │
│   SNAPSHOT_OWNER         Snapshot_Line      Entity (holder)         │
│   SNAPSHOT_ISSUER        Snapshot           Entity (issuer)         │
│   RIGHT_ATTACHED_TO      Special_Right      Share_Class             │
│   COMPARED_SNAPSHOT      Recon_Run          Snapshot                │
│                                                                     │
│   Rendering Colors (egui):                                          │
│   ────────────────────────                                          │
│   ORDINARY_EQUITY     → Blue (#3498DB)                              │
│   PREFERENCE_EQUITY   → Purple (#9B59B6)                            │
│   RESTRICTED_STOCK    → Orange (#E67E22)                            │
│   NON_VOTING          → Gray (#95A5A6)                              │
│   CONVERTIBLE_*       → Teal (#1ABC9C)                              │
│                                                                     │
│   Control indicators:                                               │
│   ───────────────────                                               │
│   has_control (>50%)              → Crown icon                      │
│   has_significant_influence       → Star icon                       │
│   has_board_rights                → Chair icon                      │
│   has_veto                        → Shield icon                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

Generated: 2026-01-10


---

## 17. Investor Register Visualization

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   INVESTOR REGISTER VISUALIZATION                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   PROBLEM: Scale Mismatch                                                    │
│   ───────────────────────                                                    │
│                                                                              │
│   UBO/Control View: 5-50 nodes         Economic Investors: 100-100,000+     │
│   ┌─────────────┐                      ┌─────────────────────────────┐      │
│   │  WORKS ✓    │                      │        BREAKS ✗             │      │
│   │   ┌─┐ ┌─┐   │                      │  ┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐  │      │
│   │   │A│─│B│   │                      │  │ ││ ││ ││ ││ ││ ││ ││ │  │      │
│   │   └┬┘ └─┘   │                      │  └─┘└─┘└─┘└─┘└─┘└─┘└─┘└─┘  │      │
│   │    │        │                      │     ... x 10,000 more       │      │
│   │   ┌┴┐       │                      │                             │      │
│   │   │C│       │                      │                             │      │
│   │   └─┘       │                      │                             │      │
│   └─────────────┘                      └─────────────────────────────┘      │
│                                                                              │
│   SOLUTION: Dual-Mode Visualization                                          │
│   ─────────────────────────────────                                          │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  CONTROL VIEW (Taxonomy Graph)                                       │   │
│   │                                                                      │   │
│   │  Threshold: >5% OR board rights OR special rights                   │   │
│   │                                                                      │   │
│   │       ┌──────────────────────┐     ┌──────────────────────┐        │   │
│   │       │ AllianzGI            │     │ Sequoia Fund VII     │        │   │
│   │       │ 35.2% ⚡ INSTITUTION │     │ 22.1% 🪑 LP/GP       │        │   │
│   │       │ UBOs: 3 identified   │     │ LPs: 12              │        │   │
│   │       │ [🔍 View UBO Chain]  │     │ [🔍 View LP Struct]  │        │   │
│   │       └──────────────────────┘     └──────────────────────┘        │   │
│   │              │                            │                         │   │
│   │       ┌──────┴─────────┐          ┌──────┴─────────┐               │   │
│   │       │ Management Co  │          │ Founders Pool  │               │   │
│   │       │ 8.3%           │          │ 12.4%          │               │   │
│   │       │ INSTITUTION    │          │ PROPER_PERSON  │               │   │
│   │       └────────────────┘          │ ✓ Terminal     │               │   │
│   │                                   └────────────────┘               │   │
│   │                                                                      │   │
│   │  ┌────────────────────────────────────────────────────────────────┐│   │
│   │  │  📊 AGGREGATE: 4,847 other investors (22.0% economic)          ││   │
│   │  │     Click to expand breakdown → Table panel below              ││   │
│   │  └────────────────────────────────────────────────────────────────┘│   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 18. Threshold Rules

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          THRESHOLD RULES                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Condition                              Visualization                       │
│   ─────────                              ─────────────                       │
│   >disclosure_threshold_pct (5%)         Individual taxonomy node            │
│   Has BOARD_APPOINTMENT right            Individual taxonomy node            │
│   Has any VETO_* right                   Individual taxonomy node            │
│   >significant_threshold_pct (25%)       Individual node + ⚡ indicator      │
│   >control_threshold_pct (50%)           Individual node + ⚡ + control edge │
│   Below all thresholds, no rights        Collapsed into aggregate node       │
│                                                                              │
│   Configured per issuer in: kyc.issuer_control_config                       │
│                                                                              │
│   Node Type Indicators:                                                      │
│   ─────────────────────                                                      │
│                                                                              │
│   TERMINAL (Proper Person)              INSTITUTIONAL (Has UBO Structure)   │
│   ┌────────────────────────┐            ┌────────────────────────┐          │
│   │ John Smith             │            │ AllianzGI              │          │
│   │ 8.3% ⚡                │            │ 35.2% ⚡               │          │
│   │ PROPER_PERSON          │            │ LIMITED_COMPANY        │          │
│   │ ✓ Verified             │            │ UBOs: 3 identified     │          │
│   │                        │            │ └ Allianz SE (100%)    │          │
│   │  [End of chain]        │            │ └ Public float...      │          │
│   │                        │            │ [🔍 View UBO Chain]    │          │
│   └────────────────────────┘            └────────────────────────┘          │
│        Green background                      Blue background                 │
│                                                                              │
│   Institutional fields:                                                      │
│   ─────────────────────                                                      │
│   is_terminal         → true = proper person, false = institution           │
│   has_ubo_structure   → Institution has navigable ownership                 │
│   cbu_id              → Link to institution's CBU graph                     │
│   known_ubos          → Pre-fetched UBO summary (max 5)                     │
│   chain_depth         → Levels to reach all proper persons                  │
│   ubo_discovery_status→ COMPLETE, PARTIAL, PENDING, NOT_REQUIRED           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 19. Aggregate Node & Drill-Down

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     AGGREGATE NODE & DRILL-DOWN                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Aggregate Node (in graph):                                                 │
│   ──────────────────────────                                                 │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  📊 4,847 other investors (22.0% economic)                          │   │
│   │  ┌─────────────────────────────────────────────────────────────┐   │   │
│   │  │████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│   │   │
│   │  │ INST  PROF  RETAIL  NOM                                     │   │   │
│   │  └─────────────────────────────────────────────────────────────┘   │   │
│   │                                                      [▶ Expand]    │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   Expanded Panel (below graph):                                             │
│   ─────────────────────────────                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  Summary by Type:                                                    │   │
│   │  ┌────────────────────────────────────────────────────────────────┐ │   │
│   │  │ Type           │ Count │ Units      │ % Econ │ Avg Holding   │ │   │
│   │  ├────────────────────────────────────────────────────────────────┤ │   │
│   │  │ INSTITUTIONAL  │    23 │  1,250,000 │   7.5% │    54,348     │ │   │
│   │  │ PROFESSIONAL   │   184 │    890,000 │   5.3% │     4,837     │ │   │
│   │  │ RETAIL         │ 4,521 │  1,200,000 │   7.2% │       265     │ │   │
│   │  │ NOMINEE        │     8 │    320,000 │   1.9% │    40,000     │ │   │
│   │  └────────────────────────────────────────────────────────────────┘ │   │
│   │                                                                      │   │
│   │  [🔍 Search...] [Filter: Type ▼] [Filter: Status ▼] [📥 Export]     │   │
│   │                                                                      │   │
│   │  Showing: INSTITUTIONAL (23 investors)                     Page 1/1 │   │
│   │  ┌────────────────────────────────────────────────────────────────┐ │   │
│   │  │ Name                    │ Units    │ % Econ │ KYC Status     │ │   │
│   │  ├────────────────────────────────────────────────────────────────┤ │   │
│   │  │ BlackRock Fund A        │  450,000 │   2.7% │ ✓ Approved     │ │   │
│   │  │ Vanguard Total Market   │  320,000 │   1.9% │ ✓ Approved     │ │   │
│   │  │ State Street ETF        │  180,000 │   1.1% │ ✓ Approved     │ │   │
│   │  └────────────────────────────────────────────────────────────────┘ │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   API Response Structure:                                                    │
│   ───────────────────────                                                    │
│   InvestorRegisterView {                                                     │
│       issuer: IssuerSummary,                                                │
│       thresholds: ThresholdConfig,                                          │
│       control_holders: Vec<ControlHolderNode>,   // Individual nodes        │
│       aggregate: Option<AggregateInvestorsNode>, // Collapsed rest          │
│       total_investor_count: i32,                                            │
│       has_dilution_data: bool,                                              │
│   }                                                                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

Generated: 2026-01-10
