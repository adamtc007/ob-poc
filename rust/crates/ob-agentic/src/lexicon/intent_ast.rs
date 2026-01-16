//! Intent AST types for the nom grammar parser.
//!
//! The IntentAst represents the structured output of parsing natural language
//! intent. Each variant corresponds to a specific DSL operation or query.

use super::tokenizer::ResolvedEntity;
use super::tokens::VerbClass;

/// Resolved or unresolved entity reference.
#[derive(Debug, Clone)]
pub enum EntityRef {
    /// Fully resolved entity with UUID.
    Resolved(ResolvedEntity),

    /// Unresolved entity (name only, needs resolution).
    Unresolved {
        name: String,
        entity_type: Option<String>,
    },

    /// Pronoun reference (resolved from session context).
    Pronoun {
        text: String,
        referent: Option<ResolvedEntity>,
    },
}

impl EntityRef {
    /// Get the display name of this entity.
    pub fn name(&self) -> &str {
        match self {
            EntityRef::Resolved(r) => &r.name,
            EntityRef::Unresolved { name, .. } => name,
            EntityRef::Pronoun { text, referent } => {
                referent.as_ref().map(|r| r.name.as_str()).unwrap_or(text)
            }
        }
    }

    /// Get the resolved ID if available.
    pub fn id(&self) -> Option<&str> {
        match self {
            EntityRef::Resolved(r) => Some(&r.id),
            EntityRef::Pronoun { referent, .. } => referent.as_ref().map(|r| r.id.as_str()),
            EntityRef::Unresolved { .. } => None,
        }
    }

    /// Check if this entity is resolved.
    pub fn is_resolved(&self) -> bool {
        match self {
            EntityRef::Resolved(_) => true,
            EntityRef::Pronoun { referent, .. } => referent.is_some(),
            EntityRef::Unresolved { .. } => false,
        }
    }
}

/// Governing law for ISDA agreements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoverningLaw {
    NewYork,
    English,
    German,
    French,
    Singapore,
    HongKong,
    Japanese,
}

impl GoverningLaw {
    /// Parse from string representation.
    pub fn parse(s: &str) -> Option<Self> {
        let s_lower = s.to_lowercase();
        if s_lower.contains("ny") || s_lower.contains("new york") {
            Some(GoverningLaw::NewYork)
        } else if s_lower.contains("english") || s_lower.contains("uk") {
            Some(GoverningLaw::English)
        } else if s_lower.contains("german") {
            Some(GoverningLaw::German)
        } else if s_lower.contains("french") {
            Some(GoverningLaw::French)
        } else if s_lower.contains("singapore") {
            Some(GoverningLaw::Singapore)
        } else if s_lower.contains("hong kong") || s_lower.contains("hk") {
            Some(GoverningLaw::HongKong)
        } else if s_lower.contains("japan") {
            Some(GoverningLaw::Japanese)
        } else {
            None
        }
    }

    /// Get the DSL code for this law.
    pub fn code(&self) -> &'static str {
        match self {
            GoverningLaw::NewYork => "NY_LAW",
            GoverningLaw::English => "ENGLISH_LAW",
            GoverningLaw::German => "GERMAN_LAW",
            GoverningLaw::French => "FRENCH_LAW",
            GoverningLaw::Singapore => "SINGAPORE_LAW",
            GoverningLaw::HongKong => "HONG_KONG_LAW",
            GoverningLaw::Japanese => "JAPANESE_LAW",
        }
    }
}

/// CSA (Credit Support Annex) type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsaType {
    /// Variation Margin only.
    Vm,
    /// Initial Margin only.
    Im,
    /// Two-way margin (both VM and IM).
    TwoWay,
    /// Title transfer (ownership transfers).
    TitleTransfer,
    /// Pledge (security interest).
    Pledge,
}

impl CsaType {
    /// Parse from string representation.
    pub fn parse(s: &str) -> Option<Self> {
        let s_lower = s.to_lowercase();
        if s_lower == "vm" || s_lower.contains("variation margin") {
            Some(CsaType::Vm)
        } else if s_lower == "im" || s_lower.contains("initial margin") {
            Some(CsaType::Im)
        } else if s_lower.contains("two") && s_lower.contains("way") {
            Some(CsaType::TwoWay)
        } else if s_lower.contains("title") {
            Some(CsaType::TitleTransfer)
        } else if s_lower.contains("pledge") {
            Some(CsaType::Pledge)
        } else {
            None
        }
    }

    /// Get the DSL code for this type.
    pub fn code(&self) -> &'static str {
        match self {
            CsaType::Vm => "VM",
            CsaType::Im => "IM",
            CsaType::TwoWay => "TWO_WAY",
            CsaType::TitleTransfer => "TITLE_TRANSFER",
            CsaType::Pledge => "PLEDGE",
        }
    }
}

/// Instrument type code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentCode(pub String);

impl InstrumentCode {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into().to_uppercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Role assignment in a CBU context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleCode(pub String);

impl RoleCode {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into().to_uppercase().replace(' ', "_"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Market identifier (MIC code).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketCode(pub String);

impl MarketCode {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into().to_uppercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Currency code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyCode(pub String);

impl CurrencyCode {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into().to_uppercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The main Intent AST enum.
///
/// Each variant represents a specific intent that can be mapped to DSL operations.
#[derive(Debug, Clone)]
pub enum IntentAst {
    // ========== OTC Derivatives Domain ==========
    /// Add a counterparty for OTC trading.
    CounterpartyCreate {
        counterparty: EntityRef,
        instruments: Vec<InstrumentCode>,
        governing_law: Option<GoverningLaw>,
    },

    /// Establish an ISDA master agreement.
    IsdaEstablish {
        counterparty: EntityRef,
        governing_law: GoverningLaw,
        instruments: Vec<InstrumentCode>,
    },

    /// Add a CSA to an existing ISDA.
    CsaAdd {
        counterparty: EntityRef,
        csa_type: CsaType,
        currency: Option<CurrencyCode>,
    },

    /// Add instrument coverage to an ISDA.
    IsdaAddCoverage {
        counterparty: EntityRef,
        instruments: Vec<InstrumentCode>,
    },

    // ========== Exchange-Traded Domain ==========
    /// Add trading universe (markets/instruments).
    UniverseAdd {
        cbu: EntityRef,
        markets: Vec<MarketCode>,
        instruments: Vec<InstrumentCode>,
        currencies: Vec<CurrencyCode>,
    },

    /// Create SSI (Standing Settlement Instructions).
    SsiCreate {
        cbu: EntityRef,
        market: MarketCode,
        currency: CurrencyCode,
        custodian: Option<EntityRef>,
    },

    /// Add booking rule.
    BookingRuleAdd {
        cbu: EntityRef,
        market: Option<MarketCode>,
        instrument: Option<InstrumentCode>,
        currency: Option<CurrencyCode>,
        ssi: EntityRef,
    },

    // ========== Entity Management ==========
    /// Assign a role to an entity within a CBU.
    RoleAssign {
        cbu: EntityRef,
        entity: EntityRef,
        role: RoleCode,
    },

    /// Remove a role from an entity.
    RoleRemove {
        cbu: EntityRef,
        entity: EntityRef,
        role: RoleCode,
    },

    /// Create a new entity (person, company, etc.).
    EntityCreate {
        entity_type: String,
        name: String,
        attributes: Vec<(String, String)>,
    },

    // ========== Product/Service Management ==========
    /// Add a product to a CBU.
    ProductAdd { cbu: EntityRef, product: String },

    /// Provision a service resource.
    ServiceProvision {
        cbu: EntityRef,
        service: String,
        resource_type: String,
    },

    // ========== Query Intents ==========
    /// List entities matching criteria.
    EntityList {
        entity_type: Option<String>,
        filters: Vec<(String, String)>,
    },

    /// Show details of an entity.
    EntityShow { entity: EntityRef },

    /// List counterparties for a CBU.
    CounterpartyList { cbu: Option<EntityRef> },

    /// Show ISDA details.
    /// - If counterparty is Some, show ISDA for that counterparty.
    /// - If counterparty is None, list all ISDAs.
    IsdaShow { counterparty: Option<EntityRef> },

    // ========== Trading Profile Domain ==========
    /// Materialize a trading profile (apply draft to operational tables).
    TradingProfileMaterialize { cbu: EntityRef },

    /// Add an instrument class to trading profile.
    TradingProfileAddInstrument {
        cbu: EntityRef,
        instrument: InstrumentCode,
    },

    /// Add a market to trading profile.
    TradingProfileAddMarket { cbu: EntityRef, market: MarketCode },

    /// Submit trading profile for approval.
    TradingProfileSubmit { cbu: EntityRef },

    /// Approve a submitted trading profile.
    TradingProfileApprove { cbu: EntityRef },

    /// Validate trading profile is ready for go-live.
    TradingProfileValidate { cbu: EntityRef },

    // ========== Session/Navigation Domain ==========
    /// Load a CBU into the session.
    SessionLoadCbu { cbu: EntityRef },

    /// Load all CBUs in a jurisdiction.
    SessionLoadJurisdiction { jurisdiction: String },

    /// Load all CBUs under an apex entity (galaxy).
    SessionLoadGalaxy { apex: EntityRef },

    /// Clear the session (unload all CBUs).
    SessionClear,

    /// Undo last session action.
    SessionUndo,

    /// Redo undone session action.
    SessionRedo,

    // ========== View/Visualization Domain (ESPER) ==========
    /// Drill into an entity (zoom in).
    ViewDrill { entity: EntityRef },

    /// Surface up from drill (zoom out).
    ViewSurface,

    /// Trace relationships (money, control, risk, documents).
    ViewTrace {
        entity: EntityRef,
        trace_type: String,
    },

    /// X-ray view (show hidden layers).
    ViewXray { layer: String },

    /// Illuminate/highlight an aspect.
    ViewIlluminate { aspect: String },

    // ========== KYC/Compliance Domain ==========
    /// Create a KYC case for a CBU.
    KycCaseCreate {
        cbu: EntityRef,
        case_type: Option<String>,
    },

    /// Escalate a KYC case.
    KycCaseEscalate {
        cbu: EntityRef,
        reason: Option<String>,
    },

    /// Assign a KYC case to a user.
    KycCaseAssign { cbu: EntityRef, assignee: EntityRef },

    /// Close a KYC case.
    KycCaseClose { cbu: EntityRef },

    /// Set risk rating for a CBU.
    KycSetRiskRating { cbu: EntityRef, rating: String },

    // ========== Ownership/UBO Domain ==========
    /// Compute ownership/UBO for an entity.
    OwnershipCompute { entity: EntityRef },

    /// Who controls this entity?
    OwnershipWhoControls { entity: EntityRef },

    /// Trace the ownership chain.
    OwnershipTraceChain {
        entity: EntityRef,
        depth: Option<i32>,
    },

    /// Analyze ownership gaps.
    OwnershipAnalyzeGaps { entity: EntityRef },

    // ========== GLEIF/Research Domain ==========
    /// Import GLEIF/LEI data for an entity.
    GleifImport { entity: EntityRef },

    /// Import GLEIF ownership tree.
    GleifImportTree { lei: String },

    /// Search GLEIF by name.
    GleifSearch { query: String },

    /// Lookup LEI details.
    GleifLookup { lei: String },

    // ========== BODS/Beneficial Ownership Domain ==========
    /// Discover UBOs for an entity.
    BodsDiscoverUbos { entity: EntityRef },

    /// Import BODS statement.
    BodsImport { entity: EntityRef },

    // ========== Investor Register/Cap Table Domain ==========
    /// Show investor register / cap table.
    InvestorRegisterShow { entity: EntityRef },

    /// List holders of an entity.
    InvestorRegisterListHolders {
        entity: EntityRef,
        threshold_pct: Option<f64>,
    },

    // ========== Fallback ==========
    /// Unrecognized intent (requires clarification).
    Unknown {
        verb_class: Option<VerbClass>,
        raw_text: String,
    },
}

impl IntentAst {
    /// Get the primary verb class for this intent.
    pub fn verb_class(&self) -> VerbClass {
        match self {
            // Create operations
            IntentAst::CounterpartyCreate { .. }
            | IntentAst::IsdaEstablish { .. }
            | IntentAst::CsaAdd { .. }
            | IntentAst::UniverseAdd { .. }
            | IntentAst::SsiCreate { .. }
            | IntentAst::BookingRuleAdd { .. }
            | IntentAst::EntityCreate { .. }
            | IntentAst::ProductAdd { .. }
            | IntentAst::ServiceProvision { .. }
            | IntentAst::KycCaseCreate { .. } => VerbClass::Create,

            // Update operations
            IntentAst::IsdaAddCoverage { .. }
            | IntentAst::TradingProfileAddInstrument { .. }
            | IntentAst::TradingProfileAddMarket { .. }
            | IntentAst::KycSetRiskRating { .. } => VerbClass::Update,

            // Link operations
            IntentAst::RoleAssign { .. } | IntentAst::KycCaseAssign { .. } => VerbClass::Link,

            // Unlink operations
            IntentAst::RoleRemove { .. } => VerbClass::Unlink,

            // Query operations
            IntentAst::EntityList { .. }
            | IntentAst::EntityShow { .. }
            | IntentAst::CounterpartyList { .. }
            | IntentAst::IsdaShow { .. }
            | IntentAst::InvestorRegisterShow { .. }
            | IntentAst::InvestorRegisterListHolders { .. }
            | IntentAst::GleifLookup { .. }
            | IntentAst::GleifSearch { .. } => VerbClass::Query,

            // Trading profile operations
            IntentAst::TradingProfileMaterialize { .. }
            | IntentAst::TradingProfileSubmit { .. }
            | IntentAst::TradingProfileApprove { .. }
            | IntentAst::TradingProfileValidate { .. } => VerbClass::TradingProfile,

            // Session operations
            IntentAst::SessionLoadCbu { .. }
            | IntentAst::SessionLoadJurisdiction { .. }
            | IntentAst::SessionLoadGalaxy { .. }
            | IntentAst::SessionClear
            | IntentAst::SessionUndo
            | IntentAst::SessionRedo => VerbClass::Session,

            // View/ESPER operations
            IntentAst::ViewDrill { .. }
            | IntentAst::ViewSurface
            | IntentAst::ViewTrace { .. }
            | IntentAst::ViewXray { .. }
            | IntentAst::ViewIlluminate { .. } => VerbClass::View,

            // KYC operations (non-create)
            IntentAst::KycCaseEscalate { .. } | IntentAst::KycCaseClose { .. } => VerbClass::Kyc,

            // Ownership operations
            IntentAst::OwnershipCompute { .. }
            | IntentAst::OwnershipWhoControls { .. }
            | IntentAst::OwnershipTraceChain { .. }
            | IntentAst::OwnershipAnalyzeGaps { .. } => VerbClass::Ownership,

            // Research operations
            IntentAst::GleifImport { .. }
            | IntentAst::GleifImportTree { .. }
            | IntentAst::BodsDiscoverUbos { .. }
            | IntentAst::BodsImport { .. } => VerbClass::Research,

            IntentAst::Unknown { verb_class, .. } => verb_class.unwrap_or(VerbClass::Query),
        }
    }

    /// Check if this intent is in the OTC domain.
    pub fn is_otc_domain(&self) -> bool {
        matches!(
            self,
            IntentAst::CounterpartyCreate { .. }
                | IntentAst::IsdaEstablish { .. }
                | IntentAst::CsaAdd { .. }
                | IntentAst::IsdaAddCoverage { .. }
                | IntentAst::CounterpartyList { .. }
                | IntentAst::IsdaShow { .. }
        )
    }

    /// Check if this intent is in the exchange-traded domain.
    pub fn is_exchange_domain(&self) -> bool {
        matches!(
            self,
            IntentAst::UniverseAdd { .. }
                | IntentAst::SsiCreate { .. }
                | IntentAst::BookingRuleAdd { .. }
        )
    }

    /// Get the DSL domain for this intent.
    pub fn dsl_domain(&self) -> &'static str {
        match self {
            // OTC/ISDA domain
            IntentAst::CounterpartyCreate { .. } => "entity",
            IntentAst::IsdaEstablish { .. } => "isda",
            IntentAst::CsaAdd { .. } => "isda",
            IntentAst::IsdaAddCoverage { .. } => "isda",
            IntentAst::IsdaShow { .. } => "isda",

            // Exchange-traded domain
            IntentAst::UniverseAdd { .. } => "cbu-custody",
            IntentAst::SsiCreate { .. } => "cbu-custody",
            IntentAst::BookingRuleAdd { .. } => "cbu-custody",

            // Entity/role domain
            IntentAst::RoleAssign { .. } => "cbu",
            IntentAst::RoleRemove { .. } => "cbu",
            IntentAst::EntityCreate { .. } => "entity",
            IntentAst::EntityList { .. } => "entity",
            IntentAst::EntityShow { .. } => "entity",
            IntentAst::CounterpartyList { .. } => "entity",

            // Product/service domain
            IntentAst::ProductAdd { .. } => "cbu",
            IntentAst::ServiceProvision { .. } => "service-resource",

            // Trading profile domain
            IntentAst::TradingProfileMaterialize { .. } => "trading-profile",
            IntentAst::TradingProfileAddInstrument { .. } => "trading-profile",
            IntentAst::TradingProfileAddMarket { .. } => "trading-profile",
            IntentAst::TradingProfileSubmit { .. } => "trading-profile",
            IntentAst::TradingProfileApprove { .. } => "trading-profile",
            IntentAst::TradingProfileValidate { .. } => "trading-profile",

            // Session domain
            IntentAst::SessionLoadCbu { .. } => "session",
            IntentAst::SessionLoadJurisdiction { .. } => "session",
            IntentAst::SessionLoadGalaxy { .. } => "session",
            IntentAst::SessionClear => "session",
            IntentAst::SessionUndo => "session",
            IntentAst::SessionRedo => "session",

            // View domain
            IntentAst::ViewDrill { .. } => "view",
            IntentAst::ViewSurface => "view",
            IntentAst::ViewTrace { .. } => "view",
            IntentAst::ViewXray { .. } => "view",
            IntentAst::ViewIlluminate { .. } => "view",

            // KYC domain
            IntentAst::KycCaseCreate { .. } => "kyc-case",
            IntentAst::KycCaseEscalate { .. } => "kyc-case",
            IntentAst::KycCaseAssign { .. } => "kyc-case",
            IntentAst::KycCaseClose { .. } => "kyc-case",
            IntentAst::KycSetRiskRating { .. } => "kyc-case",

            // Ownership domain
            IntentAst::OwnershipCompute { .. } => "ownership",
            IntentAst::OwnershipWhoControls { .. } => "ownership",
            IntentAst::OwnershipTraceChain { .. } => "ownership",
            IntentAst::OwnershipAnalyzeGaps { .. } => "ownership",

            // GLEIF domain
            IntentAst::GleifImport { .. } => "gleif",
            IntentAst::GleifImportTree { .. } => "gleif",
            IntentAst::GleifSearch { .. } => "gleif",
            IntentAst::GleifLookup { .. } => "gleif",

            // BODS domain
            IntentAst::BodsDiscoverUbos { .. } => "bods",
            IntentAst::BodsImport { .. } => "bods",

            // Investor register domain
            IntentAst::InvestorRegisterShow { .. } => "capital",
            IntentAst::InvestorRegisterListHolders { .. } => "capital",

            IntentAst::Unknown { .. } => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governing_law_parse() {
        assert_eq!(GoverningLaw::parse("NY law"), Some(GoverningLaw::NewYork));
        assert_eq!(
            GoverningLaw::parse("English law"),
            Some(GoverningLaw::English)
        );
        assert_eq!(GoverningLaw::parse("unknown"), None);
    }

    #[test]
    fn test_csa_type_parse() {
        assert_eq!(CsaType::parse("VM"), Some(CsaType::Vm));
        assert_eq!(CsaType::parse("variation margin"), Some(CsaType::Vm));
        assert_eq!(CsaType::parse("two-way"), Some(CsaType::TwoWay));
    }

    #[test]
    fn test_intent_domain() {
        let intent = IntentAst::IsdaEstablish {
            counterparty: EntityRef::Unresolved {
                name: "Test".to_string(),
                entity_type: None,
            },
            governing_law: GoverningLaw::NewYork,
            instruments: vec![],
        };

        assert!(intent.is_otc_domain());
        assert!(!intent.is_exchange_domain());
        assert_eq!(intent.dsl_domain(), "isda");
    }
}
