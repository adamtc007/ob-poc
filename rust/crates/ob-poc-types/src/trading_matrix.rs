//! Trading Matrix AST Types
//!
//! This module defines the canonical type hierarchy for Trading Matrix documents.
//! The document IS the AST - a typed tree structure that is:
//!
//! 1. Built incrementally by DSL verb execution
//! 2. Stored as-is in JSONB
//! 3. Served directly to UI without SQL reconstruction
//! 4. Rendered using these same types in client WASM
//!
//! ## Design Philosophy
//!
//! The trading matrix is a **mini-language with AST**. The taxonomy is a formally
//! defined struct that cannot be built or proven correct by other means. NOM parsing
//! validates DSL commands, and the executor builds the typed tree incrementally.
//!
//! ## Tree Structure
//!
//! ```text
//! TradingMatrixDocument
//! └── children: Vec<TradingMatrixNode>
//!     ├── Category("Trading Universe")
//!     │   ├── InstrumentClass("EQUITY")
//!     │   │   ├── Market("XNYS")
//!     │   │   │   └── UniverseEntry(...)
//!     │   │   └── Market("XLON")
//!     │   └── InstrumentClass("OTC_IRS")
//!     │       └── Counterparty("Goldman Sachs")
//!     ├── Category("Settlement Instructions")
//!     │   └── Ssi("US Equities SSI")
//!     │       └── BookingRule("US Equity DVP")
//!     ├── Category("Settlement Chains")
//!     │   └── SettlementChain("US→EU Cross-Border")
//!     │       └── SettlementHop(...)
//!     ├── Category("Tax Configuration")
//!     │   └── TaxJurisdiction("DE")
//!     │       └── TaxConfig(...)
//!     └── Category("ISDA Agreements")
//!         └── IsdaAgreement("Goldman Sachs")
//!             └── CsaAgreement("VM CSA")
//! ```

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ============================================================================
// NODE ID
// ============================================================================

/// Path-based identifier for tree navigation.
///
/// The ID is a vector of path segments that uniquely identifies a node's
/// position in the tree. For example:
/// - `["_UNIVERSE"]` - Trading Universe category
/// - `["_UNIVERSE", "EQUITY"]` - Equity instrument class
/// - `["_UNIVERSE", "EQUITY", "XNYS"]` - NYSE market under equities
/// - `["_UNIVERSE", "EQUITY", "XNYS", "abc123"]` - Universe entry
///
/// This path-based approach enables:
/// - Stable IDs across re-renders
/// - O(1) parent lookup (just pop the last segment)
/// - Natural breadcrumb navigation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TradingMatrixNodeId(pub Vec<String>);

impl TradingMatrixNodeId {
    /// Create a new node ID from path segments
    pub fn new(segments: Vec<String>) -> Self {
        Self(segments)
    }

    /// Create a child ID by appending a segment
    pub fn child(&self, segment: impl Into<String>) -> Self {
        let mut segments = self.0.clone();
        segments.push(segment.into());
        Self(segments)
    }

    /// Get parent ID (or None if at root)
    pub fn parent(&self) -> Option<Self> {
        if self.0.len() <= 1 {
            None
        } else {
            let mut segments = self.0.clone();
            segments.pop();
            Some(Self(segments))
        }
    }

    /// Check if this ID is a direct child of another
    pub fn is_child_of(&self, parent: &Self) -> bool {
        self.0.len() == parent.0.len() + 1 && self.0[..parent.0.len()] == parent.0[..]
    }

    /// Get the last segment (node's own identifier)
    pub fn last_segment(&self) -> Option<&str> {
        self.0.last().map(|s| s.as_str())
    }

    /// Get depth (0 = root category)
    pub fn depth(&self) -> usize {
        self.0.len().saturating_sub(1)
    }

    /// Create a root ID for a category
    pub fn category(name: &str) -> Self {
        Self(vec![format!("_{}", name.to_uppercase())])
    }
}

impl From<Vec<String>> for TradingMatrixNodeId {
    fn from(segments: Vec<String>) -> Self {
        Self(segments)
    }
}

impl AsRef<[String]> for TradingMatrixNodeId {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

// ============================================================================
// CORPORATE ACTIONS TYPES
// ============================================================================

/// Who makes CA elections.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaElector {
    /// Investment manager makes elections
    #[default]
    InvestmentManager,
    /// Fund administrator makes elections
    Admin,
    /// Client/investor makes elections directly
    Client,
}

/// Type of CA proceeds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaProceedsType {
    /// Cash proceeds
    Cash,
    /// Stock/securities proceeds
    Stock,
}

/// CA notification policy settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaNotificationPolicy {
    /// Notification channels: email, portal, swift
    pub channels: Vec<String>,
    /// SLA hours for notification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sla_hours: Option<i32>,
    /// Contact for escalation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escalation_contact: Option<String>,
}

/// CA election policy settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaElectionPolicy {
    /// Who makes the election decision
    pub elector: CaElector,
    /// Whether evidence/documentation is required
    #[serde(default)]
    pub evidence_required: bool,
    /// Value threshold below which auto-instruct applies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_instruct_threshold: Option<Decimal>,
}

/// Default election option for a specific event type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaDefaultOption {
    /// Event type code (e.g., "DVOP", "RHTS")
    pub event_type: String,
    /// Default option: CASH, STOCK, ROLLOVER, LAPSE, DECLINE
    pub default_option: String,
}

/// Cutoff rule for specific market/depository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaCutoffRule {
    /// Optional event type (if rule is event-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    /// Market MIC code (e.g., "XNYS")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_code: Option<String>,
    /// Depository code (e.g., "DTCC", "CREST")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depository_code: Option<String>,
    /// Days before market deadline to set internal cutoff
    pub days_before: i32,
    /// Days before cutoff to send warning
    #[serde(default = "default_warning_days")]
    pub warning_days: i32,
    /// Days before cutoff to escalate
    #[serde(default = "default_escalation_days")]
    pub escalation_days: i32,
}

fn default_warning_days() -> i32 {
    3
}

fn default_escalation_days() -> i32 {
    1
}

/// Mapping of CA proceeds to a settlement instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaProceedsSsiMapping {
    /// Type of proceeds (cash or stock)
    pub proceeds_type: CaProceedsType,
    /// Currency code (if currency-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// SSI name or reference
    pub ssi_reference: String,
}

/// Corporate Actions section of the trading matrix.
///
/// This represents the CA policy configuration stored in the matrix JSONB.
/// Intent verbs write here; materialize projects to operational tables.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TradingMatrixCorporateActions {
    /// Enabled event type codes (e.g., ["DVCA", "DVOP", "RHTS"])
    #[serde(default)]
    pub enabled_event_types: Vec<String>,
    /// Notification policy settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_policy: Option<CaNotificationPolicy>,
    /// Election policy settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub election_policy: Option<CaElectionPolicy>,
    /// Default election options per event type
    #[serde(default)]
    pub default_options: Vec<CaDefaultOption>,
    /// Cutoff/deadline rules
    #[serde(default)]
    pub cutoff_rules: Vec<CaCutoffRule>,
    /// Proceeds SSI mappings
    #[serde(default)]
    pub proceeds_ssi_mappings: Vec<CaProceedsSsiMapping>,
}

// ============================================================================
// STATUS COLOR
// ============================================================================

/// Visual status indicator for nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StatusColor {
    /// Good/active/verified
    Green,
    /// Warning/pending/needs attention
    Yellow,
    /// Error/suspended/blocked
    Red,
    /// Inactive/disabled/historical
    #[default]
    Gray,
}

// ============================================================================
// NODE TYPE (TAGGED ENUM)
// ============================================================================

/// Node type discriminator with type-specific metadata.
///
/// This is the heart of the trading matrix AST. Each variant represents
/// a specific type of node with its associated data. The enum is tagged
/// with `type` for JSON serialization compatibility with TypeScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TradingMatrixNodeType {
    // ========================================================================
    // CATEGORY NODES (virtual groupings)
    // ========================================================================
    /// A category grouping (e.g., "Trading Universe", "ISDA Agreements")
    Category {
        /// Category name
        name: String,
    },

    // ========================================================================
    // UNIVERSE LAYER
    // ========================================================================
    /// An instrument class (e.g., EQUITY, GOVT_BOND, OTC_IRS)
    InstrumentClass {
        /// Class code (e.g., "EQUITY")
        class_code: String,
        /// CFI code prefix (e.g., "ES" for equities)
        #[serde(skip_serializing_if = "Option::is_none")]
        cfi_prefix: Option<String>,
        /// Whether this is an OTC instrument class
        is_otc: bool,
    },

    /// A market/exchange (ISO 10383 MIC)
    Market {
        /// Market Identifier Code (e.g., "XNYS")
        mic: String,
        /// Market name (e.g., "New York Stock Exchange")
        market_name: String,
        /// ISO country code (e.g., "US")
        country_code: String,
    },

    /// A counterparty entity (for OTC instruments)
    Counterparty {
        /// Entity UUID
        entity_id: String,
        /// Entity name
        entity_name: String,
        /// LEI (if available)
        #[serde(skip_serializing_if = "Option::is_none")]
        lei: Option<String>,
    },

    /// A universe entry (specific tradeable combination)
    UniverseEntry {
        /// Universe entry UUID
        universe_id: String,
        /// Allowed currencies for this entry
        currencies: Vec<String>,
        /// Settlement types (e.g., ["DVP", "FOP"])
        settlement_types: Vec<String>,
        /// Whether positions are held
        is_held: bool,
        /// Whether actively traded
        is_traded: bool,
    },

    // ========================================================================
    // SSI LAYER
    // ========================================================================
    /// Standing Settlement Instruction
    Ssi {
        /// SSI UUID
        ssi_id: String,
        /// SSI name (e.g., "US Equities SSI")
        ssi_name: String,
        /// SSI type: SECURITIES, CASH, COLLATERAL
        ssi_type: String,
        /// Status: PENDING, ACTIVE, SUSPENDED
        status: String,
        /// Safekeeping account number
        #[serde(skip_serializing_if = "Option::is_none")]
        safekeeping_account: Option<String>,
        /// Safekeeping custodian BIC
        #[serde(skip_serializing_if = "Option::is_none")]
        safekeeping_bic: Option<String>,
        /// Cash account number
        #[serde(skip_serializing_if = "Option::is_none")]
        cash_account: Option<String>,
        /// Cash agent BIC
        #[serde(skip_serializing_if = "Option::is_none")]
        cash_bic: Option<String>,
        /// Place of settlement BIC
        #[serde(skip_serializing_if = "Option::is_none")]
        pset_bic: Option<String>,
        /// Cash currency
        #[serde(skip_serializing_if = "Option::is_none")]
        cash_currency: Option<String>,
    },

    /// ALERT-style booking rule for SSI routing
    BookingRule {
        /// Rule UUID
        rule_id: String,
        /// Rule name
        rule_name: String,
        /// Priority (lower = higher priority)
        priority: i32,
        /// Specificity score (higher = more specific match criteria)
        specificity_score: i32,
        /// Whether rule is active
        is_active: bool,
        /// Match criteria (for display/debugging)
        #[serde(skip_serializing_if = "Option::is_none")]
        match_criteria: Option<BookingMatchCriteria>,
    },

    // ========================================================================
    // SETTLEMENT CHAIN LAYER
    // ========================================================================
    /// A multi-hop settlement chain
    SettlementChain {
        /// Chain UUID
        chain_id: String,
        /// Chain name (e.g., "US→EU Cross-Border")
        chain_name: String,
        /// Number of intermediary hops
        hop_count: usize,
        /// Whether chain is active
        is_active: bool,
        /// Market MIC (if market-specific)
        #[serde(skip_serializing_if = "Option::is_none")]
        mic: Option<String>,
        /// Currency (if currency-specific)
        #[serde(skip_serializing_if = "Option::is_none")]
        currency: Option<String>,
    },

    /// A hop in a settlement chain
    SettlementHop {
        /// Hop UUID
        hop_id: String,
        /// Sequence number (1-based)
        sequence: i32,
        /// Intermediary BIC
        #[serde(skip_serializing_if = "Option::is_none")]
        intermediary_bic: Option<String>,
        /// Intermediary name
        #[serde(skip_serializing_if = "Option::is_none")]
        intermediary_name: Option<String>,
        /// Role: AGENT, CSD, ICSD, CUSTODIAN
        role: String,
    },

    // ========================================================================
    // TAX LAYER
    // ========================================================================
    /// A tax jurisdiction
    TaxJurisdiction {
        /// Jurisdiction UUID
        jurisdiction_id: String,
        /// Jurisdiction code (e.g., "DE")
        jurisdiction_code: String,
        /// Jurisdiction name (e.g., "Germany")
        jurisdiction_name: String,
        /// Default withholding rate (percentage)
        #[serde(skip_serializing_if = "Option::is_none")]
        default_withholding_rate: Option<f64>,
        /// Whether reclaim is available
        reclaim_available: bool,
    },

    /// Tax configuration for a jurisdiction
    TaxConfig {
        /// Status UUID
        status_id: String,
        /// Investor type (e.g., "FUND", "PENSION")
        investor_type: String,
        /// Whether tax exempt
        tax_exempt: bool,
        /// Documentation status: VALIDATED, SUBMITTED, EXPIRED
        #[serde(skip_serializing_if = "Option::is_none")]
        documentation_status: Option<String>,
        /// Treaty rate (if applicable)
        #[serde(skip_serializing_if = "Option::is_none")]
        treaty_rate: Option<f64>,
    },

    // ========================================================================
    // OTC/ISDA LAYER
    // ========================================================================
    /// An ISDA Master Agreement
    IsdaAgreement {
        /// ISDA UUID
        isda_id: String,
        /// Counterparty name
        counterparty_name: String,
        /// Governing law (e.g., "NY", "ENGLISH")
        #[serde(skip_serializing_if = "Option::is_none")]
        governing_law: Option<String>,
        /// Agreement date (ISO 8601)
        #[serde(skip_serializing_if = "Option::is_none")]
        agreement_date: Option<String>,
        /// Counterparty entity ID
        #[serde(skip_serializing_if = "Option::is_none")]
        counterparty_entity_id: Option<String>,
        /// Counterparty LEI
        #[serde(skip_serializing_if = "Option::is_none")]
        counterparty_lei: Option<String>,
    },

    /// A Credit Support Annex (CSA)
    CsaAgreement {
        /// CSA UUID
        csa_id: String,
        /// CSA type: VM, VM_IM, IM
        csa_type: String,
        /// Threshold currency
        #[serde(skip_serializing_if = "Option::is_none")]
        threshold_currency: Option<String>,
        /// Threshold amount
        #[serde(skip_serializing_if = "Option::is_none")]
        threshold_amount: Option<f64>,
        /// Minimum transfer amount
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum_transfer_amount: Option<f64>,
        /// Collateral SSI reference name
        #[serde(skip_serializing_if = "Option::is_none")]
        collateral_ssi_ref: Option<String>,
    },

    /// ISDA product coverage entry
    ProductCoverage {
        /// Coverage UUID
        coverage_id: String,
        /// Asset class (e.g., "RATES", "FX", "CREDIT")
        asset_class: String,
        /// Base products (e.g., ["IRS", "XCCY"])
        base_products: Vec<String>,
    },

    // ========================================================================
    // INVESTMENT MANAGER LAYER
    // ========================================================================
    /// Investment manager mandate
    InvestmentManagerMandate {
        /// Mandate UUID
        mandate_id: String,
        /// Manager entity ID
        manager_entity_id: String,
        /// Manager name
        manager_name: String,
        /// Manager LEI
        #[serde(skip_serializing_if = "Option::is_none")]
        manager_lei: Option<String>,
        /// Priority (lower = higher priority)
        priority: i32,
        /// Role (e.g., "DISCRETIONARY", "ADVISORY")
        role: String,
        /// Whether can trade
        can_trade: bool,
        /// Whether can settle
        can_settle: bool,
    },

    // ========================================================================
    // PRICING LAYER
    // ========================================================================
    /// Pricing rule
    PricingRule {
        /// Rule UUID
        rule_id: String,
        /// Priority (lower = higher priority)
        priority: i32,
        /// Source (e.g., "BLOOMBERG", "REUTERS", "INTERNAL")
        source: String,
        /// Fallback source
        #[serde(skip_serializing_if = "Option::is_none")]
        fallback_source: Option<String>,
        /// Price type (e.g., "MID", "BID", "ASK", "CLOSE")
        #[serde(skip_serializing_if = "Option::is_none")]
        price_type: Option<String>,
    },

    // ========================================================================
    // CORPORATE ACTIONS LAYER
    // ========================================================================
    /// Corporate actions policy summary node
    CorporateActionsPolicy {
        /// Number of enabled event types
        enabled_count: usize,
        /// Whether custom election defaults exist
        has_custom_elections: bool,
        /// Whether cutoff rules are configured
        has_cutoff_rules: bool,
        /// Elector type: investment_manager, admin, client
        #[serde(skip_serializing_if = "Option::is_none")]
        elector: Option<String>,
    },

    /// Individual CA event type configuration
    CaEventTypeConfig {
        /// Event type code (e.g., "DVCA", "DVOP")
        event_code: String,
        /// Event type name (e.g., "Cash Dividend")
        event_name: String,
        /// Processing mode: AUTO_INSTRUCT, MANUAL, DEFAULT_ONLY, THRESHOLD
        processing_mode: String,
        /// Default election option if applicable
        #[serde(skip_serializing_if = "Option::is_none")]
        default_option: Option<String>,
        /// Whether this event type is elective
        is_elective: bool,
    },

    /// CA cutoff rule node
    CaCutoffRuleNode {
        /// Rule identifier (market or depository code)
        rule_key: String,
        /// Days before market deadline
        days_before: i32,
        /// Warning days
        warning_days: i32,
        /// Escalation days
        escalation_days: i32,
    },

    /// CA proceeds SSI mapping node
    CaProceedsMappingNode {
        /// Proceeds type: cash or stock
        proceeds_type: String,
        /// Currency (if specified)
        #[serde(skip_serializing_if = "Option::is_none")]
        currency: Option<String>,
        /// Target SSI name
        ssi_reference: String,
    },
}

/// Match criteria for booking rules (for display/debugging)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookingMatchCriteria {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counterparty_entity_id: Option<String>,
}

// ============================================================================
// NODE
// ============================================================================

/// A node in the trading matrix tree.
///
/// Nodes are the building blocks of the AST. Each node has:
/// - A unique path-based ID
/// - A typed payload (via `node_type`)
/// - Display labels
/// - Children (forming the tree structure)
/// - Visual hints for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrixNode {
    /// Unique path-based identifier
    pub id: TradingMatrixNodeId,

    /// Node type with type-specific metadata
    pub node_type: TradingMatrixNodeType,

    /// Primary display label
    pub label: String,

    /// Secondary display label (subtitle)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sublabel: Option<String>,

    /// Child nodes
    #[serde(default)]
    pub children: Vec<TradingMatrixNode>,

    /// Status color for visual indicator
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_color: Option<StatusColor>,

    /// Whether this node's children have been loaded (for lazy loading)
    #[serde(default = "default_true")]
    pub is_loaded: bool,

    /// Leaf count (computed for summary display)
    #[serde(default)]
    pub leaf_count: usize,
}

fn default_true() -> bool {
    true
}

impl TradingMatrixNode {
    /// Create a new node with the given type and label
    pub fn new(
        id: TradingMatrixNodeId,
        node_type: TradingMatrixNodeType,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id,
            node_type,
            label: label.into(),
            sublabel: None,
            children: Vec::new(),
            status_color: None,
            is_loaded: true,
            leaf_count: 0,
        }
    }

    /// Add a sublabel
    pub fn with_sublabel(mut self, sublabel: impl Into<String>) -> Self {
        let s = sublabel.into();
        if !s.is_empty() {
            self.sublabel = Some(s);
        }
        self
    }

    /// Set status color
    pub fn with_status(mut self, status: StatusColor) -> Self {
        self.status_color = Some(status);
        self
    }

    /// Add a child node
    pub fn add_child(&mut self, child: TradingMatrixNode) {
        self.children.push(child);
    }

    /// Create a category node
    pub fn category(name: &str) -> Self {
        Self::new(
            TradingMatrixNodeId::category(name),
            TradingMatrixNodeType::Category {
                name: name.to_string(),
            },
            name,
        )
    }

    /// Recursively compute leaf count
    pub fn compute_leaf_count(&mut self) {
        if self.children.is_empty() {
            self.leaf_count = 1;
        } else {
            for child in &mut self.children {
                child.compute_leaf_count();
            }
            self.leaf_count = self.children.iter().map(|c| c.leaf_count).sum();
        }
    }

    /// Find a node by ID (recursive)
    pub fn find_by_id(&self, id: &TradingMatrixNodeId) -> Option<&TradingMatrixNode> {
        if &self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node by ID (recursive, mutable)
    pub fn find_by_id_mut(&mut self, id: &TradingMatrixNodeId) -> Option<&mut TradingMatrixNode> {
        if &self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_by_id_mut(id) {
                return Some(found);
            }
        }
        None
    }
}

// ============================================================================
// DOCUMENT
// ============================================================================

/// Document metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradingMatrixMetadata {
    /// Document source (e.g., "yaml_import", "dsl_build")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Source reference (e.g., file path, DSL statement ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,

    /// Who created/last modified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_by: Option<String>,

    /// Additional notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Regulatory framework (e.g., "UCITS", "AIFMD")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulatory_framework: Option<String>,
}

/// Complete Trading Matrix document.
///
/// This is stored as JSONB in `cbu_trading_profiles.document` and served
/// directly to the UI via the API. It IS the AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrixDocument {
    /// CBU UUID
    pub cbu_id: String,

    /// CBU name
    pub cbu_name: String,

    /// Document version (incremented on each save)
    pub version: i32,

    /// Document status
    #[serde(default)]
    pub status: DocumentStatus,

    /// Top-level category nodes
    pub children: Vec<TradingMatrixNode>,

    /// Total leaf count (computed)
    #[serde(default)]
    pub total_leaf_count: usize,

    /// Document metadata
    #[serde(default)]
    pub metadata: TradingMatrixMetadata,

    /// Created timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Last modified timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    /// Corporate actions configuration (stored as typed field for direct access)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corporate_actions: Option<TradingMatrixCorporateActions>,
}

/// Document lifecycle status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocumentStatus {
    #[default]
    Draft,
    Validated,
    PendingReview,
    Active,
    Superseded,
    Archived,
}

impl TradingMatrixDocument {
    /// Create a new empty document for a CBU
    pub fn new(cbu_id: impl Into<String>, cbu_name: impl Into<String>) -> Self {
        Self {
            cbu_id: cbu_id.into(),
            cbu_name: cbu_name.into(),
            version: 1,
            status: DocumentStatus::Draft,
            children: Vec::new(),
            total_leaf_count: 0,
            metadata: TradingMatrixMetadata::default(),
            created_at: None,
            updated_at: None,
            corporate_actions: None,
        }
    }

    /// Compute total leaf count for all nodes
    pub fn compute_leaf_counts(&mut self) {
        for child in &mut self.children {
            child.compute_leaf_count();
        }
        self.total_leaf_count = self.children.iter().map(|c| c.leaf_count).sum();
    }

    /// Find or create a category node
    pub fn ensure_category(&mut self, name: &str) -> &mut TradingMatrixNode {
        let category_id = TradingMatrixNodeId::category(name);

        // Check if category exists
        let exists = self.children.iter().any(|c| c.id == category_id);

        if !exists {
            self.children.push(TradingMatrixNode::category(name));
        }

        // Return mutable reference
        self.children
            .iter_mut()
            .find(|c| c.id == category_id)
            .expect("category should exist after ensure")
    }

    /// Find a node by ID
    pub fn find_by_id(&self, id: &TradingMatrixNodeId) -> Option<&TradingMatrixNode> {
        for child in &self.children {
            if let Some(found) = child.find_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node by ID (mutable)
    pub fn find_by_id_mut(&mut self, id: &TradingMatrixNodeId) -> Option<&mut TradingMatrixNode> {
        for child in &mut self.children {
            if let Some(found) = child.find_by_id_mut(id) {
                return Some(found);
            }
        }
        None
    }
}

// ============================================================================
// API RESPONSE (for backwards compatibility with existing API)
// ============================================================================

/// API response for trading matrix.
///
/// This is the top-level response returned by `GET /api/cbu/:id/trading-matrix`.
/// It's essentially a thin wrapper around `TradingMatrixDocument`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrixResponse {
    pub cbu_id: String,
    pub cbu_name: String,
    pub children: Vec<TradingMatrixNode>,
    pub total_leaf_count: usize,
}

impl From<TradingMatrixDocument> for TradingMatrixResponse {
    fn from(doc: TradingMatrixDocument) -> Self {
        Self {
            cbu_id: doc.cbu_id,
            cbu_name: doc.cbu_name,
            children: doc.children,
            total_leaf_count: doc.total_leaf_count,
        }
    }
}

// ============================================================================
// DSL OPERATIONS (for incremental building)
// ============================================================================

/// Operations for building the trading matrix AST incrementally.
///
/// These are the semantic operations that DSL verbs translate into.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TradingMatrixOp {
    /// Add an instrument class to the universe
    AddInstrumentClass {
        class_code: String,
        cfi_prefix: Option<String>,
        is_otc: bool,
    },

    /// Add a market under an instrument class
    AddMarket {
        parent_class: String,
        mic: String,
        market_name: String,
        country_code: String,
    },

    /// Add a counterparty under an instrument class (for OTC)
    AddCounterparty {
        parent_class: String,
        entity_id: String,
        entity_name: String,
        lei: Option<String>,
    },

    /// Add a universe entry under a market or counterparty
    AddUniverseEntry {
        parent_class: String,
        parent_market_or_counterparty: String,
        universe_id: String,
        currencies: Vec<String>,
        settlement_types: Vec<String>,
        is_held: bool,
        is_traded: bool,
    },

    /// Add an SSI
    AddSsi {
        ssi_id: String,
        ssi_name: String,
        ssi_type: String,
        safekeeping_account: Option<String>,
        safekeeping_bic: Option<String>,
        cash_account: Option<String>,
        cash_bic: Option<String>,
        cash_currency: Option<String>,
        pset_bic: Option<String>,
    },

    /// Activate an SSI
    ActivateSsi { ssi_id: String },

    /// Suspend an SSI
    SuspendSsi { ssi_id: String },

    /// Add a booking rule under an SSI
    AddBookingRule {
        ssi_ref: String, // SSI name (not ID)
        rule_id: String,
        rule_name: String,
        priority: i32,
        match_criteria: BookingMatchCriteria,
    },

    /// Add a settlement chain
    AddSettlementChain {
        chain_id: String,
        chain_name: String,
        mic: Option<String>,
        currency: Option<String>,
    },

    /// Add a hop to a settlement chain
    AddSettlementHop {
        chain_ref: String, // Chain name
        hop_id: String,
        sequence: i32,
        role: String,
        intermediary_bic: Option<String>,
        intermediary_name: Option<String>,
    },

    /// Add an ISDA agreement
    AddIsda {
        isda_id: String,
        counterparty_entity_id: String,
        counterparty_name: String,
        counterparty_lei: Option<String>,
        governing_law: Option<String>,
        agreement_date: Option<String>,
    },

    /// Add a CSA under an ISDA
    AddCsa {
        isda_ref: String, // ISDA counterparty name or ID
        csa_id: String,
        csa_type: String,
        threshold_currency: Option<String>,
        threshold_amount: Option<f64>,
        minimum_transfer_amount: Option<f64>,
        collateral_ssi_ref: Option<String>,
    },

    /// Add product coverage to an ISDA
    AddProductCoverage {
        isda_ref: String,
        coverage_id: String,
        asset_class: String,
        base_products: Vec<String>,
    },

    /// Add tax jurisdiction
    AddTaxJurisdiction {
        jurisdiction_id: String,
        jurisdiction_code: String,
        jurisdiction_name: String,
        default_withholding_rate: Option<f64>,
        reclaim_available: bool,
    },

    /// Add tax config under jurisdiction
    AddTaxConfig {
        jurisdiction_ref: String, // Jurisdiction code
        status_id: String,
        investor_type: String,
        tax_exempt: bool,
        documentation_status: Option<String>,
        treaty_rate: Option<f64>,
    },

    /// Add an investment manager mandate
    AddImMandate {
        manager_id: String,
        manager_entity_id: String,
        manager_name: String,
        manager_lei: Option<String>,
        priority: i32,
        role: String, // e.g., "DISCRETIONARY", "ADVISORY"
        can_trade: bool,
        can_settle: bool,
        /// Scope constraints (optional)
        scope_instrument_classes: Vec<String>,
        scope_markets: Vec<String>,
        scope_currencies: Vec<String>,
    },

    /// Update IM mandate scope
    UpdateImScope {
        manager_ref: String, // Manager name or ID
        scope_instrument_classes: Option<Vec<String>>,
        scope_markets: Option<Vec<String>>,
        scope_currencies: Option<Vec<String>>,
    },

    /// Add eligible collateral to a CSA
    AddCsaEligibleCollateral {
        isda_ref: String, // ISDA counterparty name
        csa_ref: String,  // CSA type (e.g., "VM", "IM")
        collateral_id: String,
        collateral_type: String, // e.g., "CASH", "GOVT_BOND", "CORP_BOND"
        currency: Option<String>,
        haircut_pct: Option<f64>,
        concentration_limit_pct: Option<f64>,
    },

    /// Link SSI to CSA for collateral movements
    LinkCsaSsi {
        isda_ref: String,
        csa_ref: String,
        ssi_ref: String, // SSI name
    },

    /// Set base currency for the trading profile
    SetBaseCurrency { currency: String },

    /// Add an allowed currency to the profile
    AddAllowedCurrency { currency: String },

    /// Remove a node by ID
    RemoveNode { node_id: TradingMatrixNodeId },

    /// Update a node's status
    SetNodeStatus {
        node_id: TradingMatrixNodeId,
        status: StatusColor,
    },
}

// ============================================================================
// CATEGORY PRESETS
// ============================================================================

/// Standard category names used in trading matrix documents.
pub mod categories {
    pub const UNIVERSE: &str = "Trading Universe";
    pub const SSI: &str = "Standing Settlement Instructions";
    pub const CHAINS: &str = "Settlement Chains";
    pub const TAX: &str = "Tax Configuration";
    pub const ISDA: &str = "ISDA Agreements";
    pub const PRICING: &str = "Pricing Configuration";
    pub const MANAGERS: &str = "Investment Managers";
    pub const CORPORATE_ACTIONS: &str = "Corporate Actions";
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_child() {
        let parent = TradingMatrixNodeId::category("UNIVERSE");
        let child = parent.child("EQUITY");

        assert_eq!(child.0, vec!["_UNIVERSE".to_string(), "EQUITY".to_string()]);
        assert!(child.is_child_of(&parent));
        assert_eq!(child.parent(), Some(parent));
    }

    #[test]
    fn test_node_id_depth() {
        let root = TradingMatrixNodeId::category("UNIVERSE");
        let level1 = root.child("EQUITY");
        let level2 = level1.child("XNYS");

        assert_eq!(root.depth(), 0);
        assert_eq!(level1.depth(), 1);
        assert_eq!(level2.depth(), 2);
    }

    #[test]
    fn test_document_ensure_category() {
        let mut doc = TradingMatrixDocument::new("cbu-123", "Test CBU");

        doc.ensure_category(categories::UNIVERSE);
        doc.ensure_category(categories::SSI);
        doc.ensure_category(categories::UNIVERSE); // Should not duplicate

        assert_eq!(doc.children.len(), 2);
    }

    #[test]
    fn test_node_find_by_id() {
        let mut doc = TradingMatrixDocument::new("cbu-123", "Test CBU");

        let universe = doc.ensure_category(categories::UNIVERSE);
        let equity_id = universe.id.child("EQUITY");
        universe.add_child(TradingMatrixNode::new(
            equity_id.clone(),
            TradingMatrixNodeType::InstrumentClass {
                class_code: "EQUITY".to_string(),
                cfi_prefix: Some("ES".to_string()),
                is_otc: false,
            },
            "Equity",
        ));

        let found = doc.find_by_id(&equity_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "Equity");
    }

    #[test]
    fn test_node_type_serialization() {
        let node_type = TradingMatrixNodeType::IsdaAgreement {
            isda_id: "isda-123".to_string(),
            counterparty_name: "Goldman Sachs".to_string(),
            governing_law: Some("NY".to_string()),
            agreement_date: Some("2024-01-15".to_string()),
            counterparty_entity_id: None,
            counterparty_lei: None,
        };

        let json = serde_json::to_string(&node_type).unwrap();
        assert!(json.contains(r#""type":"isda_agreement""#));
        assert!(json.contains(r#""counterparty_name":"Goldman Sachs""#));
    }

    #[test]
    fn test_document_compute_leaf_counts() {
        let mut doc = TradingMatrixDocument::new("cbu-123", "Test CBU");

        let universe = doc.ensure_category(categories::UNIVERSE);
        let equity_id = universe.id.child("EQUITY");

        let mut equity = TradingMatrixNode::new(
            equity_id.clone(),
            TradingMatrixNodeType::InstrumentClass {
                class_code: "EQUITY".to_string(),
                cfi_prefix: None,
                is_otc: false,
            },
            "Equity",
        );

        // Add two leaf nodes
        equity.add_child(TradingMatrixNode::new(
            equity_id.child("XNYS"),
            TradingMatrixNodeType::Market {
                mic: "XNYS".to_string(),
                market_name: "NYSE".to_string(),
                country_code: "US".to_string(),
            },
            "NYSE",
        ));
        equity.add_child(TradingMatrixNode::new(
            equity_id.child("XLON"),
            TradingMatrixNodeType::Market {
                mic: "XLON".to_string(),
                market_name: "LSE".to_string(),
                country_code: "GB".to_string(),
            },
            "LSE",
        ));

        universe.add_child(equity);

        doc.compute_leaf_counts();

        assert_eq!(doc.total_leaf_count, 2);
    }
}
