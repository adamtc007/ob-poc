# Custody Onboarding DSL Verb Reference

## cbu domain

### cbu.ensure
Create or update a CBU by natural key (name + jurisdiction).
```clojure
(cbu.ensure 
  :name "string" 
  :jurisdiction "LU|IE|US|GB|..." 
  :client-type "FUND|CORPORATE|INDIVIDUAL" 
  :product-id @product              ;; optional, FK to products
  :commercial-client-entity-id @entity  ;; optional, head office entity
  :as @variable)
```

## cbu-custody domain (Three-Layer Model)

### Layer 1: Universe

#### cbu-custody.add-universe
Declare what a CBU trades (instrument class + market + currencies).
```clojure
(cbu-custody.add-universe 
  :cbu-id @cbu 
  :instrument-class "EQUITY|EQUITY_ADR|EQUITY_ETF|GOVT_BOND|CORP_BOND|FIXED_INCOME|MONEY_MARKET|OTC_IRS|OTC_CDS|OTC_FX|OTC_EQD|FX_SPOT|FX_FORWARD"
  :market "XNYS|XNAS|XLON|XETR|XPAR|XAMS|XTKS|XHKG|XSWX|XASX|XSES|XTSE"  ;; optional for OTC
  :currencies ["USD" "GBP" "EUR"]
  :settlement-types ["DVP" "FOP" "RVP"]  ;; optional, default ["DVP"]
  :counterparty @entity                   ;; optional, for OTC counterparty-specific
  :is-held true                           ;; optional, default true
  :is-traded true)                        ;; optional, default true
```

#### cbu-custody.list-universe
List CBU's traded universe.
```clojure
(cbu-custody.list-universe :cbu-id @cbu)
```

### Layer 2: Standing Settlement Instructions (SSIs)

#### cbu-custody.create-ssi
Create Standing Settlement Instruction (pure account data).
```clojure
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "US Primary"
  :type "SECURITIES|CASH|COLLATERAL|FX_NOSTRO"
  :safekeeping-account "SAFE-001"         ;; securities account number
  :safekeeping-bic "BABOROCP"             ;; custodian BIC
  :safekeeping-name "Account Name"        ;; optional
  :cash-account "CASH-001"                ;; cash account number
  :cash-bic "BABOROCP"                    ;; cash agent BIC
  :cash-currency "USD"                    ;; settlement currency
  :collateral-account "COLL-001"          ;; optional, for collateral SSIs
  :collateral-bic "BABOROCP"              ;; optional
  :pset-bic "DTCYUS33"                    ;; place of settlement BIC
  :effective-date "2024-12-01"
  :as @ssi)
```

#### cbu-custody.activate-ssi
Activate an SSI (sets status to ACTIVE).
```clojure
(cbu-custody.activate-ssi :ssi-id @ssi)
```

#### cbu-custody.suspend-ssi
Suspend an SSI (sets status to SUSPENDED).
```clojure
(cbu-custody.suspend-ssi :ssi-id @ssi)
```

#### cbu-custody.list-ssis
List SSIs for a CBU.
```clojure
(cbu-custody.list-ssis 
  :cbu-id @cbu 
  :status "PENDING|ACTIVE|SUSPENDED"  ;; optional filter
  :type "SECURITIES|CASH|COLLATERAL|FX_NOSTRO")  ;; optional filter
```

### Layer 3: Booking Rules (ALERT-style routing)

#### cbu-custody.add-booking-rule
Add routing rule. NULL criteria = wildcard (matches any). Lower priority = higher precedence.
```clojure
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi
  :name "Rule Name"
  :priority 10                        ;; lower = higher priority
  :instrument-class "EQUITY"          ;; optional, NULL = any
  :security-type "COMMON"             ;; optional, ALERT security type
  :market "XNYS"                      ;; optional, NULL = any
  :currency "USD"                     ;; optional, NULL = any
  :settlement-type "DVP"              ;; optional, NULL = any
  :counterparty @entity               ;; optional, for OTC
  :isda-asset-class "RATES"           ;; optional, for OTC
  :isda-base-product "IRS"            ;; optional, for OTC
  :effective-date "2024-12-01")
```

#### cbu-custody.list-booking-rules
List booking rules for a CBU (ordered by priority).
```clojure
(cbu-custody.list-booking-rules 
  :cbu-id @cbu 
  :is-active true)  ;; optional filter
```

#### cbu-custody.update-rule-priority
Update booking rule priority.
```clojure
(cbu-custody.update-rule-priority :rule-id @rule :priority 20)
```

#### cbu-custody.deactivate-rule
Deactivate a booking rule.
```clojure
(cbu-custody.deactivate-rule :rule-id @rule)
```

### Validation & Lookup

#### cbu-custody.validate-booking-coverage
Validate all universe entries have matching booking rules.
```clojure
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
```

#### cbu-custody.derive-required-coverage
Compare universe to booking rules, find gaps.
```clojure
(cbu-custody.derive-required-coverage :cbu-id @cbu)
```

#### cbu-custody.lookup-ssi
Find SSI for given trade characteristics (simulate ALERT lookup).
```clojure
(cbu-custody.lookup-ssi
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :security-type "COMMON"         ;; optional
  :market "XNYS"                  ;; optional
  :currency "USD"
  :settlement-type "DVP"          ;; optional
  :counterparty-bic "MLOIUS33")   ;; optional, for OTC
```

## isda domain (OTC Derivatives)

### isda.create
Create ISDA master agreement with counterparty.
```clojure
(isda.create
  :cbu-id @cbu
  :counterparty @entity
  :agreement-date "2024-12-01"
  :governing-law "NY|ENGLISH"
  :effective-date "2024-12-01"
  :as @isda)
```

### isda.add-coverage
Add instrument class coverage to ISDA.
```clojure
(isda.add-coverage 
  :isda-id @isda 
  :instrument-class "OTC_IRS|OTC_CDS|OTC_FX|OTC_EQD"
  :isda-taxonomy @taxonomy)  ;; optional, FK to isda_product_taxonomy
```

### isda.add-csa
Add Credit Support Annex for collateral management.
```clojure
(isda.add-csa
  :isda-id @isda
  :csa-type "VM|IM"              ;; Variation Margin or Initial Margin
  :threshold 250000              ;; optional, threshold amount
  :threshold-currency "USD"      ;; optional
  :mta 50000                     ;; optional, minimum transfer amount
  :collateral-ssi @ssi           ;; optional, SSI for collateral movements
  :effective-date "2024-12-01"
  :as @csa)
```

### isda.list
List ISDA agreements for CBU.
```clojure
(isda.list 
  :cbu-id @cbu 
  :counterparty @entity)  ;; optional filter
```

## entity-settlement domain (Counterparty SSIs from ALERT)

### entity-settlement.set-identity
Set primary settlement identity for an entity (BIC, LEI, ALERT ID).
```clojure
(entity-settlement.set-identity
  :entity-id @entity
  :bic "MLOIUS33"                ;; primary BIC
  :lei "549300EXAMPLE000001"     ;; optional, LEI code
  :alert-id "ML001"              ;; optional, ALERT participant ID
  :ctm-id "CTM-ML-001"           ;; optional, CTM participant ID
  :as @identity)
```

### entity-settlement.add-ssi
Add counterparty SSI (from ALERT or manual entry).
```clojure
(entity-settlement.add-ssi
  :entity-id @entity
  :instrument-class "EQUITY"      ;; optional
  :security-type "COMMON"         ;; optional
  :market "XNYS"                  ;; optional
  :currency "USD"                 ;; optional
  :counterparty-bic "MLOIUS33"
  :safekeeping-account "12345"    ;; optional
  :source "ALERT|MANUAL|CTM"      ;; optional, default ALERT
  :source-reference "ALERT-REF"   ;; optional
  :effective-date "2024-12-01"
  :as @entity-ssi)
```

## subcustodian domain (Bank's Sub-custodian Network)

### subcustodian.ensure
Create or update sub-custodian entry for market/currency.
```clojure
(subcustodian.ensure
  :market "XNYS"                     ;; MIC code
  :currency "USD"
  :subcustodian-bic "BABOROCP"
  :subcustodian-name "BNP Paribas"   ;; optional
  :local-agent-bic "CITIUS33"        ;; optional
  :local-agent-account "12345"       ;; optional
  :pset "DTCYUS33"                   ;; place of settlement BIC
  :csd-participant "DTC-001"         ;; optional, CSD participant ID
  :is-primary true                   ;; optional, default true
  :effective-date "2024-12-01"
  :as @network)
```

### subcustodian.list-by-market
List sub-custodian entries for a market.
```clojure
(subcustodian.list-by-market 
  :market "XNYS" 
  :currency "USD")  ;; optional filter
```

### subcustodian.lookup
Find sub-custodian for market/currency.
```clojure
(subcustodian.lookup 
  :market "XNYS" 
  :currency "USD" 
  :as-of-date "2024-12-01")  ;; optional
```

## entity domain

### entity.create-limited-company
Create a company entity.
```clojure
(entity.create-limited-company 
  :name "Acme Corp" 
  :jurisdiction "US" 
  :company-number "12345"  ;; optional, registration number
  :as @company)
```

### entity.create-proper-person
Create a natural person entity.
```clojure
(entity.create-proper-person 
  :first-name "John" 
  :last-name "Smith" 
  :date-of-birth "1980-01-15"  ;; optional
  :nationality "US"             ;; optional
  :as @person)
```

### entity.read
Read an entity by ID.
```clojure
(entity.read :entity-id @entity)
```

### entity.list
List entities with filters.
```clojure
(entity.list 
  :entity-type "limited_company|proper_person|trust|partnership"
  :jurisdiction "US"
  :limit 100
  :offset 0)
```

## Reference Data Domains

### instrument-class.ensure
Create or update instrument class with taxonomy mappings.
```clojure
(instrument-class.ensure
  :code "EQUITY"
  :name "Equities"
  :settlement-cycle "T+2"
  :swift-family "MT54x"
  :cfi-category "E"
  :smpg-group "EQUITIES"
  :isda-asset-class "EQUITY"
  :requires-isda false
  :parent "CASH_SECURITIES"  ;; optional, parent class code
  :as @class)
```

### market.ensure
Create or update market reference.
```clojure
(market.ensure
  :mic "XNYS"
  :name "New York Stock Exchange"
  :country-code "US"
  :primary-currency "USD"
  :csd-bic "DTCYUS33"
  :timezone "America/New_York"
  :as @market)
```

### security-type.ensure
Create or update ALERT security type.
```clojure
(security-type.ensure
  :class "EQUITY"         ;; parent instrument class
  :code "COMMON"
  :name "Common Stock"
  :cfi-pattern "ES*"      ;; optional, CFI pattern
  :as @sectype)
```

## Document Types for Custody Onboarding

### Banking Documents
| Type Code | Description |
|-----------|-------------|
| ACCOUNT_OPENING_FORM | Account opening form |
| ACCOUNT_MANDATE | Account mandate / signing authorities |
| ACCOUNT_CONTROL_AGREEMENT | Account control agreement |

### Corporate Documents (for entity verification)
| Type Code | Description |
|-----------|-------------|
| CERT_OF_INCORPORATION | Certificate of Incorporation |
| ARTICLES_OF_ASSOCIATION | Articles of Association / Bylaws |
| REGISTER_OF_DIRECTORS | Register of Directors |
| REGISTER_OF_SHAREHOLDERS | Register of Shareholders |
| BOARD_RESOLUTION | Board Resolution (for account opening) |
| SPECIMEN_SIGNATURES | Specimen Signature Card |
| SIGNATORY_LIST | Authorized Signatory List |
| LEI_CERTIFICATE | LEI Certificate |

### ISDA Documents
| Type Code | Description |
|-----------|-------------|
| MIFID_CLASSIFICATION | MiFID II Classification Letter |
| EMIR_CLASSIFICATION | EMIR Classification Letter |

### Regulatory Documents
| Type Code | Description |
|-----------|-------------|
| REGULATORY_LICENSE | Regulatory License |
| FCA_REGISTER_EXTRACT | FCA Register Extract |
| SEC_REGISTRATION | SEC Registration |
| CFTC_REGISTRATION | CFTC Registration |
| POWER_OF_ATTORNEY | Power of Attorney |

## Database Tables (custody schema)

| Table | Purpose |
|-------|---------|
| cbu_instrument_universe | What CBU trades (Layer 1) |
| cbu_ssi | Standing Settlement Instructions (Layer 2) |
| cbu_ssi_agent_override | Agent overrides for SSIs |
| ssi_booking_rules | ALERT-style routing rules (Layer 3) |
| isda_agreements | ISDA master agreements |
| isda_product_coverage | Instrument coverage under ISDA |
| isda_product_taxonomy | ISDA product taxonomy codes |
| csa_agreements | Credit Support Annexes |
| entity_settlement_identity | Entity BIC/LEI for settlement |
| entity_ssi | Counterparty SSIs from ALERT |
| subcustodian_network | Bank's sub-custodian network |
| instrument_classes | Instrument classification (CFI/SMPG/ISDA) |
| security_types | ALERT security type codes |
| markets | ISO 10383 MIC market codes |
