# Product-Service-Resource Taxonomy Peer Review Extract - 2026-05-03

## Purpose

This document extracts the current DB, SemOS, DAG, and DSL state for external business peer review. The intended reviewer has stronger product/service/resource business knowledge than the sandbox implementation agent.

The review target is not only technical validity. It is the business taxonomy that resolves:

- Product: commercial artifact subscribed to by a CBU.
- Service: generic lifecycle/capability that defines what a product requires.
- Service resource: proprietary implementation resource, application, account, connection, platform, or operational artifact required to deliver the service.
- Attribute dictionary: required facts for provisioning or validating those resources.

Architectural invariant for this review:

- SemOS may be cyclic, navigable, and stateful.
- The compiled REPL workbook must be acyclic.
- The DAG taxonomy is the shared semantic precursor that lets SemOS discovery and compiler DAG building use the same truth.
- Product/service/resource taxonomy is design-time catalog truth; CBU, instrument matrix, market, counterparty, jurisdiction, and product options determine what is activated for a specific CBU.

## Source Inventory

Live DB extract source:

- Database URL used: `postgresql:///data_designer`
- Schemas inspected: `"ob-poc"`, `sem_reg`, `sem_os_runtime`

Relevant YAML/DSL sources:

- `rust/config/sem_os_seeds/dag_taxonomies/product_service_taxonomy_dag.yaml`
- `rust/config/sem_os_seeds/constellation_maps/product_service_taxonomy.yaml`
- `rust/config/sem_os_seeds/constellation_families/product_service_taxonomy.yaml`
- `rust/config/packs/product-service-taxonomy.yaml`
- `rust/config/verbs/product.yaml`
- `rust/config/verbs/service.yaml`
- `rust/config/verbs/service-resource.yaml`
- `rust/config/verbs/service-pipeline.yaml`
- `rust/config/srdefs/custody.yaml`
- `rust/config/srdefs/connectivity.yaml`
- `rust/config/srdefs/iam.yaml`

## DB Tables And Row Counts

| Table | Rows |
| --- | ---: |
| `products` | 9 |
| `services` | 30 |
| `product_services` | 32 |
| `service_resource_types` | 36 |
| `service_resource_capabilities` | 13 |
| `resource_attribute_requirements` | 34 |
| `resource_dependencies` | 9 |
| `service_availability` | 23 |
| `service_delivery_map` | 278 |
| `service_intents` | 22 |
| `cbu_resource_instances` | 2322 |
| `cbu_service_readiness` | 0 |
| `cbu_service_consumption` | 0 |
| `lifecycle_resource_types` | 0 |
| `lifecycle_resource_capabilities` | 0 |

Initial business-data concerns:

- `product_services` exists and is populated, but all current rows have `is_mandatory = false`, `is_default = false`, no `display_order`, and no `configuration`.
- Only 13 active `service_resource_capabilities` rows exist for 30 active services.
- `service_resource_types` has 36 active rows, but 10 have blank `resource_type`, all 36 have blank `dictionary_group`, all 36 have blank `resource_purpose`, and all 36 have `per_market = false`, `per_currency = false`, `per_counterparty = false`.
- `lifecycle_resource_types` and `lifecycle_resource_capabilities` are empty, despite the lifecycle-resource workspace.
- Runtime service readiness and service consumption tables are empty.

## Products

| Product code | Name | Category | Framework | Family | Active |
| --- | --- | --- | --- | --- | --- |
| blank | CUSTODY | CORE | blank | blank | false |
| blank | FUND_ACCOUNTING | CORE | blank | blank | false |
| ALTS | Alternatives | INVESTMENT_SERVICES | blank | alternatives | true |
| COLLATERAL_MGMT | Collateral Management | collateral | blank | collateral | true |
| CUSTODY | Custody | custody | blank | custody_services | true |
| FUND_ACCOUNTING | Fund Accounting | fund_services | blank | fund_services | true |
| MARKETS_FX | Markets FX | markets | blank | markets | true |
| MIDDLE_OFFICE | Middle Office | operations | blank | middle_office | true |
| TRANSFER_AGENCY | Transfer Agency | fund_services | blank | fund_services | true |

Active product service counts:

| Product code | Product | Service count |
| --- | --- | ---: |
| ALTS | Alternatives | 2 |
| COLLATERAL_MGMT | Collateral Management | 2 |
| CUSTODY | Custody | 11 |
| FUND_ACCOUNTING | Fund Accounting | 7 |
| MARKETS_FX | Markets FX | 1 |
| MIDDLE_OFFICE | Middle Office | 4 |
| TRANSFER_AGENCY | Transfer Agency | 5 |

## Services

| Service code | Name | Category | Lifecycle tags | Active |
| --- | --- | --- | --- | --- |
| ASSET_PRICING | Asset Pricing | Valuation | `{}` | true |
| CAPSTOCK_AUTO | CapStock Automation | Corporate Actions | `{}` | true |
| CASH_MGMT | Cash Management | Treasury | `{core}` | true |
| COLLATERAL_MGMT | Collateral Management | Operations | `{}` | true |
| CORP_ACTIONS | Corporate Actions | Operations | `{corporate_actions}` | true |
| EXPENSE_MGMT | Expense Management | Accounting | `{}` | true |
| FUND_REPORTING | Fund Reporting | Reporting | `{}` | true |
| FX_EXECUTION | FX Execution | Trading | `{}` | true |
| HEDGE_FUND_ACCOUNTING | Hedge Fund Accounting | FUND_SERVICES | `{}` | true |
| HEDGE_FUND_TA | Hedge Fund TA | TRANSFER_AGENCY | `{}` | true |
| INCOME_COLLECT | Income Collection | Operations | `{}` | true |
| INVESTOR_ACCT | Investor Accounting | Accounting | `{investor_services}` | true |
| INVESTOR_REG | Investor Register | Transfer Agency | `{investor_services,regulatory}` | true |
| KYC_SERVICE | KYC as a Service | Compliance | `{}` | true |
| MANCO_REPORTING | ManCo Reporting | Reporting | `{}` | true |
| MIFID_REG | MiFID Regulatory | Regulatory | `{}` | true |
| NAV_CALC | NAV Calculation | Valuation | `{core,valuation}` | true |
| NAV_DISSEM | NAV Dissemination | Valuation | `{}` | true |
| PERF_MEASURE | Performance Measurement | Analytics | `{}` | true |
| POSITION_MGMT | Position Management | IBOR | `{}` | true |
| PROXY_VOTING | Proxy Voting | Governance | `{}` | true |
| RECON_POSITIONS | Positions Reconciliation | Reconciliation | `{}` | true |
| RECON_TRANSACTIONS | Transactions Reconciliation | Reconciliation | `{}` | true |
| REG_REPORTING | Regulatory Reporting | Reporting | `{reporting,regulatory}` | true |
| REPORTING | Client Reporting | reporting | `{reporting,regulatory}` | true |
| SAFEKEEPING | Asset Safekeeping | Custody | `{core,regulatory}` | true |
| SETTLEMENT | Trade Settlement | Settlement | `{core}` | true |
| TRADE_CAPTURE | Trade Capture | IBOR | `{}` | true |
| VAR_MARGIN | Variation Margining | Collateral | `{}` | true |
| WITHHOLD_TAX | Withholding Tax | Tax | `{}` | true |

## Product To Service Map

All rows below currently have `is_mandatory = false`, `is_default = false`, blank `display_order`, and blank `configuration`.

| Product | Service |
| --- | --- |
| ALTS - Alternatives | HEDGE_FUND_ACCOUNTING - Hedge Fund Accounting |
| ALTS - Alternatives | HEDGE_FUND_TA - Hedge Fund TA |
| COLLATERAL_MGMT - Collateral Management | COLLATERAL_MGMT - Collateral Management |
| COLLATERAL_MGMT - Collateral Management | VAR_MARGIN - Variation Margining |
| CUSTODY - Custody | CASH_MGMT - Cash Management |
| CUSTODY - Custody | CORP_ACTIONS - Corporate Actions |
| CUSTODY - Custody | INCOME_COLLECT - Income Collection |
| CUSTODY - Custody | MIFID_REG - MiFID Regulatory |
| CUSTODY - Custody | PROXY_VOTING - Proxy Voting |
| CUSTODY - Custody | RECON_POSITIONS - Positions Reconciliation |
| CUSTODY - Custody | RECON_TRANSACTIONS - Transactions Reconciliation |
| CUSTODY - Custody | REG_REPORTING - Regulatory Reporting |
| CUSTODY - Custody | SAFEKEEPING - Asset Safekeeping |
| CUSTODY - Custody | SETTLEMENT - Trade Settlement |
| CUSTODY - Custody | WITHHOLD_TAX - Withholding Tax |
| FUND_ACCOUNTING - Fund Accounting | ASSET_PRICING - Asset Pricing |
| FUND_ACCOUNTING - Fund Accounting | EXPENSE_MGMT - Expense Management |
| FUND_ACCOUNTING - Fund Accounting | FUND_REPORTING - Fund Reporting |
| FUND_ACCOUNTING - Fund Accounting | NAV_CALC - NAV Calculation |
| FUND_ACCOUNTING - Fund Accounting | NAV_DISSEM - NAV Dissemination |
| FUND_ACCOUNTING - Fund Accounting | PERF_MEASURE - Performance Measurement |
| FUND_ACCOUNTING - Fund Accounting | REG_REPORTING - Regulatory Reporting |
| MARKETS_FX - Markets FX | FX_EXECUTION - FX Execution |
| MIDDLE_OFFICE - Middle Office | POSITION_MGMT - Position Management |
| MIDDLE_OFFICE - Middle Office | REG_REPORTING - Regulatory Reporting |
| MIDDLE_OFFICE - Middle Office | REPORTING - Client Reporting |
| MIDDLE_OFFICE - Middle Office | TRADE_CAPTURE - Trade Capture |
| TRANSFER_AGENCY - Transfer Agency | CAPSTOCK_AUTO - CapStock Automation |
| TRANSFER_AGENCY - Transfer Agency | INVESTOR_ACCT - Investor Accounting |
| TRANSFER_AGENCY - Transfer Agency | INVESTOR_REG - Investor Register |
| TRANSFER_AGENCY - Transfer Agency | KYC_SERVICE - KYC as a Service |
| TRANSFER_AGENCY - Transfer Agency | MANCO_REPORTING - ManCo Reporting |

Peer-review issue: this table identifies candidate services, but it does not encode which services are mandatory, default, optional, jurisdiction-dependent, fund-type-dependent, instrument-matrix-dependent, market-dependent, or counterparty-dependent.

## Service To Resource Capability Map

| Service | Resource | Resource type | Required | Priority | Supported options |
| --- | --- | --- | --- | ---: | --- |
| CORP_ACTIONS - Corporate Actions | CA_PLATFORM - Corporate Actions Platform | platform | true | 100 | `{}` |
| FUND_REPORTING - Fund Reporting | REPORTING_HUB - Reporting Hub | platform | true | 100 | `{}` |
| INCOME_COLLECT - Income Collection | SWIFT_CONN - SWIFT Connection | connection | true | 100 | `{}` |
| INVESTOR_ACCT - Investor Accounting | INVESTOR_LEDGER - Investor Ledger | application | true | 100 | `{}` |
| NAV_CALC - NAV Calculation | NAV_ENGINE - NAV Calculation Engine | application | true | 100 | `{}` |
| POSITION_MGMT - Position Management | IBOR_SYSTEM - IBOR System | application | true | 100 | `{}` |
| SAFEKEEPING - Asset Safekeeping | CUSTODY_ACCT - Custody Account | account | true | 100 | `{}` |
| SETTLEMENT - Trade Settlement | APAC_CLEAR - APAC Clearinghouse | settlement_system | true | 80 | `{"speed":["T2"],"markets":["APAC_EQUITY"]}` |
| SETTLEMENT - Trade Settlement | EUROCLEAR - Euroclear Settlement | settlement_system | true | 90 | `{"speed":["T1","T2"],"markets":["EU_EQUITY"]}` |
| SETTLEMENT - Trade Settlement | DTCC_SETTLE - DTCC Settlement System | settlement_system | true | 100 | `{"speed":["T0","T1","T2"],"markets":["US_EQUITY"]}` |
| SETTLEMENT - Trade Settlement | SETTLE_ACCT - Settlement Account | account | true | 100 | `{}` |
| SETTLEMENT - Trade Settlement | SWIFT_CONN - SWIFT Connection | connection | true | 100 | `{}` |
| TRADE_CAPTURE - Trade Capture | IBOR_SYSTEM - IBOR System | application | true | 100 | `{}` |

Active services with no active resource capability rows:

`ASSET_PRICING`, `CAPSTOCK_AUTO`, `CASH_MGMT`, `COLLATERAL_MGMT`, `EXPENSE_MGMT`, `FX_EXECUTION`, `HEDGE_FUND_ACCOUNTING`, `HEDGE_FUND_TA`, `INVESTOR_REG`, `KYC_SERVICE`, `MANCO_REPORTING`, `MIFID_REG`, `NAV_DISSEM`, `PERF_MEASURE`, `PROXY_VOTING`, `RECON_POSITIONS`, `RECON_TRANSACTIONS`, `REG_REPORTING`, `REPORTING`, `VAR_MARGIN`, `WITHHOLD_TAX`.

Peer-review issue: settlement has some option metadata, but most services do not yet resolve to concrete resources.

## Service Resource Type Catalog

All active resource rows currently have `per_market = false`, `per_currency = false`, and `per_counterparty = false`.

| Resource code | Name | Type | Owner | Vendor | SRDEF id | Active |
| --- | --- | --- | --- | --- | --- | --- |
| ALERT_CONNECTION | ALERT SSI Enrichment | CONNECTIVITY | DTCC | blank | SRDEF::DTCC::CONNECTIVITY::ALERT_CONNECTION | true |
| ALTS_GENEVA | Alts Geneva | blank | Operations | blank | SRDEF::Operations::Resource::ALTS_GENEVA | true |
| ALTS_PRADO | Alts Prado | blank | Operations | blank | SRDEF::Operations::Resource::ALTS_PRADO | true |
| APAC_CLEAR | APAC Clearinghouse | settlement_system | Operations | ASX | SRDEF::Operations::settlement_system::APAC_CLEAR | true |
| API_ENDPOINT | REST/gRPC API Endpoint | CONNECTIVITY | CLIENT | blank | SRDEF::CLIENT::CONNECTIVITY::API_ENDPOINT | true |
| BLOOMBERG_BVAL | Bloomberg BVAL | PRICING | BLOOMBERG | blank | SRDEF::BLOOMBERG::PRICING::BLOOMBERG_BVAL | true |
| BLOOMBERG_TERMINAL | Bloomberg Terminal Feed | PRICING | BLOOMBERG | blank | SRDEF::BLOOMBERG::PRICING::BLOOMBERG_TERMINAL | true |
| CA_PLATFORM | Corporate Actions Platform | platform | Operations | Internal | SRDEF::Operations::platform::CA_PLATFORM | true |
| CASH_SWEEP_ENGINE | Cash Sweep Engine | CASH_MANAGEMENT | BNY | blank | SRDEF::BNY::CASH_MANAGEMENT::CASH_SWEEP_ENGINE | true |
| COLLATERAL_GLOBAL1 | Collateral Global1 | blank | Operations | blank | SRDEF::Operations::Resource::COLLATERAL_GLOBAL1 | true |
| CSD_GATEWAY | CSD Direct Connection | SETTLEMENT | BNY | blank | SRDEF::BNY::SETTLEMENT::CSD_GATEWAY | true |
| CTM_CONNECTION | CTM Trade Matching | CONNECTIVITY | DTCC | blank | SRDEF::DTCC::CONNECTIVITY::CTM_CONNECTION | true |
| CUSTODY_ACCT | Custody Account | account | Operations | Internal | SRDEF::Operations::account::CUSTODY_ACCT | true |
| CUSTODY_GSP | Custody GSP | blank | Operations | blank | SRDEF::Operations::Resource::CUSTODY_GSP | true |
| CUSTODY_IMMS | Custody IMMS | blank | Operations | blank | SRDEF::Operations::Resource::CUSTODY_IMMS | true |
| CUSTODY_SMARTSTREAM | Custody SmartStream | blank | Operations | blank | SRDEF::Operations::Resource::CUSTODY_SMARTSTREAM | true |
| CUSTODY_SWIFT | Custody SWIFT | blank | Operations | blank | SRDEF::Operations::Resource::CUSTODY_SWIFT | true |
| DTCC_SETTLE | DTCC Settlement System | settlement_system | Operations | DTCC | SRDEF::Operations::settlement_system::DTCC_SETTLE | true |
| EUROCLEAR | Euroclear Settlement | settlement_system | Operations | Euroclear | SRDEF::Operations::settlement_system::EUROCLEAR | true |
| FA_EAGLE | FA Eagle | blank | Operations | blank | SRDEF::Operations::Resource::FA_EAGLE | true |
| FA_INVESTONE | FA InvestOne | blank | Operations | blank | SRDEF::Operations::Resource::FA_INVESTONE | true |
| FIX_SESSION | FIX Protocol Session | CONNECTIVITY | CLIENT | blank | SRDEF::CLIENT::CONNECTIVITY::FIX_SESSION | true |
| IBOR_SYSTEM | IBOR System | application | Middle Office | Internal | SRDEF::Middle Office::application::IBOR_SYSTEM | true |
| ICE_PRICING | ICE Data Services | PRICING | ICE | blank | SRDEF::ICE::PRICING::ICE_PRICING | true |
| INVESTOR_LEDGER | Investor Ledger | application | Fund Services | Internal | SRDEF::Fund Services::application::INVESTOR_LEDGER | true |
| MARKIT_PRICING | Markit Pricing Service | PRICING | MARKIT | blank | SRDEF::MARKIT::PRICING::MARKIT_PRICING | true |
| NAV_ENGINE | NAV Calculation Engine | application | Fund Services | Internal | SRDEF::Fund Services::application::NAV_ENGINE | true |
| PNL_ENGINE | P&L Engine | application | Middle Office | Internal | SRDEF::Middle Office::application::PNL_ENGINE | true |
| REFINITIV_FEED | Refinitiv Real-Time Feed | PRICING | REFINITIV | blank | SRDEF::REFINITIV::PRICING::REFINITIV_FEED | true |
| REPORTING_HUB | Reporting Hub | platform | Technology | Internal | SRDEF::Technology::platform::REPORTING_HUB | true |
| RUFUS_TA | Rufus TA | blank | Operations | blank | SRDEF::Operations::Resource::RUFUS_TA | true |
| SETTLE_ACCT | Settlement Account | account | Operations | Multi-CSD | SRDEF::Operations::account::SETTLE_ACCT | true |
| SETTLEMENT_INSTRUCTION_ENGINE | Settlement Instruction Generator | SETTLEMENT | BNY | blank | SRDEF::BNY::SETTLEMENT::SETTLEMENT_INSTRUCTION_ENGINE | true |
| STIF_ACCOUNT | Short-Term Investment Fund | CASH_MANAGEMENT | BNY | blank | SRDEF::BNY::CASH_MANAGEMENT::STIF_ACCOUNT | true |
| SWIFT_CONN | SWIFT Connection | connection | Technology | SWIFT | SRDEF::Technology::connection::SWIFT_CONN | true |
| SWIFT_GATEWAY | SWIFT Message Gateway | CONNECTIVITY | BNY | blank | SRDEF::BNY::CONNECTIVITY::SWIFT_GATEWAY | true |

Peer-review issue: the DB catalog has implementation assets, but does not yet encode the dimensionality the business requirement calls out: market, currency, counterparty, instruction channel, investment manager BIC, reconciliation party, and similar service options.

## Resource Dependencies

| Resource | Depends on | Type | Inject arg | Priority | Active |
| --- | --- | --- | --- | ---: | --- |
| CA_PLATFORM | CUSTODY_ACCT | required | custody-account-url | 100 | true |
| CUSTODY_ACCT | SETTLE_ACCT | required | settlement-account-url | 100 | true |
| DTCC_SETTLE | SETTLE_ACCT | required | settlement-account-url | 100 | true |
| EUROCLEAR | SETTLE_ACCT | required | settlement-account-url | 100 | true |
| IBOR_SYSTEM | CUSTODY_ACCT | required | custody-account-url | 100 | true |
| INVESTOR_LEDGER | NAV_ENGINE | required | nav-engine-url | 100 | true |
| NAV_ENGINE | CUSTODY_ACCT | required | custody-account-url | 100 | true |
| REPORTING_HUB | CUSTODY_ACCT | required | custody-account-url | 100 | true |
| SWIFT_CONN | CUSTODY_ACCT | required | custody-account-url | 100 | true |

## Resource Attribute Requirements

| Resource | Attribute | Mandatory | Requirement type | Source policy |
| --- | --- | --- | --- | --- |
| APAC_CLEAR | account_number | true | required | `["derived","entity","cbu","document","manual"]` |
| APAC_CLEAR | custodian_code | true | required | `["derived","entity","cbu","document","manual"]` |
| APAC_CLEAR | settlement_currency | false | required | `["derived","entity","cbu","document","manual"]` |
| CUSTODY_ACCT | resource.account.account_number | true | required | `["derived","entity","cbu","document","manual"]` |
| CUSTODY_ACCT | resource.account.account_name | true | required | `["derived","entity","cbu","document","manual"]` |
| CUSTODY_ACCT | resource.account.base_currency | true | required | `["derived","entity","cbu","document","manual"]` |
| CUSTODY_ACCT | resource.account.account_type | true | required | `["derived","entity","cbu","document","manual"]` |
| CUSTODY_ACCT | resource.account.sub_custodian | false | required | `["derived","entity","cbu","document","manual"]` |
| CUSTODY_ACCT | resource.account.market_codes | false | required | `["derived","entity","cbu","document","manual"]` |
| DTCC_SETTLE | account_number | true | required | `["derived","entity","cbu","document","manual"]` |
| DTCC_SETTLE | bic_code | true | required | `["derived","entity","cbu","document","manual"]` |
| DTCC_SETTLE | settlement_currency | false | required | `["derived","entity","cbu","document","manual"]` |
| EUROCLEAR | account_number | true | required | `["derived","entity","cbu","document","manual"]` |
| EUROCLEAR | iban | true | required | `["derived","entity","cbu","document","manual"]` |
| EUROCLEAR | settlement_currency | false | required | `["derived","entity","cbu","document","manual"]` |
| IBOR_SYSTEM | resource.ibor.portfolio_code | true | required | `["derived","entity","cbu","document","manual"]` |
| IBOR_SYSTEM | resource.ibor.accounting_basis | true | required | `["derived","entity","cbu","document","manual"]` |
| IBOR_SYSTEM | resource.account.base_currency | true | required | `["derived","entity","cbu","document","manual"]` |
| IBOR_SYSTEM | resource.ibor.position_source | true | required | `["derived","entity","cbu","document","manual"]` |
| IBOR_SYSTEM | resource.ibor.reconciliation_enabled | false | required | `["derived","entity","cbu","document","manual"]` |
| NAV_ENGINE | resource.fund.fund_code | true | required | `["derived","entity","cbu","document","manual"]` |
| NAV_ENGINE | resource.fund.valuation_frequency | true | required | `["derived","entity","cbu","document","manual"]` |
| NAV_ENGINE | resource.fund.pricing_source | true | required | `["derived","entity","cbu","document","manual"]` |
| NAV_ENGINE | resource.fund.nav_cutoff_time | true | required | `["derived","entity","cbu","document","manual"]` |
| NAV_ENGINE | resource.fund.share_classes | false | required | `["derived","entity","cbu","document","manual"]` |
| SETTLE_ACCT | resource.account.account_number | true | required | `["derived","entity","cbu","document","manual"]` |
| SETTLE_ACCT | resource.settlement.bic_code | true | required | `["derived","entity","cbu","document","manual"]` |
| SETTLE_ACCT | resource.settlement.settlement_currency | true | required | `["derived","entity","cbu","document","manual"]` |
| SETTLE_ACCT | resource.settlement.csd_participant_id | false | required | `["derived","entity","cbu","document","manual"]` |
| SETTLE_ACCT | resource.settlement.netting_enabled | false | required | `["derived","entity","cbu","document","manual"]` |
| SWIFT_CONN | resource.settlement.bic_code | true | required | `["derived","entity","cbu","document","manual"]` |
| SWIFT_CONN | resource.swift.logical_terminal | true | required | `["derived","entity","cbu","document","manual"]` |
| SWIFT_CONN | resource.swift.message_types | true | required | `["derived","entity","cbu","document","manual"]` |
| SWIFT_CONN | resource.swift.rma_status | false | required | `["derived","entity","cbu","document","manual"]` |

Peer-review issue: there are attribute requirements for only 8 resource types. Field-level mapping columns are blank in the DB extract, so these are dictionary attributes rather than explicit resource-field bindings.

## Service Availability

| Service | Regulatory | Commercial | Operational | Delivery model | Count |
| --- | --- | --- | --- | --- | ---: |
| CASH_MGMT | permitted | offered | supported | direct | 4 |
| CASH_MGMT | permitted | offered | supported | sub_custodian | 1 |
| CORP_ACTIONS | permitted | offered | supported | direct | 1 |
| INVESTOR_ACCT | permitted | offered | limited | partner | 1 |
| INVESTOR_ACCT | permitted | offered | supported | direct | 1 |
| NAV_CALC | permitted | not_offered | not_supported | blank | 1 |
| NAV_CALC | permitted | offered | supported | direct | 2 |
| REG_REPORTING | permitted | offered | supported | direct | 1 |
| REPORTING | permitted | offered | supported | direct | 1 |
| SAFEKEEPING | permitted | offered | supported | direct | 4 |
| SAFEKEEPING | permitted | offered | supported | sub_custodian | 1 |
| SETTLEMENT | permitted | offered | supported | direct | 4 |
| SETTLEMENT | permitted | offered | supported | partner | 1 |

Peer-review issue: availability exists for a subset of services and appears booking-principal/jurisdiction aware at table level, but the review should confirm whether the statuses and delivery models are sufficient for fund types, jurisdictions, and service options.

## Runtime Materialization

Service intents:

| Product | Service | Status | Count |
| --- | --- | --- | ---: |
| CUSTODY | CASH_MGMT | active | 2 |
| CUSTODY | CORP_ACTIONS | active | 2 |
| CUSTODY | INCOME_COLLECT | active | 2 |
| CUSTODY | MIFID_REG | active | 2 |
| CUSTODY | PROXY_VOTING | active | 2 |
| CUSTODY | RECON_POSITIONS | active | 2 |
| CUSTODY | RECON_TRANSACTIONS | active | 2 |
| CUSTODY | REG_REPORTING | active | 2 |
| CUSTODY | SAFEKEEPING | active | 2 |
| CUSTODY | SETTLEMENT | active | 2 |
| CUSTODY | WITHHOLD_TAX | active | 2 |

Service delivery map:

| Product | Service | Delivery status | Count |
| --- | --- | --- | ---: |
| ALTS | HEDGE_FUND_ACCOUNTING | PENDING | 3 |
| ALTS | HEDGE_FUND_TA | PENDING | 3 |
| CUSTODY | CASH_MGMT | PENDING | 20 |
| CUSTODY | CORP_ACTIONS | PENDING | 20 |
| CUSTODY | INCOME_COLLECT | PENDING | 20 |
| CUSTODY | MIFID_REG | PENDING | 20 |
| CUSTODY | PROXY_VOTING | PENDING | 20 |
| CUSTODY | RECON_POSITIONS | PENDING | 20 |
| CUSTODY | RECON_TRANSACTIONS | PENDING | 20 |
| CUSTODY | REG_REPORTING | PENDING | 20 |
| CUSTODY | SAFEKEEPING | PENDING | 20 |
| CUSTODY | SETTLEMENT | PENDING | 20 |
| CUSTODY | WITHHOLD_TAX | PENDING | 20 |
| FUND_ACCOUNTING | ASSET_PRICING | PENDING | 6 |
| FUND_ACCOUNTING | EXPENSE_MGMT | PENDING | 6 |
| FUND_ACCOUNTING | FUND_REPORTING | PENDING | 6 |
| FUND_ACCOUNTING | NAV_CALC | PENDING | 6 |
| FUND_ACCOUNTING | NAV_DISSEM | PENDING | 6 |
| FUND_ACCOUNTING | PERF_MEASURE | PENDING | 6 |
| FUND_ACCOUNTING | REG_REPORTING | PENDING | 6 |
| TRANSFER_AGENCY | CAPSTOCK_AUTO | PENDING | 2 |
| TRANSFER_AGENCY | INVESTOR_ACCT | PENDING | 2 |
| TRANSFER_AGENCY | INVESTOR_REG | PENDING | 2 |
| TRANSFER_AGENCY | KYC_SERVICE | PENDING | 2 |
| TRANSFER_AGENCY | MANCO_REPORTING | PENDING | 2 |

Resource instances:

| Resource | Type | Status | Count |
| --- | --- | --- | ---: |
| APAC_CLEAR | settlement_system | PENDING | 295 |
| CA_PLATFORM | platform | PENDING | 291 |
| CUSTODY_ACCT | account | PENDING | 304 |
| DTCC_SETTLE | settlement_system | PENDING | 277 |
| EUROCLEAR | settlement_system | PENDING | 277 |
| FA_EAGLE | blank | PENDING | 4 |
| FA_INVESTONE | blank | PENDING | 1 |
| INVESTOR_LEDGER | application | PENDING | 3 |
| NAV_ENGINE | application | PENDING | 8 |
| REPORTING_HUB | platform | PENDING | 9 |
| RUFUS_TA | blank | PENDING | 2 |
| SETTLE_ACCT | account | PENDING | 282 |
| SWIFT_CONN | connection | PENDING | 569 |

Peer-review issue: runtime materialization exists, but all sampled delivery/resource instance statuses are still `PENDING`; `cbu_service_readiness` and `cbu_service_consumption` are empty.

## SemReg Taxonomy Visibility

`sem_reg.v_active_taxonomy_defs` currently exposes one related active taxonomy:

| Taxonomy FQN | Name | Domain | Root | Max depth | Axis |
| --- | --- | --- | --- | ---: | --- |
| taxonomy.instrument-class | Instrument Classification | trading | taxonomy.instrument-class.root | 3 | `"instrument_class"` |

Visible nodes:

| Taxonomy | Node | Name | Parent | Sort |
| --- | --- | --- | --- | ---: |
| taxonomy.instrument-class | taxonomy.instrument-class.root | All Instruments | blank | 0 |
| taxonomy.instrument-class | taxonomy.instrument-class.equity | Equity | taxonomy.instrument-class.root | 1 |
| taxonomy.instrument-class | taxonomy.instrument-class.fixed-income | Fixed Income | taxonomy.instrument-class.root | 2 |
| taxonomy.instrument-class | taxonomy.instrument-class.derivatives | Derivatives | taxonomy.instrument-class.root | 3 |
| taxonomy.instrument-class | taxonomy.instrument-class.otc | OTC | taxonomy.instrument-class.derivatives | 1 |
| taxonomy.instrument-class | taxonomy.instrument-class.listed | Listed | taxonomy.instrument-class.derivatives | 2 |

Peer-review issue: active SemReg taxonomy views do not currently expose product/service/resource taxonomy definitions. Product/service/resource truth appears to live in `"ob-poc"` catalog tables plus YAML/DAG, not in active `sem_reg` taxonomy rows.

## Product-Service DAG Extract

File: `rust/config/sem_os_seeds/dag_taxonomies/product_service_taxonomy_dag.yaml`

Workspace:

- `workspace: product_maintenance`
- `dag_id: product_service_taxonomy_dag`
- Purpose: product-maintenance workspace for design-time product, service, servicing-resource, and attribute dictionary taxonomy.
- Relationship to CBU: pre-CBU catalog. Downstream workspaces select products and services for specific CBUs. The taxonomy says what is available; the CBU DAG says what is activated.

Overall lifecycle:

- `anchor_selection`: user chooses a product, service, or resource anchor.
- `browsing`: user navigates product to services to resources to attributes, or laterally.

Slots:

| Slot | Stateful | Meaning |
| --- | --- | --- |
| workspace_root | false | Aggregation root for taxonomy exploration |
| product | false | Read-only product catalog view |
| service | true | Service-definition lifecycle |
| service_version | true | Per-version service lifecycle |
| service_resource | false | Read-only servicing resource view |
| attribute | false | Read-only attribute dictionary view |

Service lifecycle:

- Source entity: `"ob-poc".services`
- State column: `lifecycle_status`
- States: `ungoverned`, `draft`, `active`, `deprecated`, `retired`
- Transitions:
  - `ungoverned` to `draft` via `service.define`
  - `draft` to `active` via backend changeset publish
  - `active` to `draft` via `service.propose-revision`
  - `active` to `deprecated` via `service.deprecate`
  - `deprecated` to `retired` via `service.retire`

Service version lifecycle:

- Source entity: `"ob-poc".service_versions`
- State column: `lifecycle_status`
- States: `drafted`, `reviewed`, `published`, `superseded`, `retired`
- Transitions:
  - `drafted` to `reviewed` via `service-version.submit-for-review`
  - `reviewed` to `published` via `service-version.publish`
  - `published` to `superseded` via backend newer published version
  - `published` or `superseded` to `retired` via `service-version.retire`

Cross-slot and cross-workspace constraints are currently empty for this workspace.

Product module gates are always on for:

- `workspace_root`
- `product`
- `service`
- `service_version`
- `service_resource`
- `attribute`

## Constellation Map Extract

File: `rust/config/sem_os_seeds/constellation_maps/product_service_taxonomy.yaml`

Constellation:

- `constellation: product.service.taxonomy`
- Jurisdiction: `ALL`
- Description: design-time catalog view consumed by onboarding and activation flows.

Slots:

| Slot | Table | PK in map | Depends on | Verbs |
| --- | --- | --- | --- | --- |
| product | `products` | `product_id` | none | `product.read`, `product.list` |
| service | `services` | `service_id` | product | `service.read`, `service.list`, `service.list-by-product` |
| service_resource | `service_resource_types` | `resource_type_id` | service | `service-resource.read`, `service-resource.list`, `service-resource.list-by-service`, `service-resource.list-attributes` |
| resource_dictionary | `attribute_registry` | `id` | service_resource | `attribute.read`, `attribute.list` |

Potential technical/business alignment issue:

- DB table `service_resource_types` has primary identifier column `resource_id`, while the constellation map says `pk: resource_type_id` for the `service_resource` slot.
- DB table `resource_attribute_requirements` joins to `service_resource_types.resource_id`, while the map's `resource_dictionary` slot references `attribute_registry` and `resource_type_id`. The reviewer may not care about column naming, but this can affect navigability and should be reconciled.

## Constellation Family Extract

File: `rust/config/sem_os_seeds/constellation_families/product_service_taxonomy.yaml`

- `family_id: product_service_taxonomy`
- Label: Product Service Resource Taxonomy
- Domain: `product`
- Selection rule: always targets `product.service.taxonomy`
- Candidate entity kinds: `product`, `service`, `resource_type`, `attribute`
- Triggers: product, service, resource, service resource, servicing resource, product catalog, service catalog, resource dictionary, attribute dictionary.
- Does not require an entity instance and allows draft instances.

## Journey Pack Extract

File: `rust/config/packs/product-service-taxonomy.yaml`

Pack:

- `id: product-service-taxonomy`
- Workspace: `product_maintenance`
- Required context: `client_group_id`
- Optional context: `product_id`, `service_id`, `resource_id`
- Required question: start from product, service, or resource dictionary.

Allowed verbs:

- `product.list`
- `product.read`
- `service.list`
- `service.read`
- `service.list-by-product`
- `service.list-versions`
- `service.show`
- `service-resource.list`
- `service-resource.read`
- `service-resource.list-by-service`
- `service-resource.list-attributes`
- `attribute.list`
- `attribute.read`
- `service.define`
- `service.propose-revision`
- `service.deprecate`
- `service.retire`
- `service-version.draft`
- `service-version.submit-for-review`
- `service-version.publish`
- `service-version.retire`
- `service-version.update`
- `service-version.compare`

Forbidden verbs:

- `service-resource.provision`
- `service-resource.activate`
- `service-resource.set-attr`
- `cbu.create`

Templates:

- Product-first: `product.read`, then `service.list-by-product`.
- Service-first: `service.read`, then `service-resource.list-by-service`.
- Resource-first: `service-resource.read`, then `service-resource.list-attributes`.

## Service Resource Verb Surface

File: `rust/config/verbs/service-resource.yaml`

Observed verb groups:

- Read/list type catalog:
  - `service-resource.read`
  - `service-resource.list`
  - `service-resource.list-by-service`
  - `service-resource.list-attributes`
- Instance provisioning/lifecycle:
  - `service-resource.provision`
  - `service-resource.set-attr`
  - `service-resource.activate`
  - `service-resource.suspend`
  - `service-resource.decommission`
  - `service-resource.reactivate`
- Discovery/validation support:
  - `service-resource.sync-definitions`
  - `service-resource.check-attribute-gaps`

Peer-review issue: the journey pack correctly forbids provisioning in the design-time product maintenance workspace. CBU activation flows must still be able to call provisioning verbs after resolving product, service, resource, and option gates.

## SRDEF YAML Extracts

The SRDEF files are richer than the current DB rows in several places and likely represent intended business semantics that are not fully materialized in `service_resource_types` and `service_resource_capabilities`.

### Custody SRDEFs

File: `rust/config/srdefs/custody.yaml`

Domain: `CUSTODY`

SRDEFs:

- `custody_securities`
  - Name: Securities Custody Account
  - Resource type: Account
  - Purpose: hold securities positions in custody
  - Provisioning strategy: request
  - Owner: CUSTODY
  - Triggered by: `SAFEKEEPING`, `SETTLEMENT`, `CUSTODY_SAFEKEEPING`, `CUSTODY_SETTLEMENT`, `PRIME_BROKERAGE`
  - Attributes: `market_scope`, `settlement_currency`, `account_structure`, `tax_jurisdiction`
  - `per_market: true`
  - `per_currency: false`
- `custody_cash`
  - Name: Cash Custody Account
  - Resource type: Account
  - Purpose: hold cash balances
  - Provisioning strategy: request
  - Triggered by: `SAFEKEEPING`, `SETTLEMENT`, `CASH_MGMT`, `CUSTODY_SAFEKEEPING`, `CUSTODY_SETTLEMENT`, `CASH_MANAGEMENT`
  - Attributes: `settlement_currency`, `interest_calculation`
  - Depends on: `SRDEF::CUSTODY::Account::custody_securities`
  - `per_market: false`
  - `per_currency: true`
- `settlement_ssi`
  - Name: Standing Settlement Instructions
  - Resource type: InstructionSet
  - Purpose: define default settlement instructions
  - Provisioning strategy: create
  - Triggered by: `SETTLEMENT`, `CUSTODY_SETTLEMENT`
  - Attributes: `ssi_mode`, `default_agent_bic`, `default_account_number`
  - Depends on: `SRDEF::CUSTODY::Account::custody_securities`
  - `per_market: true`
  - `per_currency: false`

### Connectivity SRDEFs

File: `rust/config/srdefs/connectivity.yaml`

Domain: `CONNECTIVITY`

SRDEFs:

- `swift_messaging`
  - Name: SWIFT Messaging Channel
  - Resource type: Connectivity
  - Purpose: enable SWIFT messaging for settlements
  - Provisioning strategy: request
  - Triggered by: `SETTLEMENT`, `CORP_ACTIONS`, `CUSTODY_SETTLEMENT`, `CORPORATE_ACTIONS`, `PROXY_VOTING`
  - Attributes: `sender_bic`, `receiver_bic`, `message_types`
  - `per_counterparty: true`
- `fix_connectivity`
  - Name: FIX Protocol Connection
  - Triggered by: `ORDER_ROUTING`, `EXECUTION_MANAGEMENT`
  - Attributes: `fix_sender_comp_id`, `fix_target_comp_id`, `fix_version`, `heartbeat_interval`
  - `per_counterparty: true`
- `api_gateway`
  - Name: API Gateway Access
  - Triggered by: `API_ACCESS`, `DATA_FEEDS`, `REPORTING`
  - Attributes: `api_client_id`, `allowed_scopes`, `rate_limit_tier`, `ip_whitelist`

### IAM SRDEFs

File: `rust/config/srdefs/iam.yaml`

Domain: `IAM`

SRDEFs:

- `platform_access`
  - Triggered by: `SAFEKEEPING`, `SETTLEMENT`, `CUSTODY_SAFEKEEPING`, `CUSTODY_SETTLEMENT`, `REPORTING`, `CORP_ACTIONS`, `CORPORATE_ACTIONS`
  - Attributes: `user_principal`, `role_assignments`, `mfa_required`, `session_timeout_minutes`
- `service_account`
  - Triggered by: `API_ACCESS`, `DATA_FEEDS`, `AUTOMATION`
  - Attributes: `service_account_name`, `service_roles`, `credential_rotation_days`, `audit_logging`
  - Depends on: `SRDEF::CONNECTIVITY::Connectivity::api_gateway`
- `data_permissions`
  - Triggered by: `REPORTING`, `DATA_FEEDS`, `ANALYTICS`
  - Attributes: `data_domains`, `access_level`, `data_retention_days`
  - Depends on: `SRDEF::IAM::Entitlement::platform_access`

## Business Review Questions

Please review against real-world product, service, and resource semantics:

1. Are products at the right commercial granularity?
   - Example concern: is `CUSTODY` too broad without fund type, market, or asset-class options?
   - Are `ALTS`, `MIDDLE_OFFICE`, `TRANSFER_AGENCY`, `FUND_ACCOUNTING`, `MARKETS_FX`, and `COLLATERAL_MGMT` correct product-level commercial artifacts?

2. Which product-service links are mandatory, default, optional, or conditional?
   - Current DB encodes every link as non-mandatory and non-default.
   - The review should propose mandatory/default/optional classifications and conditions.

3. What service options are missing from product-service metadata?
   - Settlement markets.
   - Reconciliation parties.
   - Instruction channels.
   - Investment manager BIC and standing instruction behavior.
   - Custody account model: omnibus, segregated, hybrid.
   - Sub-custodian or direct market access model.
   - NAV frequency, pricing source, valuation cutoff.
   - Transfer agency investor register/accounting variants.
   - Hedge fund/private equity operational variants.

4. Does the service-resource map represent the right implementation assets?
   - Many services have no resources.
   - Some implementation resources exist in the resource catalog but are not linked to services.
   - SRDEF files imply richer resources than the DB links.

5. Which resources are per market, per currency, or per counterparty?
   - Current DB says none are per market/currency/counterparty.
   - SRDEF YAML says some custody and connectivity resources are per market, per currency, or per counterparty.
   - The reviewer should identify the correct dimensionality for each resource.

6. How should instrument matrix data feed resource population?
   - The user requirement says investment-manager aspects of the instrument matrix, including BIC and how the investment manager instructs BNY, must populate when the instrument matrix is attached to a CBU.
   - The reviewer should identify which product/service/resource attributes come from instrument matrix versus CBU profile versus entity data versus manual input.

7. How should service availability constrain activation?
   - Availability exists by service and booking principal dimensions, but coverage is partial.
   - Confirm whether regulatory, commercial, operational, and delivery model statuses are sufficient.

8. What is the correct boundary between design-time taxonomy and CBU runtime activation?
   - Product/service/resource taxonomy should describe what is available.
   - CBU/product/instrument-matrix selection should resolve what is required.
   - Runtime service delivery/resource instances should track what has been requested, provisioned, activated, blocked, or failed.

9. Should SemReg expose the product/service/resource taxonomy?
   - Current active SemReg views only show instrument classification in this extract.
   - Product/service/resource truth is mostly in `"ob-poc"` catalog tables and YAML.

10. Are there naming or table-shape mismatches that would confuse business review or SemOS navigation?
    - `service_resource_types` DB uses `resource_id`, while one constellation map uses `resource_type_id`.
    - `resource_dictionary` map references `attribute_registry`, while live attribute requirements join through `resource_attribute_requirements` and `dictionary`.

## Requested Peer Review Output

Please return:

- A corrected product to service matrix.
- Mandatory/default/optional/conditional classification for each product-service link.
- A corrected service to resource matrix.
- Resource dimensionality: per CBU, per market, per currency, per counterparty, per account, per fund/share class, or other.
- Attribute-source classification: derived, CBU profile, legal entity, instrument matrix, product option, document, manual.
- Missing product options and service options needed for realistic CBU activation.
- Missing services or resources.
- Rows that should be deleted as dead data or renamed for business clarity.
- Any corrections needed to the DAG/constellation/pack semantics.

