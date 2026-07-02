//! ob-poc catalogue manifest exporter.
//!
//! Reads the ob-poc verb YAML catalogue (`rust/config/verbs/*.yaml`),
//! intersects it with a publication allowlist
//! (`rust/config/manifest-allowlist.yaml`), and emits a v0.6 §7
//! manifest that federated DSL peers (bpmn-lite, dmn-lite) consume at
//! their compile time.
//!
//! The crate ships as a thin library + CLI binary so the conversion
//! logic is unit-testable. Only the YAML fields actually needed for
//! §7 are deserialised — `dsl-core::VerbConfig` is a huge surface and
//! out of scope for this build tool.
#![deny(unreachable_pub)]
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use dsl_manifest::{
    DecisionEntry, InputSpec, Manifest, OutputSpec, ResourceDependency, Signature, TypeEntry,
    VerbEntry,
};
use serde::Deserialize;

// ── Source YAML schema (subset of dsl-core::VerbConfig) ───────────────

#[derive(Debug, Deserialize)]
struct SourceVerbsConfig {
    #[serde(default)]
    domains: BTreeMap<String, SourceDomainConfig>,
}

#[derive(Debug, Deserialize)]
struct SourceDomainConfig {
    #[serde(default)]
    verbs: BTreeMap<String, SourceVerbConfig>,
}

#[derive(Debug, Deserialize)]
struct SourceVerbConfig {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    effect_class: Option<String>,
    #[serde(default)]
    args: Vec<SourceArg>,
    #[serde(default)]
    produces: Option<SourceProduces>,
}

#[derive(Debug, Deserialize)]
struct SourceArg {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
    #[serde(default = "default_true")]
    required: bool,
    #[serde(default)]
    lookup: Option<SourceLookup>,
}

#[derive(Debug, Deserialize)]
struct SourceLookup {
    #[serde(default)]
    entity_type: Option<String>,
    #[serde(default)]
    resolution_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SourceProduces {
    #[serde(rename = "type")]
    type_name: String,
}

fn default_true() -> bool {
    true
}

// ── Allowlist YAML schema ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Allowlist {
    #[serde(default)]
    pub public_verbs: Vec<String>,
    #[serde(default)]
    pub public_decisions: Vec<String>,
}

impl Allowlist {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("read allowlist at {}", path.as_ref().display()))?;
        let allow: Allowlist = serde_yaml::from_str(&text).context("parse allowlist")?;
        Ok(allow)
    }
}

// ── Exporter ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ExporterConfig {
    pub domain: String,
    pub catalogue_version: String,
    pub generated_at: String,
    pub manifest_version: String,
    pub min_consumer_manifest_version: Option<String>,
}

impl ExporterConfig {
    pub fn new(domain: impl Into<String>, catalogue_version: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            catalogue_version: catalogue_version.into(),
            generated_at: chrono_like_now(),
            manifest_version: "1.0".into(),
            min_consumer_manifest_version: Some("1.0".into()),
        }
    }
}

/// `chrono`'s a heavy dep for this tool; emit an ISO-8601 stamp by
/// hand so we stay light. `SystemTime` precision is seconds.
fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, min, sec) = naive_utc_from_unix(secs as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn naive_utc_from_unix(mut secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let sec = (secs.rem_euclid(60)) as u32;
    secs = secs.div_euclid(60);
    let min = (secs.rem_euclid(60)) as u32;
    secs = secs.div_euclid(60);
    let hour = (secs.rem_euclid(24)) as u32;
    let mut days = secs.div_euclid(24);

    // Days since 1970-01-01 → Y/M/D via simple Civil-from-days conversion
    // (Howard Hinnant's algorithm).
    days += 719_468;
    let era = if days >= 0 {
        days / 146_097
    } else {
        (days - 146_096) / 146_097
    };
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, hour, min, sec)
}

/// Read every `*.yaml` (skipping `_meta.yaml`) under `verbs_dir`.
/// Returns a flat verb id → SourceVerbConfig map keyed by `domain.verb`.
fn load_source_catalogue(verbs_dir: &Path) -> Result<BTreeMap<String, SourceVerbConfig>> {
    let mut out: BTreeMap<String, SourceVerbConfig> = BTreeMap::new();

    let read_dir = std::fs::read_dir(verbs_dir)
        .with_context(|| format!("read verb dir {}", verbs_dir.display()))?;
    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem.starts_with('_') {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let parsed: SourceVerbsConfig = match serde_yaml::from_str(&text) {
            Ok(p) => p,
            Err(err) => {
                // Tolerant: skip files that don't match the verb schema
                // (ob-poc has misc YAML files under config/verbs/).
                eprintln!(
                    "ob-poc-manifest-export: skipping {} (not a verb file): {err}",
                    path.display()
                );
                continue;
            }
        };
        for (domain_name, domain) in parsed.domains {
            for (verb_name, verb) in domain.verbs {
                let id = format!("{domain_name}.{verb_name}");
                if out.contains_key(&id) {
                    bail!("duplicate verb id '{id}' across YAML files");
                }
                out.insert(id, verb);
            }
        }
    }
    Ok(out)
}

/// Convert a raw type token from the verb YAML into a §7 type name.
///
/// Primitive scalar tokens (`string`, `uuid`, `i64`, `bool`, …) survive
/// verbatim — `v0.6 §7.4` treats them as a separate `primitive` kind.
/// Domain entity tokens are PascalCased (`cbu` → `CBU`, `kyc-case`
/// → `KycCase`); a small map of well-known acronyms keeps the §10
/// demo's `CBU` reading naturally.
fn manifest_type_name(raw: &str) -> String {
    if is_primitive_token(raw) {
        return raw.to_owned();
    }
    match raw {
        "cbu" => "CBU".into(),
        "kyc-case" => "KycCase".into(),
        "instrument-matrix" | "instrument_matrix" => "InstrumentMatrix".into(),
        other => kebab_to_pascal(other),
    }
}

fn is_primitive_token(s: &str) -> bool {
    matches!(
        s,
        "string" | "uuid" | "i32" | "i64" | "u32" | "u64" | "bool" | "f32" | "f64" | "json"
    )
}

fn kebab_to_pascal(s: &str) -> String {
    s.split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_ascii_uppercase().to_string() + c.as_str(),
            }
        })
        .collect()
}

fn manifest_type_kind(name: &str) -> &'static str {
    if is_primitive_token(name) {
        "primitive"
    } else {
        "entity"
    }
}

fn convert_verb(id: &str, src: &SourceVerbConfig) -> VerbEntry {
    let inputs: Vec<InputSpec> = src
        .args
        .iter()
        .map(|a| InputSpec {
            name: a.name.clone(),
            type_name: manifest_type_name(&a.type_name),
            required: a.required,
        })
        .collect();

    let output = src.produces.as_ref().map(|p| OutputSpec {
        produces: Some(manifest_type_name(&p.type_name)),
    });

    let resource_dependencies: Vec<ResourceDependency> = src
        .args
        .iter()
        .filter_map(|a| {
            let lookup = a.lookup.as_ref()?;
            let kind = match lookup.resolution_mode.as_deref() {
                Some("entity") => "EntityUuid",
                _ => "NaturalKey",
            };
            Some(ResourceDependency {
                kind: kind.to_owned(),
                from_input: a.name.clone(),
                entity_type: lookup.entity_type.as_deref().map(manifest_type_name),
            })
        })
        .collect();

    let effect_class = src
        .effect_class
        .clone()
        .unwrap_or_else(|| "read_modify_write".to_owned());
    let authority_required = default_authority_for(id, &effect_class);

    VerbEntry {
        id: id.to_owned(),
        signature: Signature { inputs, output },
        effect_class,
        coordination_policy: None,
        transaction_policy: None,
        resource_dependencies,
        fsm_applicability: None,
        authority_required,
        description: src.description.clone(),
    }
}

fn default_authority_for(verb_id: &str, effect_class: &str) -> String {
    let domain = verb_id.split('.').next().unwrap_or("verb");
    if effect_class == "read_snapshot" {
        format!("{domain}.read")
    } else {
        format!("{domain}.write")
    }
}

/// Run the conversion against an in-memory `source_verbs` map. Exposed
/// for unit tests so they don't need filesystem fixtures.
pub(crate) fn export_from_sources(
    config: &ExporterConfig,
    source_verbs: BTreeMap<String, SourceVerbConfig>,
    allowlist: &Allowlist,
) -> Result<Manifest> {
    if allowlist.public_verbs.is_empty() {
        bail!("allowlist declares no public_verbs — nothing to export");
    }

    let mut verbs: Vec<VerbEntry> = Vec::with_capacity(allowlist.public_verbs.len());
    let mut missing: Vec<String> = Vec::new();
    let mut type_names: BTreeSet<String> = BTreeSet::new();

    for id in &allowlist.public_verbs {
        match source_verbs.get(id) {
            Some(src) => {
                let v = convert_verb(id, src);
                for input in &v.signature.inputs {
                    type_names.insert(input.type_name.clone());
                }
                if let Some(out) = &v.signature.output {
                    if let Some(produced) = &out.produces {
                        type_names.insert(produced.clone());
                    }
                }
                for rd in &v.resource_dependencies {
                    if let Some(t) = &rd.entity_type {
                        type_names.insert(t.clone());
                    }
                }
                verbs.push(v);
            }
            None => missing.push(id.clone()),
        }
    }
    if !missing.is_empty() {
        bail!(
            "allowlist references verbs not present in the catalogue: {}",
            missing.join(", ")
        );
    }

    let types: Vec<TypeEntry> = type_names
        .into_iter()
        .map(|name| TypeEntry {
            kind: manifest_type_kind(&name).to_owned(),
            uuid_type: (manifest_type_kind(&name) == "entity").then_some("UUIDv7".into()),
            name,
            description: None,
            values: Vec::new(),
        })
        .collect();

    if !allowlist.public_decisions.is_empty() {
        bail!(
            "allowlist declares public_decisions ({}); ob-poc does not own DMN decisions — \
             move them to the dmn-lite allowlist",
            allowlist.public_decisions.join(", ")
        );
    }
    let decisions: Vec<DecisionEntry> = Vec::new();

    let yaml = build_yaml(config, &verbs, &decisions, &types);
    // Validate by round-tripping through the manifest loader so we
    // catch any schema drift before the binary writes the output.
    Manifest::load_from_yaml(&yaml).map_err(|e| anyhow!("emitted manifest failed validation: {e}"))
}

fn build_yaml(
    config: &ExporterConfig,
    verbs: &[VerbEntry],
    decisions: &[DecisionEntry],
    types: &[TypeEntry],
) -> String {
    // Compose a YAML document by hand from the typed sub-records so we
    // don't depend on dsl-manifest's `to_yaml` (which uses serde_yaml's
    // default ordering). We want a stable layout: top-level identity
    // fields first, then verbs, then decisions, then types.
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "manifest_version: {:?}", config.manifest_version);
    let _ = writeln!(s, "domain: {:?}", config.domain);
    let _ = writeln!(s, "catalogue_version: {:?}", config.catalogue_version);
    let _ = writeln!(s, "generated_at: {:?}", config.generated_at);
    if let Some(min) = &config.min_consumer_manifest_version {
        let _ = writeln!(s, "min_consumer_manifest_version: {min:?}");
    }
    let _ = writeln!(s, "breaking_changes_since: []");

    s.push('\n');
    // A3 §2.4 — ob-poc declares all three federated services. Entity +
    // SemOs are stubbed in v0.6 per A3 §6 discipline #4: declared
    // (returns NOT_IMPLEMENTED) ≠ absent (returns gRPC UNIMPLEMENTED).
    s.push_str("services:\n");
    s.push_str("  - kind: InvocationService\n");
    s.push_str("    available: true\n");
    s.push_str("    capabilities: [\"Submit\", \"Validate\"]\n");
    s.push_str("  - kind: EntityService\n");
    s.push_str("    available: true\n");
    s.push_str("    capabilities: [\"Resolve\"]\n");
    s.push_str("  - kind: SemOsService\n");
    s.push_str("    available: true\n");
    s.push_str("    capabilities: [\"FetchDagPacks\"]\n");

    s.push('\n');
    s.push_str("verbs:\n");
    for v in verbs {
        emit_verb(&mut s, v);
    }
    s.push('\n');
    let _ = writeln!(
        s,
        "decisions: {}",
        if decisions.is_empty() { "[]" } else { "" }
    );
    if !decisions.is_empty() {
        for d in decisions {
            emit_decision(&mut s, d);
        }
    }
    s.push('\n');
    s.push_str("types:\n");
    for t in types {
        emit_type(&mut s, t);
    }
    s
}

fn emit_verb(s: &mut String, v: &VerbEntry) {
    use std::fmt::Write as _;
    let _ = writeln!(s, "  - id: {:?}", v.id);
    s.push_str("    signature:\n");
    if v.signature.inputs.is_empty() {
        s.push_str("      inputs: []\n");
    } else {
        s.push_str("      inputs:\n");
        for i in &v.signature.inputs {
            let _ = writeln!(s, "        - name: {:?}", i.name);
            let _ = writeln!(s, "          type: {:?}", i.type_name);
            let _ = writeln!(s, "          required: {}", i.required);
        }
    }
    if let Some(out) = &v.signature.output {
        s.push_str("      output:\n");
        match &out.produces {
            Some(p) => {
                let _ = writeln!(s, "        produces: {p:?}");
            }
            None => s.push_str("        produces: null\n"),
        }
    }
    let _ = writeln!(s, "    effect_class: {:?}", v.effect_class);
    let _ = writeln!(s, "    authority_required: {:?}", v.authority_required);
    if !v.resource_dependencies.is_empty() {
        s.push_str("    resource_dependencies:\n");
        for rd in &v.resource_dependencies {
            let _ = writeln!(s, "      - kind: {:?}", rd.kind);
            let _ = writeln!(s, "        from_input: {:?}", rd.from_input);
            if let Some(et) = &rd.entity_type {
                let _ = writeln!(s, "        entity_type: {et:?}");
            }
        }
    }
    if let Some(desc) = &v.description {
        let _ = writeln!(s, "    description: {desc:?}");
    }
}

fn emit_decision(s: &mut String, d: &DecisionEntry) {
    use std::fmt::Write as _;
    let _ = writeln!(s, "  - id: {:?}", d.id);
    if d.inputs.is_empty() {
        s.push_str("    inputs: []\n");
    } else {
        s.push_str("    inputs:\n");
        for i in &d.inputs {
            let _ = writeln!(s, "      - name: {:?}", i.name);
            let _ = writeln!(s, "        type: {:?}", i.type_name);
            let _ = writeln!(s, "        required: {}", i.required);
        }
    }
    s.push_str("    output:\n");
    let _ = writeln!(s, "      type: {:?}", d.output.type_name);
    if !d.output.enum_values.is_empty() {
        s.push_str("      enum_values:\n");
        for v in &d.output.enum_values {
            let _ = writeln!(s, "        - {v:?}");
        }
    }
    if let Some(desc) = &d.description {
        let _ = writeln!(s, "    description: {desc:?}");
    }
}

fn emit_type(s: &mut String, t: &TypeEntry) {
    use std::fmt::Write as _;
    let _ = writeln!(s, "  - name: {:?}", t.name);
    let _ = writeln!(s, "    kind: {:?}", t.kind);
    if let Some(uuid_type) = &t.uuid_type {
        let _ = writeln!(s, "    uuid_type: {uuid_type:?}");
    }
    if let Some(desc) = &t.description {
        let _ = writeln!(s, "    description: {desc:?}");
    }
    if !t.values.is_empty() {
        s.push_str("    values:\n");
        for v in &t.values {
            let _ = writeln!(s, "      - {v:?}");
        }
    }
}

/// Top-level entry — read every input, build a manifest, write YAML.
pub fn export_to_yaml(
    verbs_dir: &Path,
    allowlist: &Allowlist,
    config: &ExporterConfig,
) -> Result<String> {
    let source = load_source_catalogue(verbs_dir)?;
    let manifest = export_from_sources(config, source, allowlist)?;
    // Re-emit through our stable layout writer.
    let yaml = build_yaml(
        config,
        &manifest.verbs,
        &manifest.decisions,
        &manifest.types,
    );
    Ok(yaml)
}

/// Convenience: read everything from disk and write to disk.
pub fn export_to_path(
    verbs_dir: &Path,
    allowlist_path: &Path,
    output_path: &Path,
    domain: &str,
    catalogue_version: &str,
) -> Result<PathBuf> {
    let allow = Allowlist::from_path(allowlist_path)?;
    let cfg = ExporterConfig::new(domain, catalogue_version);
    let yaml = export_to_yaml(verbs_dir, &allow, &cfg)?;
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(output_path, &yaml)
        .with_context(|| format!("write {}", output_path.display()))?;
    Ok(output_path.to_path_buf())
}

#[cfg(test)]
mod tests;
