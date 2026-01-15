//! Configuration type definitions
//!
//! These structs map directly to the YAML configuration files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// TOP-LEVEL CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbsConfig {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub domains: HashMap<String, DomainConfig>,
}

fn default_version() -> String {
    "1.0".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CsgRulesConfig {
    pub version: String,
    #[serde(default)]
    pub constraints: Vec<ConstraintRule>,
    #[serde(default)]
    pub warnings: Vec<WarningRule>,
    #[serde(default)]
    pub jurisdiction_rules: Vec<JurisdictionRule>,
    #[serde(default)]
    pub composite_rules: Vec<CompositeRule>,
}

// =============================================================================
// DOMAIN & VERB CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DomainConfig {
    pub description: String,
    #[serde(default)]
    pub verbs: HashMap<String, VerbConfig>,
    #[serde(default)]
    pub dynamic_verbs: Vec<DynamicVerbConfig>,
    /// Domain-level invocation hints for agent intent matching.
    /// These are general phrases that suggest this domain.
    /// Example: ["counterparty", "ISDA", "CSA"] for OTC domain
    #[serde(default)]
    pub invocation_hints: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbConfig {
    pub description: String,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    #[serde(default)]
    pub handler: Option<String>,
    #[serde(default)]
    pub graph_query: Option<GraphQueryConfig>,
    #[serde(default)]
    pub args: Vec<ArgConfig>,
    #[serde(default)]
    pub returns: Option<ReturnsConfig>,
    /// Dataflow: what this verb produces (binding type)
    #[serde(default)]
    pub produces: Option<VerbProduces>,
    /// Dataflow: what this verb consumes (required bindings)
    #[serde(default)]
    pub consumes: Vec<VerbConsumes>,
    /// Lifecycle constraints and transitions for this verb
    #[serde(default)]
    pub lifecycle: Option<VerbLifecycle>,
    /// Verb metadata for tiering, source of truth, and organizational tags
    #[serde(default)]
    pub metadata: Option<VerbMetadata>,
    /// Natural language phrases that should trigger this verb.
    /// Used by the agent for intent-to-verb matching.
    /// Example: ["add counterparty", "create counterparty", "onboard counterparty"]
    #[serde(default)]
    pub invocation_phrases: Vec<String>,
}

// =============================================================================
// VERB METADATA (Tiering & Organization)
// =============================================================================

/// Metadata for verb classification, tiering, and organization
///
/// Used by:
/// - Verb linter to enforce tiering rules
/// - Agent to select appropriate verbs
/// - Documentation generation
/// - Verb inventory reports
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct VerbMetadata {
    /// Verb tier classification:
    /// - `reference`: Global catalogs, templates, taxonomies (scope: global)
    /// - `intent`: Authoring surface for CBU business policy (scope: cbu)
    /// - `projection`: Writes operational tables from matrix (internal only)
    /// - `diagnostics`: Read-only inspection of state
    /// - `composite`: Multi-table orchestration verbs
    #[serde(default)]
    pub tier: Option<VerbTier>,

    /// Source of truth for data this verb writes:
    /// - `matrix`: Trading matrix document is canonical
    /// - `catalog`: Global reference catalog
    /// - `operational`: Operational tables (derived/projected)
    #[serde(default)]
    pub source_of_truth: Option<SourceOfTruth>,

    /// Scope of the verb:
    /// - `global`: Operates on global reference data
    /// - `cbu`: Operates within CBU context
    #[serde(default)]
    pub scope: Option<VerbScope>,

    /// Whether this verb writes to operational (projection) tables
    #[serde(default)]
    pub writes_operational: bool,

    /// Primary noun this verb operates on (for grouping):
    /// e.g., "trading_matrix", "ssi", "gateway", "booking_rule", "corporate_actions"
    #[serde(default)]
    pub noun: Option<String>,

    /// Whether this verb is internal-only (not exposed to agent/user)
    #[serde(default)]
    pub internal: bool,

    /// Organizational tags for search and grouping
    #[serde(default)]
    pub tags: Vec<String>,

    /// If this verb replaces another (for migration tracking)
    #[serde(default)]
    pub replaces: Option<String>,

    // =========================================================================
    // Lifecycle fields (for deprecation, migration, governance)
    // =========================================================================
    /// Verb lifecycle status: active (default) or deprecated
    #[serde(default)]
    pub status: VerbStatus,

    /// For deprecated verbs: the canonical verb that replaces this one
    /// Format: "domain.verb-name" (e.g., "trading-profile.add-standing-instruction")
    #[serde(default)]
    pub replaced_by: Option<String>,

    /// Version when this verb was introduced (for documentation)
    #[serde(default)]
    pub since_version: Option<String>,

    /// Version when this verb will be removed (for deprecated verbs)
    #[serde(default)]
    pub removal_version: Option<String>,

    /// Whether this verb performs dangerous operations (delete on regulated nouns)
    /// Requires explicit confirmation or elevated permissions
    #[serde(default)]
    pub dangerous: bool,
}

/// Verb tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbTier {
    /// Global catalogs, templates, taxonomies
    Reference,
    /// Authoring surface for CBU business policy (matrix is source of truth)
    Intent,
    /// Writes operational tables from matrix (internal only)
    Projection,
    /// Read-only inspection of state
    Diagnostics,
    /// Multi-table orchestration verbs
    Composite,
}

/// Source of truth for data
///
/// Different domains have different canonical sources:
/// - Trading profile verbs → matrix (JSONB document)
/// - Entity/ownership verbs → entity (entity_relationships table)
/// - KYC/case verbs → workflow (case state machine)
/// - Research verbs → external (APIs like GLEIF, Companies House)
/// - Fund/investor verbs → register (capital structure)
/// - Reference data verbs → catalog (seeded lookup tables)
/// - Session/view verbs → session (ephemeral UI state)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceOfTruth {
    /// Trading matrix document is canonical (trading-profile domain)
    Matrix,
    /// Global reference catalog (instrument classes, markets, currencies)
    Catalog,
    /// Operational tables - derived/projected from another source
    Operational,
    /// Session state - ephemeral, not persisted business data
    Session,
    /// Entity graph - entity_relationships is the source (UBO, ownership, control)
    Entity,
    /// Case workflow - KYC case state machine is canonical
    Workflow,
    /// External API - data sourced from GLEIF, Companies House, SEC, etc.
    External,
    /// Capital register - fund/investor holdings structure
    Register,
    /// Document catalog - document artifacts and metadata
    Document,
}

/// Scope of a verb
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbScope {
    /// Operates on global reference data
    Global,
    /// Operates within CBU context
    Cbu,
}

/// Verb lifecycle status
///
/// Used for deprecation tracking and migration:
/// - `active`: Normal verb, fully supported
/// - `deprecated`: Marked for removal, should use `replaced_by` instead
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbStatus {
    /// Normal verb, fully supported (default)
    #[default]
    Active,
    /// Marked for removal, use `replaced_by` instead
    Deprecated,
}

// =============================================================================
// DATAFLOW CONFIG
// =============================================================================

/// Dataflow: what a verb produces when executed with :as @binding
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbProduces {
    /// The type of entity produced: "cbu", "entity", "case", "resource_instance", etc.
    #[serde(rename = "type")]
    pub produced_type: String,
    /// Static subtype for entities: "proper_person", "limited_company", "fund_umbrella", etc.
    #[serde(default)]
    pub subtype: Option<String>,
    /// Dynamic subtype from argument value (e.g., "resource-type" for service-resource.provision)
    /// When set, the subtype is extracted from the named argument at runtime
    #[serde(default)]
    pub subtype_from_arg: Option<String>,
    /// True if this is a lookup (resolved existing) rather than create (new)
    #[serde(default)]
    pub resolved: bool,
    /// Initial state when creating a new entity (for lifecycle tracking)
    #[serde(default)]
    pub initial_state: Option<String>,
}

impl VerbProduces {
    /// Resolve the subtype for a given verb call's arguments
    /// Returns static subtype if set, otherwise extracts from subtype_from_arg
    pub fn resolve_subtype(&self, args: &[super::super::ast::Argument]) -> Option<String> {
        // Static subtype takes precedence
        if let Some(ref st) = self.subtype {
            return Some(st.clone());
        }

        // Dynamic subtype from arg
        if let Some(ref arg_name) = self.subtype_from_arg {
            return args
                .iter()
                .find(|a| a.key == *arg_name)
                .and_then(|a| a.value.as_string())
                .map(|s| s.to_string());
        }

        None
    }
}

/// Dataflow: what a verb consumes (dependencies)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbConsumes {
    /// Which argument carries the reference (e.g., "cbu-id", "entity-id")
    pub arg: String,
    /// Expected type of the binding (e.g., "cbu", "entity", "case")
    #[serde(rename = "type")]
    pub consumed_type: String,
    /// Whether this dependency is required (default: true)
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

/// Verb lifecycle configuration - constraints and state transitions
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct VerbLifecycle {
    /// Which argument contains the entity ID this verb operates on
    #[serde(default)]
    pub entity_arg: Option<String>,

    /// Required states for the entity before this verb can execute
    #[serde(default)]
    pub requires_states: Vec<String>,

    /// State the entity transitions to after this verb executes
    #[serde(default)]
    pub transitions_to: Option<String>,

    /// Argument that specifies the target state (for generic set-status verbs)
    #[serde(default)]
    pub transitions_to_arg: Option<String>,

    /// Precondition checks to run before execution
    #[serde(default)]
    pub precondition_checks: Vec<String>,

    /// Tables this verb writes to (for DAG ordering).
    /// Format: "schema.table" (e.g., "custody.cbu_ssi", "ob-poc.cbus")
    /// Used by topo_sort to ensure write-before-read ordering.
    #[serde(default)]
    pub writes_tables: Vec<String>,

    /// Tables this verb reads from (for DAG ordering).
    /// Format: "schema.table" (e.g., "custody.cbu_ssi", "ob-poc.cbus")
    /// Used by topo_sort to order this verb after any verb that writes to these tables.
    #[serde(default)]
    pub reads_tables: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbBehavior {
    Crud,
    Plugin,
    /// Graph query operations - visualization, traversal, path-finding
    GraphQuery,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrudConfig {
    pub operation: CrudOperation,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub returning: Option<String>,
    #[serde(default)]
    pub conflict_keys: Option<Vec<String>>,
    /// Named constraint for ON CONFLICT (used when conflict_keys has computed columns)
    /// Format: ON CONFLICT ON CONSTRAINT {conflict_constraint}
    #[serde(default)]
    pub conflict_constraint: Option<String>,
    // For junction operations
    #[serde(default)]
    pub junction: Option<String>,
    #[serde(default)]
    pub from_col: Option<String>,
    #[serde(default)]
    pub to_col: Option<String>,
    #[serde(default)]
    pub role_table: Option<String>,
    #[serde(default)]
    pub role_col: Option<String>,
    #[serde(default)]
    pub fk_col: Option<String>,
    #[serde(default)]
    pub filter_col: Option<String>,
    // For joins
    #[serde(default)]
    pub primary_table: Option<String>,
    #[serde(default)]
    pub join_table: Option<String>,
    #[serde(default)]
    pub join_col: Option<String>,
    // For entity creation
    #[serde(default)]
    pub base_table: Option<String>,
    #[serde(default)]
    pub extension_table: Option<String>,
    #[serde(default)]
    pub extension_table_column: Option<String>,
    #[serde(default)]
    pub type_id_column: Option<String>,
    /// Explicit type_code for entity_create operations (e.g., "fund_umbrella")
    /// If not set, derived from verb name (e.g., "create-umbrella" → "UMBRELLA")
    #[serde(default)]
    pub type_code: Option<String>,
    // For list operations
    #[serde(default)]
    pub order_by: Option<String>,
    // For update operations with fixed values
    #[serde(default)]
    pub set_values: Option<std::collections::HashMap<String, serde_yaml::Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrudOperation {
    Insert,
    Select,
    Update,
    Delete,
    Upsert,
    Link,
    Unlink,
    RoleLink,
    RoleUnlink,
    ListByFk,
    ListParties,
    SelectWithJoin,
    EntityCreate,
    EntityUpsert,
}

// =============================================================================
// GRAPH QUERY CONFIG
// =============================================================================

/// Configuration for graph query operations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphQueryConfig {
    /// The type of graph query operation
    pub operation: GraphQueryOperation,
    /// Root entity type for the query (e.g., "cbu", "entity")
    #[serde(default)]
    pub root_type: Option<String>,
    /// Edge types to include in traversal
    #[serde(default)]
    pub edge_types: Vec<String>,
    /// Maximum traversal depth (default: 10)
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// Default view mode for visualization queries
    #[serde(default)]
    pub default_view_mode: Option<String>,
}

fn default_max_depth() -> u32 {
    10
}

/// Types of graph query operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphQueryOperation {
    /// Build full graph view from root entity
    View,
    /// Focus on a specific node with neighborhood
    Focus,
    /// Filter graph by criteria
    Filter,
    /// Group nodes by attribute
    GroupBy,
    /// Find shortest path between two nodes
    Path,
    /// Find all connected nodes from a starting point
    FindConnected,
    /// Compare two graph states (snapshots)
    Compare,
    /// Find all ancestors of a node (BFS upward)
    Ancestors,
    /// Find all descendants of a node (BFS downward)
    Descendants,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArgConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub maps_to: Option<String>,
    #[serde(default)]
    pub lookup: Option<LookupConfig>,
    #[serde(default)]
    pub valid_values: Option<Vec<String>>,
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub validation: Option<ArgValidation>,
    /// Fuzzy check config - for upsert verbs, check for similar existing records
    /// and emit a warning if fuzzy matches are found
    #[serde(default)]
    pub fuzzy_check: Option<FuzzyCheckConfig>,
}

/// Configuration for fuzzy match checking on upsert args
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FuzzyCheckConfig {
    /// Entity type to search (e.g., "cbu", "entity")
    pub entity_type: String,
    /// Field to search by (defaults to arg name if not specified)
    #[serde(default)]
    pub search_key: Option<String>,
    /// Minimum score threshold for warnings (0.0-1.0, default 0.5)
    #[serde(default = "default_fuzzy_threshold")]
    pub threshold: f32,
}

fn default_fuzzy_threshold() -> f32 {
    0.5
}

/// Validation rules for an argument
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArgValidation {
    /// Valid enum values
    #[serde(default)]
    pub r#enum: Option<Vec<String>>,
    /// Minimum value (for numbers)
    #[serde(default)]
    pub min: Option<f64>,
    /// Maximum value (for numbers)
    #[serde(default)]
    pub max: Option<f64>,
    /// Regex pattern (for strings)
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgType {
    String,
    Integer,
    Decimal,
    Boolean,
    Date,
    Timestamp,
    Uuid,
    /// Array of UUIDs
    #[serde(rename = "uuid_array")]
    UuidArray,
    /// Alias for UuidArray (used in view.yaml)
    #[serde(rename = "uuid_list")]
    UuidList,
    Json,
    Lookup,
    StringList,
    /// Map of key-value pairs (for template params, etc.)
    Map,
    /// Symbol reference (@binding)
    SymbolRef,
    /// Generic object/struct (for complex nested args)
    Object,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LookupConfig {
    pub table: String,
    #[serde(default)]
    pub schema: Option<String>,
    /// The entity type for this lookup (e.g., "proper_person", "limited_company", "role", "jurisdiction")
    /// This becomes the ref_type in the LookupRef triplet: (ref_type, search_key, primary_key)
    #[serde(default)]
    pub entity_type: Option<String>,
    /// Search key configuration - either a simple column name or composite search key
    ///
    /// Simple (backwards compatible):
    /// ```yaml
    /// search_key: name
    /// ```
    ///
    /// Composite (for high-volume tables like persons, companies):
    /// ```yaml
    /// search_key:
    ///   primary: name
    ///   discriminators:
    ///     - field: date_of_birth
    ///       from_arg: dob
    ///       selectivity: 0.95
    ///     - field: nationality
    ///       from_arg: nationality
    ///       selectivity: 0.7
    ///   resolution_tiers: [exact, composite, contextual, fuzzy]
    ///   min_confidence: 0.8
    /// ```
    #[serde(alias = "code_column")]
    pub search_key: SearchKeyConfig,
    /// The column containing primary key (UUID for entities, code for reference data)
    #[serde(alias = "id_column")]
    pub primary_key: String,
    /// Resolution mode: how the LSP/UI should resolve this reference.
    ///
    /// - `reference`: Small static lookup tables (< 100 items) - use autocomplete dropdown
    /// - `entity`: Large/growing tables (people, CBUs, cases) - use search modal
    ///
    /// Defaults to "reference" if not specified (backwards compatible).
    #[serde(default)]
    pub resolution_mode: Option<ResolutionMode>,
}

// =============================================================================
// SEARCH KEY CONFIG (S-expression syntax for composite entity resolution)
// =============================================================================

/// Search key configuration using s-expression syntax.
///
/// At scale (100k+ persons), a simple name search returns too many "John Smith" matches.
/// Composite search keys allow disambiguation via additional fields (DOB, nationality, etc.)
///
/// ## Syntax Examples
///
/// Simple (backwards compatible - just column name):
/// ```yaml
/// search_key: name
/// search_key: search_name
/// ```
///
/// Composite with discriminators (s-expression with nested lists):
/// ```yaml
/// # Primary column + discriminator columns
/// search_key: "(search_name date_of_birth nationality)"
///
/// # With selectivity scores for discriminators
/// search_key: "(search_name (date_of_birth :selectivity 0.95) (nationality :selectivity 0.7))"
///
/// # With options
/// search_key: "(search_name (date_of_birth :selectivity 0.95 :required true) :min-confidence 0.85)"
/// ```
///
/// The s-expression is parsed into a `CompositeSearchKey` structure.
#[derive(Debug, Clone)]
pub enum SearchKeyConfig {
    /// Simple: single column name (backwards compatible)
    Simple(String),
    /// Composite: parsed from s-expression
    Composite(CompositeSearchKey),
}

impl SearchKeyConfig {
    /// Parse a search key from string (simple name or s-expression)
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        if trimmed.starts_with('(') {
            // S-expression - parse it
            CompositeSearchKey::parse(trimmed).map(SearchKeyConfig::Composite)
        } else {
            // Simple column name
            Ok(SearchKeyConfig::Simple(trimmed.to_string()))
        }
    }

    /// Get the primary search column name
    pub fn primary_column(&self) -> &str {
        match self {
            SearchKeyConfig::Simple(col) => col,
            SearchKeyConfig::Composite(c) => &c.primary,
        }
    }

    /// Check if this is a simple (single-column) search key
    pub fn is_simple(&self) -> bool {
        matches!(self, SearchKeyConfig::Simple(_))
    }

    /// Get all columns needed for this search key
    pub fn all_columns(&self) -> Vec<&str> {
        match self {
            SearchKeyConfig::Simple(col) => vec![col.as_str()],
            SearchKeyConfig::Composite(c) => {
                let mut cols = vec![c.primary.as_str()];
                for d in &c.discriminators {
                    cols.push(d.field.as_str());
                }
                cols
            }
        }
    }

    /// Get discriminator fields if this is a composite key
    pub fn discriminators(&self) -> &[SearchDiscriminator] {
        match self {
            SearchKeyConfig::Simple(_) => &[],
            SearchKeyConfig::Composite(c) => &c.discriminators,
        }
    }

    /// Get resolution tiers (defaults to [Fuzzy] for simple keys)
    pub fn resolution_tiers(&self) -> Vec<ResolutionTier> {
        match self {
            SearchKeyConfig::Simple(_) => vec![ResolutionTier::Fuzzy],
            SearchKeyConfig::Composite(c) => {
                if c.resolution_tiers.is_empty() {
                    vec![
                        ResolutionTier::Exact,
                        ResolutionTier::Composite,
                        ResolutionTier::Contextual,
                        ResolutionTier::Fuzzy,
                    ]
                } else {
                    c.resolution_tiers.clone()
                }
            }
        }
    }

    /// Get minimum confidence threshold (defaults to 0.8)
    pub fn min_confidence(&self) -> f32 {
        match self {
            SearchKeyConfig::Simple(_) => 0.8,
            SearchKeyConfig::Composite(c) => c.min_confidence,
        }
    }

    /// Serialize back to s-expression string
    pub fn to_sexpr(&self) -> String {
        match self {
            SearchKeyConfig::Simple(col) => col.clone(),
            SearchKeyConfig::Composite(c) => c.to_sexpr(),
        }
    }
}

impl Default for SearchKeyConfig {
    fn default() -> Self {
        SearchKeyConfig::Simple("name".to_string())
    }
}

// Custom serde: deserialize from string (simple or s-expr), serialize to string
impl<'de> Deserialize<'de> for SearchKeyConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        SearchKeyConfig::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for SearchKeyConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_sexpr())
    }
}

/// Composite search key with discriminators for disambiguation
#[derive(Debug, Clone)]
pub struct CompositeSearchKey {
    /// Primary search field (always indexed, always searched first)
    pub primary: String,

    /// Discriminator fields that narrow the search when available
    pub discriminators: Vec<SearchDiscriminator>,

    /// Resolution tiers in priority order (how to resolve based on available fields)
    /// Default: [exact, composite, contextual, fuzzy]
    pub resolution_tiers: Vec<ResolutionTier>,

    /// Minimum confidence threshold for auto-resolution (0.0-1.0)
    /// Below this threshold, returns ambiguous candidates instead of single match
    pub min_confidence: f32,
}

impl CompositeSearchKey {
    /// Parse a composite search key from s-expression
    ///
    /// Syntax: `(primary_col [discriminator...] [:option value]...)`
    ///
    /// Where discriminator is either:
    /// - `col_name` - simple column with default selectivity
    /// - `(col_name :selectivity 0.95 [:required true])` - column with options
    ///
    /// Options:
    /// - `:min-confidence 0.85` - minimum match confidence
    /// - `:tiers (exact composite fuzzy)` - resolution tier order
    pub fn parse(input: &str) -> Result<Self, String> {
        let tokens = tokenize_sexpr(input)?;
        if tokens.is_empty() {
            return Err("Empty s-expression".to_string());
        }

        let mut primary: Option<String> = None;
        let mut discriminators = Vec::new();
        let mut min_confidence = 0.8f32;
        let mut resolution_tiers = Vec::new();

        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i] {
                SExprToken::Symbol(s) if s.starts_with(':') => {
                    // Keyword option
                    let Some(key) = s.strip_prefix(':') else {
                        unreachable!()
                    };
                    i += 1;
                    if i >= tokens.len() {
                        return Err(format!("Missing value for :{}", key));
                    }
                    match key {
                        "min-confidence" => {
                            if let SExprToken::Symbol(v) = &tokens[i] {
                                min_confidence = v
                                    .parse()
                                    .map_err(|_| format!("Invalid :min-confidence value: {}", v))?;
                            }
                        }
                        "tiers" => {
                            if let SExprToken::List(tier_tokens) = &tokens[i] {
                                for t in tier_tokens {
                                    if let SExprToken::Symbol(tier_name) = t {
                                        resolution_tiers.push(parse_tier(tier_name)?);
                                    }
                                }
                            }
                        }
                        _ => {} // Ignore unknown options
                    }
                    i += 1;
                }
                SExprToken::Symbol(s) => {
                    // Column name
                    if primary.is_none() {
                        primary = Some(s.clone());
                    } else {
                        // Simple discriminator
                        discriminators.push(SearchDiscriminator {
                            field: s.clone(),
                            from_arg: None,
                            selectivity: 0.5,
                            required: false,
                        });
                    }
                    i += 1;
                }
                SExprToken::List(inner) => {
                    // Discriminator with options: (col_name :selectivity 0.95 ...)
                    let disc = parse_discriminator(inner)?;
                    discriminators.push(disc);
                    i += 1;
                }
            }
        }

        let primary = primary.ok_or_else(|| "Missing primary column in search key".to_string())?;

        Ok(CompositeSearchKey {
            primary,
            discriminators,
            resolution_tiers,
            min_confidence,
        })
    }

    /// Serialize to s-expression string
    pub fn to_sexpr(&self) -> String {
        let mut parts = vec![self.primary.clone()];

        for d in &self.discriminators {
            if d.selectivity == 0.5 && !d.required && d.from_arg.is_none() {
                // Simple form
                parts.push(d.field.clone());
            } else {
                // Full form with options
                let mut disc_parts = vec![d.field.clone()];
                if d.selectivity != 0.5 {
                    disc_parts.push(format!(":selectivity {}", d.selectivity));
                }
                if d.required {
                    disc_parts.push(":required true".to_string());
                }
                if let Some(ref arg) = d.from_arg {
                    disc_parts.push(format!(":from-arg {}", arg));
                }
                parts.push(format!("({})", disc_parts.join(" ")));
            }
        }

        if self.min_confidence != 0.8 {
            parts.push(format!(":min-confidence {}", self.min_confidence));
        }

        if !self.resolution_tiers.is_empty() {
            let tiers: Vec<&str> = self.resolution_tiers.iter().map(|t| t.as_str()).collect();
            parts.push(format!(":tiers ({})", tiers.join(" ")));
        }

        format!("({})", parts.join(" "))
    }
}

// =============================================================================
// S-EXPRESSION TOKENIZER (for search key parsing)
// =============================================================================

/// Token types for s-expression parsing
#[derive(Debug, Clone, PartialEq)]
enum SExprToken {
    Symbol(String),
    List(Vec<SExprToken>),
}

/// Tokenize an s-expression string
fn tokenize_sexpr(input: &str) -> Result<Vec<SExprToken>, String> {
    let trimmed = input.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return Err("S-expression must be wrapped in parentheses".to_string());
    }

    // Remove outer parens
    let inner = &trimmed[1..trimmed.len() - 1];
    tokenize_inner(inner)
}

fn tokenize_inner(input: &str) -> Result<Vec<SExprToken>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        if c == '(' {
            // Nested list
            chars.next();
            let mut depth = 1;
            let mut nested = String::new();
            while depth > 0 {
                match chars.next() {
                    Some('(') => {
                        depth += 1;
                        nested.push('(');
                    }
                    Some(')') => {
                        depth -= 1;
                        if depth > 0 {
                            nested.push(')');
                        }
                    }
                    Some(ch) => nested.push(ch),
                    None => return Err("Unclosed parenthesis".to_string()),
                }
            }
            let inner_tokens = tokenize_inner(&nested)?;
            tokens.push(SExprToken::List(inner_tokens));
        } else {
            // Symbol (including keywords like :selectivity)
            let mut symbol = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() || ch == '(' || ch == ')' {
                    break;
                }
                symbol.push(ch);
                chars.next();
            }
            if !symbol.is_empty() {
                tokens.push(SExprToken::Symbol(symbol));
            }
        }
    }

    Ok(tokens)
}

/// Parse a discriminator from tokens
fn parse_discriminator(tokens: &[SExprToken]) -> Result<SearchDiscriminator, String> {
    if tokens.is_empty() {
        return Err("Empty discriminator".to_string());
    }

    let field = match &tokens[0] {
        SExprToken::Symbol(s) => s.clone(),
        _ => return Err("Discriminator must start with field name".to_string()),
    };

    let mut selectivity = 0.5f32;
    let mut required = false;
    let mut from_arg = None;

    let mut i = 1;
    while i < tokens.len() {
        if let SExprToken::Symbol(s) = &tokens[i] {
            if let Some(key) = s.strip_prefix(':') {
                i += 1;
                if i >= tokens.len() {
                    break;
                }
                if let SExprToken::Symbol(val) = &tokens[i] {
                    match key {
                        "selectivity" => {
                            selectivity = val.parse().unwrap_or(0.5);
                        }
                        "required" => {
                            required = val == "true";
                        }
                        "from-arg" => {
                            from_arg = Some(val.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
        i += 1;
    }

    Ok(SearchDiscriminator {
        field,
        from_arg,
        selectivity,
        required,
    })
}

/// Parse a resolution tier from string
fn parse_tier(s: &str) -> Result<ResolutionTier, String> {
    match s {
        "exact" => Ok(ResolutionTier::Exact),
        "composite" => Ok(ResolutionTier::Composite),
        "contextual" => Ok(ResolutionTier::Contextual),
        "fuzzy" => Ok(ResolutionTier::Fuzzy),
        _ => Err(format!("Unknown resolution tier: {}", s)),
    }
}

impl ResolutionTier {
    /// Convert tier to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionTier::Exact => "exact",
            ResolutionTier::Composite => "composite",
            ResolutionTier::Contextual => "contextual",
            ResolutionTier::Fuzzy => "fuzzy",
        }
    }
}

/// A discriminator field that helps narrow search results
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchDiscriminator {
    /// Database column name (e.g., "date_of_birth", "nationality")
    pub field: String,

    /// DSL argument that provides this value (e.g., "dob", "nationality")
    /// If omitted, uses the field name
    #[serde(default)]
    pub from_arg: Option<String>,

    /// How much this field narrows the search (0.0-1.0)
    /// 1.0 = unique identifier (source_id)
    /// 0.95 = nearly unique (name + dob)
    /// 0.7 = helpful but not unique (nationality)
    #[serde(default = "default_selectivity")]
    pub selectivity: f32,

    /// Is this field required for resolution?
    /// If true, resolution fails if field is not provided
    #[serde(default)]
    pub required: bool,
}

fn default_selectivity() -> f32 {
    0.5
}

impl SearchDiscriminator {
    /// Get the argument name (uses field name if from_arg not specified)
    pub fn arg_name(&self) -> &str {
        self.from_arg.as_deref().unwrap_or(&self.field)
    }
}

/// Resolution tier - determines which search strategy to use
///
/// The system tries tiers in order until it finds a match or exhausts options.
/// More specific tiers (Exact) are faster and more confident than fuzzy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionTier {
    /// Source system ID - exact O(1) lookup
    /// Requires: source_system + source_id
    /// Confidence: 1.0
    Exact,

    /// Composite index - name + DOB + nationality
    /// Requires: name + at least one discriminator
    /// Confidence: 0.95
    Composite,

    /// Context-scoped search - name within CBU/case scope
    /// Requires: name + context (cbu_id, case_id)
    /// Confidence: 0.85
    Contextual,

    /// Fuzzy substring/phonetic search
    /// Requires: name only
    /// Confidence: varies by match score
    Fuzzy,
}

/// How entity references should be resolved in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionMode {
    /// Small static lookup tables (roles, jurisdictions, currencies)
    /// UI: Autocomplete dropdown with all values
    #[default]
    Reference,
    /// Large/growing entity tables (people, CBUs, funds, cases)
    /// UI: Search modal with refinement
    Entity,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReturnsConfig {
    #[serde(rename = "type")]
    pub return_type: ReturnTypeConfig,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub capture: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnTypeConfig {
    Uuid,
    String,
    Record,
    RecordSet,
    Affected,
    Void,
    /// Custom operation return types
    EntityQueryResult,
    TemplateInvokeResult,
    TemplateBatchResult,
    BatchControlResult,
    /// Alias for TemplateBatchResult (used in YAML)
    BatchResult,
    /// Graph query result - returns GraphViewModel
    GraphResult,
    /// Path query result - returns list of paths
    PathResult,
    /// View state result - returns view/UI state
    ViewState,
    /// Layout result - returns layout configuration
    LayoutResult,
    /// Selection info result - returns selection details
    SelectionInfo,
    /// Generic object return type
    Object,
}

// =============================================================================
// DYNAMIC VERB CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DynamicVerbConfig {
    pub pattern: String,
    #[serde(default)]
    pub source: Option<DynamicSourceConfig>,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    #[serde(default)]
    pub base_args: Vec<ArgConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DynamicSourceConfig {
    pub table: String,
    pub schema: Option<String>,
    /// Column containing the type code (e.g., "proper_person", "limited_company")
    #[serde(alias = "code_column")]
    pub type_code_column: String,
    /// Column containing the display name
    pub name_column: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
}

// =============================================================================
// CSG RULE CONFIGS
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstraintRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub when: RuleCondition,
    pub requires: RuleRequirement,
    pub error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WarningRule {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub when: Option<RuleCondition>,
    #[serde(default)]
    pub check: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JurisdictionRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: RuleSeverity,
    pub when: JurisdictionCondition,
    #[serde(default)]
    pub requires_document: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompositeRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: RuleSeverity,
    pub applies_to: AppliesTo,
    pub checks: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RuleCondition {
    #[serde(default)]
    pub verb: Option<String>,
    #[serde(default)]
    pub verb_pattern: Option<String>,
    #[serde(default)]
    pub arg: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub missing_arg: Option<String>,
    #[serde(default)]
    pub greater_than: Option<f64>,
    #[serde(default)]
    pub less_than: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RuleRequirement {
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_type_in: Option<Vec<String>>,
    #[serde(default)]
    pub via_arg: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JurisdictionCondition {
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_type_in: Option<Vec<String>>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub jurisdiction_in: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AppliesTo {
    #[serde(default)]
    pub client_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_behavior_serde() {
        let yaml = "crud";
        let behavior: VerbBehavior = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(behavior, VerbBehavior::Crud);

        let yaml = "plugin";
        let behavior: VerbBehavior = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(behavior, VerbBehavior::Plugin);
    }

    #[test]
    fn test_crud_operation_serde() {
        let yaml = "insert";
        let op: CrudOperation = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(op, CrudOperation::Insert);

        let yaml = "upsert";
        let op: CrudOperation = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(op, CrudOperation::Upsert);

        let yaml = "role_link";
        let op: CrudOperation = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(op, CrudOperation::RoleLink);
    }

    #[test]
    fn test_arg_type_serde() {
        let yaml = "string";
        let arg_type: ArgType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(arg_type, ArgType::String);

        let yaml = "uuid";
        let arg_type: ArgType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(arg_type, ArgType::Uuid);

        let yaml = "string_list";
        let arg_type: ArgType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(arg_type, ArgType::StringList);
    }

    #[test]
    fn test_return_type_serde() {
        let yaml = "uuid";
        let ret_type: ReturnTypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ret_type, ReturnTypeConfig::Uuid);

        let yaml = "record_set";
        let ret_type: ReturnTypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ret_type, ReturnTypeConfig::RecordSet);
    }

    #[test]
    fn test_resolution_mode_serde() {
        // Test explicit values
        let yaml = "reference";
        let mode: ResolutionMode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(mode, ResolutionMode::Reference);

        let yaml = "entity";
        let mode: ResolutionMode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(mode, ResolutionMode::Entity);

        // Test default
        assert_eq!(ResolutionMode::default(), ResolutionMode::Reference);
    }

    #[test]
    fn test_lookup_config_with_resolution_mode() {
        let yaml = r#"
table: cbus
schema: ob-poc
entity_type: cbu
search_key: name
primary_key: cbu_id
resolution_mode: entity
"#;
        let config: LookupConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.resolution_mode, Some(ResolutionMode::Entity));

        // Test without resolution_mode (defaults to None, which means Reference)
        let yaml = r#"
table: roles
schema: ob-poc
entity_type: role
search_key: name
primary_key: role_id
"#;
        let config: LookupConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.resolution_mode, None);
    }

    #[test]
    fn test_search_key_config_simple() {
        // Simple string search key (backwards compatible)
        let yaml = r#"
table: cbus
schema: ob-poc
entity_type: cbu
search_key: name
primary_key: cbu_id
"#;
        let config: LookupConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.search_key.is_simple());
        assert_eq!(config.search_key.primary_column(), "name");
        assert!(config.search_key.discriminators().is_empty());
    }

    #[test]
    fn test_search_key_config_composite_sexpr() {
        // Composite search key using s-expression syntax
        let yaml = r#"
table: entity_proper_persons
schema: ob-poc
entity_type: proper_person
search_key: "(search_name (date_of_birth :selectivity 0.95 :from-arg dob) (nationality :selectivity 0.7))"
primary_key: entity_id
resolution_mode: entity
"#;
        let config: LookupConfig = serde_yaml::from_str(yaml).unwrap();

        // Check it parsed as composite
        assert!(!config.search_key.is_simple());
        assert_eq!(config.search_key.primary_column(), "search_name");

        // Check discriminators
        let discriminators = config.search_key.discriminators();
        assert_eq!(discriminators.len(), 2);

        assert_eq!(discriminators[0].field, "date_of_birth");
        assert_eq!(discriminators[0].arg_name(), "dob");
        assert!((discriminators[0].selectivity - 0.95).abs() < 0.001);

        assert_eq!(discriminators[1].field, "nationality");
        assert_eq!(discriminators[1].arg_name(), "nationality"); // defaults to field name
        assert!((discriminators[1].selectivity - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_search_key_config_composite_simple_form() {
        // Composite with simple discriminator list (no options)
        let yaml = r#"
table: entities
search_key: "(name date_of_birth nationality)"
primary_key: entity_id
"#;
        let config: LookupConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(!config.search_key.is_simple());
        assert_eq!(config.search_key.primary_column(), "name");

        let discriminators = config.search_key.discriminators();
        assert_eq!(discriminators.len(), 2);
        assert_eq!(discriminators[0].field, "date_of_birth");
        assert_eq!(discriminators[1].field, "nationality");

        // Default selectivity
        assert!((discriminators[0].selectivity - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_search_key_config_composite_with_options() {
        // Composite with global options
        let yaml = r#"
table: entities
search_key: "(name (dob :selectivity 0.95) :min-confidence 0.9)"
primary_key: entity_id
"#;
        let config: LookupConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(!config.search_key.is_simple());
        assert_eq!(config.search_key.primary_column(), "name");
        assert!((config.search_key.min_confidence() - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_search_key_all_columns() {
        // Test all_columns() method
        let sk = SearchKeyConfig::parse("(search_name date_of_birth nationality)").unwrap();
        let cols = sk.all_columns();
        assert_eq!(cols.len(), 3);
        assert!(cols.contains(&"search_name"));
        assert!(cols.contains(&"date_of_birth"));
        assert!(cols.contains(&"nationality"));
    }

    #[test]
    fn test_search_key_to_sexpr_roundtrip() {
        // Test that to_sexpr produces valid s-expression that can be re-parsed
        let original = "(search_name (date_of_birth :selectivity 0.95) nationality)";
        let parsed = SearchKeyConfig::parse(original).unwrap();
        let serialized = parsed.to_sexpr();
        let reparsed = SearchKeyConfig::parse(&serialized).unwrap();

        assert_eq!(parsed.primary_column(), reparsed.primary_column());
        assert_eq!(
            parsed.discriminators().len(),
            reparsed.discriminators().len()
        );
    }

    #[test]
    fn test_tokenize_sexpr() {
        let tokens = tokenize_sexpr("(name dob)").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], SExprToken::Symbol("name".to_string()));
        assert_eq!(tokens[1], SExprToken::Symbol("dob".to_string()));
    }

    #[test]
    fn test_tokenize_sexpr_nested() {
        let tokens = tokenize_sexpr("(name (dob :selectivity 0.95))").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], SExprToken::Symbol("name".to_string()));
        if let SExprToken::List(inner) = &tokens[1] {
            assert_eq!(inner.len(), 3);
            assert_eq!(inner[0], SExprToken::Symbol("dob".to_string()));
            assert_eq!(inner[1], SExprToken::Symbol(":selectivity".to_string()));
            assert_eq!(inner[2], SExprToken::Symbol("0.95".to_string()));
        } else {
            panic!("Expected nested list");
        }
    }
}
