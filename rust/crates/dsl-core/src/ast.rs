//! AST v2 - Clean, self-describing AST for DSL
//!
//! This AST design separates concerns clearly:
//! - **Literals**: Terminal values (strings, numbers, booleans)
//! - **SymbolRef**: `@name` bindings within DSL (resolved at execution)
//! - **EntityRef**: External entity references that need gateway resolution
//! - **Containers**: Lists and Maps
//! - **VerbCall**: The core operation node
//!
//! ## Key Design Principle: Self-Describing AST
//!
//! Every `EntityRef` node contains all information needed for resolution:
//! - `entity_type`: What kind of entity (from YAML lookup.entity_type)
//! - `search_column`: Which column to search (from YAML lookup.search_key)
//! - `value`: The user's input
//! - `resolved_key`: The resolved primary key (None until validated)
//!
//! A tree-walk immediately identifies "breaks" (unresolved EntityRefs)
//! without needing to consult YAML configuration.
//!
//! ## Pipeline Flow
//!
//! ```text
//! Source → Parser → Raw AST (Literals + Strings)
//!                        ↓
//!               YAML Enrichment Pass
//!                        ↓
//!              Enriched AST (Strings → EntityRef where lookup config exists)
//!                        ↓
//!               Validator Tree-Walk
//!                        ↓
//!              Resolved AST (EntityRef.resolved_key populated)
//!                        ↓
//!                   Executor
//! ```

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// CORE AST TYPES
// =============================================================================

/// A complete DSL program
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Program {
    /// Render the program back to DSL source (for execution - shows UUIDs when resolved)
    pub fn to_dsl_string(&self) -> String {
        self.statements
            .iter()
            .map(|s| s.to_dsl_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Render for USER display - human readable, no UUIDs
    /// Use this for: chat UI, agent responses, DSL review panels
    pub fn to_user_dsl_string(&self) -> String {
        self.statements
            .iter()
            .map(|s| s.to_user_dsl_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A single statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    VerbCall(VerbCall),
    Comment(String),
}

impl Statement {
    /// Render the statement back to DSL source (for execution)
    pub fn to_dsl_string(&self) -> String {
        match self {
            Statement::VerbCall(vc) => vc.to_dsl_string(),
            Statement::Comment(c) => format!("; {}", c),
        }
    }

    /// Render for USER display - human readable, no UUIDs
    pub fn to_user_dsl_string(&self) -> String {
        match self {
            Statement::VerbCall(vc) => vc.to_user_dsl_string(),
            Statement::Comment(c) => format!("; {}", c),
        }
    }
}

/// A verb call: (domain.verb :key value ... :as @symbol)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerbCall {
    pub domain: String,
    pub verb: String,
    pub arguments: Vec<Argument>,
    /// Optional symbol binding: :as @name
    pub binding: Option<String>,
    pub span: Span,
}

impl VerbCall {
    /// Render the verb call back to DSL source (for execution - shows UUIDs)
    pub fn to_dsl_string(&self) -> String {
        let mut parts = vec![format!("({}.{}", self.domain, self.verb)];

        for arg in &self.arguments {
            parts.push(format!(":{} {}", arg.key, arg.value.to_dsl_string()));
        }

        if let Some(ref binding) = self.binding {
            parts.push(format!(":as @{}", binding));
        }

        parts.push(")".to_string());
        parts.join(" ")
    }

    /// Render for USER display - human readable, no UUIDs
    pub fn to_user_dsl_string(&self) -> String {
        let mut parts = vec![format!("({}.{}", self.domain, self.verb)];

        for arg in &self.arguments {
            parts.push(format!(":{} {}", arg.key, arg.value.to_user_dsl_string()));
        }

        if let Some(ref binding) = self.binding {
            parts.push(format!(":as @{}", binding));
        }

        parts.push(")".to_string());
        parts.join(" ")
    }
}

impl VerbCall {
    /// Get full verb name: "domain.verb"
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.domain, self.verb)
    }

    /// Find an argument by key name
    pub fn get_arg(&self, key: &str) -> Option<&Argument> {
        self.arguments.iter().find(|a| a.key == key)
    }

    /// Find an argument value by key name
    pub fn get_value(&self, key: &str) -> Option<&AstNode> {
        self.get_arg(key).map(|a| &a.value)
    }
}

/// A keyword-value argument
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Argument {
    /// Argument key (without colon): "name", "entity-id"
    pub key: String,
    /// Argument value
    pub value: AstNode,
    /// Source span for error reporting
    pub span: Span,
}

// =============================================================================
// AST NODE - THE CORE ENUM
// =============================================================================

/// AST Node - all possible node types in the tree
///
/// This enum cleanly separates:
/// - Literals: Terminal values that don't need resolution
/// - SymbolRef: `@name` bindings resolved at execution time
/// - EntityRef: External references that need gateway resolution
/// - Containers: Lists and Maps
/// - Nested: Nested verb calls
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AstNode {
    /// Literal value - no resolution needed
    Literal(Literal),

    /// Symbol reference: @name
    /// Resolved at execution time from :as bindings
    SymbolRef { name: String, span: Span },

    /// Entity reference - needs gateway resolution
    /// This is the "break" that validators look for
    EntityRef {
        /// Entity type from YAML lookup.entity_type
        /// e.g., "entity", "cbu", "role", "jurisdiction"
        entity_type: String,

        /// Column to search from YAML lookup.search_key
        /// e.g., "name", "code", "jurisdiction_code"
        search_column: String,

        /// The user's input value
        value: String,

        /// Resolved primary key - None until validated
        /// Can be UUID string or code (for roles, jurisdictions)
        resolved_key: Option<String>,

        /// Source span for error reporting
        span: Span,
    },

    /// List of nodes: [a, b, c]
    List { items: Vec<AstNode>, span: Span },

    /// Map of key-value pairs: {:key value}
    Map {
        entries: Vec<(String, AstNode)>,
        span: Span,
    },

    /// Nested verb call
    Nested(Box<VerbCall>),
}

impl AstNode {
    // =========================================================================
    // DSL RENDERING
    // =========================================================================

    /// Render the node back to DSL source (for execution - shows UUIDs when resolved)
    pub fn to_dsl_string(&self) -> String {
        match self {
            AstNode::Literal(lit) => lit.to_dsl_string(),
            AstNode::SymbolRef { name, .. } => format!("@{}", name),
            AstNode::EntityRef {
                value,
                resolved_key,
                ..
            } => {
                // If resolved, use the resolved key (UUID), otherwise use original value
                if let Some(key) = resolved_key {
                    format!("\"{}\"", key)
                } else {
                    format!("\"{}\"", value)
                }
            }
            AstNode::List { items, .. } => {
                let inner: Vec<String> = items.iter().map(|i| i.to_dsl_string()).collect();
                format!("[{}]", inner.join(" "))
            }
            AstNode::Map { entries, .. } => {
                let pairs: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!(":{} {}", k, v.to_dsl_string()))
                    .collect();
                format!("{{{}}}", pairs.join(" "))
            }
            AstNode::Nested(vc) => vc.to_dsl_string(),
        }
    }

    /// Render for USER display - human readable, no UUIDs
    /// Use this for: chat UI, agent responses, DSL review panels
    ///
    /// Always shows the human-readable `value` field from EntityRef,
    /// never the resolved UUID. This lets users review intent, not implementation.
    pub fn to_user_dsl_string(&self) -> String {
        match self {
            AstNode::Literal(lit) => lit.to_dsl_string(),
            AstNode::SymbolRef { name, .. } => format!("@{}", name),
            AstNode::EntityRef { value, .. } => {
                // Always show the human-readable search value, never the UUID
                format!("\"{}\"", value)
            }
            AstNode::List { items, .. } => {
                let inner: Vec<String> = items.iter().map(|i| i.to_user_dsl_string()).collect();
                format!("[{}]", inner.join(" "))
            }
            AstNode::Map { entries, .. } => {
                let pairs: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!(":{} {}", k, v.to_user_dsl_string()))
                    .collect();
                format!("{{{}}}", pairs.join(" "))
            }
            AstNode::Nested(vc) => vc.to_user_dsl_string(),
        }
    }

    // =========================================================================
    // CONSTRUCTORS
    // =========================================================================

    /// Create a string literal
    pub fn string(s: impl Into<String>) -> Self {
        AstNode::Literal(Literal::String(s.into()))
    }

    /// Create an integer literal
    pub fn integer(i: i64) -> Self {
        AstNode::Literal(Literal::Integer(i))
    }

    /// Create an unresolved entity reference
    pub fn entity_ref(
        entity_type: impl Into<String>,
        search_column: impl Into<String>,
        value: impl Into<String>,
        span: Span,
    ) -> Self {
        AstNode::EntityRef {
            entity_type: entity_type.into(),
            search_column: search_column.into(),
            value: value.into(),
            resolved_key: None,
            span,
        }
    }

    /// Create a resolved entity reference
    pub fn resolved_entity_ref(
        entity_type: impl Into<String>,
        search_column: impl Into<String>,
        value: impl Into<String>,
        resolved_key: impl Into<String>,
        span: Span,
    ) -> Self {
        AstNode::EntityRef {
            entity_type: entity_type.into(),
            search_column: search_column.into(),
            value: value.into(),
            resolved_key: Some(resolved_key.into()),
            span,
        }
    }

    /// Create a symbol reference
    pub fn symbol_ref(name: impl Into<String>, span: Span) -> Self {
        AstNode::SymbolRef {
            name: name.into(),
            span,
        }
    }

    // =========================================================================
    // PREDICATES
    // =========================================================================

    /// Is this an unresolved entity reference?
    pub fn is_unresolved_entity_ref(&self) -> bool {
        matches!(
            self,
            AstNode::EntityRef {
                resolved_key: None,
                ..
            }
        )
    }

    /// Is this a resolved entity reference?
    pub fn is_resolved_entity_ref(&self) -> bool {
        matches!(
            self,
            AstNode::EntityRef {
                resolved_key: Some(_),
                ..
            }
        )
    }

    /// Is this any kind of entity reference?
    pub fn is_entity_ref(&self) -> bool {
        matches!(self, AstNode::EntityRef { .. })
    }

    /// Is this a symbol reference?
    pub fn is_symbol_ref(&self) -> bool {
        matches!(self, AstNode::SymbolRef { .. })
    }

    /// Is this a literal?
    pub fn is_literal(&self) -> bool {
        matches!(self, AstNode::Literal(_))
    }

    // =========================================================================
    // EXTRACTORS
    // =========================================================================

    /// Get as string (from literal or entity ref value)
    pub fn as_string(&self) -> Option<&str> {
        match self {
            AstNode::Literal(Literal::String(s)) => Some(s),
            AstNode::EntityRef { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Get as UUID (from resolved entity ref)
    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            AstNode::EntityRef {
                resolved_key: Some(key),
                ..
            } => Uuid::parse_str(key).ok(),
            AstNode::Literal(Literal::String(s)) => Uuid::parse_str(s).ok(),
            AstNode::Literal(Literal::Uuid(u)) => Some(*u),
            _ => None,
        }
    }

    /// Get the resolved key from an entity ref
    pub fn resolved_key(&self) -> Option<&str> {
        match self {
            AstNode::EntityRef { resolved_key, .. } => resolved_key.as_deref(),
            _ => None,
        }
    }

    /// Get symbol name if this is a symbol ref
    pub fn as_symbol(&self) -> Option<&str> {
        match self {
            AstNode::SymbolRef { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Get integer value
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            AstNode::Literal(Literal::Integer(i)) => Some(*i),
            _ => None,
        }
    }

    /// Get decimal value
    pub fn as_decimal(&self) -> Option<Decimal> {
        match self {
            AstNode::Literal(Literal::Decimal(d)) => Some(*d),
            AstNode::Literal(Literal::Integer(i)) => Some(Decimal::from(*i)),
            _ => None,
        }
    }

    /// Get boolean value
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            AstNode::Literal(Literal::Boolean(b)) => Some(*b),
            _ => None,
        }
    }

    /// Get list items
    pub fn as_list(&self) -> Option<&[AstNode]> {
        match self {
            AstNode::List { items, .. } => Some(items),
            _ => None,
        }
    }

    /// Get map entries
    pub fn as_map(&self) -> Option<&[(String, AstNode)]> {
        match self {
            AstNode::Map { entries, .. } => Some(entries),
            _ => None,
        }
    }

    /// Get the span of this node
    pub fn span(&self) -> Span {
        match self {
            AstNode::Literal(lit) => lit.span(),
            AstNode::SymbolRef { span, .. } => *span,
            AstNode::EntityRef { span, .. } => *span,
            AstNode::List { span, .. } => *span,
            AstNode::Map { span, .. } => *span,
            AstNode::Nested(vc) => vc.span,
        }
    }

    // =========================================================================
    // RESOLUTION
    // =========================================================================

    /// Resolve an entity ref with a primary key
    /// Returns a new node with resolved_key set
    pub fn with_resolved_key(&self, key: String) -> Self {
        match self {
            AstNode::EntityRef {
                entity_type,
                search_column,
                value,
                span,
                ..
            } => AstNode::EntityRef {
                entity_type: entity_type.clone(),
                search_column: search_column.clone(),
                value: value.clone(),
                resolved_key: Some(key),
                span: *span,
            },
            _ => self.clone(),
        }
    }
}

// =============================================================================
// LITERAL VALUES
// =============================================================================

/// Literal values - terminal nodes that don't need resolution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    /// String literal: "hello"
    String(String),

    /// Integer literal: 42, -17
    Integer(i64),

    /// Decimal literal: 3.14
    Decimal(Decimal),

    /// Boolean literal: true, false
    Boolean(bool),

    /// Null literal: nil
    Null,

    /// UUID literal (parsed from string)
    Uuid(Uuid),
}

impl Literal {
    /// Get the span (literals don't track span individually, return default)
    pub fn span(&self) -> Span {
        Span::default()
    }

    /// Render the literal back to DSL source
    pub fn to_dsl_string(&self) -> String {
        match self {
            Literal::String(s) => format!("\"{}\"", s.replace('\"', "\\\"")),
            Literal::Integer(i) => i.to_string(),
            Literal::Decimal(d) => d.to_string(),
            Literal::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
            Literal::Null => "nil".to_string(),
            Literal::Uuid(u) => format!("\"{}\"", u),
        }
    }
}

// =============================================================================
// SOURCE SPAN
// =============================================================================

/// Source span for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset of start
    pub start: usize,
    /// Byte offset of end
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Create a span covering two spans
    pub fn merge(a: Span, b: Span) -> Span {
        Span {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }

    /// Length in bytes
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Is this span empty?
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Create a synthetic span (for generated code)
    ///
    /// Synthetic spans use a special marker (usize::MAX) to indicate
    /// they don't correspond to actual source text.
    pub fn synthetic() -> Self {
        Self {
            start: usize::MAX,
            end: usize::MAX,
        }
    }

    /// Check if this span is synthetic (generated, not from source)
    pub fn is_synthetic(&self) -> bool {
        self.start == usize::MAX && self.end == usize::MAX
    }
}

// =============================================================================
// TREE WALKING UTILITIES
// =============================================================================

/// Visitor trait for walking the AST
pub trait AstVisitor {
    /// Visit a statement
    fn visit_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::VerbCall(vc) => self.visit_verb_call(vc),
            Statement::Comment(_) => {}
        }
    }

    /// Visit a verb call
    fn visit_verb_call(&mut self, vc: &VerbCall) {
        for arg in &vc.arguments {
            self.visit_argument(arg);
        }
    }

    /// Visit an argument
    fn visit_argument(&mut self, arg: &Argument) {
        self.visit_node(&arg.value);
    }

    /// Visit an AST node
    fn visit_node(&mut self, node: &AstNode) {
        match node {
            AstNode::Literal(_) => {}
            AstNode::SymbolRef { .. } => self.visit_symbol_ref(node),
            AstNode::EntityRef { .. } => self.visit_entity_ref(node),
            AstNode::List { items, .. } => {
                for item in items {
                    self.visit_node(item);
                }
            }
            AstNode::Map { entries, .. } => {
                for (_, value) in entries {
                    self.visit_node(value);
                }
            }
            AstNode::Nested(vc) => self.visit_verb_call(vc),
        }
    }

    /// Visit a symbol reference - override to handle @name refs
    fn visit_symbol_ref(&mut self, _node: &AstNode) {}

    /// Visit an entity reference - override to handle resolution
    fn visit_entity_ref(&mut self, _node: &AstNode) {}
}

/// Collect all unresolved entity refs in the AST
pub fn find_unresolved_refs(program: &Program) -> Vec<&AstNode> {
    struct Collector<'a> {
        refs: Vec<&'a AstNode>,
    }

    impl<'a> AstVisitor for Collector<'a> {
        fn visit_entity_ref(&mut self, node: &AstNode) {
            if node.is_unresolved_entity_ref() {
                // Safety: we're collecting references to nodes in the program
                // This is a bit awkward but avoids cloning
                self.refs.push(unsafe { &*(node as *const AstNode) });
            }
        }
    }

    let mut collector = Collector { refs: Vec::new() };
    for stmt in &program.statements {
        collector.visit_statement(stmt);
    }
    collector.refs
}

/// Collect all symbol refs in the AST
pub fn find_symbol_refs(program: &Program) -> Vec<(String, Span)> {
    struct Collector {
        refs: Vec<(String, Span)>,
    }

    impl AstVisitor for Collector {
        fn visit_symbol_ref(&mut self, node: &AstNode) {
            if let AstNode::SymbolRef { name, span } = node {
                self.refs.push((name.clone(), *span));
            }
        }
    }

    let mut collector = Collector { refs: Vec::new() };
    for stmt in &program.statements {
        collector.visit_statement(stmt);
    }
    collector.refs
}

/// Entity reference resolution statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EntityRefStats {
    /// Total number of EntityRef nodes in the AST
    pub total_refs: i32,
    /// Number of unresolved EntityRef nodes (resolved_key is None)
    pub unresolved_count: i32,
}

impl EntityRefStats {
    /// Returns true if all EntityRefs are resolved
    pub fn is_fully_resolved(&self) -> bool {
        self.unresolved_count == 0
    }

    /// Returns the number of resolved EntityRefs
    pub fn resolved_count(&self) -> i32 {
        self.total_refs - self.unresolved_count
    }

    /// Returns resolution progress as a percentage (0-100)
    pub fn resolution_percentage(&self) -> u8 {
        if self.total_refs == 0 {
            100
        } else {
            ((self.resolved_count() as f64 / self.total_refs as f64) * 100.0) as u8
        }
    }
}

/// Count EntityRef nodes in the AST and how many are unresolved
///
/// This is used to populate `unresolved_count` and `total_refs` columns
/// in `dsl_instance_versions` for tracking resolution progress.
///
/// # Example
/// ```ignore
/// let stats = count_entity_refs(&program);
/// if stats.is_fully_resolved() {
///     // Ready for execution
/// } else {
///     println!("{}/{} refs resolved", stats.resolved_count(), stats.total_refs);
/// }
/// ```
pub fn count_entity_refs(program: &Program) -> EntityRefStats {
    struct Counter {
        total: i32,
        unresolved: i32,
    }

    impl AstVisitor for Counter {
        fn visit_entity_ref(&mut self, node: &AstNode) {
            self.total += 1;
            if node.is_unresolved_entity_ref() {
                self.unresolved += 1;
            }
        }
    }

    let mut counter = Counter {
        total: 0,
        unresolved: 0,
    };
    for stmt in &program.statements {
        counter.visit_statement(stmt);
    }

    EntityRefStats {
        total_refs: counter.total,
        unresolved_count: counter.unresolved,
    }
}

/// Location of an unresolved EntityRef in the AST
#[derive(Debug, Clone)]
pub struct UnresolvedRefLocation {
    /// Statement index in AST (0-based)
    pub statement_index: usize,
    /// Argument key containing the EntityRef
    pub arg_key: String,
    /// Entity type for search (e.g., "cbu", "entity", "product")
    pub entity_type: String,
    /// The search text entered by user
    pub search_text: String,
}

/// Extract locations of all unresolved EntityRefs in the AST
///
/// Returns a list of (statement_index, arg_key, entity_type, search_text) for each
/// unresolved EntityRef, which the UI uses to show resolution popups.
///
/// # Example
/// ```ignore
/// let unresolved = find_unresolved_ref_locations(&program);
/// for loc in unresolved {
///     println!("Statement {}, arg '{}': resolve '{}' as {}",
///         loc.statement_index, loc.arg_key, loc.search_text, loc.entity_type);
/// }
/// ```
pub fn find_unresolved_ref_locations(program: &Program) -> Vec<UnresolvedRefLocation> {
    let mut results = Vec::new();

    for (stmt_idx, stmt) in program.statements.iter().enumerate() {
        if let Statement::VerbCall(vc) = stmt {
            for arg in &vc.arguments {
                if let AstNode::EntityRef {
                    entity_type,
                    value,
                    resolved_key,
                    ..
                } = &arg.value
                {
                    if resolved_key.is_none() {
                        results.push(UnresolvedRefLocation {
                            statement_index: stmt_idx,
                            arg_key: arg.key.clone(),
                            entity_type: entity_type.clone(),
                            search_text: value.clone(),
                        });
                    }
                }
            }
        }
    }

    results
}

// =============================================================================
// VIEWPORT VERB AST TYPES
// =============================================================================

/// Viewport DSL verbs for navigation and focus control
///
/// These verbs follow the Blade Runner Esper machine vocabulary:
/// - ENHANCE - Polymorphic detail increase based on focus context
/// - NAVIGATE - Spatial movement without changing focus
/// - ASCEND/DESCEND - Hierarchical focus stack navigation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ViewportVerb {
    /// Acquire focus on target: VIEWPORT.focus(target)
    Focus { target: FocusTarget, span: Span },

    /// Polymorphic detail change: VIEWPORT.enhance(+|-|n|max|reset)
    Enhance { arg: EnhanceArg, span: Span },

    /// Move without changing focus: VIEWPORT.navigate(target|direction)
    Navigate { target: NavTarget, span: Span },

    /// Pop focus stack: VIEWPORT.ascend()
    Ascend { span: Span },

    /// Push and focus: VIEWPORT.descend(target)
    Descend { target: FocusTarget, span: Span },

    /// Switch view lens: VIEWPORT.view(view_type)
    View { view_type: ViewType, span: Span },

    /// Fit content in view: VIEWPORT.fit(zone?)
    Fit {
        zone: Option<ConfidenceZone>,
        span: Span,
    },

    /// Export current view: VIEWPORT.export(format)
    Export { format: ExportFormat, span: Span },
}

impl ViewportVerb {
    /// Get the span of this verb
    pub fn span(&self) -> Span {
        match self {
            ViewportVerb::Focus { span, .. } => *span,
            ViewportVerb::Enhance { span, .. } => *span,
            ViewportVerb::Navigate { span, .. } => *span,
            ViewportVerb::Ascend { span } => *span,
            ViewportVerb::Descend { span, .. } => *span,
            ViewportVerb::View { span, .. } => *span,
            ViewportVerb::Fit { span, .. } => *span,
            ViewportVerb::Export { span, .. } => *span,
        }
    }

    /// Get the verb name for display
    pub fn verb_name(&self) -> &'static str {
        match self {
            ViewportVerb::Focus { .. } => "focus",
            ViewportVerb::Enhance { .. } => "enhance",
            ViewportVerb::Navigate { .. } => "navigate",
            ViewportVerb::Ascend { .. } => "ascend",
            ViewportVerb::Descend { .. } => "descend",
            ViewportVerb::View { .. } => "view",
            ViewportVerb::Fit { .. } => "fit",
            ViewportVerb::Export { .. } => "export",
        }
    }

    /// Render the verb back to DSL source
    pub fn to_dsl_string(&self) -> String {
        match self {
            ViewportVerb::Focus { target, .. } => {
                format!("(viewport.focus {})", target.to_dsl_string())
            }
            ViewportVerb::Enhance { arg, .. } => {
                format!("(viewport.enhance {})", arg.to_dsl_string())
            }
            ViewportVerb::Navigate { target, .. } => {
                format!("(viewport.navigate {})", target.to_dsl_string())
            }
            ViewportVerb::Ascend { .. } => "(viewport.ascend)".to_string(),
            ViewportVerb::Descend { target, .. } => {
                format!("(viewport.descend {})", target.to_dsl_string())
            }
            ViewportVerb::View { view_type, .. } => {
                format!("(viewport.view {})", view_type.to_dsl_string())
            }
            ViewportVerb::Fit { zone, .. } => match zone {
                Some(z) => format!("(viewport.fit {})", z.to_dsl_string()),
                None => "(viewport.fit)".to_string(),
            },
            ViewportVerb::Export { format, .. } => {
                format!("(viewport.export {})", format.to_dsl_string())
            }
        }
    }
}

/// Focus target for viewport verbs
///
/// Supports hierarchical focus: CBU → Entity → Matrix → Type → Config
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FocusTarget {
    /// Focus a CBU container: cbu:ref
    Cbu { cbu_ref: String, span: Span },

    /// Focus an entity within CBU context: entity:ref
    Entity { entity_ref: String, span: Span },

    /// Focus a member relationship: member:ref
    Member { member_ref: String, span: Span },

    /// Focus an edge/relationship: edge:ref
    Edge { edge_ref: String, span: Span },

    /// Focus the instrument matrix: matrix
    Matrix { span: Span },

    /// Focus an instrument type node: type:InstrumentType
    InstrumentType { instrument_type: String, span: Span },

    /// Focus a config node (MIC, BIC, Pricing): config:node
    Config { config_node: String, span: Span },

    /// Focus a symbol reference: @symbol
    Symbol { name: String, span: Span },
}

impl FocusTarget {
    /// Get the span of this target
    pub fn span(&self) -> Span {
        match self {
            FocusTarget::Cbu { span, .. } => *span,
            FocusTarget::Entity { span, .. } => *span,
            FocusTarget::Member { span, .. } => *span,
            FocusTarget::Edge { span, .. } => *span,
            FocusTarget::Matrix { span } => *span,
            FocusTarget::InstrumentType { span, .. } => *span,
            FocusTarget::Config { span, .. } => *span,
            FocusTarget::Symbol { span, .. } => *span,
        }
    }

    /// Render the target to DSL string
    pub fn to_dsl_string(&self) -> String {
        match self {
            FocusTarget::Cbu { cbu_ref, .. } => format!(":cbu \"{}\"", cbu_ref),
            FocusTarget::Entity { entity_ref, .. } => format!(":entity \"{}\"", entity_ref),
            FocusTarget::Member { member_ref, .. } => format!(":member \"{}\"", member_ref),
            FocusTarget::Edge { edge_ref, .. } => format!(":edge \"{}\"", edge_ref),
            FocusTarget::Matrix { .. } => ":matrix".to_string(),
            FocusTarget::InstrumentType {
                instrument_type, ..
            } => format!(":type \"{}\"", instrument_type),
            FocusTarget::Config { config_node, .. } => format!(":config \"{}\"", config_node),
            FocusTarget::Symbol { name, .. } => format!("@{}", name),
        }
    }
}

/// Enhance argument for detail level changes
///
/// ENHANCE is polymorphic - the same verb increases detail differently
/// based on the current focus context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnhanceArg {
    /// Increase detail level by 1: +
    Plus,

    /// Decrease detail level by 1: -
    Minus,

    /// Set to specific level: n (0-5 depending on entity type)
    Level(u8),

    /// Set to maximum detail for current focus: max
    Max,

    /// Reset to default level for current focus: reset
    Reset,
}

impl EnhanceArg {
    /// Render the argument to DSL string
    pub fn to_dsl_string(&self) -> String {
        match self {
            EnhanceArg::Plus => "+".to_string(),
            EnhanceArg::Minus => "-".to_string(),
            EnhanceArg::Level(n) => n.to_string(),
            EnhanceArg::Max => "max".to_string(),
            EnhanceArg::Reset => "reset".to_string(),
        }
    }
}

/// Navigation target for moving without changing focus
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NavTarget {
    /// Navigate to a specific entity
    Entity { entity_ref: String, span: Span },

    /// Navigate in a direction
    Direction { direction: NavDirection, span: Span },

    /// Navigate to a symbol reference
    Symbol { name: String, span: Span },
}

impl NavTarget {
    /// Get the span of this target
    pub fn span(&self) -> Span {
        match self {
            NavTarget::Entity { span, .. } => *span,
            NavTarget::Direction { span, .. } => *span,
            NavTarget::Symbol { span, .. } => *span,
        }
    }

    /// Render the target to DSL string
    pub fn to_dsl_string(&self) -> String {
        match self {
            NavTarget::Entity { entity_ref, .. } => format!("\"{}\"", entity_ref),
            NavTarget::Direction { direction, .. } => direction.to_dsl_string(),
            NavTarget::Symbol { name, .. } => format!("@{}", name),
        }
    }
}

/// Navigation directions for spatial movement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
    In,
    Out,
}

impl NavDirection {
    /// Render the direction to DSL string
    pub fn to_dsl_string(&self) -> String {
        match self {
            NavDirection::Left => "left".to_string(),
            NavDirection::Right => "right".to_string(),
            NavDirection::Up => "up".to_string(),
            NavDirection::Down => "down".to_string(),
            NavDirection::In => "in".to_string(),
            NavDirection::Out => "out".to_string(),
        }
    }

    /// Parse a direction from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "left" => Some(NavDirection::Left),
            "right" => Some(NavDirection::Right),
            "up" => Some(NavDirection::Up),
            "down" => Some(NavDirection::Down),
            "in" => Some(NavDirection::In),
            "out" => Some(NavDirection::Out),
            _ => None,
        }
    }
}

/// View type for different visualization lenses
///
/// Each view type shows the same data through a different lens,
/// emphasizing different relationships and attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewType {
    /// Entity structure and relationships
    Structure,

    /// Ownership chains and UBO tracing
    Ownership,

    /// Account hierarchy and balances
    Accounts,

    /// Compliance status and issues
    Compliance,

    /// Geographic distribution
    Geographic,

    /// Temporal changes and history
    Temporal,

    /// Instrument matrix and trading config
    Instruments,
}

impl ViewType {
    /// Render the view type to DSL string
    pub fn to_dsl_string(&self) -> String {
        match self {
            ViewType::Structure => "structure".to_string(),
            ViewType::Ownership => "ownership".to_string(),
            ViewType::Accounts => "accounts".to_string(),
            ViewType::Compliance => "compliance".to_string(),
            ViewType::Geographic => "geographic".to_string(),
            ViewType::Temporal => "temporal".to_string(),
            ViewType::Instruments => "instruments".to_string(),
        }
    }

    /// Parse a view type from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "structure" => Some(ViewType::Structure),
            "ownership" => Some(ViewType::Ownership),
            "accounts" => Some(ViewType::Accounts),
            "compliance" => Some(ViewType::Compliance),
            "geographic" => Some(ViewType::Geographic),
            "temporal" => Some(ViewType::Temporal),
            "instruments" => Some(ViewType::Instruments),
            _ => None,
        }
    }

    /// Get all view types
    pub fn all() -> &'static [ViewType] {
        &[
            ViewType::Structure,
            ViewType::Ownership,
            ViewType::Accounts,
            ViewType::Compliance,
            ViewType::Geographic,
            ViewType::Temporal,
            ViewType::Instruments,
        ]
    }
}

/// Confidence zone for filtering and rendering
///
/// Entities are assigned to zones based on confidence score:
/// - Core: ≥0.95 (solid rendering)
/// - Shell: ≥0.70 (normal rendering)
/// - Penumbra: ≥0.40 (dashed/faded rendering)
/// - All: Include everything regardless of confidence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceZone {
    /// High confidence (≥0.95)
    Core,

    /// Medium confidence (≥0.70)
    Shell,

    /// Low confidence (≥0.40)
    Penumbra,

    /// Include all regardless of confidence
    All,
}

impl ConfidenceZone {
    /// Render the zone to DSL string
    pub fn to_dsl_string(&self) -> String {
        match self {
            ConfidenceZone::Core => "core".to_string(),
            ConfidenceZone::Shell => "shell".to_string(),
            ConfidenceZone::Penumbra => "penumbra".to_string(),
            ConfidenceZone::All => "all".to_string(),
        }
    }

    /// Parse a confidence zone from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "core" => Some(ConfidenceZone::Core),
            "shell" => Some(ConfidenceZone::Shell),
            "penumbra" => Some(ConfidenceZone::Penumbra),
            "all" => Some(ConfidenceZone::All),
            _ => None,
        }
    }

    /// Get the minimum confidence score for this zone
    pub fn min_confidence(&self) -> f32 {
        match self {
            ConfidenceZone::Core => 0.95,
            ConfidenceZone::Shell => 0.70,
            ConfidenceZone::Penumbra => 0.40,
            ConfidenceZone::All => 0.0,
        }
    }

    /// Determine which zone a confidence score belongs to
    pub fn from_score(score: f32) -> Self {
        if score >= 0.95 {
            ConfidenceZone::Core
        } else if score >= 0.70 {
            ConfidenceZone::Shell
        } else if score >= 0.40 {
            ConfidenceZone::Penumbra
        } else {
            ConfidenceZone::All // Speculative, but included in All
        }
    }
}

/// Export format for viewport content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    /// PNG image export
    Png,

    /// SVG vector export
    Svg,

    /// GraphML for graph tools
    GraphMl,

    /// Print-ready hardcopy (Esper tribute)
    Hardcopy,
}

impl ExportFormat {
    /// Render the format to DSL string
    pub fn to_dsl_string(&self) -> String {
        match self {
            ExportFormat::Png => "png".to_string(),
            ExportFormat::Svg => "svg".to_string(),
            ExportFormat::GraphMl => "graphml".to_string(),
            ExportFormat::Hardcopy => "hardcopy".to_string(),
        }
    }

    /// Parse an export format from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "png" => Some(ExportFormat::Png),
            "svg" => Some(ExportFormat::Svg),
            "graphml" => Some(ExportFormat::GraphMl),
            "hardcopy" => Some(ExportFormat::Hardcopy),
            _ => None,
        }
    }

    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Png => "png",
            ExportFormat::Svg => "svg",
            ExportFormat::GraphMl => "graphml",
            ExportFormat::Hardcopy => "pdf",
        }
    }

    /// Get the MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            ExportFormat::Png => "image/png",
            ExportFormat::Svg => "image/svg+xml",
            ExportFormat::GraphMl => "application/xml",
            ExportFormat::Hardcopy => "application/pdf",
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_ref_resolution() {
        let unresolved = AstNode::entity_ref("entity", "name", "John Smith", Span::new(0, 10));

        assert!(unresolved.is_unresolved_entity_ref());
        assert!(!unresolved.is_resolved_entity_ref());
        assert_eq!(unresolved.as_string(), Some("John Smith"));
        assert!(unresolved.as_uuid().is_none());

        let resolved =
            unresolved.with_resolved_key("550e8400-e29b-41d4-a716-446655440000".to_string());

        assert!(!resolved.is_unresolved_entity_ref());
        assert!(resolved.is_resolved_entity_ref());
        assert_eq!(resolved.as_string(), Some("John Smith"));
        assert!(resolved.as_uuid().is_some());
    }

    #[test]
    fn test_symbol_ref() {
        let sym = AstNode::symbol_ref("cbu", Span::new(5, 9));

        assert!(sym.is_symbol_ref());
        assert_eq!(sym.as_symbol(), Some("cbu"));
    }

    #[test]
    fn test_literal_types() {
        assert_eq!(AstNode::string("hello").as_string(), Some("hello"));
        assert_eq!(AstNode::integer(42).as_integer(), Some(42));
    }

    #[test]
    fn test_verb_call_helpers() {
        let vc = VerbCall {
            domain: "cbu".to_string(),
            verb: "ensure".to_string(),
            arguments: vec![
                Argument {
                    key: "name".to_string(),
                    value: AstNode::string("Test Fund"),
                    span: Span::default(),
                },
                Argument {
                    key: "jurisdiction".to_string(),
                    value: AstNode::entity_ref("jurisdiction", "code", "LU", Span::default()),
                    span: Span::default(),
                },
            ],
            binding: Some("fund".to_string()),
            span: Span::default(),
        };

        assert_eq!(vc.full_name(), "cbu.ensure");
        assert!(vc.get_arg("name").is_some());
        assert!(vc.get_arg("unknown").is_none());
        assert_eq!(
            vc.get_value("name").and_then(|v| v.as_string()),
            Some("Test Fund")
        );
    }

    #[test]
    fn test_find_unresolved() {
        let program = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "ensure".to_string(),
                arguments: vec![
                    Argument {
                        key: "name".to_string(),
                        value: AstNode::string("Test"),
                        span: Span::default(),
                    },
                    Argument {
                        key: "jurisdiction".to_string(),
                        value: AstNode::entity_ref("jurisdiction", "code", "LU", Span::default()),
                        span: Span::default(),
                    },
                ],
                binding: None,
                span: Span::default(),
            })],
        };

        let unresolved = find_unresolved_refs(&program);
        assert_eq!(unresolved.len(), 1);
        assert!(unresolved[0].is_unresolved_entity_ref());
    }

    #[test]
    fn test_count_entity_refs_empty() {
        let program = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "ensure".to_string(),
                arguments: vec![Argument {
                    key: "name".to_string(),
                    value: AstNode::string("Test"),
                    span: Span::default(),
                }],
                binding: None,
                span: Span::default(),
            })],
        };

        let stats = count_entity_refs(&program);
        assert_eq!(stats.total_refs, 0);
        assert_eq!(stats.unresolved_count, 0);
        assert!(stats.is_fully_resolved());
        assert_eq!(stats.resolution_percentage(), 100);
    }

    #[test]
    fn test_count_entity_refs_all_unresolved() {
        let program = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "assign-role".to_string(),
                arguments: vec![
                    Argument {
                        key: "cbu-id".to_string(),
                        value: AstNode::entity_ref("cbu", "name", "Apex Fund", Span::default()),
                        span: Span::default(),
                    },
                    Argument {
                        key: "entity-id".to_string(),
                        value: AstNode::entity_ref("entity", "name", "John Smith", Span::default()),
                        span: Span::default(),
                    },
                    Argument {
                        key: "role".to_string(),
                        value: AstNode::entity_ref("role", "name", "DIRECTOR", Span::default()),
                        span: Span::default(),
                    },
                ],
                binding: None,
                span: Span::default(),
            })],
        };

        let stats = count_entity_refs(&program);
        assert_eq!(stats.total_refs, 3);
        assert_eq!(stats.unresolved_count, 3);
        assert!(!stats.is_fully_resolved());
        assert_eq!(stats.resolved_count(), 0);
        assert_eq!(stats.resolution_percentage(), 0);
    }

    #[test]
    fn test_count_entity_refs_partially_resolved() {
        let program = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "assign-role".to_string(),
                arguments: vec![
                    Argument {
                        key: "cbu-id".to_string(),
                        value: AstNode::resolved_entity_ref(
                            "cbu",
                            "name",
                            "Apex Fund",
                            "11111111-1111-1111-1111-111111111111",
                            Span::default(),
                        ),
                        span: Span::default(),
                    },
                    Argument {
                        key: "entity-id".to_string(),
                        value: AstNode::entity_ref("entity", "name", "John Smith", Span::default()),
                        span: Span::default(),
                    },
                    Argument {
                        key: "role".to_string(),
                        value: AstNode::resolved_entity_ref(
                            "role",
                            "name",
                            "DIRECTOR",
                            "DIRECTOR",
                            Span::default(),
                        ),
                        span: Span::default(),
                    },
                ],
                binding: None,
                span: Span::default(),
            })],
        };

        let stats = count_entity_refs(&program);
        assert_eq!(stats.total_refs, 3);
        assert_eq!(stats.unresolved_count, 1);
        assert!(!stats.is_fully_resolved());
        assert_eq!(stats.resolved_count(), 2);
        assert_eq!(stats.resolution_percentage(), 66); // 2/3 = 66%
    }

    #[test]
    fn test_count_entity_refs_in_nested_structures() {
        let program = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "test".to_string(),
                verb: "complex".to_string(),
                arguments: vec![
                    Argument {
                        key: "list-arg".to_string(),
                        value: AstNode::List {
                            items: vec![
                                AstNode::entity_ref("entity", "name", "Person1", Span::default()),
                                AstNode::entity_ref("entity", "name", "Person2", Span::default()),
                            ],
                            span: Span::default(),
                        },
                        span: Span::default(),
                    },
                    Argument {
                        key: "map-arg".to_string(),
                        value: AstNode::Map {
                            entries: vec![(
                                "owner".to_string(),
                                AstNode::entity_ref("entity", "name", "Owner", Span::default()),
                            )],
                            span: Span::default(),
                        },
                        span: Span::default(),
                    },
                ],
                binding: None,
                span: Span::default(),
            })],
        };

        let stats = count_entity_refs(&program);
        assert_eq!(stats.total_refs, 3); // 2 in list + 1 in map
        assert_eq!(stats.unresolved_count, 3);
    }

    #[test]
    fn test_entity_ref_stats_helpers() {
        let stats = EntityRefStats {
            total_refs: 4,
            unresolved_count: 1,
        };

        assert!(!stats.is_fully_resolved());
        assert_eq!(stats.resolved_count(), 3);
        assert_eq!(stats.resolution_percentage(), 75);

        let fully_resolved = EntityRefStats {
            total_refs: 5,
            unresolved_count: 0,
        };
        assert!(fully_resolved.is_fully_resolved());
        assert_eq!(fully_resolved.resolution_percentage(), 100);
    }

    // =========================================================================
    // VIEWPORT VERB TYPE TESTS
    // =========================================================================

    #[test]
    fn test_viewport_verb_to_dsl_string() {
        let span = Span::default();

        // Focus verb
        let focus = ViewportVerb::Focus {
            target: FocusTarget::Cbu {
                cbu_ref: "Apex Fund".to_string(),
                span,
            },
            span,
        };
        assert_eq!(focus.to_dsl_string(), "(viewport.focus :cbu \"Apex Fund\")");
        assert_eq!(focus.verb_name(), "focus");

        // Enhance verb
        let enhance = ViewportVerb::Enhance {
            arg: EnhanceArg::Plus,
            span,
        };
        assert_eq!(enhance.to_dsl_string(), "(viewport.enhance +)");

        // Navigate verb
        let navigate = ViewportVerb::Navigate {
            target: NavTarget::Direction {
                direction: NavDirection::Up,
                span,
            },
            span,
        };
        assert_eq!(navigate.to_dsl_string(), "(viewport.navigate up)");

        // Ascend verb
        let ascend = ViewportVerb::Ascend { span };
        assert_eq!(ascend.to_dsl_string(), "(viewport.ascend)");

        // Descend verb
        let descend = ViewportVerb::Descend {
            target: FocusTarget::Matrix { span },
            span,
        };
        assert_eq!(descend.to_dsl_string(), "(viewport.descend :matrix)");

        // View verb
        let view = ViewportVerb::View {
            view_type: ViewType::Ownership,
            span,
        };
        assert_eq!(view.to_dsl_string(), "(viewport.view ownership)");

        // Fit verb without zone
        let fit = ViewportVerb::Fit { zone: None, span };
        assert_eq!(fit.to_dsl_string(), "(viewport.fit)");

        // Fit verb with zone
        let fit_zone = ViewportVerb::Fit {
            zone: Some(ConfidenceZone::Core),
            span,
        };
        assert_eq!(fit_zone.to_dsl_string(), "(viewport.fit core)");

        // Export verb
        let export = ViewportVerb::Export {
            format: ExportFormat::Svg,
            span,
        };
        assert_eq!(export.to_dsl_string(), "(viewport.export svg)");
    }

    #[test]
    fn test_focus_target_variants() {
        let span = Span::default();

        assert_eq!(
            FocusTarget::Cbu {
                cbu_ref: "Test".to_string(),
                span
            }
            .to_dsl_string(),
            ":cbu \"Test\""
        );

        assert_eq!(
            FocusTarget::Entity {
                entity_ref: "John".to_string(),
                span
            }
            .to_dsl_string(),
            ":entity \"John\""
        );

        assert_eq!(FocusTarget::Matrix { span }.to_dsl_string(), ":matrix");

        assert_eq!(
            FocusTarget::Symbol {
                name: "fund".to_string(),
                span
            }
            .to_dsl_string(),
            "@fund"
        );
    }

    #[test]
    fn test_enhance_arg_variants() {
        assert_eq!(EnhanceArg::Plus.to_dsl_string(), "+");
        assert_eq!(EnhanceArg::Minus.to_dsl_string(), "-");
        assert_eq!(EnhanceArg::Level(3).to_dsl_string(), "3");
        assert_eq!(EnhanceArg::Max.to_dsl_string(), "max");
        assert_eq!(EnhanceArg::Reset.to_dsl_string(), "reset");
    }

    #[test]
    fn test_nav_direction_from_str() {
        assert_eq!(NavDirection::from_str("left"), Some(NavDirection::Left));
        assert_eq!(NavDirection::from_str("RIGHT"), Some(NavDirection::Right));
        assert_eq!(NavDirection::from_str("Up"), Some(NavDirection::Up));
        assert_eq!(NavDirection::from_str("down"), Some(NavDirection::Down));
        assert_eq!(NavDirection::from_str("in"), Some(NavDirection::In));
        assert_eq!(NavDirection::from_str("out"), Some(NavDirection::Out));
        assert_eq!(NavDirection::from_str("invalid"), None);
    }

    #[test]
    fn test_view_type_from_str() {
        assert_eq!(ViewType::from_str("structure"), Some(ViewType::Structure));
        assert_eq!(ViewType::from_str("OWNERSHIP"), Some(ViewType::Ownership));
        assert_eq!(ViewType::from_str("accounts"), Some(ViewType::Accounts));
        assert_eq!(ViewType::from_str("Compliance"), Some(ViewType::Compliance));
        assert_eq!(ViewType::from_str("geographic"), Some(ViewType::Geographic));
        assert_eq!(ViewType::from_str("temporal"), Some(ViewType::Temporal));
        assert_eq!(
            ViewType::from_str("instruments"),
            Some(ViewType::Instruments)
        );
        assert_eq!(ViewType::from_str("invalid"), None);
    }

    #[test]
    fn test_view_type_all() {
        let all = ViewType::all();
        assert_eq!(all.len(), 7);
        assert!(all.contains(&ViewType::Structure));
        assert!(all.contains(&ViewType::Instruments));
    }

    #[test]
    fn test_confidence_zone_from_str() {
        assert_eq!(ConfidenceZone::from_str("core"), Some(ConfidenceZone::Core));
        assert_eq!(
            ConfidenceZone::from_str("SHELL"),
            Some(ConfidenceZone::Shell)
        );
        assert_eq!(
            ConfidenceZone::from_str("Penumbra"),
            Some(ConfidenceZone::Penumbra)
        );
        assert_eq!(ConfidenceZone::from_str("all"), Some(ConfidenceZone::All));
        assert_eq!(ConfidenceZone::from_str("invalid"), None);
    }

    #[test]
    fn test_confidence_zone_min_confidence() {
        assert_eq!(ConfidenceZone::Core.min_confidence(), 0.95);
        assert_eq!(ConfidenceZone::Shell.min_confidence(), 0.70);
        assert_eq!(ConfidenceZone::Penumbra.min_confidence(), 0.40);
        assert_eq!(ConfidenceZone::All.min_confidence(), 0.0);
    }

    #[test]
    fn test_confidence_zone_from_score() {
        assert_eq!(ConfidenceZone::from_score(0.99), ConfidenceZone::Core);
        assert_eq!(ConfidenceZone::from_score(0.95), ConfidenceZone::Core);
        assert_eq!(ConfidenceZone::from_score(0.94), ConfidenceZone::Shell);
        assert_eq!(ConfidenceZone::from_score(0.70), ConfidenceZone::Shell);
        assert_eq!(ConfidenceZone::from_score(0.69), ConfidenceZone::Penumbra);
        assert_eq!(ConfidenceZone::from_score(0.40), ConfidenceZone::Penumbra);
        assert_eq!(ConfidenceZone::from_score(0.39), ConfidenceZone::All);
        assert_eq!(ConfidenceZone::from_score(0.0), ConfidenceZone::All);
    }

    #[test]
    fn test_export_format_from_str() {
        assert_eq!(ExportFormat::from_str("png"), Some(ExportFormat::Png));
        assert_eq!(ExportFormat::from_str("SVG"), Some(ExportFormat::Svg));
        assert_eq!(
            ExportFormat::from_str("GraphML"),
            Some(ExportFormat::GraphMl)
        );
        assert_eq!(
            ExportFormat::from_str("hardcopy"),
            Some(ExportFormat::Hardcopy)
        );
        assert_eq!(ExportFormat::from_str("invalid"), None);
    }

    #[test]
    fn test_export_format_extension() {
        assert_eq!(ExportFormat::Png.extension(), "png");
        assert_eq!(ExportFormat::Svg.extension(), "svg");
        assert_eq!(ExportFormat::GraphMl.extension(), "graphml");
        assert_eq!(ExportFormat::Hardcopy.extension(), "pdf");
    }

    #[test]
    fn test_export_format_mime_type() {
        assert_eq!(ExportFormat::Png.mime_type(), "image/png");
        assert_eq!(ExportFormat::Svg.mime_type(), "image/svg+xml");
        assert_eq!(ExportFormat::GraphMl.mime_type(), "application/xml");
        assert_eq!(ExportFormat::Hardcopy.mime_type(), "application/pdf");
    }

    #[test]
    fn test_viewport_verb_spans() {
        let span = Span::new(10, 50);

        let focus = ViewportVerb::Focus {
            target: FocusTarget::Matrix { span },
            span,
        };
        assert_eq!(focus.span(), span);

        let ascend = ViewportVerb::Ascend { span };
        assert_eq!(ascend.span(), span);
    }
}
