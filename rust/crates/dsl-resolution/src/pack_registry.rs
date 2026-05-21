//! Decision pack registry — stores and looks up `(decision-pack ...)` definitions.

use std::collections::HashMap;

use dsl_ast::AtomBag;
use dsl_diagnostics::DiagnosticBag;

// ---------------------------------------------------------------------------
// Pack types
// ---------------------------------------------------------------------------

/// A single typed parameter declared by a decision pack.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PackParam {
    pub name: String,
    /// ParamType as a string token (e.g. `"symbol"`, `"list-of-condition-expr"`).
    pub param_type: String,
    pub required: bool,
    pub description: Option<String>,
    pub default_value: Option<String>,
}

/// A resolved decision pack extracted from a `(decision-pack ...)` structural atom.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionPack {
    pub name: String,
    pub version: String,
    pub description: String,
    pub domain_scope: Vec<String>,
    pub parameters: Vec<PackParam>,
    pub example_utterances: Vec<String>,
    /// Machine-readable structural signature (the `:structural-signature` map slot).
    pub structural_signature: Option<serde_json::Value>,
    pub governance_ref: Option<String>,
    /// Raw template body string (simplified representation for v0.1).
    pub template_raw: String,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// In-memory registry of decision packs, keyed by `(name, version)`.
#[derive(Debug, Clone, Default)]
pub struct PackRegistry {
    packs: HashMap<(String, String), DecisionPack>,
}

impl PackRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pack.  Duplicate `(name, version)` keys overwrite silently.
    pub fn register(&mut self, pack: DecisionPack) -> Result<(), String> {
        let key = (pack.name.clone(), pack.version.clone());
        self.packs.insert(key, pack);
        Ok(())
    }

    /// Look up a pack by exact name and version.
    pub fn lookup(&self, name: &str, version: &str) -> Option<&DecisionPack> {
        self.packs.get(&(name.to_string(), version.to_string()))
    }

    /// Look up the pack with the lexicographically highest version for `name`.
    /// For semantic versioning this works correctly for simple version strings.
    pub fn lookup_latest(&self, name: &str) -> Option<&DecisionPack> {
        self.packs
            .iter()
            .filter(|((n, _), _)| n == name)
            .max_by_key(|((_, v), _)| v.clone())
            .map(|(_, p)| p)
    }

    /// All registered packs.
    pub fn list_active(&self) -> Vec<&DecisionPack> {
        self.packs.values().collect()
    }

    /// Number of packs currently registered.
    pub fn len(&self) -> usize {
        self.packs.len()
    }

    /// `true` when the registry contains no packs.
    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Directory loader
// ---------------------------------------------------------------------------

/// Parse every `.dsl` file in `dir` and index any `(decision-pack ...)` atoms
/// found into `registry`.  Parse and resolution diagnostics are accumulated in
/// `diagnostics`.
pub fn load_packs_from_dir(
    dir: &std::path::Path,
    registry: &mut PackRegistry,
    diagnostics: &mut DiagnosticBag,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("dsl") {
            let source = std::fs::read_to_string(&path)?;
            let (source_file, parse_diag) = dsl_parser::parse(&source);
            // Merge parse diagnostics
            for d in parse_diag.diagnostics {
                diagnostics.push(d);
            }
            let bag = AtomBag::from_source_file(source_file, diagnostics);
            crate::resolve::resolve(&bag, registry, diagnostics);
        }
    }
    Ok(())
}
