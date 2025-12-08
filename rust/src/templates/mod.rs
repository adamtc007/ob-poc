//! CBU Templates - Market-accurate fund structures for demo/testing
//!
//! These templates generate valid DSL for common fund structures with:
//! - Realistic UBO ownership chains (holding companies, trusts, GPs)
//! - Proper service provider relationships
//! - Product/service mappings (Custody, Fund Accounting, Transfer Agency)
//!
//! Fund Types:
//! - Hedge Fund (Cayman LP with offshore feeder structure)
//! - Luxembourg SICAV (UCITS with ManCo and full service provider chain)
//! - US 40 Act Mutual Fund (RIC with advisor and board)
//! - Segregated Portfolio Company (multi-strategy platform)

use serde::{Deserialize, Serialize};

/// Available CBU template types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateType {
    /// Cayman hedge fund with master-feeder structure
    HedgeFund,
    /// Luxembourg SICAV (UCITS) with full service provider chain
    LuxSicav,
    /// US 40 Act mutual fund (RIC)
    Us40Act,
    /// Segregated Portfolio Company (multi-strategy)
    Spc,
}

impl TemplateType {
    pub fn all() -> &'static [TemplateType] {
        &[
            TemplateType::HedgeFund,
            TemplateType::LuxSicav,
            TemplateType::Us40Act,
            TemplateType::Spc,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            TemplateType::HedgeFund => "hedge_fund",
            TemplateType::LuxSicav => "lux_sicav",
            TemplateType::Us40Act => "us_40_act",
            TemplateType::Spc => "spc",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            TemplateType::HedgeFund => "Cayman hedge fund - Master/Feeder with GP/LP, offshore UBO chain, Custody + Prime Brokerage",
            TemplateType::LuxSicav => "Luxembourg UCITS SICAV - ManCo, Depositary, TA, Fund Accounting, full CSSF compliant structure",
            TemplateType::Us40Act => "US 40 Act RIC - Investment Advisor, Independent Board, Custody + TA + Fund Accounting",
            TemplateType::Spc => "Cayman SPC - Multi-strategy platform with segregated portfolios, institutional UBO",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace(['-', '_'], "").as_str() {
            "hedgefund" | "hf" | "cayman" | "master" => Some(TemplateType::HedgeFund),
            "luxsicav" | "sicav" | "ucits" | "luxembourg" => Some(TemplateType::LuxSicav),
            "us40act" | "40act" | "ric" | "mutual" | "mutualfund" => Some(TemplateType::Us40Act),
            "spc" | "segregated" | "platform" => Some(TemplateType::Spc),
            _ => None,
        }
    }
}

/// Parameters for generating a CBU from a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParams {
    /// Fund name (required)
    pub fund_name: String,
    /// Optional: override default jurisdiction
    pub jurisdiction: Option<String>,
    /// Optional: Custom UBO chain (defaults provided if empty)
    pub ubos: Vec<UboParams>,
    /// Include KYC case with workstreams and screenings
    pub include_kyc: bool,
    /// Include share classes / investor registry
    pub include_share_classes: bool,
    /// Include product/service provisioning (Custody, Fund Accounting, TA)
    pub include_products: bool,
    /// Include custody setup (universe, SSI, booking rules)
    pub include_custody_setup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboParams {
    pub first_name: String,
    pub last_name: String,
    pub nationality: Option<String>,
    pub ownership_pct: u8,
    /// Intermediate holding entity (for realistic chains)
    pub via_entity: Option<String>,
}

impl Default for TemplateParams {
    fn default() -> Self {
        Self {
            fund_name: "Demo Fund".to_string(),
            jurisdiction: None,
            ubos: vec![],
            include_kyc: true,
            include_share_classes: true,
            include_products: true,
            include_custody_setup: false,
        }
    }
}

/// Generate DSL for a given template type
pub fn generate_template(template: TemplateType, params: &TemplateParams) -> String {
    match template {
        TemplateType::HedgeFund => generate_hedge_fund(params),
        TemplateType::LuxSicav => generate_lux_sicav(params),
        TemplateType::Us40Act => generate_us_40_act(params),
        TemplateType::Spc => generate_spc(params),
    }
}

// =============================================================================
// HEDGE FUND - Cayman Master/Feeder with offshore UBO chain
// =============================================================================
// Typical structure:
// - Cayman Master Fund LP (trading vehicle)
// - Cayman Feeder LP (offshore investors)
// - Delaware Feeder LP (US taxable investors)
// - Cayman GP Ltd (controlled by founders via BVI holding)
// - Investment Manager (UK FCA regulated)
// UBO Chain: Natural Person → BVI HoldCo → Cayman GP → Master Fund
// =============================================================================

fn generate_hedge_fund(params: &TemplateParams) -> String {
    let fund_name = &params.fund_name;
    let jurisdiction = params.jurisdiction.as_deref().unwrap_or("KY");

    let mut dsl = format!(
        r#";; =============================================================================
;; HEDGE FUND: {fund_name}
;; =============================================================================
;; Structure: Cayman Master/Feeder with GP/LP
;;
;; Legal Structure:
;;   {fund_name} Master Fund LP (KY) ← trading entity
;;     ├── {fund_name} Offshore Feeder LP (KY) ← non-US investors
;;     └── {fund_name} US Feeder LP (DE) ← US taxable investors
;;
;; Control Chain:
;;   {fund_name} GP Ltd (KY) ← General Partner
;;     └── {fund_name} Holdings Ltd (VG) ← BVI holding company
;;         └── UBOs (natural persons)
;;
;; Service Providers:
;;   - Investment Manager: {fund_name} Investment Management LLP (UK, FCA)
;;   - Administrator: Citco Fund Services (KY)
;;   - Prime Broker: Goldman Sachs International
;;   - Auditor: PwC Cayman
;; =============================================================================

;; -----------------------------------------------------------------------------
;; SECTION 1: INVESTMENT MANAGER (Commercial Client / Head Office)
;; The FCA-regulated entity that contracted with the bank
;; -----------------------------------------------------------------------------

(entity.create-limited-company
  :name "{fund_name} Investment Management LLP"
  :jurisdiction "GB"
  :as @investment-manager)

;; -----------------------------------------------------------------------------
;; SECTION 2: CBU (Client Business Unit)
;; -----------------------------------------------------------------------------

(cbu.ensure
  :name "{fund_name}"
  :jurisdiction "{jurisdiction}"
  :client-type "FUND"
  :nature-purpose "Global macro hedge fund pursuing long/short equity and event-driven strategies"
  :commercial-client-entity-id @investment-manager
  :as @fund)

;; -----------------------------------------------------------------------------
;; SECTION 3: MASTER FUND STRUCTURE
;; -----------------------------------------------------------------------------

;; Master Fund - the trading entity
(entity.create-partnership-limited
  :name "{fund_name} Master Fund LP"
  :jurisdiction "{jurisdiction}"
  :as @master-fund)

;; Offshore Feeder - for non-US investors
(entity.create-partnership-limited
  :name "{fund_name} Offshore Feeder LP"
  :jurisdiction "{jurisdiction}"
  :as @offshore-feeder)

;; US Feeder - for US taxable investors (Delaware LP for tax transparency)
(entity.create-partnership-limited
  :name "{fund_name} US Feeder LP"
  :jurisdiction "US"
  :as @us-feeder)

;; Assign hedge fund structure roles
;; Master Fund = pooling vehicle that executes trades
(cbu.assign-role :cbu-id @fund :entity-id @master-fund :role "PRINCIPAL")
(cbu.assign-role :cbu-id @fund :entity-id @master-fund :role "MASTER_FUND")

;; Feeders = investor-facing vehicles that feed capital to master
(cbu.assign-role :cbu-id @fund :entity-id @offshore-feeder :role "FEEDER_FUND")
(cbu.assign-role :cbu-id @fund :entity-id @us-feeder :role "FEEDER_FUND")

;; Investment Manager = the management company (FCA/SEC registered adviser)
(cbu.assign-role :cbu-id @fund :entity-id @investment-manager :role "INVESTMENT_MANAGER")
(cbu.assign-role :cbu-id @fund :entity-id @investment-manager :role "SPONSOR")

;; -----------------------------------------------------------------------------
;; SECTION 4: GENERAL PARTNER & CONTROL STRUCTURE
;; -----------------------------------------------------------------------------

;; General Partner (controls the Master Fund)
(entity.create-limited-company
  :name "{fund_name} GP Ltd"
  :jurisdiction "{jurisdiction}"
  :as @gp)

;; BVI Holding Company (owns the GP - typical offshore structure)
(entity.create-limited-company
  :name "{fund_name} Holdings Ltd"
  :jurisdiction "VG"
  :as @holdco)

(cbu.assign-role :cbu-id @fund :entity-id @gp :role "GENERAL_PARTNER")
(cbu.assign-role :cbu-id @fund :entity-id @holdco :role "SHAREHOLDER")

;; Ownership: HoldCo owns 100% of GP
(ubo.add-ownership
  :owner-entity-id @holdco
  :owned-entity-id @gp
  :percentage 100
  :ownership-type "DIRECT")

;; -----------------------------------------------------------------------------
;; SECTION 5: UBO STRUCTURE (Founders via BVI HoldCo)
;; -----------------------------------------------------------------------------

;; Founder 1: Marcus Chen - CIO, 60% economic interest
(entity.create-proper-person
  :first-name "Marcus"
  :last-name "Chen"
  :nationality "SG"
  :as @marcus)

;; Founder 2: Sarah Goldberg - COO, 40% economic interest
(entity.create-proper-person
  :first-name "Sarah"
  :last-name "Goldberg"
  :nationality "US"
  :as @sarah)

;; UBO roles (control via GP)
(cbu.assign-role :cbu-id @fund :entity-id @marcus :role "BENEFICIAL_OWNER" :ownership-percentage 60)
(cbu.assign-role :cbu-id @fund :entity-id @sarah :role "BENEFICIAL_OWNER" :ownership-percentage 40)

;; Ownership chain: UBOs → HoldCo → GP → Master Fund
(ubo.add-ownership :owner-entity-id @marcus :owned-entity-id @holdco :percentage 60 :ownership-type "DIRECT")
(ubo.add-ownership :owner-entity-id @sarah :owned-entity-id @holdco :percentage 40 :ownership-type "DIRECT")

;; Register UBO determinations
(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @master-fund
  :ubo-person-id @marcus
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "CONTROL_VIA_GP"
  :ownership-percentage 60
  :workflow-type "ONBOARDING")

(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @master-fund
  :ubo-person-id @sarah
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "CONTROL_VIA_GP"
  :ownership-percentage 40
  :workflow-type "ONBOARDING")

;; -----------------------------------------------------------------------------
;; SECTION 6: DIRECTORS (GP Board)
;; -----------------------------------------------------------------------------

;; Independent Director 1 - Cayman resident
(entity.create-proper-person
  :first-name "James"
  :last-name "Morrison"
  :nationality "KY"
  :as @director1)

;; Independent Director 2 - professional director
(entity.create-proper-person
  :first-name "Elizabeth"
  :last-name "Park"
  :nationality "GB"
  :as @director2)

(cbu.assign-role :cbu-id @fund :entity-id @director1 :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @director2 :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @marcus :role "DIRECTOR")

;; -----------------------------------------------------------------------------
;; SECTION 7: SERVICE PROVIDERS
;; -----------------------------------------------------------------------------

;; Administrator (NAV calculation, investor services)
(entity.create-limited-company
  :name "Citco Fund Services (Cayman)"
  :jurisdiction "{jurisdiction}"
  :as @administrator)

(cbu.assign-role :cbu-id @fund :entity-id @administrator :role "ADMINISTRATOR")

;; Prime Broker
(entity.create-limited-company
  :name "Goldman Sachs International"
  :jurisdiction "GB"
  :as @prime-broker)

(cbu.assign-role :cbu-id @fund :entity-id @prime-broker :role "PRIME_BROKER")

;; Auditor
(entity.create-limited-company
  :name "PricewaterhouseCoopers Cayman"
  :jurisdiction "{jurisdiction}"
  :as @auditor)

(cbu.assign-role :cbu-id @fund :entity-id @auditor :role "AUDITOR")

;; Legal Counsel
(entity.create-limited-company
  :name "Maples and Calder"
  :jurisdiction "{jurisdiction}"
  :as @legal)

(cbu.assign-role :cbu-id @fund :entity-id @legal :role "LEGAL_COUNSEL")

"#
    );

    // Products and Services
    if params.include_products {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 8: PRODUCTS & SERVICES
;; -----------------------------------------------------------------------------

;; Custody Product - for fund assets
(service-resource.provision
  :cbu-id @fund
  :resource-type "CUSTODY_ACCT"
  :instance-url "https://custody.bank.com/accounts/master"
  :as @custody-master)

;; TODO: (service-resource.set-attr :instance-id @custody-master :attr "account_name" :value "Master Fund Custody")
;; TODO: (service-resource.set-attr :instance-id @custody-master :attr "base_currency" :value "USD")

;; Fund Accounting - NAV calculation service
(service-resource.provision
  :cbu-id @fund
  :resource-type "FA_EAGLE"
  :instance-url "https://fundaccounting.bank.com/funds/master"
  :as @fa-master)

;; TODO: (service-resource.set-attr :instance-id @fa-master :attr "nav_frequency" :value "DAILY")
;; TODO: (service-resource.set-attr :instance-id @fa-master :attr "accounting_basis" :value "US_GAAP")

;; Investor Ledger - for feeder funds
(service-resource.provision
  :cbu-id @fund
  :resource-type "INVESTOR_LEDGER"
  :instance-url "https://ta.bank.com/registry/offshore"
  :as @ta-offshore)

;; TODO: (service-resource.set-attr :instance-id @ta-offshore :attr "fund_entity" :value "Offshore Feeder")

"#);
    }

    // Share Classes
    if params.include_share_classes {
        dsl.push_str(
            r#"
;; -----------------------------------------------------------------------------
;; SECTION 9: SHARE CLASSES
;; -----------------------------------------------------------------------------

;; Master Fund share classes
(share-class.create
  :cbu-id @fund
  :entity-id @master-fund
  :name "Master - Class A"
  :currency "USD"
  :class-category "FUND"
  :nav-per-share 1000.00
  :management-fee-bps 200
  :performance-fee-bps 2000
  :high-water-mark true
  :as @master-a)

;; Offshore Feeder classes
(share-class.create
  :cbu-id @fund
  :entity-id @offshore-feeder
  :name "Offshore - Class A USD"
  :isin "KY0000000001"
  :currency "USD"
  :class-category "FUND"
  :nav-per-share 1000.00
  :management-fee-bps 200
  :performance-fee-bps 2000
  :high-water-mark true
  :minimum-investment 1000000.00
  :redemption-notice-days 30
  :lock-up-period-months 12
  :as @offshore-a)

(share-class.create
  :cbu-id @fund
  :entity-id @offshore-feeder
  :name "Offshore - Class B EUR"
  :isin "KY0000000002"
  :currency "EUR"
  :class-category "FUND"
  :nav-per-share 1000.00
  :management-fee-bps 150
  :performance-fee-bps 2000
  :minimum-investment 5000000.00
  :as @offshore-b)

"#,
        );
    }

    // KYC Case
    if params.include_kyc {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 10: KYC CASE
;; -----------------------------------------------------------------------------

(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :notes "Hedge fund onboarding - Master/Feeder structure with offshore UBO chain"
  :as @case)

;; Entity workstreams
(entity-workstream.create :case-id @case :entity-id @master-fund :as @ws-master)
(entity-workstream.create :case-id @case :entity-id @gp :as @ws-gp)
(entity-workstream.create :case-id @case :entity-id @holdco :discovery-reason "SHAREHOLDER" :as @ws-holdco)
(entity-workstream.create :case-id @case :entity-id @marcus :is-ubo true :ownership-percentage 60 :as @ws-marcus)
(entity-workstream.create :case-id @case :entity-id @sarah :is-ubo true :ownership-percentage 40 :as @ws-sarah)

;; Screenings for UBOs
(case-screening.run :workstream-id @ws-marcus :screening-type "PEP")
(case-screening.run :workstream-id @ws-marcus :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-marcus :screening-type "ADVERSE_MEDIA")
(case-screening.run :workstream-id @ws-sarah :screening-type "PEP")
(case-screening.run :workstream-id @ws-sarah :screening-type "SANCTIONS")

;; Corporate screenings
(case-screening.run :workstream-id @ws-holdco :screening-type "SANCTIONS")

"#);
    }

    // Custody Setup
    if params.include_custody_setup {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 11: CUSTODY SETUP (Universe, SSI, Booking Rules)
;; -----------------------------------------------------------------------------

;; Trading Universe - what the fund trades
(cbu-custody.add-universe :cbu-id @fund :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @fund :instrument-class "EQUITY" :market "XLON" :currencies ["GBP" "USD"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @fund :instrument-class "EQUITY" :market "XHKG" :currencies ["HKD" "USD"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @fund :instrument-class "GOVT_BOND" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])

;; SSI - US Markets
(cbu-custody.create-ssi
  :cbu-id @fund
  :name "US Primary"
  :type "SECURITIES"
  :safekeeping-account "MASTER-USD-001"
  :safekeeping-bic "BABOROCP"
  :cash-account "MASTER-CASH-USD"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-01-01"
  :as @ssi-us)

(cbu-custody.activate-ssi :ssi-id @ssi-us)

;; Booking Rules
(cbu-custody.add-booking-rule :cbu-id @fund :ssi-id @ssi-us :name "US Equity" :priority 10 :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
(cbu-custody.add-booking-rule :cbu-id @fund :ssi-id @ssi-us :name "US Bonds" :priority 20 :instrument-class "GOVT_BOND" :market "XNYS" :currency "USD")

"#);
    }

    dsl.push_str(&format!(
        r#"
;; =============================================================================
;; SUMMARY: {fund_name}
;; Type: Cayman Hedge Fund (Master/Feeder)
;; Structure: Master LP + Offshore Feeder + US Feeder + GP + BVI HoldCo
;; UBOs: Marcus Chen (60%), Sarah Goldberg (40%) via BVI HoldCo
;; Control: GP Ltd → Master Fund LP
;; Services: Administrator, Prime Broker, Auditor, Legal
;; =============================================================================
"#
    ));

    dsl
}

// =============================================================================
// LUXEMBOURG SICAV - UCITS with full CSSF-compliant structure
// =============================================================================

fn generate_lux_sicav(params: &TemplateParams) -> String {
    let fund_name = &params.fund_name;
    let jurisdiction = params.jurisdiction.as_deref().unwrap_or("LU");

    let mut dsl = format!(
        r#";; =============================================================================
;; LUXEMBOURG SICAV: {fund_name}
;; =============================================================================
;; Structure: UCITS SICAV with ManCo and full service provider chain
;;
;; Legal Structure:
;;   {fund_name} SICAV (LU) ← umbrella fund, multiple sub-funds
;;     ├── European Equity Sub-Fund
;;     ├── Global Bond Sub-Fund
;;     └── Multi-Asset Sub-Fund
;;
;; Governance:
;;   {fund_name} Management S.A. (LU) ← CSSF-authorized ManCo
;;     └── Board of Directors (independent majority)
;;
;; Service Providers (CSSF mandated):
;;   - Depositary: State Street Bank Luxembourg S.C.A.
;;   - Administrator: State Street Bank Luxembourg S.C.A.
;;   - Transfer Agent: European Fund Administration S.A.
;;   - Investment Manager: {fund_name} Asset Management Ltd (UK FCA)
;;   - Auditor: Deloitte Luxembourg
;; =============================================================================

;; -----------------------------------------------------------------------------
;; SECTION 1: MANAGEMENT COMPANY (CSSF Authorized)
;; -----------------------------------------------------------------------------

(entity.create-limited-company
  :name "{fund_name} Management S.A."
  :jurisdiction "{jurisdiction}"
  :registration-number "B234567"
  :as @manco)

;; -----------------------------------------------------------------------------
;; SECTION 2: CBU
;; -----------------------------------------------------------------------------

(cbu.ensure
  :name "{fund_name}"
  :jurisdiction "{jurisdiction}"
  :client-type "FUND"
  :nature-purpose "UCITS SICAV offering diversified investment strategies to retail and institutional investors"
  :commercial-client-entity-id @manco
  :as @fund)

;; -----------------------------------------------------------------------------
;; SECTION 3: SICAV LEGAL ENTITY (Umbrella)
;; -----------------------------------------------------------------------------

(entity.create-limited-company
  :name "{fund_name} SICAV"
  :jurisdiction "{jurisdiction}"
  :registration-number "B345678"
  :as @sicav)

;; SICAV = the fund vehicle that owns assets
(cbu.assign-role :cbu-id @fund :entity-id @sicav :role "PRINCIPAL")
(cbu.assign-role :cbu-id @fund :entity-id @sicav :role "ASSET_OWNER")

;; ManCo = UCITS management company (CSSF authorized AIFM)
(cbu.assign-role :cbu-id @fund :entity-id @manco :role "MANAGEMENT_COMPANY")
(cbu.assign-role :cbu-id @fund :entity-id @manco :role "INVESTMENT_MANAGER")

;; -----------------------------------------------------------------------------
;; SECTION 4: SERVICE PROVIDERS (CSSF Required)
;; -----------------------------------------------------------------------------

;; Depositary (CSSF requirement - must be Luxembourg credit institution)
(entity.create-limited-company
  :name "State Street Bank Luxembourg S.C.A."
  :jurisdiction "{jurisdiction}"
  :as @depositary)

(cbu.assign-role :cbu-id @fund :entity-id @depositary :role "DEPOSITARY")

;; Central Administrator (often same as depositary)
(entity.create-limited-company
  :name "State Street Fund Services (Luxembourg) S.A."
  :jurisdiction "{jurisdiction}"
  :as @administrator)

(cbu.assign-role :cbu-id @fund :entity-id @administrator :role "ADMINISTRATOR")

;; Transfer Agent
(entity.create-limited-company
  :name "European Fund Administration S.A."
  :jurisdiction "{jurisdiction}"
  :as @transfer-agent)

(cbu.assign-role :cbu-id @fund :entity-id @transfer-agent :role "TRANSFER_AGENT")

;; Investment Manager (delegated portfolio management - UK FCA)
(entity.create-limited-company
  :name "{fund_name} Asset Management Ltd"
  :jurisdiction "GB"
  :as @investment-manager)

(cbu.assign-role :cbu-id @fund :entity-id @investment-manager :role "INVESTMENT_MANAGER")

;; Auditor (Big 4)
(entity.create-limited-company
  :name "Deloitte Audit S.a r.l."
  :jurisdiction "{jurisdiction}"
  :as @auditor)

(cbu.assign-role :cbu-id @fund :entity-id @auditor :role "AUDITOR")

;; Global Distributor
(entity.create-limited-company
  :name "{fund_name} Distribution S.A."
  :jurisdiction "{jurisdiction}"
  :as @distributor)

(cbu.assign-role :cbu-id @fund :entity-id @distributor :role "DISTRIBUTOR")

;; -----------------------------------------------------------------------------
;; SECTION 5: MANCO OWNERSHIP & UBO
;; -----------------------------------------------------------------------------

;; ManCo parent holding (often a larger asset manager)
(entity.create-limited-company
  :name "{fund_name} Group Holdings S.a r.l."
  :jurisdiction "{jurisdiction}"
  :as @group-holdco)

(cbu.assign-role :cbu-id @fund :entity-id @group-holdco :role "SHAREHOLDER")

(ubo.add-ownership
  :owner-entity-id @group-holdco
  :owned-entity-id @manco
  :percentage 100
  :ownership-type "DIRECT")

;; Ultimate UBO - Founder/CEO
(entity.create-proper-person
  :first-name "Heinrich"
  :last-name "Mueller"
  :nationality "DE"
  :as @heinrich)

(cbu.assign-role :cbu-id @fund :entity-id @heinrich :role "BENEFICIAL_OWNER" :ownership-percentage 100)

(ubo.add-ownership
  :owner-entity-id @heinrich
  :owned-entity-id @group-holdco
  :percentage 100
  :ownership-type "DIRECT")

(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @manco
  :ubo-person-id @heinrich
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 100
  :workflow-type "ONBOARDING")

;; -----------------------------------------------------------------------------
;; SECTION 6: BOARD OF DIRECTORS (ManCo & SICAV)
;; -----------------------------------------------------------------------------

;; Independent Directors (CSSF requirement)
(entity.create-proper-person
  :first-name "Pierre"
  :last-name "Dubois"
  :nationality "LU"
  :as @director1)

(entity.create-proper-person
  :first-name "Marie"
  :last-name "Laurent"
  :nationality "FR"
  :as @director2)

(entity.create-proper-person
  :first-name "Thomas"
  :last-name "Schmidt"
  :nationality "DE"
  :as @director3)

;; Conducting Officers (CSSF requirement for ManCo)
(entity.create-proper-person
  :first-name "Jean-Claude"
  :last-name "Weber"
  :nationality "LU"
  :as @conducting1)

(entity.create-proper-person
  :first-name "Anna"
  :last-name "Keller"
  :nationality "DE"
  :as @conducting2)

(cbu.assign-role :cbu-id @fund :entity-id @director1 :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @director2 :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @director3 :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @heinrich :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @conducting1 :role "CONDUCTING_OFFICER")
(cbu.assign-role :cbu-id @fund :entity-id @conducting2 :role "CONDUCTING_OFFICER")

"#
    );

    // Products
    if params.include_products {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 7: PRODUCTS & SERVICES
;; -----------------------------------------------------------------------------

;; Custody - Depositary services
(service-resource.provision
  :cbu-id @fund
  :resource-type "CUSTODY_ACCT"
  :instance-url "https://custody.statestreet.com/lu/sicav"
  :as @custody-sicav)

;; TODO: (service-resource.set-attr :instance-id @custody-sicav :attr "account_name" :value "SICAV Umbrella")
;; TODO: (service-resource.set-attr :instance-id @custody-sicav :attr "depositary_agreement" :value "CSSF_COMPLIANT")

;; Fund Accounting - Multi-currency NAV
(service-resource.provision
  :cbu-id @fund
  :resource-type "FA_INVESTONE"
  :instance-url "https://fa.statestreet.com/lu/sicav"
  :as @fa-sicav)

;; TODO: (service-resource.set-attr :instance-id @fa-sicav :attr "nav_frequency" :value "DAILY")
;; TODO: (service-resource.set-attr :instance-id @fa-sicav :attr "accounting_basis" :value "LUX_GAAP")
;; TODO: (service-resource.set-attr :instance-id @fa-sicav :attr "multi_currency" :value "true")

;; Transfer Agency - Investor registry
(service-resource.provision
  :cbu-id @fund
  :resource-type "RUFUS_TA"
  :instance-url "https://ta.efa.lu/sicav"
  :as @ta-sicav)

;; TODO: (service-resource.set-attr :instance-id @ta-sicav :attr "nominee_structure" :value "CLEARSTREAM_EUROCLEAR")
;; TODO: (service-resource.set-attr :instance-id @ta-sicav :attr "aml_screening" :value "INTEGRATED")

"#);
    }

    // Share Classes
    if params.include_share_classes {
        dsl.push_str(
            r#"
;; -----------------------------------------------------------------------------
;; SECTION 8: SUB-FUNDS & SHARE CLASSES
;; -----------------------------------------------------------------------------

;; SUB-FUND 1: European Equity
(share-class.create
  :cbu-id @fund
  :entity-id @sicav
  :name "European Equity - Class A EUR (Retail)"
  :isin "LU0000000001"
  :currency "EUR"
  :class-category "FUND"
  :fund-type "UCITS"
  :fund-structure "OPEN_ENDED"
  :investor-eligibility "RETAIL"
  :nav-per-share 100.00
  :management-fee-bps 150
  :subscription-frequency "Daily"
  :redemption-frequency "Daily"
  :minimum-investment 1000.00
  :as @equity-retail)

(share-class.create
  :cbu-id @fund
  :entity-id @sicav
  :name "European Equity - Class I EUR (Institutional)"
  :isin "LU0000000002"
  :currency "EUR"
  :class-category "FUND"
  :fund-type "UCITS"
  :investor-eligibility "PROFESSIONAL"
  :nav-per-share 1000.00
  :management-fee-bps 75
  :minimum-investment 1000000.00
  :as @equity-inst)

;; SUB-FUND 2: Global Bond
(share-class.create
  :cbu-id @fund
  :entity-id @sicav
  :name "Global Bond - Class A USD"
  :isin "LU0000000003"
  :currency "USD"
  :class-category "FUND"
  :fund-type "UCITS"
  :nav-per-share 100.00
  :management-fee-bps 80
  :as @bond-a)

(share-class.create
  :cbu-id @fund
  :entity-id @sicav
  :name "Global Bond - Class A EUR Hedged"
  :isin "LU0000000004"
  :currency "EUR"
  :class-category "FUND"
  :fund-type "UCITS"
  :nav-per-share 100.00
  :management-fee-bps 90
  :as @bond-eur-h)

;; SUB-FUND 3: Multi-Asset
(share-class.create
  :cbu-id @fund
  :entity-id @sicav
  :name "Multi-Asset - Class A EUR"
  :isin "LU0000000005"
  :currency "EUR"
  :class-category "FUND"
  :fund-type "UCITS"
  :nav-per-share 100.00
  :management-fee-bps 120
  :as @multi-a)

"#,
        );
    }

    // KYC
    if params.include_kyc {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 9: KYC CASE
;; -----------------------------------------------------------------------------

(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :notes "UCITS SICAV onboarding - full CSSF compliant structure"
  :as @case)

(entity-workstream.create :case-id @case :entity-id @sicav :as @ws-sicav)
(entity-workstream.create :case-id @case :entity-id @manco :as @ws-manco)
(entity-workstream.create :case-id @case :entity-id @group-holdco :discovery-reason "SHAREHOLDER" :as @ws-holdco)
(entity-workstream.create :case-id @case :entity-id @heinrich :is-ubo true :ownership-percentage 100 :as @ws-ubo)

(case-screening.run :workstream-id @ws-ubo :screening-type "PEP")
(case-screening.run :workstream-id @ws-ubo :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-manco :screening-type "SANCTIONS")

"#);
    }

    dsl.push_str(&format!(
        r#"
;; =============================================================================
;; SUMMARY: {fund_name}
;; Type: Luxembourg UCITS SICAV
;; Structure: Umbrella SICAV + ManCo + Full Service Provider Chain
;; Sub-Funds: European Equity, Global Bond, Multi-Asset
;; UBO: Heinrich Mueller (100%) via Group Holdings
;; Governance: Board + Conducting Officers (CSSF compliant)
;; =============================================================================
"#
    ));

    dsl
}

// =============================================================================
// US 40 ACT MUTUAL FUND - Registered Investment Company
// =============================================================================

fn generate_us_40_act(params: &TemplateParams) -> String {
    let fund_name = &params.fund_name;
    let jurisdiction = params.jurisdiction.as_deref().unwrap_or("US");

    let mut dsl = format!(
        r#";; =============================================================================
;; US 40 ACT MUTUAL FUND: {fund_name}
;; =============================================================================
;; Structure: Registered Investment Company under Investment Company Act of 1940
;;
;; Legal Structure:
;;   {fund_name} Trust (MA Business Trust) ← the fund
;;     └── Multiple series/portfolios
;;
;; Governance:
;;   - Board of Trustees (majority independent - 40 Act requirement)
;;   - Investment Advisor (SEC registered RIA)
;;
;; Service Providers:
;;   - Custodian: Bank of New York Mellon
;;   - Transfer Agent: DST Systems / SS&C
;;   - Administrator: State Street
;;   - Distributor: {fund_name} Distributors Inc (FINRA member)
;;   - Auditor: Ernst & Young LLP
;; =============================================================================

;; -----------------------------------------------------------------------------
;; SECTION 1: INVESTMENT ADVISOR (SEC Registered RIA)
;; -----------------------------------------------------------------------------

(entity.create-limited-company
  :name "{fund_name} Advisors LLC"
  :jurisdiction "{jurisdiction}"
  :as @advisor)

;; -----------------------------------------------------------------------------
;; SECTION 2: CBU
;; -----------------------------------------------------------------------------

(cbu.ensure
  :name "{fund_name}"
  :jurisdiction "{jurisdiction}"
  :client-type "FUND"
  :nature-purpose "SEC registered investment company (RIC) under Investment Company Act of 1940"
  :commercial-client-entity-id @advisor
  :as @fund)

;; -----------------------------------------------------------------------------
;; SECTION 3: THE TRUST (Massachusetts Business Trust - typical structure)
;; -----------------------------------------------------------------------------

(entity.create-trust-discretionary
  :name "{fund_name} Trust"
  :jurisdiction "US"
  :trust-type "Massachusetts Business Trust"
  :as @trust)

;; Trust = the fund vehicle that owns assets (RIC - Regulated Investment Company)
(cbu.assign-role :cbu-id @fund :entity-id @trust :role "PRINCIPAL")
(cbu.assign-role :cbu-id @fund :entity-id @trust :role "ASSET_OWNER")

;; Investment Advisor = SEC registered adviser that manages the fund
(cbu.assign-role :cbu-id @fund :entity-id @advisor :role "INVESTMENT_ADVISOR")
(cbu.assign-role :cbu-id @fund :entity-id @advisor :role "INVESTMENT_MANAGER")

;; -----------------------------------------------------------------------------
;; SECTION 4: SERVICE PROVIDERS
;; -----------------------------------------------------------------------------

;; Custodian (must be qualified bank under 40 Act)
(entity.create-limited-company
  :name "The Bank of New York Mellon"
  :jurisdiction "{jurisdiction}"
  :as @custodian)

(cbu.assign-role :cbu-id @fund :entity-id @custodian :role "CUSTODIAN")

;; Transfer Agent
(entity.create-limited-company
  :name "SS&C GIDS Inc."
  :jurisdiction "{jurisdiction}"
  :as @transfer-agent)

(cbu.assign-role :cbu-id @fund :entity-id @transfer-agent :role "TRANSFER_AGENT")

;; Fund Administrator
(entity.create-limited-company
  :name "State Street Bank and Trust Company"
  :jurisdiction "{jurisdiction}"
  :as @administrator)

(cbu.assign-role :cbu-id @fund :entity-id @administrator :role "ADMINISTRATOR")

;; Principal Underwriter / Distributor (FINRA member)
(entity.create-limited-company
  :name "{fund_name} Distributors Inc."
  :jurisdiction "{jurisdiction}"
  :as @distributor)

(cbu.assign-role :cbu-id @fund :entity-id @distributor :role "DISTRIBUTOR")

;; Auditor
(entity.create-limited-company
  :name "Ernst & Young LLP"
  :jurisdiction "{jurisdiction}"
  :as @auditor)

(cbu.assign-role :cbu-id @fund :entity-id @auditor :role "AUDITOR")

;; -----------------------------------------------------------------------------
;; SECTION 5: ADVISOR OWNERSHIP & UBO
;; -----------------------------------------------------------------------------

;; Advisor is owned by founders
(entity.create-proper-person
  :first-name "William"
  :last-name "Thompson"
  :nationality "US"
  :as @william)

(entity.create-proper-person
  :first-name "Jennifer"
  :last-name "Adams"
  :nationality "US"
  :as @jennifer)

(cbu.assign-role :cbu-id @fund :entity-id @william :role "BENEFICIAL_OWNER" :ownership-percentage 55)
(cbu.assign-role :cbu-id @fund :entity-id @jennifer :role "BENEFICIAL_OWNER" :ownership-percentage 45)

(ubo.add-ownership :owner-entity-id @william :owned-entity-id @advisor :percentage 55 :ownership-type "DIRECT")
(ubo.add-ownership :owner-entity-id @jennifer :owned-entity-id @advisor :percentage 45 :ownership-type "DIRECT")

;; Note: UBOs of advisor, not the trust (trust is a pass-through for investors)
(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @advisor
  :ubo-person-id @william
  :relationship-type "DIRECT_OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 55
  :workflow-type "ONBOARDING")

(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @advisor
  :ubo-person-id @jennifer
  :relationship-type "DIRECT_OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 45
  :workflow-type "ONBOARDING")

;; -----------------------------------------------------------------------------
;; SECTION 6: BOARD OF TRUSTEES (40 Act: majority must be independent)
;; -----------------------------------------------------------------------------

;; Independent Trustees (not "interested persons" under 40 Act)
(entity.create-proper-person
  :first-name "Robert"
  :last-name "Kennedy"
  :nationality "US"
  :as @trustee1)

(entity.create-proper-person
  :first-name "Patricia"
  :last-name "Williams"
  :nationality "US"
  :as @trustee2)

(entity.create-proper-person
  :first-name "Michael"
  :last-name "Chang"
  :nationality "US"
  :as @trustee3)

;; Interested Trustees (affiliated with advisor)
(entity.create-proper-person
  :first-name "David"
  :last-name "Martinez"
  :nationality "US"
  :as @trustee-interested)

(cbu.assign-role :cbu-id @fund :entity-id @trustee1 :role "INDEPENDENT_TRUSTEE")
(cbu.assign-role :cbu-id @fund :entity-id @trustee2 :role "INDEPENDENT_TRUSTEE")
(cbu.assign-role :cbu-id @fund :entity-id @trustee3 :role "INDEPENDENT_TRUSTEE")
(cbu.assign-role :cbu-id @fund :entity-id @trustee-interested :role "INTERESTED_TRUSTEE")
(cbu.assign-role :cbu-id @fund :entity-id @william :role "INTERESTED_TRUSTEE")

;; Chief Compliance Officer (40 Act requirement)
(entity.create-proper-person
  :first-name "Karen"
  :last-name "Walsh"
  :nationality "US"
  :as @cco)

(cbu.assign-role :cbu-id @fund :entity-id @cco :role "CHIEF_COMPLIANCE_OFFICER")

"#
    );

    // Products
    if params.include_products {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 7: PRODUCTS & SERVICES
;; -----------------------------------------------------------------------------

;; Custody
(service-resource.provision
  :cbu-id @fund
  :resource-type "CUSTODY_ACCT"
  :instance-url "https://custody.bnymellon.com/funds/trust"
  :as @custody-trust)

;; TODO: (service-resource.set-attr :instance-id @custody-trust :attr "account_name" :value "40 Act Fund Custody")
;; TODO: (service-resource.set-attr :instance-id @custody-trust :attr "rule_17f" :value "COMPLIANT")

;; Fund Accounting
(service-resource.provision
  :cbu-id @fund
  :resource-type "FA_EAGLE"
  :instance-url "https://fa.statestreet.com/us/trust"
  :as @fa-trust)

;; TODO: (service-resource.set-attr :instance-id @fa-trust :attr "nav_frequency" :value "DAILY")
;; TODO: (service-resource.set-attr :instance-id @fa-trust :attr "accounting_basis" :value "US_GAAP")
;; TODO: (service-resource.set-attr :instance-id @fa-trust :attr "rule_22c1" :value "FORWARD_PRICING")

;; Transfer Agency
(service-resource.provision
  :cbu-id @fund
  :resource-type "RUFUS_TA"
  :instance-url "https://ta.ssc.com/funds/trust"
  :as @ta-trust)

;; TODO: (service-resource.set-attr :instance-id @ta-trust :attr "nscc_participant" :value "true")
;; TODO: (service-resource.set-attr :instance-id @ta-trust :attr "acat_enabled" :value "true")

"#);
    }

    // Share Classes
    if params.include_share_classes {
        dsl.push_str(
            r#"
;; -----------------------------------------------------------------------------
;; SECTION 8: SHARE CLASSES
;; -----------------------------------------------------------------------------

;; Class A - Front-end load (retail through broker-dealers)
(share-class.create
  :cbu-id @fund
  :entity-id @trust
  :name "Class A Shares"
  :isin "US0000000001"
  :currency "USD"
  :class-category "FUND"
  :fund-type "MUTUAL_FUND"
  :fund-structure "OPEN_ENDED"
  :investor-eligibility "RETAIL"
  :nav-per-share 25.00
  :management-fee-bps 75
  :subscription-frequency "Daily"
  :redemption-frequency "Daily"
  :minimum-investment 2500.00
  :as @class-a)

;; Class C - Level load (12b-1 fees)
(share-class.create
  :cbu-id @fund
  :entity-id @trust
  :name "Class C Shares"
  :isin "US0000000002"
  :currency "USD"
  :class-category "FUND"
  :investor-eligibility "RETAIL"
  :nav-per-share 25.00
  :management-fee-bps 75
  :minimum-investment 2500.00
  :as @class-c)

;; Class I - Institutional (no load, lower expense)
(share-class.create
  :cbu-id @fund
  :entity-id @trust
  :name "Class I Shares"
  :isin "US0000000003"
  :currency "USD"
  :class-category "FUND"
  :investor-eligibility "QUALIFIED"
  :nav-per-share 25.00
  :management-fee-bps 50
  :minimum-investment 1000000.00
  :as @class-i)

;; Class R6 - Retirement plans (lowest cost)
(share-class.create
  :cbu-id @fund
  :entity-id @trust
  :name "Class R6 Shares"
  :isin "US0000000004"
  :currency "USD"
  :class-category "FUND"
  :investor-eligibility "QUALIFIED"
  :nav-per-share 25.00
  :management-fee-bps 40
  :minimum-investment 0.00
  :as @class-r6)

"#,
        );
    }

    // KYC
    if params.include_kyc {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 9: KYC CASE
;; -----------------------------------------------------------------------------

(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :notes "40 Act mutual fund onboarding - SEC registered RIC"
  :as @case)

(entity-workstream.create :case-id @case :entity-id @trust :as @ws-trust)
(entity-workstream.create :case-id @case :entity-id @advisor :as @ws-advisor)
(entity-workstream.create :case-id @case :entity-id @william :is-ubo true :ownership-percentage 55 :as @ws-william)
(entity-workstream.create :case-id @case :entity-id @jennifer :is-ubo true :ownership-percentage 45 :as @ws-jennifer)

(case-screening.run :workstream-id @ws-william :screening-type "PEP")
(case-screening.run :workstream-id @ws-william :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-jennifer :screening-type "PEP")
(case-screening.run :workstream-id @ws-jennifer :screening-type "SANCTIONS")

"#);
    }

    dsl.push_str(&format!(
        r#"
;; =============================================================================
;; SUMMARY: {fund_name}
;; Type: US 40 Act Mutual Fund (Registered Investment Company)
;; Structure: Massachusetts Business Trust + SEC-registered RIA
;; Board: 3 Independent + 2 Interested Trustees (40 Act compliant)
;; UBOs: William Thompson (55%), Jennifer Adams (45%) - Advisor ownership
;; Share Classes: A (retail), C (level load), I (institutional), R6 (retirement)
;; =============================================================================
"#
    ));

    dsl
}

// =============================================================================
// SEGREGATED PORTFOLIO COMPANY - Multi-strategy platform
// =============================================================================

fn generate_spc(params: &TemplateParams) -> String {
    let fund_name = &params.fund_name;
    let jurisdiction = params.jurisdiction.as_deref().unwrap_or("KY");

    let mut dsl = format!(
        r#";; =============================================================================
;; SEGREGATED PORTFOLIO COMPANY: {fund_name}
;; =============================================================================
;; Structure: Cayman SPC (Segregated Portfolio Company) platform
;;
;; Legal Structure:
;;   {fund_name} SPC (KY) ← core company
;;     ├── SP1: Real Estate Opportunities
;;     ├── SP2: Private Credit
;;     └── SP3: Venture Growth
;;
;; Key Feature: Each SP is legally ring-fenced - creditors of one SP have
;; no recourse to assets of other SPs or the core.
;;
;; Typical Use Cases:
;;   - Multi-manager platforms
;;   - Insurance linked securities
;;   - Structured finance vehicles
;;   - Family office investment platforms
;; =============================================================================

;; -----------------------------------------------------------------------------
;; SECTION 1: SPONSOR / PROMOTER
;; -----------------------------------------------------------------------------

(entity.create-limited-company
  :name "{fund_name} Sponsors Ltd"
  :jurisdiction "GB"
  :as @sponsor)

;; -----------------------------------------------------------------------------
;; SECTION 2: CBU
;; -----------------------------------------------------------------------------

(cbu.ensure
  :name "{fund_name}"
  :jurisdiction "{jurisdiction}"
  :client-type "FUND"
  :nature-purpose "Segregated Portfolio Company offering ring-fenced alternative investment strategies"
  :commercial-client-entity-id @sponsor
  :as @fund)

;; -----------------------------------------------------------------------------
;; SECTION 3: SPC CORE ENTITY
;; -----------------------------------------------------------------------------

(entity.create-limited-company
  :name "{fund_name} SPC"
  :jurisdiction "{jurisdiction}"
  :as @spc)

;; SPC = the umbrella vehicle (similar to hedge fund structure)
(cbu.assign-role :cbu-id @fund :entity-id @spc :role "PRINCIPAL")
(cbu.assign-role :cbu-id @fund :entity-id @spc :role "MASTER_FUND")

;; Sponsor = the PE/HF firm behind the SPC
(cbu.assign-role :cbu-id @fund :entity-id @sponsor :role "SPONSOR")
(cbu.assign-role :cbu-id @fund :entity-id @sponsor :role "INVESTMENT_MANAGER")

;; -----------------------------------------------------------------------------
;; SECTION 4: SEGREGATED PORTFOLIOS
;; -----------------------------------------------------------------------------

;; SP1: Real Estate Opportunities
(entity.create-limited-company
  :name "{fund_name} SP - Real Estate Opportunities"
  :jurisdiction "{jurisdiction}"
  :as @sp-realestate)

;; Each SP is like a feeder - holds assets for specific strategy
(cbu.assign-role :cbu-id @fund :entity-id @sp-realestate :role "SEGREGATED_PORTFOLIO")
(cbu.assign-role :cbu-id @fund :entity-id @sp-realestate :role "FEEDER_FUND")

;; SP2: Private Credit
(entity.create-limited-company
  :name "{fund_name} SP - Private Credit"
  :jurisdiction "{jurisdiction}"
  :as @sp-credit)

(cbu.assign-role :cbu-id @fund :entity-id @sp-credit :role "SEGREGATED_PORTFOLIO")
(cbu.assign-role :cbu-id @fund :entity-id @sp-credit :role "FEEDER_FUND")

;; SP3: Venture Growth
(entity.create-limited-company
  :name "{fund_name} SP - Venture Growth"
  :jurisdiction "{jurisdiction}"
  :as @sp-venture)

(cbu.assign-role :cbu-id @fund :entity-id @sp-venture :role "SEGREGATED_PORTFOLIO")
(cbu.assign-role :cbu-id @fund :entity-id @sp-venture :role "FEEDER_FUND")

;; -----------------------------------------------------------------------------
;; SECTION 5: SERVICE PROVIDERS
;; -----------------------------------------------------------------------------

;; Administrator
(entity.create-limited-company
  :name "Maples Fund Services (Cayman) Limited"
  :jurisdiction "{jurisdiction}"
  :as @administrator)

(cbu.assign-role :cbu-id @fund :entity-id @administrator :role "ADMINISTRATOR")

;; Custodian (for liquid assets)
(entity.create-limited-company
  :name "Credit Suisse AG, Cayman Islands Branch"
  :jurisdiction "{jurisdiction}"
  :as @custodian)

(cbu.assign-role :cbu-id @fund :entity-id @custodian :role "CUSTODIAN")

;; Auditor
(entity.create-limited-company
  :name "KPMG Cayman Islands"
  :jurisdiction "{jurisdiction}"
  :as @auditor)

(cbu.assign-role :cbu-id @fund :entity-id @auditor :role "AUDITOR")

;; Legal Counsel
(entity.create-limited-company
  :name "Walkers (Cayman)"
  :jurisdiction "{jurisdiction}"
  :as @legal)

(cbu.assign-role :cbu-id @fund :entity-id @legal :role "LEGAL_COUNSEL")

;; -----------------------------------------------------------------------------
;; SECTION 6: UBO STRUCTURE (Institutional Sponsor)
;; -----------------------------------------------------------------------------

;; Sponsor owned by family office
(entity.create-limited-company
  :name "Branson Family Office Ltd"
  :jurisdiction "GB"
  :as @family-office)

(cbu.assign-role :cbu-id @fund :entity-id @family-office :role "SHAREHOLDER")

(ubo.add-ownership
  :owner-entity-id @family-office
  :owned-entity-id @sponsor
  :percentage 100
  :ownership-type "DIRECT")

;; Ultimate UBO
(entity.create-proper-person
  :first-name "Richard"
  :last-name "Branson"
  :nationality "GB"
  :as @richard)

(cbu.assign-role :cbu-id @fund :entity-id @richard :role "BENEFICIAL_OWNER" :ownership-percentage 100)

(ubo.add-ownership
  :owner-entity-id @richard
  :owned-entity-id @family-office
  :percentage 100
  :ownership-type "DIRECT")

(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @spc
  :ubo-person-id @richard
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "CONTROL_VIA_SPONSOR"
  :ownership-percentage 100
  :workflow-type "ONBOARDING")

;; -----------------------------------------------------------------------------
;; SECTION 7: DIRECTORS
;; -----------------------------------------------------------------------------

(entity.create-proper-person
  :first-name "Andrew"
  :last-name "Campbell"
  :nationality "KY"
  :as @director1)

(entity.create-proper-person
  :first-name "Susan"
  :last-name "O'Connor"
  :nationality "IE"
  :as @director2)

(entity.create-proper-person
  :first-name "Gregory"
  :last-name "Hall"
  :nationality "GB"
  :as @director3)

(cbu.assign-role :cbu-id @fund :entity-id @director1 :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @director2 :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @director3 :role "DIRECTOR")

"#
    );

    // Products
    if params.include_products {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 8: PRODUCTS & SERVICES (per SP)
;; -----------------------------------------------------------------------------

;; Core Custody for liquid assets
(service-resource.provision
  :cbu-id @fund
  :resource-type "CUSTODY_ACCT"
  :instance-url "https://custody.cs.com/ky/spc"
  :as @custody-core)

;; TODO: (service-resource.set-attr :instance-id @custody-core :attr "account_name" :value "SPC Core Custody")

;; Fund Accounting for each SP
(service-resource.provision
  :cbu-id @fund
  :resource-type "FA_EAGLE"
  :instance-url "https://fa.maples.com/spc/realestate"
  :as @fa-realestate)

;; TODO: (service-resource.set-attr :instance-id @fa-realestate :attr "nav_frequency" :value "QUARTERLY")
;; TODO: (service-resource.set-attr :instance-id @fa-realestate :attr "portfolio" :value "Real Estate SP")

(service-resource.provision
  :cbu-id @fund
  :resource-type "FA_EAGLE"
  :instance-url "https://fa.maples.com/spc/credit"
  :as @fa-credit)

;; TODO: (service-resource.set-attr :instance-id @fa-credit :attr "nav_frequency" :value "MONTHLY")
;; TODO: (service-resource.set-attr :instance-id @fa-credit :attr "portfolio" :value "Private Credit SP")

"#);
    }

    // Share Classes
    if params.include_share_classes {
        dsl.push_str(
            r#"
;; -----------------------------------------------------------------------------
;; SECTION 9: PARTICIPATING SHARES (per SP)
;; -----------------------------------------------------------------------------

;; Real Estate SP - participating shares
(share-class.create
  :cbu-id @fund
  :entity-id @sp-realestate
  :name "Real Estate SP - Class A"
  :currency "USD"
  :class-category "FUND"
  :nav-per-share 1000.00
  :management-fee-bps 150
  :performance-fee-bps 2000
  :minimum-investment 500000.00
  :redemption-frequency "Quarterly"
  :redemption-notice-days 90
  :lock-up-period-months 24
  :as @re-class-a)

;; Private Credit SP - participating shares
(share-class.create
  :cbu-id @fund
  :entity-id @sp-credit
  :name "Private Credit SP - Class A"
  :currency "USD"
  :class-category "FUND"
  :nav-per-share 1000.00
  :management-fee-bps 125
  :performance-fee-bps 1500
  :minimum-investment 250000.00
  :redemption-frequency "Quarterly"
  :redemption-notice-days 60
  :as @credit-class-a)

;; Venture SP - participating shares (longer lock-up)
(share-class.create
  :cbu-id @fund
  :entity-id @sp-venture
  :name "Venture Growth SP - Class A"
  :currency "USD"
  :class-category "FUND"
  :nav-per-share 1000.00
  :management-fee-bps 200
  :performance-fee-bps 2000
  :high-water-mark true
  :minimum-investment 1000000.00
  :lock-up-period-months 36
  :as @venture-class-a)

"#,
        );
    }

    // KYC
    if params.include_kyc {
        dsl.push_str(r#"
;; -----------------------------------------------------------------------------
;; SECTION 10: KYC CASE
;; -----------------------------------------------------------------------------

(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :notes "SPC platform onboarding - core company plus segregated portfolios"
  :as @case)

(entity-workstream.create :case-id @case :entity-id @spc :as @ws-spc)
(entity-workstream.create :case-id @case :entity-id @sponsor :as @ws-sponsor)
(entity-workstream.create :case-id @case :entity-id @family-office :discovery-reason "SHAREHOLDER" :as @ws-fo)
(entity-workstream.create :case-id @case :entity-id @richard :is-ubo true :ownership-percentage 100 :as @ws-richard)

(case-screening.run :workstream-id @ws-richard :screening-type "PEP")
(case-screening.run :workstream-id @ws-richard :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-richard :screening-type "ADVERSE_MEDIA")
(case-screening.run :workstream-id @ws-fo :screening-type "SANCTIONS")

"#);
    }

    dsl.push_str(&format!(
        r#"
;; =============================================================================
;; SUMMARY: {fund_name}
;; Type: Cayman Segregated Portfolio Company
;; Structure: Core SPC + 3 Ring-fenced Segregated Portfolios
;; Portfolios: Real Estate, Private Credit, Venture Growth
;; UBO: Richard Branson (100%) via Family Office → Sponsor
;; Key Feature: Legal segregation between portfolios
;; =============================================================================
"#
    ));

    dsl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hedge_fund_template() {
        let params = TemplateParams {
            fund_name: "Test Alpha Fund".to_string(),
            include_kyc: true,
            include_share_classes: true,
            include_products: true,
            ..Default::default()
        };
        let dsl = generate_template(TemplateType::HedgeFund, &params);
        assert!(dsl.contains("Test Alpha Fund"));
        assert!(dsl.contains("cbu.ensure"));
        assert!(dsl.contains("GENERAL_PARTNER"));
        assert!(dsl.contains("Master Fund LP"));
        assert!(dsl.contains("Offshore Feeder"));
        assert!(dsl.contains("kyc-case.create"));
        assert!(dsl.contains("service-resource.provision"));
    }

    #[test]
    fn test_lux_sicav_template() {
        let params = TemplateParams {
            fund_name: "Europa SICAV".to_string(),
            include_kyc: false,
            include_share_classes: true,
            include_products: true,
            ..Default::default()
        };
        let dsl = generate_template(TemplateType::LuxSicav, &params);
        assert!(dsl.contains("Europa SICAV"));
        assert!(dsl.contains("MANAGEMENT_COMPANY"));
        assert!(dsl.contains("DEPOSITARY"));
        assert!(dsl.contains("share-class.create"));
        assert!(dsl.contains("UCITS"));
        assert!(dsl.contains("CONDUCTING_OFFICER"));
    }

    #[test]
    fn test_us_40_act_template() {
        let params = TemplateParams {
            fund_name: "American Growth Fund".to_string(),
            include_kyc: true,
            include_share_classes: true,
            ..Default::default()
        };
        let dsl = generate_template(TemplateType::Us40Act, &params);
        assert!(dsl.contains("American Growth Fund"));
        assert!(dsl.contains("INDEPENDENT_TRUSTEE"));
        assert!(dsl.contains("INTERESTED_TRUSTEE"));
        assert!(dsl.contains("CHIEF_COMPLIANCE_OFFICER"));
        assert!(dsl.contains("Class R6"));
    }

    #[test]
    fn test_spc_template() {
        let params = TemplateParams {
            fund_name: "Multi-Strategy Platform".to_string(),
            include_products: true,
            ..Default::default()
        };
        let dsl = generate_template(TemplateType::Spc, &params);
        assert!(dsl.contains("Multi-Strategy Platform"));
        assert!(dsl.contains("SEGREGATED_PORTFOLIO"));
        assert!(dsl.contains("Real Estate"));
        assert!(dsl.contains("Private Credit"));
        assert!(dsl.contains("Venture Growth"));
    }

    #[test]
    fn test_template_type_from_str() {
        assert_eq!(
            TemplateType::from_str("hedge_fund"),
            Some(TemplateType::HedgeFund)
        );
        assert_eq!(TemplateType::from_str("hf"), Some(TemplateType::HedgeFund));
        assert_eq!(
            TemplateType::from_str("sicav"),
            Some(TemplateType::LuxSicav)
        );
        assert_eq!(
            TemplateType::from_str("ucits"),
            Some(TemplateType::LuxSicav)
        );
        assert_eq!(TemplateType::from_str("40act"), Some(TemplateType::Us40Act));
        assert_eq!(
            TemplateType::from_str("mutual"),
            Some(TemplateType::Us40Act)
        );
        assert_eq!(TemplateType::from_str("spc"), Some(TemplateType::Spc));
        assert_eq!(TemplateType::from_str("unknown"), None);
    }
}
