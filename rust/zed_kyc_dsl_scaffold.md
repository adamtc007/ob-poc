
# Zed + KYC DSL Scaffold (LSP + Extension + Dictionary + Profiles + Examples)

A ready-to-drop starter that wires your **Nom** parser + EBNF into **Zed** with:

- A **Rust LSP** (`tower-lsp`) that calls your parser, normalizer, validators
- **AttributeProvider** abstraction with **FileProvider** + **ServiceProvider (stub)**
- **Dictionary snapshot** shape (JSON)
- **Domain profile** shape (TOML)
- **Zed dev extension** that launches your LSP and maps file types
- **Examples** + **Makefile** + **Tasks**

> Replace the `dsl/parser` stub with your real Nom parser crate when ready. This scaffold compiles on its own as a baseline (parsing is stubbed with regex for UUID detection and can be swapped for your Nom AST).

---

## 1) Repository Layout

```
kyc-dsl-scaffold/
├─ Cargo.toml                # workspace
├─ README.md
├─ .kyc-dsl.toml             # dev-time context/config for LSP
├─ dictionaries/
│  └─ attributes.snapshot.json
├─ profiles/
│  └─ kyc-ubo.toml
├─ examples/
│  └─ sample.kyc
├─ dsl/
│  └─ parser/                # stub crate: replace with your Nom parser
│     ├─ Cargo.toml
│     └─ src/lib.rs
├─ lsp/
│  ├─ Cargo.toml
│  └─ src/
│     ├─ main.rs
│     ├─ util.rs
│     ├─ config.rs
│     ├─ ast_sites.rs
│     ├─ diagnostics.rs
│     ├─ completions.rs
│     ├─ hover.rs
│     ├─ symbols.rs
│     ├─ code_actions.rs
│     ├─ formatting.rs
│     └─ provider/
│        ├─ mod.rs
│        ├─ file.rs
│        └─ service.rs
├─ extensions/
│  └─ zed-kyc-dsl/
│     ├─ extension.toml
│     ├─ languages/kyc-dsl/config.toml
│     ├─ Cargo.toml
│     └─ src/lib.rs
├─ mcp/
│  └─ kyc-dsl-mcp/
│     ├─ Cargo.toml
│     └─ src/main.rs
└─ Makefile
```

---

## 2) Top-level Cargo workspace

```toml
# Cargo.toml (workspace)
[workspace]
members = [
  "dsl/parser",
  "lsp",
  "extensions/zed-kyc-dsl",
  "mcp/kyc-dsl-mcp",
]
resolver = "2"
```

---

## 3) Dev config (picked up by the LSP)

```toml
# .kyc-dsl.toml
[dict]
mode = "file"                          # "service" enables HTTP/gRPC lookups
path = "dictionaries/attributes.snapshot.json"

[profile]
path = "profiles/kyc-ubo.toml"

[context]
product = "custody"
service = "transfer-agency"
jurisdiction = "uk"
dictionary_version = "2025-11-01"

[service]
base_url = "http://localhost:8080"
# Read token from env to avoid secrets in files
bearer_env = "KYC_API_TOKEN"
```

---

## 4) Dictionary snapshot (JSON shape)

```json
{
  "dictionary_version": "2025-11-01",
  "attributes": [
    {
      "id": "00000000-0000-0000-0000-00000000c0de",
      "key": "registered-country",
      "aliases": ["attr/registered-country", "country-of-incorporation"],
      "type": {"kind": "code", "system": "ISO-3166-1-alpha-2"},
      "enum": ["GB", "US", "IE", "LU", "DE"],
      "pattern": "^[A-Z]{2}$",
      "range": null,
      "applicability": {
        "products": ["custody"],
        "services": ["transfer-agency", "fund-accounting"],
        "jurisdictions": ["uk", "ie", "lu"]
      },
      "source_order": ["entity.cbu", "document", "product", "external.api", "manual"],
      "doc": "Jurisdiction of registration/incorporation"
    },
    {
      "id": "00000000-0000-0000-0000-000000000064",
      "key": "ownership-percentage",
      "aliases": ["attr/ownership", "ubo/percent"],
      "type": {"kind": "number"},
      "enum": null,
      "pattern": null,
      "range": {"min": 0, "max": 100},
      "applicability": {"products": ["custody"], "services": ["transfer-agency"], "jurisdictions": ["uk", "ie"]},
      "source_order": ["document", "entity.cbu"],
      "doc": "Equity ownership percentage"
    }
  ]
}
```

---

## 5) Domain profile (verbs + attribute subset)

```toml
# profiles/kyc-ubo.toml
[meta]
name = "kyc-ubo"
version = "0.1.0"

[verbs]
allowed = [
  "case.create", "case.update", "case.approve",
  "workflow.transition",
  "entity.link", "ubo.calc", "ubo.outcome",
  "document.catalog", "document.use",
  "require-attributes", "set-attributes"
]

[attributes]
# allow by explicit list or by tags (if you add tags in snapshot later)
allowed_ids = [
  "00000000-0000-0000-0000-00000000c0de",
  "00000000-0000-0000-0000-000000000064"
]
```

---

## 6) Example DSL (clojure-ish s-expr)

```clojure
;; examples/sample.kyc
(context
  :onboarding-request-id "OR-2025-0007"
  :domain :kyc-ubo
  :product :custody
  :service :transfer-agency
  :jurisdiction :uk
  :dictionary-version "2025-11-01")

(case.create :case-id "CASE-001" :cbu-id "CBU-123")

(require-attributes
  :for-entity "ENT-001"
  :attributes [
    "00000000-0000-0000-0000-00000000c0de" ; registered-country
    "00000000-0000-0000-0000-000000000064" ; ownership-percentage
  ])

(document.use
  :document-id "DOC-123"
  :usage-type "EVIDENCE"
  :evidence.of-link "LINK-42"
  :extract [
    ["00000000-0000-0000-0000-00000000c0de" {:from [:ocr :fields :country-code] :type :iso-3166-1}]
    ["00000000-0000-0000-0000-000000000064" {:from [:table 1 :row 3 :col 5] :type :number :range [0 100]}]
  ])

(set-attributes
  :entity "ENT-001"
  :values {
    "00000000-0000-0000-0000-00000000c0de" "GB"
    "00000000-0000-0000-0000-000000000064" 25
  })
```

---

## 7) Stub parser crate (replace with your Nom parser)

**`dsl/parser/Cargo.toml`**

```toml
[package]
name = "dsl-parser"
version = "0.1.0"
edition = "2021"

[dependencies]
regex = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
nom = "7"
```

**`dsl/parser/src/lib.rs`** (UUID finder + attribute sites with byte ranges; swap for Nom AST later)

```rust
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DslValue { Str(String), Num(f64), Bool(bool) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttrSiteKind { Require, ValueUse, EvidenceLink, MappingKey, MappingValue }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ByteRange { pub start: usize, pub end: usize }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttrSite { pub id: String, pub range: ByteRange, pub value: Option<DslValue>, pub kind: AttrSiteKind }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ast { pub ok: bool }

/// Stub parser: replace with real Nom-based parse that yields spans + AST
pub fn parse(text: &str) -> Result<Ast, String> {
    if text.trim().is_empty() { return Err("document is empty".into()); }
    Ok(Ast { ok: true })
}

/// TEMP: Detect UUID-like tokens in quotes and return their byte ranges.
pub fn collect_attr_sites(text: &str) -> Vec<AttrSite> {
    let re = Regex::new(r#""([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})""#).unwrap();
    let mut out = vec![];
    for m in re.find_iter(text) {
        let id = text[m.start()+1..m.end()-1].to_string(); // strip quotes
        out.push(AttrSite {
            id,
            range: ByteRange { start: m.start(), end: m.end() },
            value: None,
            kind: AttrSiteKind::ValueUse,
        });
    }
    out
}
```

---

## 8) LSP crate

**`lsp/Cargo.toml`**

```toml
[package]
name = "kyc-dsl-lsp"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tower-lsp = "0.20"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
parking_lot = "0.12"
thiserror = "1"
anyhow = "1"
toml = "0.8"
async-trait = "0.1"

# your parser crate
[dependencies.dsl-parser]
path = "../dsl/parser"
```

**`lsp/src/util.rs`**

```rust
use tower_lsp::lsp_types::{Position, Range};

/// Convert a byte offset to LSP Position by scanning the text.
pub fn byte_to_position(text: &str, byte_idx: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut count = 0usize;
    for ch in text.chars() {
        if count >= byte_idx { break; }
        count += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position::new(line, col)
}

pub fn byte_range_to_lsp(text: &str, start: usize, end: usize) -> Range {
    let s = byte_to_position(text, start);
    let e = byte_to_position(text, end);
    Range::new(s, e)
}
```

**`lsp/src/config.rs`**

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DictCfg { pub mode: Option<String>, pub path: Option<String> }

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProfileCfg { pub path: Option<String> }

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ContextCfg {
    pub product: Option<String>,
    pub service: Option<String>,
    pub jurisdiction: Option<String>,
    pub dictionary_version: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ServiceCfg { pub base_url: Option<String>, pub bearer_env: Option<String> }

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RootCfg {
    pub dict: Option<DictCfg>,
    pub profile: Option<ProfileCfg>,
    pub context: Option<ContextCfg>,
    pub service: Option<ServiceCfg>,
}

pub fn load_config() -> RootCfg {
    let txt = std::fs::read_to_string(".kyc-dsl.toml").unwrap_or_default();
    toml::from_str(&txt).unwrap_or_default()
}
```

**`lsp/src/provider/mod.rs`**

```rust
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum AttrType { String, Number, Bool, Code { system: String } }

#[derive(Debug, Clone)]
pub struct AttributeMeta {
    pub id: String,
    pub key: String,
    pub aliases: Vec<String>,
    pub ty: AttrType,
    pub enum_values: Option<Vec<String>>,
    pub pattern: Option<regex::Regex>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub products: Vec<String>,
    pub services: Vec<String>,
    pub jurisdictions: Vec<String>,
    pub source_order: Vec<String>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Context {
    pub product: Option<String>,
    pub service: Option<String>,
    pub jurisdiction: Option<String>,
}

#[derive(Debug)]
pub struct ValidationError { pub msg: String }

#[async_trait]
pub trait AttributeProvider: Send + Sync {
    async fn get(&self, id_or_alias: &str) -> Option<AttributeMeta>;
    async fn list_all(&self) -> Vec<AttributeMeta>;
    async fn applicable(&self, meta: &AttributeMeta, ctx: &Context) -> bool;
    async fn validate_value(&self, meta: &AttributeMeta, v: &dsl_parser::DslValue, ctx: &Context) -> Vec<ValidationError>;
}

#[derive(Deserialize)]
struct RawAttr {
    id: String, key: String, aliases: Option<Vec<String>>,
    #[serde(rename="type")] ty: serde_json::Value,
    #[serde(default)] enum_: Option<Vec<String>>,
    pattern: Option<String>,
    range: Option<serde_json::Value>,
    applicability: Option<serde_json::Value>,
    source_order: Option<Vec<String>>,
    doc: Option<String>
}

impl AttributeMeta {
    fn from_raw(r: RawAttr) -> anyhow::Result<Self> {
        let ty = match r.ty.get("kind").and_then(|k| k.as_str()) {
            Some("number") => AttrType::Number,
            Some("bool") => AttrType::Bool,
            Some("code") => AttrType::Code { system: r.ty.get("system").and_then(|v| v.as_str()).unwrap_or("").to_string() },
            _ => AttrType::String,
        };
        let (min,max) = if let Some(range) = r.range {
            (range.get("min").and_then(|v| v.as_f64()), range.get("max").and_then(|v| v.as_f64()))
        } else { (None,None) };
        let (products, services, jurisdictions) = if let Some(app) = r.applicability {
            (
                app.get("products").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                app.get("services").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                app.get("jurisdictions").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default()
            )
        } else { (vec![],vec![],vec![]) };
        Ok(AttributeMeta {
            id: r.id,
            key: r.key,
            aliases: r.aliases.unwrap_or_default(),
            ty,
            enum_values: r.enum_.clone(),
            pattern: r.pattern.map(|p| regex::Regex::new(&p)).transpose()?,
            min, max,
            products, services, jurisdictions,
            source_order: r.source_order.unwrap_or_default(),
            doc: r.doc,
        })
    }
}

pub fn is_uuid_like(s: &str) -> bool {
    let re = regex::Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$").unwrap();
    re.is_match(s)
}
```

**`lsp/src/provider/file.rs`**

```rust
use super::*;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct FileProvider {
    by_id: RwLock<HashMap<String, AttributeMeta>>,
    by_alias: RwLock<HashMap<String, String>>,
}

impl FileProvider {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let txt = std::fs::read_to_string(path)?;
        let json: serde_json::Value = serde_json::from_str(&txt)?;
        let mut by_id = HashMap::new();
        let mut by_alias = HashMap::new();
        if let Some(attrs) = json.get("attributes").and_then(|v| v.as_array()) {
            for a in attrs {
                let raw: super::RawAttr = serde_json::from_value(a.clone())?;
                let meta = AttributeMeta::from_raw(raw)?;
                for al in &meta.aliases { by_alias.insert(al.clone(), meta.id.clone()); }
                by_id.insert(meta.id.clone(), meta);
            }
        }
        Ok(Self { by_id: RwLock::new(by_id), by_alias: RwLock::new(by_alias) })
    }
}

#[async_trait::async_trait]
impl AttributeProvider for FileProvider {
    async fn get(&self, id_or_alias: &str) -> Option<AttributeMeta> {
        let id = if super::is_uuid_like(id_or_alias) { id_or_alias.to_string() } else {
            self.by_alias.read().await.get(id_or_alias).cloned().unwrap_or_default()
        };
        self.by_id.read().await.get(&id).cloned()
    }

    async fn list_all(&self) -> Vec<AttributeMeta> {
        self.by_id.read().await.values().cloned().collect()
    }

    async fn applicable(&self, meta: &AttributeMeta, ctx: &Context) -> bool {
        let p = ctx.product.as_deref(); let s = ctx.service.as_deref(); let j = ctx.jurisdiction.as_deref();
        let ok_p = meta.products.is_empty() || p.map(|x| meta.products.iter().any(|v| v==x)).unwrap_or(true);
        let ok_s = meta.services.is_empty() || s.map(|x| meta.services.iter().any(|v| v==x)).unwrap_or(true);
        let ok_j = meta.jurisdictions.is_empty() || j.map(|x| meta.jurisdictions.iter().any(|v| v==x)).unwrap_or(true);
        ok_p && ok_s && ok_j
    }

    async fn validate_value(&self, meta: &AttributeMeta, v: &dsl_parser::DslValue, _ctx: &Context) -> Vec<ValidationError> {
        use dsl_parser::DslValue::*;
        let mut errs = vec![];
        match (&meta.ty, v) {
            (AttrType::Number, Num(n)) => {
                if let Some(min) = meta.min { if *n < min { errs.push(ValidationError{ msg: format!("value {} < min {}", n, min) }); } }
                if let Some(max) = meta.max { if *n > max { errs.push(ValidationError{ msg: format!("value {} > max {}", n, max) }); } }
            },
            (AttrType::Code{..}, Str(s)) => {
                if let Some(ev) = &meta.enum_values { if !ev.iter().any(|x| x==s) { errs.push(ValidationError{ msg: format!("{} not in enum", s) }); } }
                if let Some(p) = &meta.pattern { if !p.is_match(s) { errs.push(ValidationError{ msg: "pattern mismatch".into() }); } }
            },
            (AttrType::String, Str(_)) | (AttrType::Bool, Bool(_)) => {},
            _ => errs.push(ValidationError{ msg: "type mismatch".into() })
        }
        errs
    }
}
```

**`lsp/src/provider/service.rs`**

```rust
use super::*;

pub struct ServiceProvider;

#[async_trait::async_trait]
impl AttributeProvider for ServiceProvider {
    async fn get(&self, _id_or_alias: &str) -> Option<AttributeMeta> { None /* TODO: HTTP call */ }
    async fn list_all(&self) -> Vec<AttributeMeta> { vec![] }
    async fn applicable(&self, _meta: &AttributeMeta, _ctx: &Context) -> bool { true }
    async fn validate_value(&self, _meta: &AttributeMeta, _v: &dsl_parser::DslValue, _ctx: &Context) -> Vec<ValidationError> { vec![] }
}
```

**`lsp/src/ast_sites.rs`**

```rust
use dsl_parser::AttrSite;

pub fn attr_sites_from_text(text: &str) -> Vec<AttrSite> {
    // TEMP: regex scan; replace with AST traversal (Nom) when ready
    dsl_parser::collect_attr_sites(text)
}
```

**`lsp/src/diagnostics.rs`**

```rust
use tower_lsp::lsp_types::*;

pub fn diag_error(range: Range, msg: impl Into<String>) -> Diagnostic {
    Diagnostic{
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("kyc-dsl".into()),
        message: msg.into(),
        ..Default::default()
    }
}

pub fn diag_warn(range: Range, msg: impl Into<String>) -> Diagnostic {
    Diagnostic{
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("kyc-dsl".into()),
        message: msg.into(),
        ..Default::default()
    }
}
```

**`lsp/src/completions.rs`**

```rust
use tower_lsp::lsp_types::*;
use crate::provider::{AttributeProvider, AttributeMeta, Context};

pub async fn attr_id_completions<P: AttributeProvider>(provider: &P, ctx: &Context) -> Vec<CompletionItem> {
    let mut items = vec![];
    for meta in provider.list_all().await {
        if provider.applicable(&meta, ctx).await {
            items.push(CompletionItem {
                label: meta.id.clone(),
                detail: Some(format!("{} · {:?}", meta.key, kind_to_str(&meta))),
                kind: Some(CompletionItemKind::VALUE),
                ..Default::default()
            });
        }
    }
    items
}

fn kind_to_str(meta: &AttributeMeta) -> &'static str {
    use crate::provider::AttrType::*;
    match meta.ty { Number => "number", Bool => "bool", String => "string", Code{..} => "code" }
}
```

**`lsp/src/hover.rs`**

```rust
use tower_lsp::lsp_types::*;
use crate::provider::AttributeMeta;

pub fn hover_for_attr(meta: &AttributeMeta, range: Range) -> Option<Hover> {
    let ty = match &meta.ty {
        crate::provider::AttrType::Number => "number".to_string(),
        crate::provider::AttrType::Bool => "bool".to_string(),
        crate::provider::AttrType::String => "string".to_string(),
        crate::provider::AttrType::Code{system} => format!("code ({})", system),
    };
    let doc = meta.doc.clone().unwrap_or_default();
    let md = format!("**{}**\nType: {}\nID: `{}`\n{}", meta.key, ty, meta.id, doc);
    Some(Hover{ contents: HoverContents::Markup(MarkupContent{ kind: MarkupKind::Markdown, value: md }), range: Some(range) })
}
```

**`lsp/src/symbols.rs`**

```rust
use tower_lsp::lsp_types::*;

pub fn document_symbols(_text: &str) -> Vec<SymbolInformation> {
    // TODO: extract (context, case.create, etc.) from real AST
    vec![]
}
```

**`lsp/src/code_actions.rs`**

```rust
use tower_lsp::lsp_types::*;

pub fn normalize_alias_action(_range: Range, _from: &str, _to: &str, _uri: &Url) -> CodeActionOrCommand {
    // TODO: implement a TextEdit-based fix
    CodeActionOrCommand::CodeAction(CodeAction{ title: "Normalize to canonical attribute".into(), kind: Some(CodeActionKind::QUICKFIX), edit: None, ..Default::default() })
}
```

**`lsp/src/formatting.rs`**

```rust
use tower_lsp::lsp_types::*;

pub fn format_document(_text: &str) -> Vec<TextEdit> { vec![] }
```

**`lsp/src/main.rs`**

```rust
use tower_lsp::{LspService, Server};
use tower_lsp::lsp_types::*;
use std::sync::Arc;
use parking_lot::RwLock;

mod util;
mod config; mod ast_sites; mod diagnostics; mod completions; mod hover; mod symbols; mod code_actions; mod formatting; mod provider;
use provider::{AttributeProvider, FileProvider, ServiceProvider, Context as Ctx};

struct State<P: AttributeProvider + 'static> {
    text: RwLock<String>,
    provider: P,
    ctx: Ctx,
}

#[derive(Clone)]
struct Backend<P: AttributeProvider + 'static> {
    client: tower_lsp::Client,
    state: Arc<State<P>>,
}

#[tower_lsp::async_trait]
impl<P: AttributeProvider + 'static> tower_lsp::LanguageServer for Backend<P> {
    async fn initialize(&self, _: InitializeParams) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult{
            capabilities: ServerCapabilities{
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "kyc-dsl-lsp ready").await;
    }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        self.state.text.write().clear();
        self.state.text.write().push_str(&p.text_document.text);
        self.revalidate(p.text_document.uri).await;
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        let t = &p.content_changes[0].text;
        *self.state.text.write() = t.clone();
        self.revalidate(p.text_document.uri).await;
    }

    async fn hover(&self, p: HoverParams) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let text = self.state.text.read().clone();
        let pos = p.text_document_position_params.position;
        let byte = util::position_to_byte(&text, pos);
        let sites = ast_sites::attr_sites_from_text(&text);
        for s in sites {
            if byte >= s.range.start && byte <= s.range.end {
                if let Some(meta) = self.state.provider.get(&s.id).await {
                    let range = util::byte_range_to_lsp(&text, s.range.start, s.range.end);
                    return Ok(hover::hover_for_attr(&meta, range));
                }
            }
        }
        Ok(None)
    }

    async fn completion(&self, p: CompletionParams) -> tower_lsp::jsonrpc::Result<Option<CompletionResponse>> {
        // naive: always offer attribute IDs; refine with context/position later
        let items = completions::attr_id_completions(&self.state.provider, &self.state.ctx).await;
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn document_symbol(&self, p: DocumentSymbolParams) -> tower_lsp::jsonrpc::Result<Option<DocumentSymbolResponse>> {
        let text = self.state.text.read().clone();
        let syms = symbols::document_symbols(&text);
        Ok(Some(DocumentSymbolResponse::Flat(syms)))
    }

    async fn formatting(&self, p: DocumentFormattingParams) -> tower_lsp::jsonrpc::Result<Option<Vec<TextEdit>>> {
        Ok(Some(formatting::format_document(&self.state.text.read())))
    }
}

impl<P: AttributeProvider + 'static> Backend<P> {
    async fn revalidate(&self, uri: Url) {
        use diagnostics::*;
        let text = self.state.text.read().clone();
        let sites = ast_sites::attr_sites_from_text(&text);
        let ctx = self.state.ctx.clone();
        let mut diags: Vec<Diagnostic> = vec![];

        for site in sites {
            // L1: looks like UUID (already ensured by regex); else would be a parse error
            // L2: existence in dictionary
            let Some(meta) = self.state.provider.get(&site.id).await else {
                diags.push(diag_error(util::byte_range_to_lsp(&text, site.range.start, site.range.end), "Unknown attribute (not in dictionary)"));
                continue;
            };
            // L3: applicability in current context
            if !self.state.provider.applicable(&meta, &ctx).await {
                diags.push(diag_warn(util::byte_range_to_lsp(&text, site.range.start, site.range.end), "Attribute not applicable in current context"));
            }
            // L4: type/value checks (only when an inline value is found by real AST later)
            if let Some(v) = site.value.as_ref() {
                for e in self.state.provider.validate_value(&meta, v, &ctx).await {
                    diags.push(diag_error(util::byte_range_to_lsp(&text, site.range.start, site.range.end), e.msg));
                }
            }
        }

        self.client.publish_diagnostics(uri, diags, None).await;
    }
}

fn main_provider() -> Box<dyn AttributeProvider> {
    let cfg = config::load_config();
    if cfg.dict.as_ref().and_then(|d| d.mode.as_deref()) == Some("service") {
        Box::new(ServiceProvider)
    } else {
        let path = cfg.dict.as_ref().and_then(|d| d.path.as_deref()).unwrap_or("dictionaries/attributes.snapshot.json");
        Box::new(FileProvider::new(path).expect("load dict"))
    }
}

#[tokio::main]
async fn main() {
    let cfg = config::load_config();
    let ctx = Ctx {
        product: cfg.context.as_ref().and_then(|c| c.product.clone()),
        service: cfg.context.as_ref().and_then(|c| c.service.clone()),
        jurisdiction: cfg.context.as_ref().and_then(|c| c.jurisdiction.clone()),
    };
    let provider = main_provider();
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let state = Arc::new(State { text: Default::default(), provider: *provider, ctx });
    let (service, socket) = LspService::new(|client| Backend { client, state: state.clone() });
    Server::new(stdin, stdout, socket).serve(service).await;
}
```

**Note:** We referenced `util::position_to_byte` above; add it now:

Update **`lsp/src/util.rs`** to include `position_to_byte`:

```rust
use tower_lsp::lsp_types::{Position, Range};

pub fn byte_to_position(text: &str, byte_idx: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut count = 0usize;
    for ch in text.chars() {
        if count >= byte_idx { break; }
        count += ch.len_utf8();
        if ch == '\n' { line += 1; col = 0; } else { col += 1; }
    }
    Position::new(line, col)
}

pub fn byte_range_to_lsp(text: &str, start: usize, end: usize) -> Range {
    let s = byte_to_position(text, start);
    let e = byte_to_position(text, end);
    Range::new(s, e)
}

pub fn position_to_byte(text: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut idx = 0usize;
    for ch in text.chars() {
        if line == pos.line && col == pos.character { break; }
        idx += ch.len_utf8();
        if ch == '\n' { line += 1; col = 0; } else { col += 1; }
    }
    idx
}
```

---

## 9) Zed dev extension

**`extensions/zed-kyc-dsl/extension.toml`**

```toml
id = "kyc-dsl"
name = "KYC DSL"
version = "0.0.1"
schema_version = 1

[language_servers.kyc-dsl-lsp]
name = "KYC DSL LSP"
languages = ["KYC DSL"]
```

**`extensions/zed-kyc-dsl/languages/kyc-dsl/config.toml`**

```toml
name = "KYC DSL"
# Start with minimal setup; you can add a Tree-sitter grammar later
path_suffixes = ["kyc", "dsl", "sexpr"]
line_comments = ["; "]
tab_size = 2
```

**`extensions/zed-kyc-dsl/Cargo.toml`**

```toml
[package]
name = "zed-kyc-dsl-ext"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
zed_extension_api = "0.0.7"    # adjust to your Zed version
```

**`extensions/zed-kyc-dsl/src/lib.rs`**

```rust
use zed_extension_api as zed;
use zed::lsp::LanguageServerId;

struct Ext;

impl zed::Extension for Ext {
    fn language_server_command(&mut self, id: &LanguageServerId, _worktree: &zed::Worktree) -> zed::Result<zed::Command> {
        if id.as_str() == "kyc-dsl-lsp" {
            Ok(zed::Command {
                command: "target/debug/kyc-dsl-lsp".into(),   // ensure you've built the LSP first
                args: vec![],
                env: std::collections::BTreeMap::new(),
            })
        } else {
            Err("unknown language server".into())
        }
    }
}

zed::register_extension!(Ext);
```

> For quick highlighting, you can also temporarily map your file types to Scheme in Zed user settings (visual only):
>
> ```json
> { "file_types": { "Scheme": ["*.kyc", "*.dsl", "*.sexpr"] } }
> ```

---

## 10) Optional MCP server (Rust)

**`mcp/kyc-dsl-mcp/Cargo.toml`**

```toml
[package]
name = "kyc-dsl-mcp"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features=["derive"] }
serde_json = "1"
anyhow = "1"
axum = "0.7"
tokio = { version = "1", features=["macros", "rt-multi-thread"] }
```

**`mcp/kyc-dsl-mcp/src/main.rs`**

```rust
use axum::{routing::post, Router, Json};
use serde::{Serialize, Deserialize};

#[derive(Deserialize)] struct ParseReq { dsl: String, source: String }
#[derive(Serialize)] struct ParseResp { ast: serde_json::Value, errors: Vec<String> }

async fn parse(Json(_r): Json<ParseReq>) -> Json<ParseResp> {
    // TODO: call into your real Nom parser crate
    Json(ParseResp { ast: serde_json::json!({"root":"Program"}), errors: vec![] })
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/parse", post(parse));
    let addr = "127.0.0.1:7070".parse().unwrap();
    println!("MCP-ish server on http://{addr}");
    axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
}
```

---

## 11) Makefile

```makefile
# Makefile
.PHONY: all lsp ext mcp run install-dev

all: lsp ext

lsp:
\tcd lsp && cargo build

ext:
\tcd extensions/zed-kyc-dsl && cargo build

mcp:
\tcd mcp/kyc-dsl-mcp && cargo run

run:
\tcd lsp && RUST_LOG=info cargo run

install-dev:
\t@echo "Zed → Extensions → Install Dev Extension → select extensions/zed-kyc-dsl"
```

---

## 12) Quickstart

1) Build the LSP and the dev extension:
```bash
cargo build -p kyc-dsl-lsp && cargo build -p zed-kyc-dsl-ext
```
2) Open Zed → **Extensions → Install Dev Extension** → select `extensions/zed-kyc-dsl`.
3) Open `examples/sample.kyc`. You should see the language set to **KYC DSL**.
4) Try editing attribute IDs: the LSP parses (stub) and validates against `dictionaries/attributes.snapshot.json`.
5) When your Nom parser is ready, replace `dsl/parser` and wire its spans into diagnostics for precise errors.
6) Flip `.kyc-dsl.toml` `dict.mode = "service"` when ready to enable live presence checks.

---

## 13) Hook points (swap to Nom + AST)

- Replace `dsl/parser` with your Nom crate and:
  - Return rich `ParseError { span, len, expected }`
  - Implement a real AST walk that emits `AttrSite { id, range: ByteRange, value, kind }`
- In `lsp/src/main.rs::revalidate`:
  - Map your Nom spans → byte ranges (or line/col) → LSP `Range`
  - For each site: `provider.get` → `applicable` → `validate_value` → diagnostics
- Fill `hover.rs`, `symbols.rs`, `code_actions.rs`, and `completions.rs` using canonical verbs/keys, alias map, and validators.

---

### Notes
- This scaffold keeps editing **offline & fast** (L0–L4). You can add L5 runtime checks by implementing `ServiceProvider`.
- Profiles are **filters**, not syntax; swap profiles to change allowed verbs/attributes without touching EBNF.
- Keep dictionary snapshots versioned in git to ensure deterministic reviews/diffs.
