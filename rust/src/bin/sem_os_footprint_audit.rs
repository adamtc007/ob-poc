use anyhow::{bail, Context, Result};
use chrono::Utc;
use sem_os_obpoc_adapter::metadata::DomainMetadata;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_yaml::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ALLOWED_WORKSPACES: &[&str] = &[
    "ProductMaintenance",
    "Deal",
    "CBU",
    "KYC",
    "InstrumentMatrix",
    "OnBoarding",
    "*",
];
const STAR_ALLOWLIST_PREFIXES: &[&str] = &["session.", "admin.", "system.", "agent."];

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let command = args
        .first()
        .cloned()
        .unwrap_or_else(|| "baseline".to_string());
    let repo = repo_root()?;

    match command.as_str() {
        "workspace-hydrate" => {
            if args.len() < 2 {
                bail!("workspace-hydrate requires <workspace> [output]");
            }
            let workspace = args.swap_remove(1);
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, Some(&workspace)));
            run_workspace_hydrate(&repo, &workspace, &output)
        }
        "state-gates" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_state_gates(&repo, &output)
        }
        "classify-missing" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_classify_missing(&repo, &output)
        }
        "derive-crud" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_derive_crud(&repo, &output)
        }
        "normalize-nonmutating" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_normalize_nonmutating(&repo, &output)
        }
        "derive-lifecycle" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_derive_lifecycle(&repo, &output)
        }
        "derive-delegated" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_derive_delegated(&repo, &output)
        }
        "plugin-batch" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_plugin_batch(&repo, &output)
        }
        "extract-plugin-core" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_extract_plugin_core(&repo, &output)
        }
        "cascade-test" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            run_cascade_test(&repo, &output)
        }
        _ => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_output_path(&repo, &command, None));
            match command.as_str() {
                "baseline" => run_baseline(&repo, &output),
                "validate" => run_validate(&repo, &output, false),
                "validate-strict" => run_validate(&repo, &output, true),
                "coverage" => run_coverage(&repo, &output),
                "cleanup" => run_cleanup(&repo, &output),
                "taxonomies" => run_taxonomies(&repo, &output),
                "workspace-affinity" => run_workspace_affinity(&repo, &output),
                other => bail!(
                    "Unknown subcommand '{}'. Expected one of: baseline, validate, validate-strict, coverage, cleanup, taxonomies, workspace-affinity, workspace-hydrate, state-gates, classify-missing, derive-crud, normalize-nonmutating, derive-lifecycle, derive-delegated, plugin-batch, extract-plugin-core, cascade-test",
                    other
                ),
            }
        }
    }
}

fn default_output_path(repo: &Path, command: &str, workspace: Option<&str>) -> PathBuf {
    repo.join("artifacts")
        .join("footprints")
        .join(default_output_name(command, workspace))
}

fn default_output_name(command: &str, workspace: Option<&str>) -> String {
    match command {
        "baseline" => "phase_s0_summary.json".to_string(),
        "validate" | "validate-strict" => "phase_s1_validation.json".to_string(),
        "coverage" => "phase_s1_coverage.json".to_string(),
        "cleanup" => "phase_s2_cleanup_report.json".to_string(),
        "taxonomies" => "phase_s3_taxonomy_report.json".to_string(),
        "workspace-affinity" => "phase_s4_workspace_affinity_report.json".to_string(),
        "workspace-hydrate" => {
            let label = workspace.unwrap_or("workspace").to_ascii_lowercase();
            format!("phase_s5_{}_report.json", label)
        }
        "classify-missing" => "s6_missing_verb_classification.json".to_string(),
        "derive-crud" => "s6_crud_backfill_report.json".to_string(),
        "normalize-nonmutating" => "s6_non_mutating_normalization_report.json".to_string(),
        "derive-lifecycle" => "s6_lifecycle_backfill_report.json".to_string(),
        "derive-delegated" => "s6_delegated_report.json".to_string(),
        "plugin-batch" => "s6_plugin_batch_1_report.json".to_string(),
        "extract-plugin-core" => "s6_plugin_core_backfill_report.json".to_string(),
        "state-gates" => "phase_s7_state_gate_report.json".to_string(),
        "cascade-test" => "phase_s8_cascade_results.json".to_string(),
        _ => "footprint_report.json".to_string(),
    }
}

fn repo_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("Failed to resolve repo root from CARGO_MANIFEST_DIR")
}

fn config_dir(repo: &Path) -> PathBuf {
    repo.join("rust").join("config")
}

fn metadata_path(repo: &Path) -> PathBuf {
    config_dir(repo)
        .join("sem_os_seeds")
        .join("domain_metadata.yaml")
}

fn lexicon_path(repo: &Path) -> PathBuf {
    config_dir(repo).join("lexicon").join("verb_concepts.yaml")
}

fn macros_dir(repo: &Path) -> PathBuf {
    config_dir(repo).join("verb_schemas").join("macros")
}

fn footprint_taxonomy_dir(repo: &Path) -> PathBuf {
    config_dir(repo)
        .join("sem_os_seeds")
        .join("footprint_taxonomy")
}

fn ensure_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("Output path must have a parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create output dir {}", parent.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    ensure_parent(path)?;
    let body = serde_json::to_string_pretty(value)?;
    fs::write(path, body).with_context(|| format!("Failed to write {}", path.display()))
}

fn repo_revision(repo: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[derive(Debug)]
struct AuditContext {
    repo_revision: String,
    generated_at: String,
    live_verbs: BTreeMap<String, LiveVerb>,
    metadata: DomainMetadata,
    lexicon_verbs: BTreeSet<String>,
    macro_refs: Vec<MacroVerbRef>,
    collisions: Vec<PhraseCollision>,
    taxonomies: FootprintTaxonomies,
}

#[derive(Debug, Serialize, Clone)]
struct LiveVerb {
    fqn: String,
    domain: String,
    action: String,
    behavior: String,
    side_effects: Option<String>,
    harm_class: Option<String>,
    has_crud: bool,
    has_lifecycle: bool,
    crud_operation: Option<String>,
    crud_tables: Vec<String>,
    lookup_tables: Vec<String>,
    lookup_entity_types: Vec<String>,
    required_args: Vec<String>,
    invocation_phrase_count: usize,
    invocation_phrases: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
struct PhraseCollision {
    phrase: String,
    verb_fqns: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MacroVerbRef {
    source_file: String,
    verb_fqn: String,
    live: bool,
}

#[derive(Debug, Serialize)]
struct DomainCoverage {
    domain: String,
    live_verbs: usize,
    footprint_entries: usize,
    fully_hydrated: usize,
    partial: usize,
    missing: usize,
}

#[derive(Debug, Serialize)]
struct WorkspaceCoverage {
    workspace: String,
    live_verbs: usize,
    footprint_entries: usize,
    fully_hydrated: usize,
    partial: usize,
    missing: usize,
}

#[derive(Debug, Serialize)]
struct ValidationIssue {
    kind: String,
    subject: String,
    detail: String,
}

#[derive(Debug, Serialize)]
struct MissingVerbClassification {
    fqn: String,
    domain: String,
    workspace_affinity: Vec<String>,
    behavior: String,
    side_effects: Option<String>,
    harm_class: Option<String>,
    has_crud: bool,
    has_lifecycle: bool,
    bucket: String,
    recommended_mode: String,
    rationale: String,
}

#[derive(Debug, Serialize)]
struct CrudDerivedFootprint {
    fqn: String,
    domain: String,
    operation: Option<String>,
    entity_scope: Vec<String>,
    reads: Vec<String>,
    writes: Vec<String>,
    preconditions: Vec<String>,
    postconditions: Vec<String>,
    evidence_source: Vec<String>,
    rationale: String,
}

#[derive(Debug, Serialize)]
struct NonMutatingNormalization {
    fqn: String,
    domain: String,
    workspace_affinity: Vec<String>,
    bucket: String,
    normalized_mode: String,
    reads: Vec<String>,
    writes: Vec<String>,
    preconditions: Vec<String>,
    postconditions: Vec<String>,
    evidence_source: Vec<String>,
    rationale: String,
}

#[derive(Debug, Serialize)]
struct LifecycleDerivedFootprint {
    fqn: String,
    domain: String,
    workspace_affinity: Vec<String>,
    node_state_gates: Vec<String>,
    preconditions: Vec<String>,
    postconditions: Vec<String>,
    evidence_source: Vec<String>,
    rationale: String,
}

#[derive(Debug, Serialize)]
struct DelegatedFootprint {
    fqn: String,
    domain: String,
    workspace_affinity: Vec<String>,
    downstream_hint: Vec<String>,
    preconditions: Vec<String>,
    postconditions: Vec<String>,
    evidence_source: Vec<String>,
    rationale: String,
}

#[derive(Debug, Serialize)]
struct PluginBatchEntry {
    fqn: String,
    domain: String,
    workspace_affinity: Vec<String>,
    behavior: String,
    side_effects: Option<String>,
    required_args: Vec<String>,
    lookup_tables: Vec<String>,
    lookup_entity_types: Vec<String>,
    priority_reason: String,
}

#[derive(Debug, Serialize)]
struct PluginExtractedFootprint {
    fqn: String,
    domain: String,
    workspace_affinity: Vec<String>,
    reads: Vec<String>,
    writes: Vec<String>,
    preconditions: Vec<String>,
    postconditions: Vec<String>,
    evidence_source: Vec<String>,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct DomainWorkspaceMapFile {
    allowed_workspaces: Vec<String>,
    #[serde(default)]
    prefixes: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    exact: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceFamiliesFile {
    workspaces: BTreeMap<String, FamiliesEntry>,
}

#[derive(Debug, Deserialize)]
struct FamiliesEntry {
    #[serde(default)]
    families: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceSubjectsFile {
    workspaces: BTreeMap<String, SubjectsEntry>,
}

#[derive(Debug, Deserialize)]
struct SubjectsEntry {
    #[serde(default)]
    subject_kinds: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct NodeStateRegistryFile {
    #[serde(default)]
    aliases: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    state_machines: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct InvocationCollisionPolicyFile {
    default_classification: String,
    #[serde(default)]
    scoped_allowed: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StateMachineFile {
    state_machine: String,
    #[serde(default)]
    states: Vec<String>,
    #[serde(default)]
    transitions: Vec<StateTransition>,
}

#[derive(Debug, Deserialize)]
struct StateTransition {
    from: String,
    to: String,
    #[serde(default)]
    verbs: Vec<String>,
}

#[derive(Debug)]
struct FootprintTaxonomies {
    domain_map: DomainWorkspaceMapFile,
    workspace_families: WorkspaceFamiliesFile,
    workspace_subjects: WorkspaceSubjectsFile,
    node_state_registry: NodeStateRegistryFile,
    collision_policy: InvocationCollisionPolicyFile,
    known_constellation_families: BTreeSet<String>,
    verb_state_gates: BTreeMap<String, Vec<String>>,
}

impl FootprintTaxonomies {
    fn load(repo_root: &Path) -> Result<Self> {
        let dir = footprint_taxonomy_dir(repo_root);
        let domain_map: DomainWorkspaceMapFile =
            load_yaml_file(&dir.join("domain_to_workspace_map.yaml"))?;
        let workspace_families: WorkspaceFamiliesFile =
            load_yaml_file(&dir.join("workspace_to_constellation_families.yaml"))?;
        let workspace_subjects: WorkspaceSubjectsFile =
            load_yaml_file(&dir.join("workspace_to_subject_kinds.yaml"))?;
        let node_state_registry: NodeStateRegistryFile =
            load_yaml_file(&dir.join("node_state_registry.yaml"))?;
        let collision_policy: InvocationCollisionPolicyFile =
            load_yaml_file(&dir.join("invocation_phrase_collision_policy.yaml"))?;
        let known_constellation_families = scan_known_constellation_families(repo_root)?;
        let verb_state_gates = scan_state_gate_map(repo_root)?;
        Ok(Self {
            domain_map,
            workspace_families,
            workspace_subjects,
            node_state_registry,
            collision_policy,
            known_constellation_families,
            verb_state_gates,
        })
    }

    fn resolve_workspaces(&self, domain: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(exact) = self.domain_map.exact.get(domain) {
            out.extend(exact.iter().cloned());
        }
        for (prefix, workspaces) in &self.domain_map.prefixes {
            if domain.starts_with(prefix) {
                out.extend(workspaces.iter().cloned());
            }
        }
        dedupe_strings(out)
    }

    fn constellation_families_for(&self, workspaces: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for workspace in workspaces {
            if let Some(entry) = self.workspace_families.workspaces.get(workspace) {
                out.extend(entry.families.iter().cloned());
            }
        }
        dedupe_strings(out)
    }

    fn subject_kinds_for(&self, workspaces: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for workspace in workspaces {
            if let Some(entry) = self.workspace_subjects.workspaces.get(workspace) {
                out.extend(entry.subject_kinds.iter().cloned());
            }
        }
        dedupe_strings(out)
    }

    fn collision_classification(&self, phrase: &str) -> &'static str {
        if self
            .collision_policy
            .scoped_allowed
            .iter()
            .any(|allowed| allowed == phrase)
        {
            "scoped_allowed"
        } else {
            "fatal"
        }
    }

    fn unknown_constellation_families(&self) -> Vec<String> {
        let mut unknown = BTreeSet::new();
        for entry in self.workspace_families.workspaces.values() {
            for family in &entry.families {
                if !self.known_constellation_families.contains(family) {
                    unknown.insert(family.clone());
                }
            }
        }
        unknown.into_iter().collect()
    }

    fn unknown_workspaces(&self) -> Vec<String> {
        let mut unknown = BTreeSet::new();
        for workspaces in self.domain_map.exact.values() {
            for workspace in workspaces {
                if !self.domain_map.allowed_workspaces.contains(workspace) {
                    unknown.insert(workspace.clone());
                }
            }
        }
        for workspaces in self.domain_map.prefixes.values() {
            for workspace in workspaces {
                if !self.domain_map.allowed_workspaces.contains(workspace) {
                    unknown.insert(workspace.clone());
                }
            }
        }
        unknown.into_iter().collect()
    }

    fn state_gates_for(&self, verb_fqn: &str) -> Vec<String> {
        self.verb_state_gates
            .get(verb_fqn)
            .cloned()
            .unwrap_or_default()
    }
}

impl AuditContext {
    fn load(repo_root: &Path) -> Result<Self> {
        let metadata = DomainMetadata::from_file(&metadata_path(repo_root))
            .context("Failed to load domain metadata")?;
        let live_verbs = collect_live_verbs(&config_dir(repo_root).join("verbs"))?;
        let lexicon_verbs = collect_lexicon_verbs(&lexicon_path(repo_root))?;
        let macro_refs = collect_macro_refs(&macros_dir(repo_root), &live_verbs)?;
        let collisions = collect_collisions(live_verbs.values());
        let taxonomies = FootprintTaxonomies::load(repo_root)?;

        Ok(Self {
            repo_revision: repo_revision(repo_root),
            generated_at: Utc::now().to_rfc3339(),
            live_verbs,
            metadata,
            lexicon_verbs,
            macro_refs,
            collisions,
            taxonomies,
        })
    }
}

fn load_yaml_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let body =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_yaml::from_str(&body).with_context(|| format!("Failed to parse {}", path.display()))
}

fn scan_known_constellation_families(repo_root: &Path) -> Result<BTreeSet<String>> {
    let dir = config_dir(repo_root)
        .join("sem_os_seeds")
        .join("constellation_families");
    let mut out = BTreeSet::new();
    for path in walk_yaml_files(&dir)? {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            out.insert(stem.to_string());
        }
    }
    Ok(out)
}

fn scan_state_gate_map(repo_root: &Path) -> Result<BTreeMap<String, Vec<String>>> {
    let dir = config_dir(repo_root)
        .join("sem_os_seeds")
        .join("state_machines");
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in walk_yaml_files(&dir)? {
        let state_machine: StateMachineFile = load_yaml_file(&path)?;
        for state in &state_machine.states {
            let state_known = state_machine
                .states
                .iter()
                .any(|candidate| candidate == state);
            if !state_known {
                bail!(
                    "State machine {} references undeclared state '{}'",
                    state_machine.state_machine,
                    state
                );
            }
        }
        for transition in &state_machine.transitions {
            if !state_machine.states.contains(&transition.from) {
                bail!(
                    "State machine {} has transition from unknown state '{}'",
                    state_machine.state_machine,
                    transition.from
                );
            }
            if !state_machine.states.contains(&transition.to) {
                bail!(
                    "State machine {} has transition to unknown state '{}'",
                    state_machine.state_machine,
                    transition.to
                );
            }
            for verb in &transition.verbs {
                map.entry(verb.clone())
                    .or_default()
                    .insert(transition.from.clone());
            }
        }
    }
    Ok(map
        .into_iter()
        .map(|(verb, states)| (verb, states.into_iter().collect()))
        .collect())
}

fn collect_live_verbs(verbs_dir: &Path) -> Result<BTreeMap<String, LiveVerb>> {
    let mut out = BTreeMap::new();
    for path in walk_yaml_files(verbs_dir)? {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if file_name.starts_with('_') {
            continue;
        }
        let body = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let yaml: Value = serde_yaml::from_str(&body)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        let Some(domains) = yaml.get("domains").and_then(Value::as_mapping) else {
            continue;
        };
        for (domain_key, domain_val) in domains {
            let Some(domain) = domain_key.as_str() else {
                continue;
            };
            let Some(verbs) = domain_val.get("verbs").and_then(Value::as_mapping) else {
                continue;
            };
            for (action_key, verb_val) in verbs {
                let Some(action) = action_key.as_str() else {
                    continue;
                };
                let fqn = format!("{}.{}", domain, action);
                let behavior = verb_val
                    .get("behavior")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let metadata = verb_val.get("metadata");
                let side_effects = metadata
                    .and_then(|m| m.get("side_effects"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                let harm_class = metadata
                    .and_then(|m| m.get("harm_class"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                let has_crud = verb_val.get("crud").is_some();
                let has_lifecycle = verb_val.get("lifecycle").is_some();
                let crud = verb_val.get("crud");
                let crud_operation = crud
                    .and_then(|c| c.get("operation"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                let crud_tables = collect_crud_tables(crud);
                let (lookup_tables, lookup_entity_types, required_args) =
                    collect_arg_lookups_and_required(verb_val.get("args"));
                let mut phrases = verb_val
                    .get("invocation_phrases")
                    .and_then(Value::as_sequence)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                phrases.sort();
                phrases.dedup();
                out.insert(
                    fqn.clone(),
                    LiveVerb {
                        fqn,
                        domain: domain.to_string(),
                        action: action.to_string(),
                        behavior,
                        side_effects,
                        harm_class,
                        has_crud,
                        has_lifecycle,
                        crud_operation,
                        crud_tables,
                        lookup_tables,
                        lookup_entity_types,
                        required_args,
                        invocation_phrase_count: phrases.len(),
                        invocation_phrases: phrases,
                    },
                );
            }
        }
    }
    Ok(out)
}

fn collect_lexicon_verbs(path: &Path) -> Result<BTreeSet<String>> {
    let body =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let yaml: Value = serde_yaml::from_str(&body)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    let mapping = yaml
        .as_mapping()
        .context("verb_concepts.yaml must be a top-level mapping")?;
    let mut verbs = BTreeSet::new();
    for key in mapping.keys() {
        if let Some(name) = key.as_str() {
            if name.contains('.') {
                verbs.insert(name.to_string());
            }
        }
    }
    Ok(verbs)
}

fn collect_macro_refs(
    dir: &Path,
    live_verbs: &BTreeMap<String, LiveVerb>,
) -> Result<Vec<MacroVerbRef>> {
    let mut refs = Vec::new();
    for path in walk_yaml_files(dir)? {
        let body = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let yaml: Value = serde_yaml::from_str(&body)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        collect_macro_refs_from_value(&yaml, &path, dir, live_verbs, &mut refs, false);
    }
    refs.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then(a.verb_fqn.cmp(&b.verb_fqn))
    });
    Ok(refs)
}

fn collect_macro_refs_from_value(
    value: &Value,
    path: &Path,
    base: &Path,
    live_verbs: &BTreeMap<String, LiveVerb>,
    refs: &mut Vec<MacroVerbRef>,
    in_expands_to: bool,
) {
    match value {
        Value::Mapping(map) => {
            let mut next_in_expands_to = in_expands_to;
            for (key, val) in map {
                if key.as_str() == Some("expands-to") {
                    next_in_expands_to = true;
                }
                if in_expands_to && key.as_str() == Some("verb") {
                    if let Some(verb_fqn) = val.as_str() {
                        refs.push(MacroVerbRef {
                            source_file: path
                                .strip_prefix(base)
                                .unwrap_or(path)
                                .display()
                                .to_string(),
                            verb_fqn: verb_fqn.to_string(),
                            live: live_verbs.contains_key(verb_fqn),
                        });
                    }
                }
                collect_macro_refs_from_value(
                    val,
                    path,
                    base,
                    live_verbs,
                    refs,
                    next_in_expands_to,
                );
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_macro_refs_from_value(item, path, base, live_verbs, refs, in_expands_to);
            }
        }
        _ => {}
    }
}

fn collect_collisions<'a>(live_verbs: impl Iterator<Item = &'a LiveVerb>) -> Vec<PhraseCollision> {
    let mut index: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for verb in live_verbs {
        for phrase in &verb.invocation_phrases {
            let normalized = normalize_phrase(phrase);
            if normalized.is_empty() {
                continue;
            }
            index
                .entry(normalized)
                .or_default()
                .insert(verb.fqn.clone());
        }
    }

    index
        .into_iter()
        .filter_map(|(phrase, verbs)| {
            if verbs.len() > 1 {
                Some(PhraseCollision {
                    phrase,
                    verb_fqns: verbs.into_iter().collect(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn normalize_phrase(phrase: &str) -> String {
    phrase.trim().to_ascii_lowercase()
}

fn collect_crud_tables(crud: Option<&Value>) -> Vec<String> {
    let Some(crud) = crud else {
        return Vec::new();
    };
    let mut tables = Vec::new();
    for key in [
        "table",
        "base_table",
        "extension_table",
        "junction",
        "primary_table",
        "join_table",
        "role_table",
    ] {
        if let Some(table) = crud.get(key).and_then(Value::as_str) {
            tables.push(table.to_string());
        }
    }
    dedupe_strings(tables)
}

fn collect_arg_lookups_and_required(
    args: Option<&Value>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let Some(args) = args.and_then(Value::as_sequence) else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    let mut lookup_tables = Vec::new();
    let mut lookup_entity_types = Vec::new();
    let mut required_args = Vec::new();
    for arg in args {
        let name = arg
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if arg
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !name.is_empty()
        {
            required_args.push(name);
        }
        if let Some(lookup) = arg.get("lookup") {
            if let Some(table) = lookup.get("table").and_then(Value::as_str) {
                lookup_tables.push(table.to_string());
            }
            if let Some(entity_type) = lookup.get("entity_type").and_then(Value::as_str) {
                lookup_entity_types.push(entity_type.to_string());
            }
        }
    }
    (
        dedupe_strings(lookup_tables),
        dedupe_strings(lookup_entity_types),
        dedupe_strings(required_args),
    )
}

fn walk_yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk_yaml_files(&path)?);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("yaml" | "yml")
        ) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn footprint_categories(ctx: &AuditContext) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
    let mut full = HashSet::new();
    let mut partial = HashSet::new();
    let mut all = HashSet::new();
    for domain in ctx.metadata.domains.values() {
        for (verb_fqn, footprint) in &domain.verb_data_footprint {
            all.insert(verb_fqn.clone());
            if footprint.reads.is_empty() && footprint.writes.is_empty() {
                partial.insert(verb_fqn.clone());
            } else {
                full.insert(verb_fqn.clone());
            }
        }
    }
    (all, full, partial)
}

fn dedupe_strings(mut values: Vec<String>) -> Vec<String> {
    values.retain(|value| !value.is_empty());
    values.sort();
    values.dedup();
    values
}

fn run_baseline(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let (footprint_entries, full, partial) = footprint_categories(&ctx);
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();

    let orphans: BTreeSet<String> = footprint_entries.difference(&live_set).cloned().collect();
    let live_with_footprints: BTreeSet<String> =
        footprint_entries.intersection(&live_set).cloned().collect();
    let missing: BTreeSet<String> = live_set.difference(&footprint_entries).cloned().collect();
    let lexical_missing: BTreeSet<String> = live_set
        .difference(&ctx.lexicon_verbs.iter().cloned().collect())
        .cloned()
        .collect();
    let stale_lexicon: BTreeSet<String> = ctx
        .lexicon_verbs
        .iter()
        .filter(|verb| !live_set.contains(*verb))
        .cloned()
        .collect();
    let broken_macro_refs: Vec<&MacroVerbRef> = ctx.macro_refs.iter().filter(|r| !r.live).collect();

    let live_inventory = json!({
        "slice_id": "S0",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "total_live_verbs": ctx.live_verbs.len(),
        "verbs": ctx.live_verbs.values().collect::<Vec<_>>(),
    });
    let baseline = json!({
        "slice_id": "S0",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "total_live_verbs": ctx.live_verbs.len(),
        "footprint_entries_total": footprint_entries.len(),
        "fully_hydrated_verbs": full.intersection(&live_set).count(),
        "partially_hydrated_verbs": partial.intersection(&live_set).count(),
        "live_verbs_with_footprint_entry": live_with_footprints.len(),
        "missing_verbs": missing.len(),
        "orphans": orphans.len(),
        "lexicon_entries": ctx.lexicon_verbs.len(),
        "lexicon_missing_live_verbs": lexical_missing.len(),
        "stale_lexicon_entries": stale_lexicon.len(),
        "invocation_phrase_collisions": ctx.collisions.len(),
        "broken_macro_refs": broken_macro_refs.len(),
    });
    let orphan_payload = json!({
        "slice_id": "S0",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "orphans": orphans.iter().collect::<Vec<_>>(),
    });
    let collision_payload = json!({
        "slice_id": "S0",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "collisions": ctx.collisions,
    });
    let macro_payload = json!({
        "slice_id": "S0",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "broken_macro_refs": broken_macro_refs,
        "all_macro_refs": ctx.macro_refs,
    });
    let summary = json!({
        "slice_id": "S0",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "total_live_verbs": ctx.live_verbs.len(),
        "fully_hydrated_verbs": full.intersection(&live_set).count(),
        "partially_hydrated_verbs": partial.intersection(&live_set).count(),
        "missing_verbs": missing.len(),
        "orphans": orphans.len(),
        "exceptions": {
            "broken_macro_refs": broken_macro_refs.len(),
            "invocation_phrase_collisions": ctx.collisions.len(),
            "stale_lexicon_entries": stale_lexicon.len(),
            "lexicon_missing_live_verbs": lexical_missing.len(),
        },
        "unresolved_discrepancies": {
            "missing_footprint_verbs": missing.iter().take(50).collect::<Vec<_>>(),
            "orphan_footprint_verbs": orphans.iter().take(50).collect::<Vec<_>>(),
            "stale_lexicon_entries": stale_lexicon.iter().take(50).collect::<Vec<_>>(),
            "lexicon_missing_live_verbs": lexical_missing.iter().take(50).collect::<Vec<_>>(),
        }
    });

    let artifacts_dir = output
        .parent()
        .context("Baseline output path has no parent")?;
    write_json(
        &artifacts_dir.join("live_verb_inventory.json"),
        &live_inventory,
    )?;
    write_json(&artifacts_dir.join("baseline_coverage.json"), &baseline)?;
    write_json(
        &artifacts_dir.join("orphan_footprints.json"),
        &orphan_payload,
    )?;
    write_json(
        &artifacts_dir.join("collision_report.json"),
        &collision_payload,
    )?;
    write_json(
        &artifacts_dir.join("macro_ref_breakage.json"),
        &macro_payload,
    )?;
    write_json(output, &summary)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn run_validate(repo: &Path, output: &Path, strict: bool) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let mut issues = Vec::new();

    for (domain_key, domain) in &ctx.metadata.domains {
        for verb_fqn in domain.verb_data_footprint.keys() {
            if !live_set.contains(verb_fqn) {
                issues.push(ValidationIssue {
                    kind: "orphan_footprint".to_string(),
                    subject: verb_fqn.clone(),
                    detail: format!(
                        "Footprint in domain '{}' has no matching live verb",
                        domain_key
                    ),
                });
            }
        }
        for (verb_fqn, footprint) in &domain.verb_data_footprint {
            for workspace in &footprint.workspace_affinity {
                if !ALLOWED_WORKSPACES.contains(&workspace.as_str()) {
                    issues.push(ValidationIssue {
                        kind: "unknown_workspace".to_string(),
                        subject: verb_fqn.clone(),
                        detail: format!("Unknown workspace_affinity '{}'", workspace),
                    });
                }
            }
            if footprint.workspace_affinity.iter().any(|w| w == "*")
                && !STAR_ALLOWLIST_PREFIXES
                    .iter()
                    .any(|prefix| verb_fqn.starts_with(prefix))
            {
                issues.push(ValidationIssue {
                    kind: "invalid_star_usage".to_string(),
                    subject: verb_fqn.clone(),
                    detail: "workspace_affinity '*' is reserved for approved global verbs"
                        .to_string(),
                });
            }
        }
    }

    for workspace in ctx.taxonomies.unknown_workspaces() {
        issues.push(ValidationIssue {
            kind: "taxonomy_unknown_workspace".to_string(),
            subject: workspace.clone(),
            detail: "Workspace appears in taxonomy but is not in the allowed registry".to_string(),
        });
    }

    for family in ctx.taxonomies.unknown_constellation_families() {
        issues.push(ValidationIssue {
            kind: "unknown_constellation_family".to_string(),
            subject: family.clone(),
            detail: "Constellation family is referenced by taxonomy but has no seed file"
                .to_string(),
        });
    }

    for collision in &ctx.collisions {
        if ctx.taxonomies.collision_classification(&collision.phrase) == "fatal" {
            issues.push(ValidationIssue {
                kind: "fatal_phrase_collision".to_string(),
                subject: collision.phrase.clone(),
                detail: format!(
                    "Invocation phrase is still ambiguous across {} verbs",
                    collision.verb_fqns.len()
                ),
            });
        }
    }

    let payload = json!({
        "slice_id": "S1",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "total_live_verbs": ctx.live_verbs.len(),
        "fully_hydrated_verbs": footprint_categories(&ctx).1.intersection(&live_set).count(),
        "partially_hydrated_verbs": footprint_categories(&ctx).2.intersection(&live_set).count(),
        "missing_verbs": live_set.len().saturating_sub(footprint_categories(&ctx).0.intersection(&live_set).count()),
        "orphans": issues.iter().filter(|i| i.kind == "orphan_footprint").count(),
        "exceptions": issues,
        "strict": strict,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);

    if strict {
        let has_blocking = payload["exceptions"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if has_blocking {
            bail!("Strict validation failed; see {}", output.display());
        }
    }
    Ok(())
}

fn run_coverage(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let (all_entries, full, partial) = footprint_categories(&ctx);
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let live_with_entries: HashSet<_> = all_entries.intersection(&live_set).cloned().collect();
    let mut per_domain = Vec::new();
    let mut domain_map: HashMap<&str, Vec<&str>> = HashMap::new();
    for verb in ctx.live_verbs.values() {
        domain_map
            .entry(verb.domain.as_str())
            .or_default()
            .push(verb.fqn.as_str());
    }
    let mut domains: Vec<_> = domain_map.into_iter().collect();
    domains.sort_by(|a, b| a.0.cmp(b.0));
    for (domain, verbs) in domains {
        let live = verbs.len();
        let with_entries = verbs
            .iter()
            .filter(|v| live_with_entries.contains(**v))
            .count();
        let full_count = verbs.iter().filter(|v| full.contains(**v)).count();
        let partial_count = verbs.iter().filter(|v| partial.contains(**v)).count();
        per_domain.push(DomainCoverage {
            domain: domain.to_string(),
            live_verbs: live,
            footprint_entries: with_entries,
            fully_hydrated: full_count,
            partial: partial_count,
            missing: live.saturating_sub(with_entries),
        });
    }

    let mut per_workspace = Vec::new();
    let mut workspace_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for verb in ctx.live_verbs.values() {
        for workspace in ctx.taxonomies.resolve_workspaces(&verb.domain) {
            workspace_map
                .entry(workspace)
                .or_default()
                .push(verb.fqn.clone());
        }
    }
    for (workspace, verbs) in workspace_map {
        let live = verbs.len();
        let with_entries = verbs
            .iter()
            .filter(|v| live_with_entries.contains(*v))
            .count();
        let full_count = verbs.iter().filter(|v| full.contains(*v)).count();
        let partial_count = verbs.iter().filter(|v| partial.contains(*v)).count();
        per_workspace.push(WorkspaceCoverage {
            workspace,
            live_verbs: live,
            footprint_entries: with_entries,
            fully_hydrated: full_count,
            partial: partial_count,
            missing: live.saturating_sub(with_entries),
        });
    }

    let payload = json!({
        "slice_id": "S1",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "total_live_verbs": ctx.live_verbs.len(),
        "fully_hydrated_verbs": full.intersection(&live_set).count(),
        "partially_hydrated_verbs": partial.intersection(&live_set).count(),
        "missing_verbs": live_set.len().saturating_sub(live_with_entries.len()),
        "orphans": all_entries.len().saturating_sub(live_with_entries.len()),
        "exceptions": [],
        "overall": {
            "footprint_entry_pct": pct(live_with_entries.len(), ctx.live_verbs.len()),
            "fully_hydrated_pct": pct(full.intersection(&live_set).count(), ctx.live_verbs.len()),
            "partial_pct": pct(partial.intersection(&live_set).count(), ctx.live_verbs.len()),
            "missing_pct": pct(live_set.len().saturating_sub(live_with_entries.len()), ctx.live_verbs.len()),
        },
        "per_domain": per_domain,
        "per_workspace": per_workspace,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_cleanup(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let (footprint_entries, _, _) = footprint_categories(&ctx);
    let orphan_count = footprint_entries.difference(&live_set).count();
    let broken_macro_refs = ctx.macro_refs.iter().filter(|item| !item.live).count();
    let fatal_collisions: Vec<_> = ctx
        .collisions
        .iter()
        .filter(|collision| ctx.taxonomies.collision_classification(&collision.phrase) == "fatal")
        .collect();
    let scoped_allowed_collisions: Vec<_> = ctx
        .collisions
        .iter()
        .filter(|collision| {
            ctx.taxonomies.collision_classification(&collision.phrase) == "scoped_allowed"
        })
        .collect();

    let payload = json!({
        "slice_id": "S2",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "orphan_footprint_entries": orphan_count,
        "broken_macro_refs": broken_macro_refs,
        "fatal_collision_count": fatal_collisions.len(),
        "scoped_allowed_collision_count": scoped_allowed_collisions.len(),
        "lexicon_missing_live_verbs": live_set
            .difference(&ctx.lexicon_verbs.iter().cloned().collect())
            .count(),
        "fatal_collisions": fatal_collisions,
        "scoped_allowed_collisions": scoped_allowed_collisions,
        "policy_default": ctx.taxonomies.collision_policy.default_classification,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_taxonomies(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let live_domains: BTreeSet<String> = ctx
        .live_verbs
        .values()
        .map(|verb| verb.domain.clone())
        .collect();
    let mapped_domains: BTreeSet<String> = live_domains
        .iter()
        .filter(|domain| !ctx.taxonomies.resolve_workspaces(domain).is_empty())
        .cloned()
        .collect();
    let unmapped_domains: Vec<_> = live_domains.difference(&mapped_domains).cloned().collect();
    let mut workspace_families = BTreeMap::new();
    let mut workspace_subjects = BTreeMap::new();
    for workspace in &ctx.taxonomies.domain_map.allowed_workspaces {
        let singleton = vec![workspace.clone()];
        workspace_families.insert(
            workspace.clone(),
            ctx.taxonomies.constellation_families_for(&singleton),
        );
        workspace_subjects.insert(
            workspace.clone(),
            ctx.taxonomies.subject_kinds_for(&singleton),
        );
    }

    let payload = json!({
        "slice_id": "S3",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "live_domain_count": live_domains.len(),
        "mapped_domain_count": mapped_domains.len(),
        "unmapped_domains": unmapped_domains,
        "allowed_workspaces": ctx.taxonomies.domain_map.allowed_workspaces,
        "workspace_count": ctx.taxonomies.workspace_families.workspaces.len(),
        "unknown_workspaces": ctx.taxonomies.unknown_workspaces(),
        "known_constellation_family_count": ctx.taxonomies.known_constellation_families.len(),
        "unknown_constellation_families": ctx.taxonomies.unknown_constellation_families(),
        "workspace_constellation_families": workspace_families,
        "workspace_subject_kinds": workspace_subjects,
        "node_state_aliases": ctx.taxonomies.node_state_registry.aliases.keys().collect::<Vec<_>>(),
        "state_machine_count": ctx.taxonomies.node_state_registry.state_machines.len(),
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_workspace_affinity(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let mut per_workspace: BTreeMap<String, usize> = BTreeMap::new();
    let mut workspace_families: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut workspace_subjects: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unmapped = Vec::new();
    let mut star_verbs = Vec::new();

    for verb in ctx.live_verbs.values() {
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        if workspaces.is_empty() {
            unmapped.push(verb.fqn.clone());
            continue;
        }
        for workspace in &workspaces {
            *per_workspace.entry(workspace.clone()).or_default() += 1;
            workspace_families
                .entry(workspace.clone())
                .or_insert_with(|| {
                    let singleton = vec![workspace.clone()];
                    ctx.taxonomies.constellation_families_for(&singleton)
                });
            workspace_subjects
                .entry(workspace.clone())
                .or_insert_with(|| {
                    let singleton = vec![workspace.clone()];
                    ctx.taxonomies.subject_kinds_for(&singleton)
                });
            if workspace == "*" {
                star_verbs.push(verb.fqn.clone());
            }
        }
    }

    let payload = json!({
        "slice_id": "S4",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "total_live_verbs": ctx.live_verbs.len(),
        "mapped_verbs": ctx.live_verbs.len().saturating_sub(unmapped.len()),
        "unmapped_verbs": unmapped.len(),
        "per_workspace": per_workspace,
        "workspace_constellation_families": workspace_families,
        "workspace_subject_kinds": workspace_subjects,
        "star_usage_count": star_verbs.len(),
        "star_usage_verbs": star_verbs,
        "unmapped_examples": unmapped.iter().take(100).collect::<Vec<_>>(),
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_workspace_hydrate(repo: &Path, workspace: &str, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let mut verbs = Vec::new();
    for verb in ctx.live_verbs.values() {
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        if workspaces.iter().any(|candidate| candidate == workspace) {
            verbs.push(json!({
                "fqn": verb.fqn,
                "domain": verb.domain,
                "constellation_families": ctx.taxonomies.constellation_families_for(&[workspace.to_string()]),
                "subject_kinds": ctx.taxonomies.subject_kinds_for(&[workspace.to_string()]),
            }));
        }
    }
    let payload = json!({
        "slice_id": "S5",
        "workspace": workspace,
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "verb_count": verbs.len(),
        "families": ctx.taxonomies.constellation_families_for(&[workspace.to_string()]),
        "subject_kinds": ctx.taxonomies.subject_kinds_for(&[workspace.to_string()]),
        "verbs": verbs,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_classify_missing(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let (footprint_entries, _, _) = footprint_categories(&ctx);
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let missing: BTreeSet<String> = live_set.difference(&footprint_entries).cloned().collect();

    let mut classifications = Vec::new();
    let mut bucket_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut mode_counts: BTreeMap<String, usize> = BTreeMap::new();

    for fqn in &missing {
        let Some(verb) = ctx.live_verbs.get(fqn) else {
            continue;
        };
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        let (bucket, recommended_mode, rationale) = classify_missing_verb(&ctx, verb, &workspaces);
        *bucket_counts.entry(bucket.clone()).or_default() += 1;
        *mode_counts.entry(recommended_mode.clone()).or_default() += 1;
        classifications.push(MissingVerbClassification {
            fqn: verb.fqn.clone(),
            domain: verb.domain.clone(),
            workspace_affinity: workspaces,
            behavior: verb.behavior.clone(),
            side_effects: verb.side_effects.clone(),
            harm_class: verb.harm_class.clone(),
            has_crud: verb.has_crud,
            has_lifecycle: verb.has_lifecycle
                || !ctx.taxonomies.state_gates_for(&verb.fqn).is_empty(),
            bucket,
            recommended_mode,
            rationale,
        });
    }

    let payload = json!({
        "slice_id": "S6.1",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "missing_verb_count": classifications.len(),
        "bucket_counts": bucket_counts,
        "recommended_mode_counts": mode_counts,
        "classifications": classifications,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_derive_crud(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let (footprint_entries, _, _) = footprint_categories(&ctx);
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let missing: BTreeSet<String> = live_set.difference(&footprint_entries).cloned().collect();

    let mut entries = Vec::new();
    for fqn in &missing {
        let Some(verb) = ctx.live_verbs.get(fqn) else {
            continue;
        };
        let (bucket, _, _) =
            classify_missing_verb(&ctx, verb, &ctx.taxonomies.resolve_workspaces(&verb.domain));
        if bucket != "A_pure_crud" {
            continue;
        }
        entries.push(derive_crud_footprint(verb));
    }

    let payload = json!({
        "slice_id": "S6.2",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "derived_entry_count": entries.len(),
        "entries": entries,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_normalize_nonmutating(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let (footprint_entries, _, _) = footprint_categories(&ctx);
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let missing: BTreeSet<String> = live_set.difference(&footprint_entries).cloned().collect();

    let mut entries = Vec::new();
    let mut mode_counts: BTreeMap<String, usize> = BTreeMap::new();

    for fqn in &missing {
        let Some(verb) = ctx.live_verbs.get(fqn) else {
            continue;
        };
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        let (bucket, _, _) = classify_missing_verb(&ctx, verb, &workspaces);
        if !matches!(bucket.as_str(), "E_no_harm_read_research" | "F_system_ui") {
            continue;
        }

        let normalized_mode = if bucket == "E_no_harm_read_research" {
            "read_only"
        } else {
            "system_only"
        }
        .to_string();
        *mode_counts.entry(normalized_mode.clone()).or_default() += 1;

        let mut reads = verb.lookup_tables.clone();
        if verb.behavior == "crud" {
            reads.extend(verb.crud_tables.clone());
        }

        let mut preconditions = Vec::new();
        if !verb.required_args.is_empty() {
            preconditions.push(format!(
                "required args present: {}",
                verb.required_args.join(", ")
            ));
        }
        if !verb.lookup_entity_types.is_empty() {
            preconditions.push(format!(
                "lookup entities resolve: {}",
                verb.lookup_entity_types.join(", ")
            ));
        }

        let postconditions = if normalized_mode == "read_only" {
            vec!["returns context without mutating business state".to_string()]
        } else {
            vec!["affects session/ui/system context only".to_string()]
        };

        let rationale = if normalized_mode == "read_only" {
            "No-harm/read-research verb should be explicitly classified instead of left missing"
                .to_string()
        } else {
            "System/session/view/admin verb sits outside business-table mutation scope".to_string()
        };

        entries.push(NonMutatingNormalization {
            fqn: verb.fqn.clone(),
            domain: verb.domain.clone(),
            workspace_affinity: workspaces,
            bucket,
            normalized_mode,
            reads: dedupe_strings(reads),
            writes: Vec::new(),
            preconditions,
            postconditions,
            evidence_source: vec!["yaml_metadata".to_string()],
            rationale,
        });
    }

    let payload = json!({
        "slice_id": "S6.4",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "normalized_entry_count": entries.len(),
        "mode_counts": mode_counts,
        "entries": entries,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_derive_lifecycle(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let (footprint_entries, _, _) = footprint_categories(&ctx);
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let missing: BTreeSet<String> = live_set.difference(&footprint_entries).cloned().collect();

    let mut entries = Vec::new();
    for fqn in &missing {
        let Some(verb) = ctx.live_verbs.get(fqn) else {
            continue;
        };
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        let (bucket, _, _) = classify_missing_verb(&ctx, verb, &workspaces);
        if bucket != "B_lifecycle_stateful" {
            continue;
        }

        let gates = ctx.taxonomies.state_gates_for(&verb.fqn);
        let mut preconditions = Vec::new();
        if !gates.is_empty() {
            preconditions.push(format!("node state is one of: {}", gates.join(", ")));
        }
        if !verb.required_args.is_empty() {
            preconditions.push(format!(
                "required args present: {}",
                verb.required_args.join(", ")
            ));
        }
        if !verb.lookup_entity_types.is_empty() {
            preconditions.push(format!(
                "lookup entities resolve: {}",
                verb.lookup_entity_types.join(", ")
            ));
        }

        let postconditions = if verb.action.contains("create")
            || verb.action.contains("open")
            || verb.action.contains("start")
        {
            vec!["lifecycle progresses from the gated state".to_string()]
        } else if verb.action.contains("close")
            || verb.action.contains("complete")
            || verb.action.contains("approve")
            || verb.action.contains("reject")
        {
            vec!["lifecycle reaches a terminal or advanced state".to_string()]
        } else {
            vec!["lifecycle or status state updated".to_string()]
        };

        entries.push(LifecycleDerivedFootprint {
            fqn: verb.fqn.clone(),
            domain: verb.domain.clone(),
            workspace_affinity: workspaces,
            node_state_gates: gates,
            preconditions,
            postconditions,
            evidence_source: vec!["state_machine".to_string(), "yaml_lifecycle".to_string()],
            rationale: "Derived from lifecycle/state-machine participation and required arguments"
                .to_string(),
        });
    }

    let payload = json!({
        "slice_id": "S6.3",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "derived_entry_count": entries.len(),
        "entries": entries,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_derive_delegated(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let (footprint_entries, _, _) = footprint_categories(&ctx);
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let missing: BTreeSet<String> = live_set.difference(&footprint_entries).cloned().collect();

    let mut entries = Vec::new();
    for fqn in &missing {
        let Some(verb) = ctx.live_verbs.get(fqn) else {
            continue;
        };
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        let (bucket, _, _) = classify_missing_verb(&ctx, verb, &workspaces);
        if bucket != "D_delegated_composite" {
            continue;
        }
        let downstream_hint = match verb.fqn.as_str() {
            "kyc.open-case" => vec!["kyc-case.create".to_string()],
            "template.batch" => vec!["template.invoke".to_string()],
            "template.invoke" => vec!["macro/template expansion".to_string()],
            _ => vec!["delegated downstream execution".to_string()],
        };
        let mut preconditions = Vec::new();
        if !verb.required_args.is_empty() {
            preconditions.push(format!(
                "required args present: {}",
                verb.required_args.join(", ")
            ));
        }
        if !verb.lookup_entity_types.is_empty() {
            preconditions.push(format!(
                "lookup entities resolve: {}",
                verb.lookup_entity_types.join(", ")
            ));
        }
        entries.push(DelegatedFootprint {
            fqn: verb.fqn.clone(),
            domain: verb.domain.clone(),
            workspace_affinity: workspaces,
            downstream_hint,
            preconditions,
            postconditions: vec!["delegates execution to downstream verbs or template expansion".to_string()],
            evidence_source: vec!["yaml_behavior".to_string(), "manual_classification".to_string()],
            rationale: "Composite or durable verb should be marked delegated instead of treated as a direct table-footprint gap".to_string(),
        });
    }

    let payload = json!({
        "slice_id": "S6.6",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "derived_entry_count": entries.len(),
        "entries": entries,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_plugin_batch(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let (footprint_entries, _, _) = footprint_categories(&ctx);
    let live_set: HashSet<String> = ctx.live_verbs.keys().cloned().collect();
    let missing: BTreeSet<String> = live_set.difference(&footprint_entries).cloned().collect();
    let priority_domains = [
        "deal",
        "cbu",
        "kyc-case",
        "screening",
        "document",
        "trading-profile",
        "service-resource",
        "onboarding",
    ];
    let priority_set: HashSet<&str> = priority_domains.iter().copied().collect();

    let mut entries = Vec::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for fqn in &missing {
        let Some(verb) = ctx.live_verbs.get(fqn) else {
            continue;
        };
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        let (bucket, _, _) = classify_missing_verb(&ctx, verb, &workspaces);
        if bucket != "C_plugin_business" {
            continue;
        }
        if !priority_set.contains(verb.domain.as_str()) {
            continue;
        }
        *counts.entry(verb.domain.clone()).or_default() += 1;
        entries.push(PluginBatchEntry {
            fqn: verb.fqn.clone(),
            domain: verb.domain.clone(),
            workspace_affinity: workspaces,
            behavior: verb.behavior.clone(),
            side_effects: verb.side_effects.clone(),
            required_args: verb.required_args.clone(),
            lookup_tables: verb.lookup_tables.clone(),
            lookup_entity_types: verb.lookup_entity_types.clone(),
            priority_reason:
                "High-value business/plugin domain for handler-level footprint extraction"
                    .to_string(),
        });
    }

    let payload = json!({
        "slice_id": "S6.5",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "priority_domains": priority_domains,
        "entry_count": entries.len(),
        "counts_by_domain": counts,
        "entries": entries,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_extract_plugin_core(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let entries = vec![
        plugin_core_entry(&ctx, "cbu.assign-ownership", &["roles"], &["cbu_entity_roles", "entity_relationships"],
            &["cbu-id, owner-entity-id, owned-entity-id, percentage", "role must resolve in roles taxonomy"],
            &["ownership role assignment upserted", "ownership relationship edge upserted"],
            "Atomic dual-write in cbu_role_ops for ownership role plus ownership relationship"),
        plugin_core_entry(&ctx, "cbu.assign-control", &["roles"], &["cbu_entity_roles", "entity_relationships"],
            &["cbu-id, controller-entity-id, controlled-entity-id, role", "role must resolve in roles taxonomy"],
            &["control role assignment upserted", "control relationship edge upserted"],
            "Atomic dual-write in cbu_role_ops for control role plus control relationship"),
        plugin_core_entry(&ctx, "cbu.assign-trust-role", &["roles"], &["cbu_entity_roles", "entity_relationships"],
            &["cbu-id, trust-entity-id, participant-entity-id, role", "role must resolve in roles taxonomy"],
            &["trust role assignment upserted", "trust relationship edge upserted"],
            "Atomic dual-write in cbu_role_ops for trust role plus trust relationship"),
        plugin_core_entry(&ctx, "cbu.assign-fund-role", &["roles"], &["cbu_entity_roles", "entity_relationships"],
            &["cbu-id, entity-id, role", "role must resolve in roles taxonomy"],
            &["fund role assignment upserted", "fund relationship edge upserted when fund-entity-id is supplied"],
            "Fund-role handler writes cbu roles and optionally entity relationships"),
        plugin_core_entry(&ctx, "cbu.assign-service-provider", &["roles"], &["cbu_entity_roles"],
            &["cbu-id, provider-entity-id, role", "role must resolve in roles taxonomy"],
            &["service provider role assignment upserted"],
            "Service-provider handler writes only cbu_entity_roles"),
        plugin_core_entry(&ctx, "cbu.assign-signatory", &["roles"], &["cbu_entity_roles"],
            &["cbu-id, person-entity-id, role", "role must resolve in roles taxonomy"],
            &["signatory assignment upserted with authority metadata"],
            "Signatory handler writes cbu_entity_roles with authority limit fields"),
        plugin_core_entry(&ctx, "cbu.validate-roles", &["cbus", "cbu_entity_roles", "roles", "entity_relationships"], &[],
            &["cbu-id must exist"],
            &["validation result returned", "missing-role and orphan-relationship issues surfaced"],
            "Validation handler reads CBU, role, and relationship state but does not mutate data"),
        plugin_core_entry(&ctx, "cbu.delete-cascade", &["cbus", "cbu_entity_roles", "entities"], &["client_group_entity", "cbu_group_members", "cbu_structure_links", "entities", "cbu_entity_roles", "cbus"],
            &["cbu-id must exist", "optional delete-entities flag controls entity soft-delete path"],
            &["CBU soft-deleted", "role links removed", "group and structure links removed", "exclusive entities optionally soft-deleted"],
            "Cascade delete handler performs transactional detach/soft-delete across CBU-linked tables"),
        plugin_core_entry(&ctx, "deal.add-sla", &["deals"], &["deal_slas", "deal_events"],
            &["deal-id, sla-name, metric-name, target-value, effective-from"],
            &["deal SLA row created", "SLA_ADDED event recorded"],
            "Direct sqlx inserts into deal_slas and deal_events"),
        plugin_core_entry(&ctx, "deal.add-document", &[], &["deal_documents"],
            &["deal-id, document-id, document-type"],
            &["deal document link created or updated"],
            "Upsert into deal_documents with status defaulting to DRAFT"),
        plugin_core_entry(&ctx, "deal.update-document-status", &["deal_documents"], &["deal_documents"],
            &["deal-id, document-id, document-status"],
            &["deal document status updated"],
            "Status update hits deal_documents only; no event insert in current implementation"),
        plugin_core_entry(&ctx, "deal.add-ubo-assessment", &[], &["deal_ubo_assessments"],
            &["deal-id, entity-id"],
            &["deal UBO assessment created"],
            "Direct insert into deal_ubo_assessments"),
        plugin_core_entry(&ctx, "deal.update-ubo-assessment", &["deal_ubo_assessments"], &["deal_ubo_assessments"],
            &["assessment-id"],
            &["assessment status/risk updated", "completed_at set when status becomes COMPLETED"],
            "Direct update of deal_ubo_assessments"),
        plugin_core_entry(&ctx, "deal.remove-rate-card-line", &["fee_billing_account_targets", "deal_rate_card_lines"], &["deal_rate_card_lines"],
            &["line-id", "no dependent fee_billing_account_targets may exist"],
            &["rate card line removed"],
            "Delete guarded by dependency check on fee_billing_account_targets"),
        plugin_core_entry(&ctx, "deal.update-rate-card-line", &["deal_rate_cards", "deal_rate_card_lines"], &["deal_rate_card_lines"],
            &["line-id", "parent rate card must not be AGREED"],
            &["rate card line updated in place"],
            "Update guarded by parent deal_rate_cards status check"),
        plugin_core_entry(&ctx, "deal.update-product-status", &["deal_products"], &["deal_products", "deal_events"],
            &["deal-id, product-id, product-status"],
            &["product status updated", "PRODUCT_STATUS_CHANGED event recorded", "agreed_at set when status is AGREED"],
            "Product status update writes deal_products and appends deal event"),
        plugin_core_entry(&ctx, "deal.update-onboarding-status", &["deal_onboarding_requests", "deals"], &["deal_onboarding_requests", "deals", "deal_events"],
            &["request-id, request-status"],
            &["onboarding request status updated", "KYC/completion timestamps advanced", "deal transitions to ACTIVE when all requests complete"],
            "Onboarding status handler updates request rows and conditionally activates the parent deal"),
        plugin_core_entry(&ctx, "trading-profile.ca.enable-event-types", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, event-types"],
            &["CA enabled_event_types merged into trading profile document"],
            "CA profile mutation loads and saves the JSONB document in cbu_trading_profiles"),
        plugin_core_entry(&ctx, "trading-profile.ca.disable-event-types", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, event-types"],
            &["requested event types removed from CA enabled_event_types"],
            "CA profile mutation edits the JSONB document stored in cbu_trading_profiles"),
        plugin_core_entry(&ctx, "trading-profile.ca.set-notification-policy", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, channels"],
            &["CA notification_policy replaced in trading profile document"],
            "CA notification policy is persisted by saving the updated profile JSONB document"),
        plugin_core_entry(&ctx, "trading-profile.ca.set-election-policy", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, elector"],
            &["CA election_policy replaced in trading profile document"],
            "CA election policy is persisted through ast_db document save"),
        plugin_core_entry(&ctx, "trading-profile.ca.set-default-option", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, event-type, default-option"],
            &["CA default_options entry upserted for the target event type"],
            "Default-option mutation is a JSONB document write against cbu_trading_profiles"),
        plugin_core_entry(&ctx, "trading-profile.ca.remove-default-option", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, event-type"],
            &["matching CA default_options entry removed"],
            "Default-option removal is persisted by saving the updated trading profile document"),
        plugin_core_entry(&ctx, "trading-profile.ca.add-cutoff-rule", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, days-before", "optional event-type, market-code, depository-code filter fields"],
            &["CA cutoff rule appended to trading profile document"],
            "Cutoff rule mutation is stored in cbu_trading_profiles JSONB"),
        plugin_core_entry(&ctx, "trading-profile.ca.remove-cutoff-rule", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id", "optional market-code, depository-code filter fields"],
            &["matching CA cutoff rules removed from trading profile document"],
            "Cutoff-rule removal rewrites the JSONB profile document in cbu_trading_profiles"),
        plugin_core_entry(&ctx, "trading-profile.ca.link-proceeds-ssi", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, proceeds-type, ssi-name"],
            &["CA proceeds-to-SSI mapping upserted in trading profile document"],
            "Proceeds SSI linkage is persisted as a JSONB document mutation"),
        plugin_core_entry(&ctx, "trading-profile.ca.remove-proceeds-ssi", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, proceeds-type"],
            &["CA proceeds-to-SSI mapping removed from trading profile document"],
            "Proceeds SSI unlink is a JSONB document write against cbu_trading_profiles"),
        plugin_core_entry(&ctx, "trading-profile.link-csa-ssi", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["profile-id, counterparty-ref, ssi-name", "profile must be in DRAFT status"],
            &["CSA SSI reference updated in trading profile document"],
            "CSA SSI linkage goes through ast_db::apply_and_save after a draft-status guard"),
        plugin_core_entry(&ctx, "trading-profile.validate-go-live-ready", &["cbu_trading_profiles"], &[],
            &["profile-id"],
            &["go-live readiness checklist and issues returned without mutating persisted state"],
            "Go-live readiness validation only reads the trading profile document and returns diagnostics"),
        plugin_core_entry(&ctx, "trading-profile.create-new-version", &["cbu_trading_profiles"], &["cbu_trading_profiles"],
            &["cbu-id", "an ACTIVE profile must exist", "no working DRAFT version may already exist"],
            &["new DRAFT trading profile version inserted from the active profile"],
            "New-version flow clones the active profile into a fresh DRAFT row in cbu_trading_profiles"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.provision-lifecycle", &["CBU", "OnBoarding"], &["lifecycle_resource_types", "markets"], &["cbu_lifecycle_instances"],
            &["cbu-id, resource-type", "market and counterparty required when the resource type demands them"],
            &["lifecycle instance provisioned with PROVISIONED status", "instance bound into execution context"],
            "Lifecycle provisioning aliases into LifecycleProvisionOp and upserts cbu_lifecycle_instances"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.generate-lifecycle-plan", &["CBU", "OnBoarding"], &["v_cbu_lifecycle_gaps"], &[],
            &["cbu-id", "optional user-responses"],
            &["lifecycle gap DSL plan and pending prompts returned without direct writes"],
            "Lifecycle plan generation reads the lifecycle gap view and emits an execution plan only"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.execute-lifecycle-plan", &["CBU", "OnBoarding"], &[], &[],
            &["plan", "optional dry-run"],
            &["placeholder execution records returned", "no direct database mutation in current implementation"],
            "Current lifecycle plan execution path is a non-persistent placeholder and does not yet integrate downstream execution"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.provision", &["CBU", "OnBoarding"], &["service_resource_types", "service_resource_capabilities", "cbus", "products", "services"], &["cbu_resource_instances", "resource_instance_dependencies"],
            &["cbu-id", "resource-type", "resource-type must resolve in service_resource_types", "instance-url is generated when omitted"],
            &["resource instance inserted or reused with PENDING status", "resource dependencies recorded when depends-on is supplied", "instance bound into execution context"],
            "ResourceCreateOp in resource_ops.rs resolves the resource type, optionally derives service_id from service_resource_capabilities, inserts cbu_resource_instances, and records resource_instance_dependencies"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.set-attr", &["CBU", "OnBoarding"], &["attribute_registry", "cbu_resource_instances"], &["resource_instance_attributes"],
            &["instance-id", "attr", "value", "attr must resolve via AttributeIdentityService"],
            &["resource instance attribute upserted with observed_at timestamp and state", "value id returned"],
            "ResourceSetAttrOp in resource_ops.rs resolves the governed attribute and upserts resource_instance_attributes for the target instance"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.activate", &["CBU", "OnBoarding"], &["cbu_resource_instances", "resource_attribute_requirements", "resource_instance_attributes", "attribute_registry"], &["cbu_resource_instances"],
            &["instance-id", "instance must exist", "all mandatory resource attributes must be present before activation"],
            &["resource instance status transitioned to ACTIVE", "activated_at timestamp set"],
            "ResourceActivateOp in resource_ops.rs validates resource_attribute_requirements against resource_instance_attributes before updating cbu_resource_instances"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.suspend", &["CBU", "OnBoarding"], &["cbu_resource_instances"], &["cbu_resource_instances"],
            &["instance-id"],
            &["resource instance status transitioned to SUSPENDED"],
            "ResourceSuspendOp in resource_ops.rs updates cbu_resource_instances to SUSPENDED"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.decommission", &["CBU", "OnBoarding"], &["cbu_resource_instances"], &["cbu_resource_instances"],
            &["instance-id"],
            &["resource instance status transitioned to DECOMMISSIONED", "decommissioned_at timestamp set"],
            "ResourceDecommissionOp in resource_ops.rs applies the terminal status update to cbu_resource_instances"),
        plugin_core_entry_with_workspaces(&ctx, "service-resource.validate-attrs", &["CBU", "OnBoarding"], &["cbu_resource_instances", "resource_attribute_requirements", "attribute_registry", "resource_instance_attributes"], &[],
            &["instance-id"],
            &["validation record returned with valid flag and missing attribute names"],
            "ResourceValidateAttrsOp in resource_ops.rs reads required attributes and current instance attributes to return a validation report"),
        plugin_core_entry(&ctx, "document.catalog", &["document_types", "document_catalog"], &["document_catalog"],
            &["doc-type/document-type", "optional cbu-id or entity-id", "document_name participates in idempotent lookup when supplied"],
            &["document catalog row created or existing row rebound into context"],
            "Document catalog lookup reads document_types and existing catalog rows before inserting document_catalog"),
        plugin_core_entry(&ctx, "document.extract", &["document_catalog"], &["document_catalog"],
            &["document-id/doc-id"],
            &["extraction_status set to IN_PROGRESS on document_catalog", "downstream OCR/extraction remains TODO in current implementation"],
            "Current extract implementation only marks extraction_status on document_catalog"),
        plugin_core_entry(&ctx, "document.upload-version", &["documents"], &["document_versions"],
            &["document-id", "content-type", "either blob-ref or structured-data is required"],
            &["document version inserted", "cargo_ref URI returned", "version bound into context"],
            "Upload-version verifies the parent document, computes next version number, and inserts document_versions"),
        plugin_core_entry(&ctx, "kyc-case.state", &["cases", "cbus", "entity_workstreams", "entities", "cbu_entity_roles", "roles", "outstanding_requests"], &[],
            &["case-id"],
            &["case state snapshot returned with workstreams, awaiting requests, and attention summary"],
            "Case-state handler is a complex read-only aggregate across case, CBU, workstream, role, and outstanding-request tables"),
    ];

    let delegated_bindings = vec![delegated_plugin_binding(
        &ctx,
        "onboarding.auto-complete",
        "Handler derives semantic state, generates DSL, and executes downstream verbs through DslExecutor; the mutating footprint belongs to those delegated verbs rather than a stable direct table set",
    )];

    let payload = json!({
        "slice_id": "S6.5A",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "derived_entry_count": entries.len(),
        "unresolved_binding_count": 0,
        "delegated_binding_count": delegated_bindings.len(),
        "entries": entries,
        "unresolved_bindings": Vec::<serde_json::Value>::new(),
        "delegated_bindings": delegated_bindings,
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn plugin_core_entry(
    ctx: &AuditContext,
    fqn: &str,
    reads: &[&str],
    writes: &[&str],
    preconditions: &[&str],
    postconditions: &[&str],
    rationale: &str,
) -> PluginExtractedFootprint {
    let workspaces = ctx.taxonomies.resolve_workspaces(
        &ctx.live_verbs
            .get(fqn)
            .expect("plugin core verb must exist")
            .domain,
    );
    plugin_core_entry_with_workspaces(
        ctx,
        fqn,
        &workspaces.iter().map(String::as_str).collect::<Vec<_>>(),
        reads,
        writes,
        preconditions,
        postconditions,
        rationale,
    )
}

#[allow(clippy::too_many_arguments)]
fn plugin_core_entry_with_workspaces(
    ctx: &AuditContext,
    fqn: &str,
    workspace_affinity: &[&str],
    reads: &[&str],
    writes: &[&str],
    preconditions: &[&str],
    postconditions: &[&str],
    rationale: &str,
) -> PluginExtractedFootprint {
    let verb = ctx
        .live_verbs
        .get(fqn)
        .expect("plugin core verb must exist");
    PluginExtractedFootprint {
        fqn: fqn.to_string(),
        domain: verb.domain.clone(),
        workspace_affinity: workspace_affinity.iter().map(|s| s.to_string()).collect(),
        reads: reads.iter().map(|s| s.to_string()).collect(),
        writes: writes.iter().map(|s| s.to_string()).collect(),
        preconditions: preconditions.iter().map(|s| s.to_string()).collect(),
        postconditions: postconditions.iter().map(|s| s.to_string()).collect(),
        evidence_source: vec!["rust_handler".to_string(), "sqlx".to_string()],
        rationale: rationale.to_string(),
    }
}

fn delegated_plugin_binding(ctx: &AuditContext, fqn: &str, rationale: &str) -> serde_json::Value {
    let verb = ctx
        .live_verbs
        .get(fqn)
        .expect("delegated plugin verb must exist");
    json!({
        "fqn": fqn,
        "domain": verb.domain,
        "workspace_affinity": ctx.taxonomies.resolve_workspaces(&verb.domain),
        "status": "delegated_handler",
        "required_args": verb.required_args,
        "lookup_tables": verb.lookup_tables,
        "lookup_entity_types": verb.lookup_entity_types,
        "evidence_source": ["rust_handler", "dsl_executor", "delegated_execution"],
        "rationale": rationale,
    })
}

fn classify_missing_verb(
    ctx: &AuditContext,
    verb: &LiveVerb,
    workspaces: &[String],
) -> (String, String, String) {
    let side_effects = verb.side_effects.as_deref().unwrap_or("");
    let harm_class = verb.harm_class.as_deref().unwrap_or("");
    let has_state_gates = !ctx.taxonomies.state_gates_for(&verb.fqn).is_empty();
    let global_workspace = workspaces.iter().any(|workspace| workspace == "*");
    let no_harm = matches!(side_effects, "facts_only" | "none")
        || harm_class == "read_only"
        || verb.action.starts_with("read")
        || verb.action.starts_with("list")
        || verb.action.starts_with("show")
        || verb.action.starts_with("search")
        || verb.action.starts_with("inspect")
        || verb.action.starts_with("describe");
    let delegated_domain = matches!(
        verb.domain.as_str(),
        "batch" | "pack" | "pipeline" | "governance" | "template"
    );

    if global_workspace {
        if no_harm {
            return (
                "F_system_ui".to_string(),
                "system_only".to_string(),
                "Global/session/admin-style verb with no-harm semantics".to_string(),
            );
        }
        return (
            "F_system_ui".to_string(),
            "system_only".to_string(),
            "Global/session/admin-style verb outside business-table footprint scope".to_string(),
        );
    }

    if delegated_domain || verb.behavior == "durable" {
        return (
            "D_delegated_composite".to_string(),
            "delegated".to_string(),
            "Composite or durable orchestration verb that likely delegates downstream work"
                .to_string(),
        );
    }

    if verb.has_crud {
        return (
            "A_pure_crud".to_string(),
            "derived_crud".to_string(),
            "CRUD mapping present in verb YAML; reads/writes can be mechanically derived"
                .to_string(),
        );
    }

    if has_state_gates || verb.has_lifecycle {
        return (
            "B_lifecycle_stateful".to_string(),
            "derived_lifecycle".to_string(),
            "Verb participates in lifecycle/state machine transitions or has explicit lifecycle metadata"
                .to_string(),
        );
    }

    if no_harm {
        return (
            "E_no_harm_read_research".to_string(),
            "read_only".to_string(),
            "No-harm/read-only Sage/show/research style verb".to_string(),
        );
    }

    if matches!(verb.behavior.as_str(), "plugin" | "graph_query") {
        return (
            "C_plugin_business".to_string(),
            "explicit".to_string(),
            "Business/plugin verb likely requires handler-level extraction".to_string(),
        );
    }

    (
        "C_plugin_business".to_string(),
        "unknown".to_string(),
        "Verb needs manual review because it did not match a safer classification rule".to_string(),
    )
}

fn derive_crud_footprint(verb: &LiveVerb) -> CrudDerivedFootprint {
    let operation = verb.crud_operation.clone();
    let mut reads = verb.lookup_tables.clone();
    let mut writes = Vec::new();
    let mut preconditions = Vec::new();
    let mut postconditions = Vec::new();
    let mut evidence_source = vec!["yaml_crud".to_string()];

    match operation.as_deref().unwrap_or("unknown") {
        "select" | "select_with_join" | "list_by_fk" | "list_parties" => {
            reads.extend(verb.crud_tables.clone());
            postconditions.push("returns matching records".to_string());
        }
        "insert" | "upsert" | "entity_create" | "entity_upsert" | "junction_insert" | "link"
        | "role_link" => {
            reads.extend(verb.lookup_tables.clone());
            writes.extend(verb.crud_tables.clone());
            postconditions.push("target record created or linked".to_string());
        }
        "update" => {
            reads.extend(verb.lookup_tables.clone());
            reads.extend(verb.crud_tables.clone());
            writes.extend(verb.crud_tables.clone());
            postconditions.push("target record updated".to_string());
        }
        "delete" | "unlink" | "role_unlink" => {
            reads.extend(verb.lookup_tables.clone());
            reads.extend(verb.crud_tables.clone());
            writes.extend(verb.crud_tables.clone());
            postconditions.push("target record removed or detached".to_string());
        }
        _ => {
            reads.extend(verb.lookup_tables.clone());
            writes.extend(verb.crud_tables.clone());
            postconditions.push("crud operation applied".to_string());
        }
    }

    if !verb.required_args.is_empty() {
        preconditions.push(format!(
            "required args present: {}",
            verb.required_args.join(", ")
        ));
    }
    if !verb.lookup_entity_types.is_empty() {
        preconditions.push(format!(
            "lookup entities resolve: {}",
            verb.lookup_entity_types.join(", ")
        ));
    }
    if verb.has_lifecycle {
        evidence_source.push("yaml_lifecycle".to_string());
    }

    CrudDerivedFootprint {
        fqn: verb.fqn.clone(),
        domain: verb.domain.clone(),
        operation,
        entity_scope: dedupe_strings(verb.lookup_entity_types.clone()),
        reads: dedupe_strings(reads),
        writes: dedupe_strings(writes),
        preconditions,
        postconditions,
        evidence_source,
        rationale: "Derived mechanically from CRUD config, lookup args, and required args"
            .to_string(),
    }
}

fn run_state_gates(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let mut gated = Vec::new();
    let mut ungated = Vec::new();
    for verb in ctx.live_verbs.values() {
        let gates = ctx.taxonomies.state_gates_for(&verb.fqn);
        if gates.is_empty() {
            ungated.push(verb.fqn.clone());
        } else {
            gated.push(json!({
                "fqn": verb.fqn,
                "node_state_gates": gates,
            }));
        }
    }
    let payload = json!({
        "slice_id": "S7",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "gated_verb_count": gated.len(),
        "ungated_verb_count": ungated.len(),
        "gated_verbs": gated,
        "ungated_examples": ungated.iter().take(100).collect::<Vec<_>>(),
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn run_cascade_test(repo: &Path, output: &Path) -> Result<()> {
    let ctx = AuditContext::load(repo)?;
    let exact = resolve_verb_set(
        &ctx,
        Some("CBU"),
        Some("governance_sla"),
        Some("cbu"),
        Some("active"),
    );
    let no_state = resolve_verb_set(&ctx, Some("CBU"), Some("governance_sla"), Some("cbu"), None);
    let no_subject = resolve_verb_set(&ctx, Some("CBU"), Some("governance_sla"), None, None);
    let no_constellation = resolve_verb_set(&ctx, Some("CBU"), None, None, None);
    let legacy = resolve_verb_set(&ctx, None, None, Some("cbu"), None);

    let monotonic = exact.len() <= no_state.len()
        && no_state.len() <= no_subject.len()
        && no_subject.len() <= no_constellation.len();

    let mut reachable = 0usize;
    for verb in ctx.live_verbs.values() {
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        let families = ctx.taxonomies.constellation_families_for(&workspaces);
        let subjects = ctx.taxonomies.subject_kinds_for(&workspaces);
        let states = ctx.taxonomies.state_gates_for(&verb.fqn);
        let family = families.first().map(String::as_str);
        let subject = subjects.first().map(String::as_str);
        let state = states.first().map(String::as_str);
        if resolve_verb_set(
            &ctx,
            workspaces.first().map(String::as_str),
            family,
            subject,
            state,
        )
        .contains(&verb.fqn)
        {
            reachable += 1;
        }
    }

    let payload = json!({
        "slice_id": "S8",
        "generated_at": ctx.generated_at,
        "repo_revision": ctx.repo_revision,
        "tests": {
            "exact_5d_count": exact.len(),
            "fallback_4d_count": no_state.len(),
            "fallback_3d_count": no_subject.len(),
            "fallback_2d_count": no_constellation.len(),
            "legacy_1d_count": legacy.len(),
        },
        "monotonic_narrowing": monotonic,
        "reachable_verbs": reachable,
        "unreachable_verbs": ctx.live_verbs.len().saturating_sub(reachable),
    });
    write_json(output, &payload)?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn resolve_verb_set(
    ctx: &AuditContext,
    workspace: Option<&str>,
    constellation: Option<&str>,
    subject_kind: Option<&str>,
    node_state: Option<&str>,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for verb in ctx.live_verbs.values() {
        let workspaces = ctx.taxonomies.resolve_workspaces(&verb.domain);
        if let Some(target_workspace) = workspace {
            if !workspaces
                .iter()
                .any(|candidate| candidate == target_workspace)
            {
                continue;
            }
        }

        let families = ctx.taxonomies.constellation_families_for(&workspaces);
        if let Some(target_family) = constellation {
            if !families.iter().any(|candidate| candidate == target_family) {
                continue;
            }
        }

        let subjects = ctx.taxonomies.subject_kinds_for(&workspaces);
        if let Some(target_subject) = subject_kind {
            if !subjects.iter().any(|candidate| candidate == target_subject) {
                continue;
            }
        }

        let gates = ctx.taxonomies.state_gates_for(&verb.fqn);
        if let Some(target_state) = node_state {
            if !gates.is_empty() && !gates.iter().any(|candidate| candidate == target_state) {
                continue;
            }
        }

        out.insert(verb.fqn.clone());
    }
    out
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64 * 100.0) / total as f64
    }
}
