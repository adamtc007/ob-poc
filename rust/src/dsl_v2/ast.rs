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
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// A single statement
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    VerbCall(VerbCall),
    Comment(String),
}

/// A verb call: (domain.verb :key value ... :as @symbol)
#[derive(Debug, Clone, PartialEq)]
pub struct VerbCall {
    pub domain: String,
    pub verb: String,
    pub arguments: Vec<Argument>,
    /// Optional symbol binding: :as @name
    pub binding: Option<String>,
    pub span: Span,
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
}
