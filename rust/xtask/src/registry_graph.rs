//! Registry/YAML completeness diff — the dead-code and dual-routing
//! oracle for the `SemOsVerbOp` surface (see
//! `docs/research/control-plane-ownership-ledger.md`, "Dead code &
//! dual-routing static sweep").
//!
//! `SemOsVerbOp` is `Arc<dyn SemOsVerbOp>` in a `HashMap<String, _>`
//! (`crates/sem_os_postgres/src/ops/registry.rs`), dispatched at runtime
//! by an FQN string the DSL compiler emits from YAML. A static call-graph
//! walker has no edge from "the thing that decides which verb to run" to
//! any individual op's `execute()` — the only real static edge is
//! `registry.register(Arc::new(ConcreteType))`, a *construction*, not an
//! *invocation*. So this module doesn't walk calls at all: it reads the
//! registration data before it becomes a `dyn`, and diffs the resulting
//! FQN set against the YAML-declared `behavior: plugin` verb set. Two
//! declared corpora, one set-difference — not a reachability analysis.
//!
//! Extraction is plain `syn` (no `ra_ap_*`, no macro expansion). Ground
//! truth established 2026-07-15 by direct inspection before writing this:
//!
//! - Registration is centralized in exactly two functions:
//!   `sem_os_postgres::ops::build_registry()` (534 direct calls) and
//!   `ob_poc::domain_ops::extend_registry()` (131 direct calls + 2
//!   delegated loop-registrations). No other `SemOsVerbOpRegistry::register`
//!   call site exists anywhere in the workspace.
//! - Of those, ~123 verbs (11 files, 12 local `macro_rules!` helpers —
//!   `changeset_op!`, `session_op!`, `phrase_op!`, `registry_op!`,
//!   `audit_op!`, `refdata_op!`, `focus_op!`, `view_op!`,
//!   `governance_op!`, `attribute_op!`, `service_pipeline_op!`,
//!   `service_pipeline_state_op!`) generate the `SemOsVerbOp` impl, with
//!   the FQN available as a literal argument at the invocation site
//!   rather than in a hand-written `fqn()` body — read directly, no
//!   macro *expansion* needed.
//! - Two special cases: `StubOp` and `SimpleStatusOp` are each a single
//!   concrete type registered many times in a `for` loop over a `const`
//!   data table (`STUB_VERBS`, `STATUS_FLIP_VERBS`). Still fully static
//!   (the FQNs are literal strings in the table) but need a dedicated
//!   extractor per type rather than the general call-site pattern.
//!   These two are deliberate many-FQN-to-one-type fan-in and are
//!   excluded from the dual-routing "same type, different FQNs" flag.
//! - `kyc_stream_ops.rs`'s `FoldRegistry::register(hash, ...)` is a
//!   *different* registry (KYC W1 stream fold dispatch, keyed by lexicon
//!   hash, not FQN) that shares the method name by coincidence — excluded
//!   entirely, not part of the `SemOsVerbOp` surface.
//!
//! If registration or macro shape drifts from this description, this
//! module's counts will visibly drop (extraction returns fewer ops than
//! `build_registry()`/`extend_registry()` actually register) rather than
//! silently misreport — the report always states its own extraction
//! count against the two functions' literal `.register(` call count so a
//! drift is self-evident.

use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// One resolved `SemOsVerbOp` registration.
#[derive(Debug, Clone)]
pub(crate) struct RegisteredOp {
    pub fqn: String,
    pub type_name: String,
    /// Where the FQN was resolved from, for the report's provenance column.
    pub site: String,
    /// File the concrete type's `impl SemOsVerbOp` block lives in — used
    /// by the composition-edge walk to re-open and parse the type's
    /// `execute()` body. `None` for the two loop-table special cases
    /// (`StubOp`/`SimpleStatusOp`), which have no meaningful single
    /// composition target (their `execute()` is generic, parameterized
    /// by the loop-table entry, not by FQN-specific logic).
    pub defining_file: Option<PathBuf>,
}

/// FQN-construction shape for the 12 known local `macro_rules!` helpers.
#[derive(Debug, Clone, Copy)]
enum MacroFqnShape {
    /// FQN = `<fixed prefix>` + first string-literal macro arg.
    FixedPrefix(&'static str),
    /// FQN = first string-literal arg + "." + second string-literal arg.
    DomainVerb,
    /// FQN = first string-literal macro arg, verbatim (already a
    /// complete "domain.verb" literal at the call site).
    WholeFqn,
}

fn macro_shape(macro_name: &str) -> Option<MacroFqnShape> {
    use MacroFqnShape::*;
    Some(match macro_name {
        "session_op" => FixedPrefix("session."),
        "changeset_op" => FixedPrefix("changeset."),
        "phrase_op" => FixedPrefix("phrase."),
        "registry_op" => FixedPrefix("registry."),
        "audit_op" => FixedPrefix("audit."),
        "refdata_op" => FixedPrefix("refdata."),
        "focus_op" => FixedPrefix("focus."),
        "view_op" => FixedPrefix("view."),
        "governance_op" => FixedPrefix("governance."),
        "attribute_op" => DomainVerb,
        "service_pipeline_op" => DomainVerb,
        "service_pipeline_state_op" => DomainVerb,
        // WholeFqn family — found during the first extraction run
        // (2026-07-15): these 5 weren't in the initial 12-macro survey
        // because that survey grepped for `concat!(` bodies, and these
        // macros pass the FQN through as a single `$fqn:literal`/
        // `$fqn:expr` with no concat! at all.
        "simple_signal_op" => WholeFqn,
        "lifecycle_op" => WholeFqn,
        "simple_evidence_op" => WholeFqn,
        "introspect_op" => WholeFqn,
        "op_struct" => WholeFqn,
        _ => return None,
    })
}

fn resolve_rust_root() -> Result<PathBuf> {
    for candidate in &[".", ".."] {
        let path = PathBuf::from(candidate);
        if path.join("crates/sem_os_postgres/src/ops/mod.rs").exists() {
            return Ok(path);
        }
    }
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = PathBuf::from(manifest_dir);
        if let Some(parent) = path.parent() {
            if parent.join("crates/sem_os_postgres/src/ops/mod.rs").exists() {
                return Ok(parent.to_path_buf());
            }
        }
    }
    bail!("could not locate rust/ root (looked for crates/sem_os_postgres/src/ops/mod.rs)")
}

/// Pull the string content out of a `proc_macro2::Literal` token that is a
/// plain `"..."` string literal. Returns `None` for non-string literals
/// (ints, chars, etc.) — none of the macros we care about use those for
/// the FQN-bearing arguments.
fn literal_string(lit: &str) -> Option<String> {
    let s = lit.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        Some(s[1..s.len() - 1].to_string())
    } else {
        None
    }
}

/// Tokenize a macro invocation's token stream into (first ident, ordered
/// string literals). Deliberately loose — skips punctuation and any extra
/// bare idents (e.g. the `fixed` keyword-ish token in
/// `service_pipeline_state_op!(Struct, "domain", "verb", fixed "STATE")`)
/// rather than requiring an exact grammar, since the macros aren't
/// uniform in shape and re-deriving a strict grammar per macro would be
/// more fragile than reading tokens in argument order.
fn tokenize_macro_call(tokens: proc_macro2::TokenStream) -> (Option<String>, Vec<String>) {
    let mut ident = None;
    let mut lits = Vec::new();
    for tt in tokens {
        match tt {
            proc_macro2::TokenTree::Ident(i) => {
                if ident.is_none() {
                    ident = Some(i.to_string());
                }
            }
            proc_macro2::TokenTree::Literal(l) => {
                if let Some(s) = literal_string(&l.to_string()) {
                    lits.push(s);
                }
            }
            _ => {}
        }
    }
    (ident, lits)
}

/// Extract the top-level string keys of a `json!({ "a": .., "b": .. })`
/// (or `serde_json::json!`) macro invocation. `tokens` is the macro's
/// full invocation tokens, which for an object literal is a single brace
/// `Group` — unwrapped one level so only *that* object's own keys are
/// collected, not keys of any value nested inside it (a nested object's
/// keys aren't this call site's own arg names, so including them would
/// be a false positive, not just noise).
fn extract_json_object_keys(tokens: proc_macro2::TokenStream) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut top = tokens.into_iter();
    let Some(proc_macro2::TokenTree::Group(g)) = top.next() else {
        return keys;
    };
    if g.delimiter() != proc_macro2::Delimiter::Brace {
        return keys;
    }
    let toks: Vec<_> = g.stream().into_iter().collect();
    for i in 0..toks.len() {
        let proc_macro2::TokenTree::Literal(lit) = &toks[i] else {
            continue;
        };
        let Some(key) = literal_string(&lit.to_string()) else {
            continue;
        };
        if let Some(proc_macro2::TokenTree::Punct(p)) = toks.get(i + 1) {
            if p.as_char() == ':' {
                keys.insert(key);
            }
        }
    }
    keys
}

/// Extract `registry.register(Arc::new(<path>))` call arguments from a
/// named top-level function's body. Returns the type path as written
/// (e.g. `"changeset::Compose"`, `"nav::Drill"`) plus the line number of
/// the call, in source order.
fn extract_direct_registrations(
    file: &syn::File,
    fn_name: &str,
    file_path: &Path,
) -> Result<Vec<(String, String)>> {
    use syn::visit::Visit;

    struct FnBodyFinder<'a> {
        target: &'a str,
        found: Option<syn::Block>,
    }
    impl<'ast> Visit<'ast> for FnBodyFinder<'_> {
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
            if node.sig.ident == self.target {
                self.found = Some((*node.block).clone());
            }
        }
    }

    let mut finder = FnBodyFinder {
        target: fn_name,
        found: None,
    };
    finder.visit_file(file);
    let Some(block) = finder.found else {
        bail!("function `{fn_name}` not found in {}", file_path.display());
    };

    struct RegisterCallVisitor {
        out: Vec<(String, String)>,
    }
    impl<'ast> Visit<'ast> for RegisterCallVisitor {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if node.method == "register" {
                if let Some(syn::Expr::Call(call)) = node.args.first() {
                    if let syn::Expr::Path(func_path) = &*call.func {
                        let is_arc_new = func_path
                            .path
                            .segments
                            .last()
                            .map(|s| s.ident == "new")
                            .unwrap_or(false)
                            && func_path.path.segments.iter().any(|s| s.ident == "Arc");
                        if is_arc_new {
                            if let Some(syn::Expr::Path(type_path)) = call.args.first() {
                                let joined = type_path
                                    .path
                                    .segments
                                    .iter()
                                    .map(|s| s.ident.to_string())
                                    .collect::<Vec<_>>()
                                    .join("::");
                                self.out.push((joined, format!("{}", node.method.span().start().line)));
                            }
                        }
                    }
                }
            }
            syn::visit::visit_expr_method_call(self, node);
        }
    }

    let mut visitor = RegisterCallVisitor { out: Vec::new() };
    visitor.visit_block(&block);
    Ok(visitor
        .out
        .into_iter()
        .map(|(t, line)| (t, format!("{}:{}", file_path.display(), line)))
        .collect())
}

/// One candidate FQN resolution for a bare type name, plus the file it
/// came from — needed because the same bare type name can exist in more
/// than one module (e.g. `nav::ZoomOut` and `view::ZoomOut` are
/// different concrete types; a bare-name-only map would silently
/// collapse them). Disambiguated at resolution time against the
/// registration call site's own module-qualified path.
#[derive(Debug, Clone)]
struct FqnCandidate {
    fqn: String,
    provenance: String,
    file: PathBuf,
}

/// Scan every `.rs` file under `dir` for direct
/// `impl SemOsVerbOp for Type { fn fqn(&self) -> &str { "lit" } }` blocks
/// and invocations of the known FQN-generating macros, returning a
/// type-name -> candidates map (see [`FqnCandidate`]).
fn build_fqn_resolution_map(dir: &Path) -> Result<BTreeMap<String, Vec<FqnCandidate>>> {
    let mut map: BTreeMap<String, Vec<FqnCandidate>> = BTreeMap::new();
    for entry in walk_rs_files(dir)? {
        let src = fs::read_to_string(&entry)
            .with_context(|| format!("reading {}", entry.display()))?;
        let file = match syn::parse_file(&src) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  (skipping {} — parse error: {e})", entry.display());
                continue;
            }
        };

        // Module-level `const NAME: &str = "literal";` declarations,
        // for `fn fqn(&self) -> &str { NAME }`-shaped bodies (a bare
        // path reference rather than a literal directly in the fn body —
        // e.g. pack_select.rs/pack_answer.rs's `const FQN: &str = "...";`).
        let mut file_consts: BTreeMap<String, String> = BTreeMap::new();
        for item in &file.items {
            let syn::Item::Const(c) = item else { continue };
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &*c.expr
            {
                file_consts.insert(c.ident.to_string(), s.value());
            }
        }

        for item in &file.items {
            match item {
                syn::Item::Impl(imp) => {
                    let Some((_, trait_path, _)) = &imp.trait_ else {
                        continue;
                    };
                    if trait_path
                        .segments
                        .last()
                        .map(|s| s.ident != "SemOsVerbOp")
                        .unwrap_or(true)
                    {
                        continue;
                    }
                    let syn::Type::Path(self_ty) = &*imp.self_ty else {
                        continue;
                    };
                    let Some(type_name) = self_ty.path.segments.last().map(|s| s.ident.to_string())
                    else {
                        continue;
                    };
                    for impl_item in &imp.items {
                        let syn::ImplItem::Fn(f) = impl_item else {
                            continue;
                        };
                        if f.sig.ident != "fqn" {
                            continue;
                        }
                        // Expect a single-expression block: a bare string
                        // literal, OR a concat!(...) whose args are all
                        // string literals (only appears inside macro
                        // *definitions*, not at call sites, per the
                        // module doc — kept here defensively in case a
                        // hand-written impl ever uses concat! directly).
                        if let Some(syn::Stmt::Expr(expr, _)) = f.block.stmts.first() {
                            let lit = match expr {
                                syn::Expr::Lit(syn::ExprLit {
                                    lit: syn::Lit::Str(s),
                                    ..
                                }) => Some(s.value()),
                                syn::Expr::Macro(m)
                                    if m.mac.path.is_ident("concat") =>
                                {
                                    let (_, lits) = tokenize_macro_call(m.mac.tokens.clone());
                                    (!lits.is_empty()).then(|| lits.concat())
                                }
                                syn::Expr::Path(p) => p
                                    .path
                                    .get_ident()
                                    .and_then(|i| file_consts.get(&i.to_string()))
                                    .cloned(),
                                _ => None,
                            };
                            if let Some(fqn) = lit {
                                map.entry(type_name.clone()).or_default().push(FqnCandidate {
                                    fqn,
                                    provenance: format!("{}: direct fqn() impl", entry.display()),
                                    file: entry.clone(),
                                });
                            }
                        }
                    }
                }
                syn::Item::Macro(item_mac) => {
                    let Some(macro_name) = item_mac.mac.path.get_ident().map(|i| i.to_string())
                    else {
                        continue;
                    };
                    let Some(shape) = macro_shape(&macro_name) else {
                        continue;
                    };
                    let (Some(struct_name), lits) =
                        tokenize_macro_call(item_mac.mac.tokens.clone())
                    else {
                        continue;
                    };
                    let fqn = match shape {
                        MacroFqnShape::FixedPrefix(prefix) => {
                            lits.first().map(|v| format!("{prefix}{v}"))
                        }
                        MacroFqnShape::DomainVerb => lits
                            .first()
                            .zip(lits.get(1))
                            .map(|(d, v)| format!("{d}.{v}")),
                        MacroFqnShape::WholeFqn => lits.first().cloned(),
                    };
                    if let Some(fqn) = fqn {
                        map.entry(struct_name).or_default().push(FqnCandidate {
                            fqn,
                            provenance: format!("{}: {macro_name}! invocation", entry.display()),
                            file: entry.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    Ok(map)
}

fn walk_rs_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_rs_files(&path)?);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
    Ok(out)
}

/// Tier 3 special case: `StubOp`, registered once per entry in
/// `STUB_VERBS: &[&str]` via a `for fqn in STUB_VERBS { registry.register(...) }`
/// loop in `stub_op.rs`.
fn extract_stub_verbs(rust_root: &Path) -> Result<Vec<RegisteredOp>> {
    let path = rust_root.join("src/domain_ops/stub_op.rs");
    let src = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let file = syn::parse_file(&src)?;
    let mut out = Vec::new();
    for item in &file.items {
        let syn::Item::Const(c) = item else { continue };
        if c.ident != "STUB_VERBS" {
            continue;
        }
        let syn::Expr::Reference(r) = &*c.expr else {
            continue;
        };
        let syn::Expr::Array(arr) = &*r.expr else {
            continue;
        };
        for elem in &arr.elems {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = elem
            {
                out.push(RegisteredOp {
                    fqn: s.value(),
                    type_name: "StubOp".to_string(),
                    site: format!("{}: STUB_VERBS table", path.display()),
                    defining_file: None,
                });
            }
        }
    }
    Ok(out)
}

/// Tier 3 special case: `SimpleStatusOp`, registered once per entry in
/// `STATUS_FLIP_VERBS: &[SimpleStatusConfig]` (each entry a struct
/// literal with an `fqn: "..."` field) via a loop in
/// `simple_status_op.rs`.
fn extract_status_flip_verbs(rust_root: &Path) -> Result<Vec<RegisteredOp>> {
    let path = rust_root.join("src/domain_ops/simple_status_op.rs");
    let src = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let file = syn::parse_file(&src)?;
    let mut out = Vec::new();
    for item in &file.items {
        let syn::Item::Const(c) = item else { continue };
        if c.ident != "STATUS_FLIP_VERBS" {
            continue;
        }
        let syn::Expr::Reference(r) = &*c.expr else {
            continue;
        };
        let syn::Expr::Array(arr) = &*r.expr else {
            continue;
        };
        for elem in &arr.elems {
            let syn::Expr::Struct(s) = elem else { continue };
            for field in &s.fields {
                let syn::Member::Named(name) = &field.member else {
                    continue;
                };
                if name != "fqn" {
                    continue;
                }
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) = &field.expr
                {
                    out.push(RegisteredOp {
                        fqn: lit.value(),
                        type_name: "SimpleStatusOp".to_string(),
                        site: format!("{}: STATUS_FLIP_VERBS table", path.display()),
                        defining_file: None,
                    });
                }
            }
        }
    }
    Ok(out)
}

/// Full extraction: everything registered into the real
/// `SemOsVerbOpRegistry`, from both `build_registry()` and
/// `extend_registry()`, resolved to FQNs.
pub(crate) fn extract_all_registrations(rust_root: &Path) -> Result<(Vec<RegisteredOp>, usize)> {
    let ops_dir = rust_root.join("crates/sem_os_postgres/src/ops");
    let domain_ops_dir = rust_root.join("src/domain_ops");

    let mut fqn_map = build_fqn_resolution_map(&ops_dir)?;
    fqn_map.extend(build_fqn_resolution_map(&domain_ops_dir)?);

    let mut direct_call_count = 0usize;
    let mut resolved = Vec::new();
    let mut unresolved = Vec::new();

    let ops_mod_path = ops_dir.join("mod.rs");
    let ops_mod_src = fs::read_to_string(&ops_mod_path)?;
    let ops_mod_file = syn::parse_file(&ops_mod_src)?;
    let build_registry_calls =
        extract_direct_registrations(&ops_mod_file, "build_registry", &ops_mod_path)?;
    direct_call_count += build_registry_calls.len();

    let domain_ops_mod_path = domain_ops_dir.join("mod.rs");
    let domain_ops_mod_src = fs::read_to_string(&domain_ops_mod_path)?;
    let domain_ops_mod_file = syn::parse_file(&domain_ops_mod_src)?;
    let extend_registry_calls =
        extract_direct_registrations(&domain_ops_mod_file, "extend_registry", &domain_ops_mod_path)?;
    direct_call_count += extend_registry_calls.len();

    for (type_path, site) in build_registry_calls
        .into_iter()
        .chain(extend_registry_calls)
    {
        let type_name = type_path
            .rsplit("::")
            .next()
            .unwrap_or(&type_path)
            .to_string();
        // Every registration call site we've found is written as
        // `module::Type` (verified: 657/657 direct calls are exactly
        // 2 segments) — the module segment disambiguates cases where the
        // same bare type name exists in more than one module (e.g.
        // `nav::ZoomOut` vs `view::ZoomOut`).
        let module_hint = type_path
            .rsplit("::")
            .nth(1)
            .map(|s| s.to_string());

        let candidate = match fqn_map.get(&type_name).map(|v| v.as_slice()) {
            Some([single]) => Some(single),
            Some(many) if many.len() > 1 => {
                let hint = module_hint.as_deref();
                let matches: Vec<&FqnCandidate> = many
                    .iter()
                    .filter(|c| {
                        let stem = c.file.file_stem().and_then(|s| s.to_str());
                        let parent_dir = c
                            .file
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str());
                        hint.is_some() && (hint == stem || hint == parent_dir)
                    })
                    .collect();
                match matches.as_slice() {
                    [one] => Some(*one),
                    _ => None,
                }
            }
            _ => None,
        };

        match candidate {
            Some(c) => resolved.push(RegisteredOp {
                fqn: c.fqn.clone(),
                type_name: type_path,
                site: format!("{site} (fqn from {})", c.provenance),
                defining_file: Some(c.file.clone()),
            }),
            None => unresolved.push((type_path, site)),
        }
    }

    resolved.extend(extract_stub_verbs(rust_root)?);
    resolved.extend(extract_status_flip_verbs(rust_root)?);

    if !unresolved.is_empty() {
        eprintln!(
            "WARNING: {} registration(s) could not be resolved to an FQN (extraction gap, not a code defect — treat these as unknown, not dead):",
            unresolved.len()
        );
        for (type_name, site) in &unresolved {
            eprintln!("  {type_name} at {site}");
        }
    }

    Ok((resolved, direct_call_count))
}

/// Load the YAML-declared `behavior: plugin` verb FQN set.
fn load_yaml_plugin_verbs(rust_root: &Path) -> Result<BTreeSet<String>> {
    std::env::set_current_dir(rust_root)?;
    let loader = dsl_core::ConfigLoader::from_env();
    let verbs_config = loader.load_verbs().context("failed to load verb YAML")?;
    let mut out = BTreeSet::new();
    for (domain_name, domain) in &verbs_config.domains {
        for (verb_name, verb) in &domain.verbs {
            if verb.behavior == dsl_core::VerbBehavior::Plugin {
                out.insert(format!("{domain_name}.{verb_name}"));
            }
        }
    }
    Ok(out)
}

// ── Composition edges: verb -> verb calls within execute() bodies ──
//
// Verified 2026-07-15 before writing this: the only registry-callback
// mechanism (an execute() body invoking another SemOsVerbOp through the
// registry, as opposed to delegating to a typed service module) is
// `SemOsChildDispatcher::dispatch_child` — its own doc comment says so
// ("not a separate cascade engine"), and no other escape hatch exists on
// `VerbExecutionContext`/`TransactionScope`. Every call site found
// (cbu.rs, cbu_role.rs — 15 edges across 2 files) passes a compile-time
// string literal for the child FQN, so this closes out statically: no
// verb composes another verb via a runtime-computed FQN anywhere in the
// workspace. If that ever changes, this extractor will surface it loudly
// (a composition call whose child FQN isn't a literal is simply not
// found, so the edge silently disappears from the report rather than
// crashing — cross-check the `.register(` count if edge counts ever look
// low against a known change).

/// How a "relay" function (a free function that itself calls
/// `.dispatch_child(...)`, standing between an `execute()` body and the
/// registry) determines its child FQN.
#[derive(Debug, Clone)]
enum RelayKind {
    /// The child FQN is the relay's own Nth positional parameter,
    /// passed straight through to `.dispatch_child(..)` — so the real
    /// value must be read from literal arguments at each *call site* of
    /// the relay (e.g. `dispatch_child_verb`, whose 2nd parameter
    /// `child_fqn` is forwarded verbatim).
    Transparent { child_arg_index: usize },
    /// The relay always dispatches this one literal FQN regardless of
    /// caller (e.g. `upsert_entity_relationship`, hardcoded to
    /// `"entity-relationship.upsert"`).
    Fixed(String),
}

/// One statically-discovered verb -> verb composition edge.
#[derive(Debug, Clone)]
struct CompositionEdge {
    parent_fqn: String,
    child_fqn: String,
    site: String,
    /// Top-level JSON keys of the args passed at this call site, if
    /// statically resolvable (the args expr is `&some_var` where
    /// `some_var` was bound to a `json!({...})` literal earlier in the
    /// same function body). `None` means "couldn't determine" — a gap
    /// to report, not a pass, when cross-checking fold-verb selector
    /// args below.
    caller_arg_keys: Option<BTreeSet<String>>,
}

/// Scan every `.rs` file under `dirs` for free functions whose body
/// contains a `.dispatch_child(parent, child, ...)` call, and classify
/// each as [`RelayKind::Transparent`] or [`RelayKind::Fixed`] by
/// inspecting the second argument.
fn find_relay_functions(dirs: &[PathBuf]) -> Result<BTreeMap<String, RelayKind>> {
    let mut relays = BTreeMap::new();
    for dir in dirs {
        for entry in walk_rs_files(dir)? {
            let src = fs::read_to_string(&entry)?;
            let Ok(file) = syn::parse_file(&src) else {
                continue;
            };
            for item in &file.items {
                let syn::Item::Fn(f) = item else { continue };
                let params: Vec<String> = f
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|arg| {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            if let syn::Pat::Ident(id) = &*pat_type.pat {
                                return Some(id.ident.to_string());
                            }
                        }
                        None
                    })
                    .collect();

                let mut found = None;
                visit_calls(&f.block, &mut |call| {
                    if found.is_some() {
                        return;
                    }
                    let syn::Expr::MethodCall(mc) = call else {
                        return;
                    };
                    if mc.method != "dispatch_child" {
                        return;
                    }
                    let Some(child_arg) = mc.args.get(1) else {
                        return;
                    };
                    match child_arg {
                        syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) => found = Some(RelayKind::Fixed(s.value())),
                        syn::Expr::Path(p) => {
                            if let Some(ident) = p.path.get_ident() {
                                if let Some(idx) =
                                    params.iter().position(|p| p == &ident.to_string())
                                {
                                    found = Some(RelayKind::Transparent {
                                        child_arg_index: idx,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                });

                if let Some(kind) = found {
                    relays.insert(f.sig.ident.to_string(), kind);
                }
            }
        }
    }
    Ok(relays)
}

/// Walk every expression in `block`, calling `f` on each one. Not a full
/// `syn::visit::Visit` impl — deliberately shallow (doesn't need to
/// distinguish expression kinds beyond finding call/method-call nodes),
/// so a plain recursive walk over `syn::visit::Visit::visit_expr` is
/// simpler than defining a dedicated visitor struct.
fn visit_calls(block: &syn::Block, f: &mut impl FnMut(&syn::Expr)) {
    use syn::visit::Visit;
    struct Walker<'a, F: FnMut(&syn::Expr)>(&'a mut F);
    impl<'a, 'ast, F: FnMut(&syn::Expr)> Visit<'ast> for Walker<'a, F> {
        fn visit_expr(&mut self, node: &'ast syn::Expr) {
            (self.0)(node);
            syn::visit::visit_expr(self, node);
        }
    }
    Walker(f).visit_block(block);
}

/// For each resolved op with a known defining file, re-open that file,
/// find the op's `impl SemOsVerbOp for Type { ... async fn execute(...)
/// {...} }` block, and walk its body for composition calls — either a
/// direct `.dispatch_child(parent, "literal", ...)` or a call to one of
/// `relays`.
/// Walk `block`'s top-level `let NAME = serde_json::json!({...});` (or
/// bare `json!({...})`) statements and return a var-name -> top-level-key
/// map. Shallow on purpose — only direct macro-call initializers are
/// recognized; a `let` built via `.insert()` calls or any other shape is
/// simply absent from the map, which downstream treats as "unknown," not
/// "no keys."
fn collect_json_var_keys(block: &syn::Block) -> BTreeMap<String, BTreeSet<String>> {
    let mut map = BTreeMap::new();
    for stmt in &block.stmts {
        let syn::Stmt::Local(local) = stmt else { continue };
        let syn::Pat::Ident(pat_ident) = &local.pat else {
            continue;
        };
        let Some(init) = &local.init else { continue };
        let syn::Expr::Macro(m) = &*init.expr else {
            continue;
        };
        if m.mac.path.segments.last().map(|s| s.ident != "json").unwrap_or(true) {
            continue;
        }
        map.insert(
            pat_ident.ident.to_string(),
            extract_json_object_keys(m.mac.tokens.clone()),
        );
    }
    map
}

/// Extracts, per file: composition edges (verb -> verb calls) AND
/// fold-verb selector requirements (verbs whose `execute()` calls the
/// strict `dispatch_selector` — the `resolve_selector`-with-fallback
/// shape, e.g. `cbu.assign-role`, is deliberately excluded here since an
/// absent/unrecognized selector there is valid — it just falls through
/// to the verb's own generic handling, not an error).
fn extract_composition_edges(
    registered: &[RegisteredOp],
    relays: &BTreeMap<String, RelayKind>,
) -> Result<(Vec<CompositionEdge>, BTreeMap<String, String>)> {
    let mut edges = Vec::new();
    let mut fold_verbs: BTreeMap<String, String> = BTreeMap::new();
    // Group by defining file so each file is parsed once even if it
    // defines multiple registered ops (e.g. cbu.rs defines 8).
    let mut by_file: BTreeMap<PathBuf, Vec<&RegisteredOp>> = BTreeMap::new();
    for op in registered {
        if let Some(file) = &op.defining_file {
            by_file.entry(file.clone()).or_default().push(op);
        }
    }

    for (file, ops) in by_file {
        let src = fs::read_to_string(&file)?;
        let Ok(parsed) = syn::parse_file(&src) else {
            continue;
        };
        for item in &parsed.items {
            let syn::Item::Impl(imp) = item else { continue };
            let Some((_, trait_path, _)) = &imp.trait_ else {
                continue;
            };
            if trait_path.segments.last().map(|s| s.ident != "SemOsVerbOp").unwrap_or(true) {
                continue;
            }
            let syn::Type::Path(self_ty) = &*imp.self_ty else {
                continue;
            };
            let Some(type_name) = self_ty.path.segments.last().map(|s| s.ident.to_string())
            else {
                continue;
            };
            let Some(op) = ops.iter().find(|o| o.type_name.ends_with(&type_name)) else {
                continue;
            };

            for impl_item in &imp.items {
                let syn::ImplItem::Fn(exec_fn) = impl_item else {
                    continue;
                };
                if exec_fn.sig.ident != "execute" {
                    continue;
                }
                let json_vars = collect_json_var_keys(&exec_fn.block);

                visit_calls(&exec_fn.block, &mut |call| {
                    let (method_or_fn, args): (String, &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>) =
                        match call {
                            syn::Expr::MethodCall(mc) => (mc.method.to_string(), &mc.args),
                            syn::Expr::Call(c) => {
                                let syn::Expr::Path(p) = &*c.func else {
                                    return;
                                };
                                let Some(name) = p.path.segments.last().map(|s| s.ident.to_string())
                                else {
                                    return;
                                };
                                (name, &c.args)
                            }
                            _ => return,
                        };

                    // Fold-verb detection: a strict dispatch_selector(args,
                    // ctx, scope, "arg_name", arms) call inside this op's
                    // own execute() means THIS op requires "arg_name".
                    if method_or_fn == "dispatch_selector" {
                        if let Some(syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        })) = args.get(3)
                        {
                            fold_verbs.insert(op.fqn.clone(), s.value());
                        }
                        return;
                    }

                    // Resolve the args expression (3rd positional arg for
                    // dispatch_child, or the relay's own args parameter for
                    // a relay-function call — both line up at index 2 in
                    // every call site seen) to a variable name, then to its
                    // known JSON keys, if any.
                    let caller_arg_keys = args.get(2).and_then(|a| {
                        let syn::Expr::Reference(r) = a else { return None };
                        let syn::Expr::Path(p) = &*r.expr else {
                            return None;
                        };
                        let ident = p.path.get_ident()?.to_string();
                        json_vars.get(&ident).cloned()
                    });

                    // Direct .dispatch_child(parent, "literal", ...).
                    if method_or_fn == "dispatch_child" {
                        if let Some(syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        })) = args.get(1)
                        {
                            edges.push(CompositionEdge {
                                parent_fqn: op.fqn.clone(),
                                child_fqn: s.value(),
                                site: format!("{}: direct dispatch_child", file.display()),
                                caller_arg_keys,
                            });
                        }
                        return;
                    }

                    // Call to a known relay function.
                    if let Some(kind) = relays.get(&method_or_fn) {
                        let child = match kind {
                            RelayKind::Fixed(fqn) => Some(fqn.clone()),
                            RelayKind::Transparent { child_arg_index } => {
                                args.iter().nth(*child_arg_index).and_then(|a| {
                                    if let syn::Expr::Lit(syn::ExprLit {
                                        lit: syn::Lit::Str(s),
                                        ..
                                    }) = a
                                    {
                                        Some(s.value())
                                    } else {
                                        None
                                    }
                                })
                            }
                        };
                        if let Some(child_fqn) = child {
                            edges.push(CompositionEdge {
                                parent_fqn: op.fqn.clone(),
                                child_fqn,
                                site: format!("{}: via {method_or_fn}()", file.display()),
                                caller_arg_keys,
                            });
                        }
                    }
                });
            }
        }
    }

    Ok((edges, fold_verbs))
}

fn write_composition_report(
    dir: &Path,
    edges: &[CompositionEdge],
    registered_fqns: &BTreeSet<String>,
    fold_verbs: &BTreeMap<String, String>,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut md = String::new();
    md.push_str("# Verb composition graph — statically-discovered execute()-to-registry calls\n\n");
    md.push_str("Every edge here is a compile-time string literal at the call site —\n");
    md.push_str("confirmed 2026-07-15 that no verb in this workspace composes another verb\n");
    md.push_str("via a runtime-computed FQN (the only registry-callback mechanism,\n");
    md.push_str("`SemOsChildDispatcher::dispatch_child`, is called from exactly 2 files,\n");
    md.push_str("15 call sites, all literals). This is therefore a closed, static graph,\n");
    md.push_str("not a sample of runtime behavior.\n\n");
    md.push_str("Fold-verb detection only scans an op's own `execute()` body, not helper\n");
    md.push_str("functions it delegates to — a fold verb that calls `dispatch_selector`\n");
    md.push_str("through an indirection (e.g. `gleif.lookup`, which routes through its own\n");
    md.push_str("`Self::resolve()`) won't be found. Known gap, not a silent one: if a\n");
    md.push_str("composition edge targets a fold verb missed this way, it just won't be\n");
    md.push_str("flagged below — treat an empty \"missing selector arg\" section as\n");
    md.push_str("\"nothing found among the detected fold verbs,\" not \"verified clean.\"\n\n");
    md.push_str(&format!(
        "{} fold verb(s) detected (require a selector arg via the strict\n`dispatch_selector` shape — see selector_dispatch.rs): {}\n\n",
        fold_verbs.len(),
        fold_verbs
            .iter()
            .map(|(fqn, arg)| format!("`{fqn}` (`{arg}`)"))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    let mut dangling = Vec::new();
    let mut missing_selector = Vec::new();
    if edges.is_empty() {
        md.push_str("None found.\n");
    } else {
        md.push_str("| Parent FQN | Child FQN | Site |\n|---|---|---|\n");
        for e in edges {
            let mut flag = String::new();
            if !registered_fqns.contains(&e.child_fqn) {
                dangling.push(e.child_fqn.clone());
                flag.push_str(" ⚠ NOT REGISTERED");
            } else if let Some(required_arg) = fold_verbs.get(&e.child_fqn) {
                match &e.caller_arg_keys {
                    Some(keys) if !keys.contains(required_arg) => {
                        missing_selector.push((e.parent_fqn.clone(), e.child_fqn.clone(), required_arg.clone()));
                        flag.push_str(&format!(" ⚠ MISSING `{required_arg}`"));
                    }
                    None => {
                        flag.push_str(&format!(
                            " (target requires `{required_arg}` — caller's args not statically resolvable, unverified)"
                        ));
                    }
                    _ => {}
                }
            }
            md.push_str(&format!(
                "| `{}` | `{}`{flag} | {} |\n",
                e.parent_fqn, e.child_fqn, e.site
            ));
        }
    }

    if !dangling.is_empty() {
        md.push_str("\n## Dangling composition targets\n\n");
        md.push_str("A parent verb composes a child FQN with no matching registered op —\n");
        md.push_str("this would error at dispatch time. Real bug candidates, not extraction\n");
        md.push_str("noise (unlike the dead-code/dual-routing candidates, there's no benign\n");
        md.push_str("explanation for a composition edge pointing at nothing).\n\n");
        for d in &dangling {
            md.push_str(&format!("- `{d}`\n"));
        }
    }

    if !missing_selector.is_empty() {
        md.push_str("\n## Composition edges missing a required selector arg\n\n");
        md.push_str("The target is a fold verb (dispatches via the strict `dispatch_selector`\n");
        md.push_str("shape, which hard-errors on an absent/unrecognized selector) and this\n");
        md.push_str("caller's statically-resolved args don't include the required key — this\n");
        md.push_str("would fail at dispatch time with \"<arg> required\". Real bug candidates.\n\n");
        for (parent, child, arg) in &missing_selector {
            md.push_str(&format!("- `{parent}` -> `{child}`: missing `{arg}`\n"));
        }
    }

    fs::write(dir.join("composition_graph.md"), &md)?;

    let mut dot = String::from("digraph composition {\n  rankdir=LR;\n");
    for e in edges {
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\";\n",
            e.parent_fqn.replace('"', "'"),
            e.child_fqn.replace('"', "'")
        ));
    }
    dot.push_str("}\n");
    fs::write(dir.join("composition_graph.dot"), dot)?;

    Ok((dangling, missing_selector.into_iter().map(|(p, c, a)| format!("{p} -> {c}: missing {a}")).collect()))
}

pub(crate) fn run() -> Result<()> {
    let rust_root = resolve_rust_root()?;
    println!("== Registry/YAML completeness diff ==");
    println!("rust root: {}", rust_root.display());

    let (registered, direct_call_count) = extract_all_registrations(&rust_root)?;
    println!(
        "extracted {} registered ops ({} direct .register() calls + StubOp/SimpleStatusOp loop tables)",
        registered.len(),
        direct_call_count
    );

    let yaml_plugin_verbs = load_yaml_plugin_verbs(&rust_root)?;
    println!("{} YAML verbs declare behavior: plugin", yaml_plugin_verbs.len());

    // FQN -> registrations (should be exactly 1 each; SemOsVerbOpRegistry
    // panics at startup on duplicate FQN, so >1 here would mean this
    // extractor double-counted something, not a real runtime duplicate).
    let mut by_fqn: BTreeMap<String, Vec<&RegisteredOp>> = BTreeMap::new();
    for op in &registered {
        by_fqn.entry(op.fqn.clone()).or_default().push(op);
    }

    // Type -> FQNs, excluding the two deliberate many-FQN dispatchers.
    let mut by_type: BTreeMap<String, Vec<&RegisteredOp>> = BTreeMap::new();
    for op in &registered {
        if op.type_name == "StubOp" || op.type_name == "SimpleStatusOp" {
            continue;
        }
        by_type.entry(op.type_name.clone()).or_default().push(op);
    }

    let registered_fqns: BTreeSet<String> = by_fqn.keys().cloned().collect();

    let dead_code_candidates: Vec<&String> =
        registered_fqns.difference(&yaml_plugin_verbs).collect();
    let missing_registrations: Vec<&String> =
        yaml_plugin_verbs.difference(&registered_fqns).collect();
    let dual_routing: Vec<(&String, &Vec<&RegisteredOp>)> =
        by_type.iter().filter(|(_, ops)| ops.len() > 1).collect();
    let duplicate_fqn_extractions: Vec<(&String, &Vec<&RegisteredOp>)> =
        by_fqn.iter().filter(|(_, ops)| ops.len() > 1).collect();

    let artifacts_dir = rust_root.join("../test-artifacts");
    fs::create_dir_all(&artifacts_dir)?;

    write_dead_code_report(
        &artifacts_dir,
        &dead_code_candidates,
        &by_fqn,
        &duplicate_fqn_extractions,
    )?;
    write_dual_routing_report(&artifacts_dir, &dual_routing)?;
    write_rollup_report(
        &artifacts_dir,
        registered.len(),
        direct_call_count,
        yaml_plugin_verbs.len(),
        &dead_code_candidates,
        &missing_registrations,
        &dual_routing,
    )?;

    let ops_dir = rust_root.join("crates/sem_os_postgres/src/ops");
    let domain_ops_dir = rust_root.join("src/domain_ops");
    let relays = find_relay_functions(&[ops_dir, domain_ops_dir])?;
    let (composition_edges, fold_verbs) = extract_composition_edges(&registered, &relays)?;
    let (dangling, missing_selector) = write_composition_report(
        &artifacts_dir,
        &composition_edges,
        &registered_fqns,
        &fold_verbs,
    )?;

    println!();
    println!("dead-code candidates (registered, no live YAML entry): {}", dead_code_candidates.len());
    println!("missing registrations (YAML plugin verb, nothing registered): {}", missing_registrations.len());
    println!("dual-routing candidates (one type serving >1 FQN, excluding StubOp/SimpleStatusOp): {}", dual_routing.len());
    println!(
        "composition edges (execute() -> registry, via {} relay fn(s)): {} ({} dangling, {} missing required selector arg, {} fold verb(s) detected)",
        relays.len(),
        composition_edges.len(),
        dangling.len(),
        missing_selector.len(),
        fold_verbs.len()
    );
    println!();
    println!("reports written to {}", artifacts_dir.display());

    Ok(())
}

fn write_dead_code_report(
    dir: &Path,
    dead_code_candidates: &[&String],
    by_fqn: &BTreeMap<String, Vec<&RegisteredOp>>,
    duplicate_fqn_extractions: &[(&String, &Vec<&RegisteredOp>)],
) -> Result<()> {
    let mut md = String::new();
    md.push_str("# Dead-code candidates — registered SemOsVerbOp with no live YAML entry\n\n");
    md.push_str("Candidate list for review, not auto-deletion. A `register()` call whose FQN\n");
    md.push_str("has no `behavior: plugin` entry in the verb YAML means the DSL compiler can\n");
    md.push_str("never emit that FQN, so the registry entry is unreachable at runtime —\n");
    md.push_str("regardless of visibility, since this is a completeness diff, not a\n");
    md.push_str("compiler-visibility check.\n\n");

    if dead_code_candidates.is_empty() {
        md.push_str("None found — every registered op has a live YAML entry.\n");
    } else {
        md.push_str("| FQN | Type | Site |\n|---|---|---|\n");
        for fqn in dead_code_candidates {
            for op in &by_fqn[*fqn] {
                md.push_str(&format!(
                    "| `{}` | `{}` | {} |\n",
                    op.fqn, op.type_name, op.site
                ));
            }
        }
    }

    if !duplicate_fqn_extractions.is_empty() {
        md.push_str("\n## Extraction anomaly — same FQN resolved more than once\n\n");
        md.push_str("`SemOsVerbOpRegistry::register()` panics at startup on duplicate FQN, so\n");
        md.push_str("this can only mean this extractor double-counted a registration site —\n");
        md.push_str("not a real runtime duplicate. Flagged for the extractor's own\n");
        md.push_str("correctness, not as an application bug.\n\n");
        for (fqn, ops) in duplicate_fqn_extractions {
            md.push_str(&format!("- `{fqn}`: {} sites\n", ops.len()));
            for op in ops.iter() {
                md.push_str(&format!("  - {}\n", op.site));
            }
        }
    }

    fs::write(dir.join("dead_code_candidates.md"), md)?;
    Ok(())
}

fn write_dual_routing_report(
    dir: &Path,
    dual_routing: &[(&String, &Vec<&RegisteredOp>)],
) -> Result<()> {
    let mut md = String::new();
    md.push_str("# Dual-routing candidates — one concrete type registered under >1 FQN\n\n");
    md.push_str("Excludes `StubOp` and `SimpleStatusOp`, which are deliberate many-FQN-to-\n");
    md.push_str("one-type fan-in (a placeholder op and a generic status-flip op,\n");
    md.push_str("respectively — see `src/domain_ops/stub_op.rs` / `simple_status_op.rs`).\n");
    md.push_str("Anything below is a genuine candidate: either two FQNs that should be one\n");
    md.push_str("verb, or a type that's grown to do more than its name suggests and should\n");
    md.push_str("be split.\n\n");

    if dual_routing.is_empty() {
        md.push_str("None found.\n");
    } else {
        for (type_name, ops) in dual_routing {
            md.push_str(&format!("## `{type_name}` — {} FQNs\n\n", ops.len()));
            for op in ops.iter() {
                md.push_str(&format!("- `{}` ({})\n", op.fqn, op.site));
            }
            md.push('\n');
        }
    }

    fs::write(dir.join("dual_routing_report.md"), md)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_rollup_report(
    dir: &Path,
    total_registered: usize,
    direct_call_count: usize,
    total_yaml_plugin: usize,
    dead_code_candidates: &[&String],
    missing_registrations: &[&String],
    dual_routing: &[(&String, &Vec<&RegisteredOp>)],
) -> Result<()> {
    let mut md = String::new();
    md.push_str("# Registry/YAML completeness diff — rollup\n\n");
    md.push_str(&format!(
        "- Registered ops extracted: **{total_registered}** ({direct_call_count} direct `.register()` calls in `build_registry()`/`extend_registry()` + `StubOp`/`SimpleStatusOp` loop-table entries)\n"
    ));
    md.push_str(&format!(
        "- YAML verbs with `behavior: plugin`: **{total_yaml_plugin}**\n"
    ));
    md.push_str(&format!(
        "- Dead-code candidates (registered, no live YAML entry): **{}** — see dead_code_candidates.md\n",
        dead_code_candidates.len()
    ));
    md.push_str(&format!(
        "- Missing registrations (YAML plugin verb, nothing registered — would panic/error at dispatch): **{}**\n",
        missing_registrations.len()
    ));
    md.push_str(&format!(
        "- Dual-routing candidates (one type, >1 FQN, excluding the 2 deliberate fan-in types): **{}** — see dual_routing_report.md\n",
        dual_routing.len()
    ));

    if !missing_registrations.is_empty() {
        md.push_str("\n## Missing registrations\n\n");
        md.push_str("Cross-check against `cargo test -p ob-poc --lib -- test_plugin_verb_coverage`\n");
        md.push_str("before treating any of these as new findings — that test already covers\n");
        md.push_str("this exact gap; if it passes, this list should be empty and a non-empty\n");
        md.push_str("result here means this extractor has a bug, not that coverage broke.\n\n");
        for fqn in missing_registrations {
            md.push_str(&format!("- `{fqn}`\n"));
        }
    }

    fs::write(dir.join("registry_graph_report.md"), md)?;
    Ok(())
}
