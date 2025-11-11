# OB-POC DSL Grammar v3.1 - EBNF Documentation

## Overview

This document provides comprehensive Extended Backus-Naur Form (EBNF) grammar documentation for the **Ultimate Beneficial Ownership (UBO) and Onboarding DSL v3.1**. This declarative domain-specific language implements a **DSL-as-State** architecture with multi-domain support for financial workflows.

### Key Design Principles

- **Unified S-Expression Syntax**: `(verb :key value :key value ...)`
- **Clojure-Style Keywords**: `:` prefix for keys (e.g., `:customer-id`, `:props.legal-name`)
- **AttributeID-as-Type Pattern**: Universal data dictionary with UUID references
- **Multi-Domain Integration**: Document Library, ISDA Derivatives, KYC, UBO, Compliance
- **Homoiconicity**: Data and code share the same representation
- **Complete Audit Trail**: Declarative state with immutable versioning

### Version Information

- **Version**: 3.1
- **Generated**: 2025-11-10
- **Architecture**: DSL-as-State + AttributeID-as-Type + AI Integration
- **Database Status**: Grammar rules table currently empty (using file-based grammar)

## Grammar Structure

### Core Program Structure

```ebnf
(* Top-level program structure *)
program = form* ;

(* Core form structure - unified syntax *)
form = "(" verb (key value)* ")" | comment ;

(* Comments *)
comment = ";;" ? any character except newline ? newline? ;
```

### Keywords and Values

```ebnf
(* Keys - namespaced keywords *)
key = ":" identifier ( "." identifier )? ;

(* Values - comprehensive type system *)
value = literal | identifier | list | map | attr-ref ;

(* Literals *)
literal = string | number | boolean | date | datetime | uuid ;

(* Collections - Clojure-style syntax *)
list = "[" (value (("," | whitespace) value)*)? "]" ;
map = "{" (key whitespace value)* "}" ;

(* Attribute references - UUID-based type system *)
attr-ref = "@attr{" uuid "}" ;
```

### Identifiers and Primitive Types

```ebnf
(* Identifiers *)
identifier = (letter | "_" | "-") (letter | digit | "_" | "-")* ;

(* Primitive types *)
string = '"' character* '"' ;
number = "-"? digit+ ("." digit+)? ("%" | currency-code)? ;
boolean = "true" | "false" ;
date = string ;  (* ISO 8601 date format: YYYY-MM-DD *)
datetime = string ;  (* ISO 8601 datetime format: YYYY-MM-DDTHH:MM:SSZ *)
uuid = string ;  (* UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx *)

(* Character classes *)
letter = "a".."z" | "A".."Z" ;
digit = "0".."9" ;
character = ? any character except '"' and '\' ? | '\' escape-sequence ;
escape-sequence = '\"' | '\\' | '\n' | '\r' | '\t' ;

(* Currency support *)
currency-code = "USD" | "EUR" | "GBP" | "JPY" | "CHF" | "CAD" | "AUD" | "SGD" ;

(* Whitespace *)
whitespace = " " | "\t" | "\n" | "\r" ;
newline = "\n" | "\r\n" ;
```

## Multi-Domain Verb Vocabulary

### Verb Categories

```ebnf
(* Verbs - Multi-domain vocabulary *)
verb = workflow-verb | graph-verb | role-verb | kyc-verb
     | compliance-verb | case-verb | ubo-verb
     | document-verb | isda-verb ;
```

### Core Workflow Management

```ebnf
(* Workflow management verbs *)
workflow-verb = "define-kyc-investigation" | "workflow.transition" ;
```

**Supported Verbs:**
- `define-kyc-investigation` - Define KYC investigation workflow
- `workflow.transition` - Transition workflow state

### Graph Construction

```ebnf
(* Graph construction verbs *)
graph-verb = "entity" | "edge" ;
```

**Supported Verbs:**
- `entity` - Define business entities (companies, trusts, persons)
- `edge` - Define relationships between entities

### Role Management

```ebnf
(* Role assignment verbs *)
role-verb = "role.assign" ;
```

**Supported Verbs:**
- `role.assign` - Assign roles to entities within CBUs

### KYC Domain

```ebnf
(* KYC domain verbs *)
kyc-verb = "kyc.verify" | "kyc.assess_risk" | "kyc.collect_document"
         | "kyc.screen_sanctions" | "kyc.check_pep" | "kyc.validate_address" ;
```

**Supported Verbs:**
- `kyc.verify` - Verify customer identity
- `kyc.assess_risk` - Assess customer risk rating
- `kyc.collect_document` - Collect KYC documents
- `kyc.screen_sanctions` - Screen against sanctions lists
- `kyc.check_pep` - Check for Politically Exposed Persons
- `kyc.validate_address` - Validate customer addresses

### Compliance Domain

```ebnf
(* Compliance domain verbs *)
compliance-verb = "compliance.fatca_check" | "compliance.crs_check"
                | "compliance.aml_check" | "compliance.generate_sar"
                | "compliance.verify" ;
```

**Supported Verbs:**
- `compliance.fatca_check` - FATCA compliance checking
- `compliance.crs_check` - Common Reporting Standard checks
- `compliance.aml_check` - Anti-Money Laundering checks
- `compliance.generate_sar` - Generate Suspicious Activity Reports
- `compliance.verify` - General compliance verification

### Case Management

```ebnf
(* Case management verbs *)
case-verb = "case.create" | "case.update" | "case.close" ;
```

**Supported Verbs:**
- `case.create` - Create new cases/CBUs
- `case.update` - Update existing cases
- `case.close` - Close completed cases

### UBO Operations

```ebnf
(* UBO calculation verbs *)
ubo-verb = "ubo.calc" | "ubo.outcome" ;
```

**Supported Verbs:**
- `ubo.calc` - Calculate ultimate beneficial ownership
- `ubo.outcome` - Record UBO identification results

## NEW in v3.1: Document Library Domain

```ebnf
(* Document Library verbs (NEW in v3.1) *)
document-verb = "document.catalog" | "document.verify" | "document.extract"
              | "document.link" | "document.use" | "document.amend"
              | "document.expire" | "document.query" ;
```

**Document Lifecycle Verbs:**
- `document.catalog` - Catalog documents with rich metadata
- `document.verify` - Verify document authenticity and integrity
- `document.extract` - AI-powered data extraction from documents
- `document.link` - Create relationships between documents
- `document.use` - Track document usage across workflows
- `document.amend` - Handle document amendments and versions
- `document.expire` - Manage document lifecycle and expiration
- `document.query` - Search and query document library

## NEW in v3.1: ISDA Derivatives Domain

```ebnf
(* ISDA Derivative verbs (NEW in v3.1) *)
isda-verb = "isda.establish_master" | "isda.establish_csa" | "isda.execute_trade"
          | "isda.margin_call" | "isda.post_collateral" | "isda.value_portfolio"
          | "isda.declare_termination_event" | "isda.close_out" | "isda.amend_agreement"
          | "isda.novate_trade" | "isda.dispute" | "isda.manage_netting_set" ;
```

**Complete Derivative Lifecycle Verbs:**
- `isda.establish_master` - Set up ISDA Master Agreement
- `isda.establish_csa` - Configure Credit Support Annex
- `isda.execute_trade` - Execute derivative trades
- `isda.margin_call` - Issue collateral margin calls
- `isda.post_collateral` - Post collateral in response to calls
- `isda.value_portfolio` - Perform portfolio valuations
- `isda.declare_termination_event` - Handle credit events
- `isda.close_out` - Perform early termination calculations
- `isda.amend_agreement` - Amend existing agreements
- `isda.novate_trade` - Transfer trades via novation
- `isda.dispute` - Manage disputes and resolutions
- `isda.manage_netting_set` - Calculate netting benefits

## Semantic Constraints and Validation Rules

### Key Validation Rules

- **Clojure-Style Keywords**: Keys must use `:` prefix (e.g., `:customer-id`, `:props.legal-name`)
- **Namespace Support**: Dot notation for namespaced keys (e.g., `:domain.attribute`)
- **Verb-Specific Constraints**: Enforced at semantic validation stage

### Value Type Constraints

- **AttributeID Resolution**: `@attr{uuid}` references must resolve to valid dictionary entries
- **Runtime Validation**: Values validated against AttributeID dictionary at runtime
- **Type Safety**: AttributeID-as-Type pattern ensures type consistency

### Cross-Domain Constraints

- **Document Integration**: Document verbs can reference entities from KYC/UBO domains
- **ISDA Integration**: ISDA verbs can reference documents from Document domain
- **Referential Integrity**: All verbs maintain integrity through AttributeID system

## Verb-Specific Parameter Requirements

### Core KYC/UBO Verbs

- **`define-kyc-investigation`**: requires `:id`, `:target-entity`
- **`entity`**: requires `:id`, `:label`, optional `:props`
- **`edge`**: requires `:from`, `:to`, `:type`, optional `:props`, `:evidence`
- **`ubo.outcome`**: requires `:target`, `:at`, `:threshold`, `:ubos`
- **`role.assign`**: requires `:entity`, `:role`, `:cbu`

### Document Library Verb Constraints

- **`document.catalog`**: requires `:document-id`, `:document-type`, `:issuer`
- **`document.verify`**: requires `:document-id`, `:verification-method`
- **`document.extract`**: requires `:document-id`, `:extraction-method`
- **`document.link`**: requires `:primary-document`, `:related-document`, `:relationship-type`
- **`document.use`**: requires `:document-id`, `:used-by-process`
- **`document.amend`**: requires `:document-id`, `:amendment-type`
- **`document.expire`**: requires `:document-id`, `:expiry-reason`
- **`document.query`**: requires `:query-type`, `:search-criteria`

### ISDA Derivative Verb Constraints

- **`isda.establish_master`**: requires `:agreement-id`, `:party-a`, `:party-b`, `:version`, `:governing-law`
- **`isda.establish_csa`**: requires `:csa-id`, `:master-agreement-id`, `:base-currency`, `:threshold-party-a`, `:threshold-party-b`
- **`isda.execute_trade`**: requires `:trade-id`, `:master-agreement-id`, `:product-type`, `:notional-amount`
- **`isda.margin_call`**: requires `:call-id`, `:csa-id`, `:exposure-amount`, `:call-amount`
- **`isda.post_collateral`**: requires `:posting-id`, `:call-id`, `:collateral-type`, `:amount`
- **`isda.value_portfolio`**: requires `:valuation-id`, `:portfolio-id`, `:valuation-date`, `:net-mtm`
- **`isda.declare_termination_event`**: requires `:event-id`, `:master-agreement-id`, `:event-type`
- **`isda.close_out`**: requires `:closeout-id`, `:master-agreement-id`, `:closeout-amount`
- **`isda.amend_agreement`**: requires `:amendment-id`, `:original-agreement-id`, `:amendment-type`
- **`isda.novate_trade`**: requires `:novation-id`, `:original-trade-id`, `:transferor`, `:transferee`
- **`isda.dispute`**: requires `:dispute-id`, `:master-agreement-id`, `:dispute-type`
- **`isda.manage_netting_set`**: requires `:netting-set-id`, `:master-agreement-id`, `:included-trades`

## Example Usage Patterns

### Multi-Domain KYC + Document Workflow

```lisp
(define-kyc-investigation
  :id "zenith-capital-ubo-discovery"
  :target-entity "company-zenith-spv-001"
  :jurisdiction "KY"
  :ubo-threshold 25.0)

(document.catalog
  :document-id "doc-certificate-001"
  :document-type "CERTIFICATE_OF_INCORPORATION"
  :issuer "cayman_registrar"
  :title "Certificate of Incorporation - Zenith Capital SPV"
  :jurisdiction "KY"
  :extracted-data {
    :company.legal_name "Zenith Capital Partners LP"
    :company.registration_number "KY-123456"
    :company.incorporation_date "2020-03-15"
  })

(entity
  :id "company-zenith-spv-001"
  :label "Company"
  :props {
    :legal-name "Zenith Capital Partners LP"
    :registration-number "KY-123456"
    :jurisdiction "KY"
  }
  :document-evidence ["doc-certificate-001"])
```

### Document-Backed ISDA Workflow

```lisp
(document.catalog
  :document-id "doc-isda-master-001"
  :document-type "ISDA_MASTER_AGREEMENT"
  :issuer "isda_inc"
  :parties ["company-zenith-spv-001" "jpmorgan-chase-entity"]
  :extracted-data {
    :isda.governing_law "NY"
    :isda.master_agreement_version "2002"
    :isda.multicurrency_cross_default true
  })

(isda.establish_master
  :agreement-id "ISDA-ZENITH-JPM-001"
  :party-a "company-zenith-spv-001"
  :party-b "jpmorgan-chase-entity"
  :version "2002"
  :governing-law "NY"
  :agreement-date "2023-01-15"
  :document-id "doc-isda-master-001")

(isda.establish_csa
  :csa-id "CSA-ZENITH-JPM-001"
  :master-agreement-id "ISDA-ZENITH-JPM-001"
  :base-currency "USD"
  :threshold-party-a 0
  :threshold-party-b 5000000
  :minimum-transfer 100000
  :eligible-collateral ["cash_usd" "us_treasury"])
```

### Complete Derivative Lifecycle

```lisp
(isda.execute_trade
  :trade-id "TRADE-IRS-001"
  :master-agreement-id "ISDA-ZENITH-JPM-001"
  :product-type "IRS"
  :trade-date "2024-03-15"
  :notional-amount 50000000
  :currency "USD"
  :underlying "USD-SOFR")

(isda.value_portfolio
  :valuation-id "VAL-001"
  :portfolio-id "PORTFOLIO-ZENITH-JPM"
  :valuation-date "2024-09-15"
  :trades-valued ["TRADE-IRS-001"]
  :net-mtm -8750000)

(isda.margin_call
  :call-id "MC-001"
  :csa-id "CSA-ZENITH-JPM-001"
  :call-date "2024-09-15"
  :calling-party "jpmorgan-chase-entity"
  :called-party "company-zenith-spv-001"
  :exposure-amount 8750000
  :call-amount 5700000
  :deadline "2024-09-16T17:00:00Z")

(isda.post_collateral
  :posting-id "POST-001"
  :call-id "MC-001"
  :posting-party "company-zenith-spv-001"
  :collateral-type "cash_usd"
  :amount 5700000
  :settlement-date "2024-09-16")
```

### UBO Outcome with Document Evidence

```lisp
(ubo.outcome
  :target "company-zenith-spv-001"
  :at "2025-11-10T10:30:00Z"
  :threshold 25.0
  :ubos [{
    :entity "person-john-smith"
    :effective-percent 45.0
    :prongs {:ownership 45.0, :voting 45.0}
    :evidence ["doc-certificate-001" "doc-share-register-001"]
  }])
```

### Cross-Domain Relationship Tracking

```lisp
(document.use
  :document-id "doc-isda-master-001"
  :used-by-process "DERIVATIVE_TRADING"
  :usage-date "2024-03-15"
  :business-purpose "LEGAL_FRAMEWORK"
  :related-workflows ["ISDA-ZENITH-JPM-001"])

(role.assign
  :entity "person-john-smith"
  :role "UltimateBeneficialOwner"
  :cbu "CBU-ZENITH-001"
  :effective-percent 45.0
  :assigned-date "2025-11-10T10:30:00Z"
  :evidence-documents ["doc-certificate-001"]
  :workflow-context "zenith-capital-ubo-discovery")
```

## Domain Integration Patterns v3.1

### Pattern 1: Document-Driven KYC
Start with document cataloging, then perform KYC verification. Documents provide evidence for entity relationships.

### Pattern 2: ISDA Legal Framework Setup
Catalog legal documents, establish master agreement, set up CSA. Documents serve as legal foundation for derivative trading.

### Pattern 3: Cross-Domain Audit Trail
Each verb can reference documents, entities, and other workflow artifacts. Complete traceability from legal documents to business decisions.

### Pattern 4: Regulatory Reporting
Use `document.query` to extract data across multiple domains. Support for EMIR, Dodd-Frank, MiFID II reporting requirements.

### Pattern 5: AI-Assisted Processing
`document.extract` supports AI-powered data extraction with confidence scoring and validation against AttributeID dictionary.

## Migration from v3.0 to v3.1

### NEW Features in v3.1

**Document Library Domain:**
- 8 new document lifecycle management verbs
- AI-ready metadata and extraction patterns
- Rich document cataloging with AttributeID integration

**ISDA Derivative Domain:**
- 12 new verbs for complete derivative workflows
- Full trade lifecycle support from master agreement to close-out
- Comprehensive margin and collateral management

**Enhanced Integration:**
- Cross-domain entity and document references
- Complete audit trail capabilities
- Extended currency and datetime support

### UNCHANGED from v3.0

- Core Clojure-style syntax
- Unified `(verb :key value)` form structure
- AttributeID-as-Type pattern
- Homoiconicity and composability
- All existing KYC, UBO, Compliance, and Graph verbs

## Implementation Notes

### Database Integration

- **Grammar Rules Table**: Currently empty in database (`"ob-poc".grammar_rules`)
- **Domain Vocabularies Table**: Currently empty in database (`"ob-poc".domain_vocabularies`)
- **Current Source**: Grammar loaded from `DSL_GRAMMAR_EXPORT_V3.1.ebnf` file
- **Parser Implementation**: Rust NOM-based parser in `rust/src/parser/idiomatic_parser.rs`

### Runtime Grammar Management

- **Grammar Engine**: `rust/src/grammar/mod.rs` - loads EBNF from external files
- **EBNF Parser**: `rust/src/grammar/idiomatic_ebnf.rs` - parses EBNF grammar files
- **Dynamic Loading**: Support for runtime grammar updates via database

### Future Enhancements

- Populate database grammar tables for dynamic grammar management
- Enhanced domain-specific constraint validation
- Extended AI integration for natural language to DSL conversion
- Additional financial domain support (Trade Finance, Cash Management, etc.)

---

**Generated**: 2025-11-11
**Based on**: DSL Grammar v3.1 from `DSL_GRAMMAR_EXPORT_V3.1.ebnf`
**Database Status**: Grammar tables empty - using file-based grammar
**Total Verbs**: 70+ across 8 domains (Core, Document Library, ISDA Derivatives, KYC, UBO, Compliance, Case Management, Workflow)