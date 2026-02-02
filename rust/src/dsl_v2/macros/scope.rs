//! Symbol Scoping for Nested Macro Invocation
//!
//! Provides lexical scoping for symbols during macro expansion. When a macro
//! invokes a nested macro, the nested macro operates in a child scope that can:
//! - See symbols from the parent scope
//! - Bind new symbols that shadow parent symbols
//! - Export specific symbols back to the parent via `import-symbols`
//!
//! ## Scope Chain
//!
//! ```text
//! Root Scope (DSL execution context)
//!     │
//!     ├── @cbu (from prior statement)
//!     │
//!     └── Macro: struct.hedge.cross-border
//!             │
//!             ├── @fund-name (from args)
//!             │
//!             └── invoke-macro: struct.ie.hedge.icav
//!                     │
//!                     ├── @cbu (new binding, shadows root)
//!                     ├── @trading-profile (new binding)
//!                     │
//!                     └── import-symbols: [@cbu, @trading-profile]
//!                             │
//!                             └── Exports to parent scope
//! ```

use std::collections::HashMap;
use uuid::Uuid;

/// A bound value in a symbol scope
#[derive(Debug, Clone)]
pub enum BoundValue {
    /// UUID binding (most common - entity IDs)
    Uuid(Uuid),
    /// String binding
    String(String),
    /// Integer binding
    Int(i64),
    /// JSON value binding (for complex results)
    Json(serde_json::Value),
}

impl BoundValue {
    /// Create a UUID binding
    pub fn uuid(id: Uuid) -> Self {
        BoundValue::Uuid(id)
    }

    /// Create a string binding
    pub fn string(s: impl Into<String>) -> Self {
        BoundValue::String(s.into())
    }

    /// Try to get as UUID
    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            BoundValue::Uuid(id) => Some(*id),
            BoundValue::String(s) => Uuid::parse_str(s).ok(),
            _ => None,
        }
    }

    /// Try to get as string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            BoundValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to string representation
    pub fn to_string_value(&self) -> String {
        match self {
            BoundValue::Uuid(id) => id.to_string(),
            BoundValue::String(s) => s.clone(),
            BoundValue::Int(i) => i.to_string(),
            BoundValue::Json(v) => v.to_string(),
        }
    }
}

impl From<Uuid> for BoundValue {
    fn from(id: Uuid) -> Self {
        BoundValue::Uuid(id)
    }
}

impl From<String> for BoundValue {
    fn from(s: String) -> Self {
        BoundValue::String(s)
    }
}

impl From<&str> for BoundValue {
    fn from(s: &str) -> Self {
        BoundValue::String(s.to_string())
    }
}

/// A symbol scope with optional parent chain
#[derive(Debug, Clone)]
pub struct SymbolScope {
    /// Parent scope (None for root)
    parent: Option<Box<SymbolScope>>,

    /// Local bindings in this scope
    bindings: HashMap<String, BoundValue>,

    /// Counter for generating unique symbols
    gensym_counter: u64,

    /// Scope name (for debugging/tracing)
    name: String,
}

impl Default for SymbolScope {
    fn default() -> Self {
        Self::root()
    }
}

impl SymbolScope {
    /// Create a root scope (no parent)
    pub fn root() -> Self {
        Self {
            parent: None,
            bindings: HashMap::new(),
            gensym_counter: 0,
            name: "root".to_string(),
        }
    }

    /// Create a child scope with this scope as parent
    pub fn child(&self, name: impl Into<String>) -> Self {
        Self {
            parent: Some(Box::new(self.clone())),
            bindings: HashMap::new(),
            gensym_counter: self.gensym_counter,
            name: name.into(),
        }
    }

    /// Bind a symbol in the current scope
    pub fn bind(&mut self, symbol: impl Into<String>, value: impl Into<BoundValue>) {
        let sym = normalize_symbol(symbol.into());
        self.bindings.insert(sym, value.into());
    }

    /// Resolve a symbol by walking up the scope chain
    pub fn resolve(&self, symbol: &str) -> Option<&BoundValue> {
        let sym = normalize_symbol(symbol.to_string());

        // Check local bindings first
        if let Some(value) = self.bindings.get(&sym) {
            return Some(value);
        }

        // Walk up parent chain
        if let Some(ref parent) = self.parent {
            return parent.resolve(&sym);
        }

        None
    }

    /// Check if a symbol is bound (anywhere in scope chain)
    pub fn is_bound(&self, symbol: &str) -> bool {
        self.resolve(symbol).is_some()
    }

    /// Check if a symbol is locally bound (not inherited)
    pub fn is_locally_bound(&self, symbol: &str) -> bool {
        let sym = normalize_symbol(symbol.to_string());
        self.bindings.contains_key(&sym)
    }

    /// Generate a unique symbol name
    pub fn gensym(&mut self, prefix: &str) -> String {
        self.gensym_counter += 1;
        format!("@{}_{}", prefix, self.gensym_counter)
    }

    /// Get all locally bound symbols
    pub fn local_symbols(&self) -> impl Iterator<Item = &String> {
        self.bindings.keys()
    }

    /// Get all symbols (including inherited)
    pub fn all_symbols(&self) -> Vec<String> {
        let mut symbols: HashMap<String, ()> = HashMap::new();

        // Collect from parent first (so local overrides show)
        if let Some(ref parent) = self.parent {
            for sym in parent.all_symbols() {
                symbols.insert(sym, ());
            }
        }

        // Add local symbols
        for sym in self.bindings.keys() {
            symbols.insert(sym.clone(), ());
        }

        symbols.into_keys().collect()
    }

    /// Import specific symbols from another scope into this one
    ///
    /// Used for `import-symbols` in nested macro invocation.
    pub fn import_from(&mut self, source: &SymbolScope, symbols: &[String]) {
        for sym in symbols {
            if let Some(value) = source.resolve(sym) {
                self.bind(sym.clone(), value.clone());
            }
        }
    }

    /// Export specific symbols to a mutable target scope
    pub fn export_to(&self, target: &mut SymbolScope, symbols: &[String]) {
        for sym in symbols {
            if let Some(value) = self.resolve(sym) {
                target.bind(sym.clone(), value.clone());
            }
        }
    }

    /// Get scope name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get depth in scope chain (0 = root)
    pub fn depth(&self) -> usize {
        match &self.parent {
            Some(p) => 1 + p.depth(),
            None => 0,
        }
    }
}

/// Normalize a symbol name (ensure @ prefix)
fn normalize_symbol(mut symbol: String) -> String {
    if !symbol.starts_with('@') {
        symbol = format!("@{}", symbol);
    }
    symbol
}

/// Context for tracking symbols during recursive macro expansion
#[derive(Debug, Clone)]
pub struct MacroExpansionScope {
    /// Current scope
    pub scope: SymbolScope,

    /// Stack of macro names being expanded (for cycle detection)
    expansion_stack: Vec<String>,

    /// Maximum expansion depth (prevents infinite recursion)
    max_depth: usize,
}

impl Default for MacroExpansionScope {
    fn default() -> Self {
        Self::new()
    }
}

impl MacroExpansionScope {
    /// Create a new expansion scope
    pub fn new() -> Self {
        Self {
            scope: SymbolScope::root(),
            expansion_stack: Vec::new(),
            max_depth: 10,
        }
    }

    /// Create with custom max depth
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Push a macro onto the expansion stack (returns Err if cycle detected)
    pub fn push_macro(&mut self, macro_fqn: &str) -> Result<(), MacroScopeError> {
        // Check for cycle
        if self.expansion_stack.contains(&macro_fqn.to_string()) {
            return Err(MacroScopeError::CyclicInvocation {
                macro_fqn: macro_fqn.to_string(),
                stack: self.expansion_stack.clone(),
            });
        }

        // Check depth
        if self.expansion_stack.len() >= self.max_depth {
            return Err(MacroScopeError::MaxDepthExceeded {
                depth: self.expansion_stack.len(),
                max: self.max_depth,
            });
        }

        self.expansion_stack.push(macro_fqn.to_string());
        self.scope = self.scope.child(macro_fqn);
        Ok(())
    }

    /// Pop a macro from the expansion stack
    pub fn pop_macro(&mut self) -> Option<String> {
        let macro_fqn = self.expansion_stack.pop()?;

        // Restore parent scope
        if let Some(parent) = self.scope.parent.take() {
            self.scope = *parent;
        }

        Some(macro_fqn)
    }

    /// Get current expansion depth
    pub fn depth(&self) -> usize {
        self.expansion_stack.len()
    }

    /// Check if currently inside a specific macro
    pub fn is_expanding(&self, macro_fqn: &str) -> bool {
        self.expansion_stack.contains(&macro_fqn.to_string())
    }

    /// Get the current macro being expanded
    pub fn current_macro(&self) -> Option<&str> {
        self.expansion_stack.last().map(|s| s.as_str())
    }
}

/// Errors during macro scoping
#[derive(Debug, thiserror::Error)]
pub enum MacroScopeError {
    #[error("Cyclic macro invocation detected: {macro_fqn} already in stack {stack:?}")]
    CyclicInvocation {
        macro_fqn: String,
        stack: Vec<String>,
    },

    #[error("Maximum expansion depth {max} exceeded (current: {depth})")]
    MaxDepthExceeded { depth: usize, max: usize },

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_scope_binding() {
        let mut scope = SymbolScope::root();

        let id = Uuid::now_v7();
        scope.bind("@cbu", id);

        assert!(scope.is_bound("@cbu"));
        assert!(scope.is_bound("cbu")); // Without @ prefix
        assert_eq!(scope.resolve("@cbu").unwrap().as_uuid(), Some(id));
    }

    #[test]
    fn test_child_scope_inheritance() {
        let mut root = SymbolScope::root();
        let cbu_id = Uuid::now_v7();
        root.bind("@cbu", cbu_id);

        let child = root.child("nested-macro");

        // Child can see parent bindings
        assert!(child.is_bound("@cbu"));
        assert_eq!(child.resolve("@cbu").unwrap().as_uuid(), Some(cbu_id));

        // But it's not locally bound
        assert!(!child.is_locally_bound("@cbu"));
    }

    #[test]
    fn test_child_scope_shadowing() {
        let mut root = SymbolScope::root();
        let root_cbu = Uuid::now_v7();
        root.bind("@cbu", root_cbu);

        let mut child = root.child("nested-macro");
        let child_cbu = Uuid::now_v7();
        child.bind("@cbu", child_cbu);

        // Child sees its own binding
        assert_eq!(child.resolve("@cbu").unwrap().as_uuid(), Some(child_cbu));

        // Root still has original
        assert_eq!(root.resolve("@cbu").unwrap().as_uuid(), Some(root_cbu));
    }

    #[test]
    fn test_import_symbols() {
        let mut source = SymbolScope::root();
        source.bind("@cbu", Uuid::now_v7());
        source.bind("@trading-profile", Uuid::now_v7());
        source.bind("@secret", Uuid::now_v7());

        let mut target = SymbolScope::root();

        // Import only specific symbols
        target.import_from(
            &source,
            &["@cbu".to_string(), "@trading-profile".to_string()],
        );

        assert!(target.is_bound("@cbu"));
        assert!(target.is_bound("@trading-profile"));
        assert!(!target.is_bound("@secret")); // Not imported
    }

    #[test]
    fn test_gensym() {
        let mut scope = SymbolScope::root();

        let sym1 = scope.gensym("temp");
        let sym2 = scope.gensym("temp");
        let sym3 = scope.gensym("other");

        assert_ne!(sym1, sym2);
        assert_ne!(sym2, sym3);
        assert!(sym1.starts_with("@temp_"));
        assert!(sym3.starts_with("@other_"));
    }

    #[test]
    fn test_expansion_scope_cycle_detection() {
        let mut ctx = MacroExpansionScope::new();

        ctx.push_macro("struct.setup").unwrap();
        ctx.push_macro("struct.assign-role").unwrap();

        // Trying to push already-in-stack macro should fail
        let result = ctx.push_macro("struct.setup");
        assert!(matches!(
            result,
            Err(MacroScopeError::CyclicInvocation { .. })
        ));
    }

    #[test]
    fn test_expansion_scope_max_depth() {
        let mut ctx = MacroExpansionScope::new().with_max_depth(3);

        ctx.push_macro("m1").unwrap();
        ctx.push_macro("m2").unwrap();
        ctx.push_macro("m3").unwrap();

        // Fourth push should fail
        let result = ctx.push_macro("m4");
        assert!(matches!(
            result,
            Err(MacroScopeError::MaxDepthExceeded { .. })
        ));
    }

    #[test]
    fn test_scope_depth() {
        let root = SymbolScope::root();
        assert_eq!(root.depth(), 0);

        let child = root.child("c1");
        assert_eq!(child.depth(), 1);

        let grandchild = child.child("c2");
        assert_eq!(grandchild.depth(), 2);
    }
}
