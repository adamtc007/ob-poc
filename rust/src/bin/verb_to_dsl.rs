//! verb_to_dsl: Generate unified DSL (.dsl) files from verb YAML definitions.
//!
//! Reads every *.yaml file in the verbs directory (recursively, skipping _meta.yaml),
//! then emits one .dsl file per source YAML containing (verb ...) atoms for every
//! verb definition. Complex nested types (ArgConfig, lifecycle, crud, etc.) are
//! serialized as JSON strings in dedicated slots to ensure lossless round-trip.
//!
//! Usage:
//!   cargo run --bin verb_to_dsl -- \
//!     --verbs-dir config/verbs/ \
//!     --output-dir dsl-source/verbs/
//!
//! Design notes:
//!   - Each YAML file → one .dsl file (same relative sub-path, .yaml → .dsl)
//!   - Verbs in the same domain spread across multiple YAML files are preserved
//!     in their source file (one .dsl per YAML, not per domain).
//!   - Complex nested types are JSON-encoded in :args-json / :lifecycle-json etc.
//!   - Pattern D verbs (transition_args present) are annotated with a comment.

use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dsl_core::{
    ConfirmPolicyConfig, VerbBehavior, VerbConfig, VerbFlavour, VerbsConfig,
};

// ── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Args {
    verbs_dir: PathBuf,
    output_dir: PathBuf,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().collect();
    let mut verbs_dir = PathBuf::from("config/verbs");
    let mut output_dir = PathBuf::from("dsl-source/verbs");

    let mut i = 1;
    while i < raw.len() {
        match raw[i].as_str() {
            "--verbs-dir" if i + 1 < raw.len() => {
                verbs_dir = PathBuf::from(&raw[i + 1]);
                i += 2;
            }
            "--output-dir" if i + 1 < raw.len() => {
                output_dir = PathBuf::from(&raw[i + 1]);
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    Args {
        verbs_dir,
        output_dir,
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let args = parse_args();

    eprintln!(
        "verb_to_dsl: verbs-dir={} output-dir={}",
        args.verbs_dir.display(),
        args.output_dir.display()
    );

    // Ensure output dir exists
    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("creating output dir {}", args.output_dir.display()))?;

    // Collect all YAML files (mirrors ConfigLoader logic)
    let yaml_files = find_yaml_files(&args.verbs_dir)?;

    let mut total_files = 0usize;
    let mut total_verbs = 0usize;
    let mut pattern_d_count = 0usize;

    for yaml_path in &yaml_files {
        // Skip _meta.yaml and other underscore files
        if yaml_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('_'))
            .unwrap_or(false)
        {
            continue;
        }

        let content = std::fs::read_to_string(yaml_path)
            .with_context(|| format!("reading {}", yaml_path.display()))?;

        // Parse the YAML into VerbsConfig
        let config: VerbsConfig = match serde_yaml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  WARN: skip {} — parse error: {}", yaml_path.display(), e);
                continue;
            }
        };

        // Compute relative path for the output file
        let rel = yaml_path
            .strip_prefix(&args.verbs_dir)
            .unwrap_or(yaml_path.as_path());
        let out_rel = rel.with_extension("dsl");
        let out_path = args.output_dir.join(&out_rel);

        // Ensure parent directories exist
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }

        // Generate DSL content
        let (dsl_content, n_verbs, n_pd) = generate_dsl_for_file(yaml_path, &config);
        total_verbs += n_verbs;
        pattern_d_count += n_pd;

        std::fs::write(&out_path, &dsl_content)
            .with_context(|| format!("writing {}", out_path.display()))?;

        eprintln!(
            "  {} → {} ({} verbs{})",
            yaml_path.display(),
            out_path.display(),
            n_verbs,
            if n_pd > 0 {
                format!(", {} Pattern D", n_pd)
            } else {
                String::new()
            }
        );
        total_files += 1;
    }

    eprintln!(
        "\nDone: {} DSL files generated, {} verbs total, {} Pattern D verbs",
        total_files, total_verbs, pattern_d_count
    );

    Ok(())
}

// ── File discovery ────────────────────────────────────────────────────────────

fn find_yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_yaml_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_yaml_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_files(&path, files)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(())
}

// ── DSL generation ────────────────────────────────────────────────────────────

/// Generate the DSL content for a single YAML file.
/// Returns (content, verb_count, pattern_d_count).
fn generate_dsl_for_file(source_path: &Path, config: &VerbsConfig) -> (String, usize, usize) {
    let mut out = String::new();
    let mut total_verbs = 0usize;
    let mut pattern_d = 0usize;

    // File header comment
    let _ = writeln!(
        out,
        "; Generated by verb_to_dsl from {}\n; DO NOT EDIT — regenerate with `cargo run --bin verb_to_dsl`\n",
        source_path.display()
    );

    // Sort domains alphabetically for deterministic output
    let mut domain_names: Vec<&String> = config.domains.keys().collect();
    domain_names.sort();

    for domain_name in domain_names {
        let domain = &config.domains[domain_name];

        // Domain-level utterance-binding (invocation_hints)
        if !domain.invocation_hints.is_empty() {
            emit_utterance_binding_domain(&mut out, domain_name, &domain.invocation_hints);
        }

        // Sort verbs alphabetically
        let mut verb_names: Vec<&String> = domain.verbs.keys().collect();
        verb_names.sort();

        for verb_name in verb_names {
            let verb = &domain.verbs[verb_name];
            let fqn = format!("{}.{}", domain_name, verb_name);

            // Pattern D annotation
            let is_pattern_d = verb.transition_args.is_some();
            if is_pattern_d {
                pattern_d += 1;
                let _ = writeln!(
                    out,
                    "; Pattern D: transition_args present — see docs/verb-redesigns/"
                );
            }

            emit_verb_atom(&mut out, &fqn, verb);

            // Per-verb utterance-binding (invocation_phrases)
            if !verb.invocation_phrases.is_empty() {
                emit_utterance_binding_verb(&mut out, &fqn, &verb.invocation_phrases);
            }

            total_verbs += 1;
        }
    }

    (out, total_verbs, pattern_d)
}

// ── Atom emitters ─────────────────────────────────────────────────────────────

fn emit_utterance_binding_domain(out: &mut String, domain: &str, hints: &[String]) {
    let _ = write!(out, "(utterance-binding {} :invocation-hints [", domain);
    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            let _ = write!(out, " ");
        }
        let _ = write!(out, "{}", dsl_string(hint));
    }
    let _ = writeln!(out, "])\n");
}

fn emit_utterance_binding_verb(out: &mut String, fqn: &str, phrases: &[String]) {
    let _ = write!(out, "(utterance-binding {} :phrases [", fqn);
    for (i, p) in phrases.iter().enumerate() {
        if i > 0 {
            let _ = write!(out, " ");
        }
        let _ = write!(out, "{}", dsl_string(p));
    }
    let _ = writeln!(out, "] :verb {})\n", fqn);
}

fn emit_verb_atom(out: &mut String, fqn: &str, verb: &VerbConfig) {
    let _ = writeln!(out, "(verb {}", fqn);

    // Core slots
    let _ = writeln!(out, "  :description {}", dsl_string(&verb.description));
    let _ = writeln!(out, "  :behavior {}", behavior_str(verb.behavior));

    // Optional simple slots
    if let Some(handler) = &verb.handler {
        let _ = writeln!(out, "  :handler {}", dsl_string(handler));
    }
    if let Some(ec) = &verb.effect_class {
        let _ = writeln!(
            out,
            "  :effect-class {}",
            dsl_string(&format!("{:?}", ec).to_snake_case_str())
        );
    }
    if let Some(flavour) = &verb.flavour {
        let _ = writeln!(out, "  :flavour {}", dsl_string(flavour_str(*flavour)));
    }
    if let Some(rg) = &verb.role_guard {
        let rg_json = serde_json::to_string(rg).unwrap_or_default();
        let _ = writeln!(out, "  :role-guard {}", dsl_string(&rg_json));
    }
    if let Some(ac) = &verb.audit_class {
        let _ = writeln!(out, "  :audit-class {}", dsl_string(ac));
    }
    if let Some(cp) = &verb.confirm_policy {
        let _ = writeln!(
            out,
            "  :confirm-policy {}",
            dsl_string(confirm_policy_str(*cp))
        );
    }

    // Metadata (JSON)
    if let Some(meta) = &verb.metadata {
        let meta_json = serde_json::to_string(meta).unwrap_or_default();
        let _ = writeln!(out, "  :metadata-json {}", dsl_string(&meta_json));
    }

    // Three-axis (JSON — complex nested type with escalation rules)
    if let Some(three) = &verb.three_axis {
        let three_json = serde_json::to_string(three).unwrap_or_default();
        let _ = writeln!(out, "  :three-axis-json {}", dsl_string(&three_json));
    }

    // TransitionArgs (JSON)
    if let Some(ta) = &verb.transition_args {
        let ta_json = serde_json::to_string(ta).unwrap_or_default();
        let _ = writeln!(out, "  :transition-args-json {}", dsl_string(&ta_json));
    }

    // Produces (JSON)
    if let Some(produces) = &verb.produces {
        let p_json = serde_json::to_string(produces).unwrap_or_default();
        let _ = writeln!(out, "  :produces-json {}", dsl_string(&p_json));
    }

    // Consumes (JSON array)
    if !verb.consumes.is_empty() {
        let c_json = serde_json::to_string(&verb.consumes).unwrap_or_default();
        let _ = writeln!(out, "  :consumes-json {}", dsl_string(&c_json));
    }

    // Lifecycle (JSON)
    if let Some(lc) = &verb.lifecycle {
        let lc_json = serde_json::to_string(lc).unwrap_or_default();
        let _ = writeln!(out, "  :lifecycle-json {}", dsl_string(&lc_json));
    }

    // Policy (JSON)
    if let Some(policy) = &verb.policy {
        let p_json = serde_json::to_string(policy).unwrap_or_default();
        let _ = writeln!(out, "  :policy-json {}", dsl_string(&p_json));
    }

    // Sentences (JSON)
    if let Some(sentences) = &verb.sentences {
        let s_json = serde_json::to_string(sentences).unwrap_or_default();
        let _ = writeln!(out, "  :sentences-json {}", dsl_string(&s_json));
    }

    // Returns (JSON)
    if let Some(returns) = &verb.returns {
        let r_json = serde_json::to_string(returns).unwrap_or_default();
        let _ = writeln!(out, "  :returns-json {}", dsl_string(&r_json));
    }

    // Outputs (JSON array)
    if !verb.outputs.is_empty() {
        let o_json = serde_json::to_string(&verb.outputs).unwrap_or_default();
        let _ = writeln!(out, "  :outputs-json {}", dsl_string(&o_json));
    }

    // Writes (JSON array)
    if !verb.writes.is_empty() {
        let w_json = serde_json::to_string(&verb.writes).unwrap_or_default();
        let _ = writeln!(out, "  :writes-json {}", dsl_string(&w_json));
    }

    // Args (JSON array — ArgConfig has complex nested LookupConfig)
    if !verb.args.is_empty() {
        let a_json = serde_json::to_string(&verb.args).unwrap_or_default();
        let _ = writeln!(out, "  :args-json {}", dsl_string(&a_json));
    }

    // CrudConfig (JSON)
    if let Some(crud) = &verb.crud {
        let c_json = serde_json::to_string(crud).unwrap_or_default();
        let _ = writeln!(out, "  :crud-json {}", dsl_string(&c_json));
    }

    // DurableConfig (JSON)
    if let Some(durable) = &verb.durable {
        let d_json = serde_json::to_string(durable).unwrap_or_default();
        let _ = writeln!(out, "  :durable-json {}", dsl_string(&d_json));
    }

    // GraphQueryConfig (JSON)
    if let Some(gq) = &verb.graph_query {
        let gq_json = serde_json::to_string(gq).unwrap_or_default();
        let _ = writeln!(out, "  :graph-query-json {}", dsl_string(&gq_json));
    }

    let _ = writeln!(out, ")\n");
}

// ── String helpers ────────────────────────────────────────────────────────────

/// Wrap a string as a DSL double-quoted literal with proper escaping.
fn dsl_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn behavior_str(b: VerbBehavior) -> &'static str {
    match b {
        VerbBehavior::Crud => "\"crud\"",
        VerbBehavior::Plugin => "\"plugin\"",
        VerbBehavior::GraphQuery => "\"graph_query\"",
        VerbBehavior::Durable => "\"durable\"",
    }
}

fn flavour_str(f: VerbFlavour) -> &'static str {
    match f {
        VerbFlavour::AttributeMutating => "attribute_mutating",
        VerbFlavour::InstanceAdding => "instance_adding",
        VerbFlavour::Tollgate => "tollgate",
        VerbFlavour::Discretionary => "discretionary",
    }
}

fn confirm_policy_str(cp: ConfirmPolicyConfig) -> &'static str {
    match cp {
        ConfirmPolicyConfig::Always => "always",
        ConfirmPolicyConfig::QuickConfirm => "quick_confirm",
        ConfirmPolicyConfig::PackConfigured => "pack_configured",
    }
}

/// Cheap snake_case conversion for EffectClass debug names.
/// EffectClass variants are PascalCase (e.g. ReadModifyWrite → read_modify_write).
trait ToSnakeCaseStr {
    fn to_snake_case_str(&self) -> String;
}

impl ToSnakeCaseStr for str {
    fn to_snake_case_str(&self) -> String {
        let mut out = String::new();
        for (i, ch) in self.chars().enumerate() {
            if ch.is_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        }
        out
    }
}
