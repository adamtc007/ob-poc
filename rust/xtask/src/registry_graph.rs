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

    println!();
    println!("dead-code candidates (registered, no live YAML entry): {}", dead_code_candidates.len());
    println!("missing registrations (YAML plugin verb, nothing registered): {}", missing_registrations.len());
    println!("dual-routing candidates (one type serving >1 FQN, excluding StubOp/SimpleStatusOp): {}", dual_routing.len());
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
