//! Read-only ACP coverage inventory for authored DSL/DAG surfaces.
//!
//! This module does not route or execute work. It inventories the authored
//! configuration surface and compares it with the ACP state-anchor provider
//! registry so migration progress is visible as a structured ledger.

use anyhow::{Context, Result};
use sem_os_core::domain_pack::DomainPackManifest;
use serde::Serialize;
use serde_yaml::{Mapping, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::acp_state_anchor::{
    provider_registry, provider_supported_transition_registry, AcpStateAnchorProviderDescriptor,
};
use crate::runbook::{transition_language_pack_readiness, TransitionLanguagePackReadiness};

pub const ACP_DSL_DAG_COVERAGE_SCHEMA_VERSION: &str = "acp_dsl_dag_coverage_v1";

#[derive(Debug, Clone, Serialize)]
pub struct AcpDslDagCoverageReport {
    pub schema_version: &'static str,
    pub config_root: String,
    pub summary: AcpDslDagCoverageSummary,
    pub rows: Vec<AcpDslDagCoverageRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcpDslDagCoverageSummary {
    pub total_rows: usize,
    pub core_acp_covered_rows: usize,
    pub full_loop_covered_rows: usize,
    pub core_uncovered_rows: usize,
    pub full_loop_uncovered_rows: usize,
    pub partial_rows: usize,
    pub prose_only_failure_count: usize,
    pub provider_count: usize,
    pub pack_count: usize,
    pub dag_taxonomy_count: usize,
    pub state_machine_count: usize,
    pub domain_pack_count: usize,
    pub verb_config_count: usize,
    pub core_coverage_percent: f64,
    pub full_loop_coverage_percent: f64,
    pub rows_by_surface: BTreeMap<String, usize>,
    pub rows_by_status: BTreeMap<String, usize>,
    pub providers: Vec<AcpDslDagProviderCoverage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcpDslDagProviderCoverage {
    pub provider_id: String,
    pub task: String,
    pub subject_kind: String,
    pub language_pack_boundary: String,
    pub dry_run_only: bool,
    pub mutation_authority: bool,
    pub supported_verbs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpDslDagSurfaceKind {
    PackAllowedVerb,
    VerbConfig,
    DagProgressionVerb,
    DagTransitionVia,
    StateMachineTransition,
    DomainPackTransition,
}

impl AcpDslDagSurfaceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::PackAllowedVerb => "pack_allowed_verb",
            Self::VerbConfig => "verb_config",
            Self::DagProgressionVerb => "dag_progression_verb",
            Self::DagTransitionVia => "dag_transition_via",
            Self::StateMachineTransition => "state_machine_transition",
            Self::DomainPackTransition => "domain_pack_transition",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpDslDagCoverageStatus {
    Covered,
    CoreOnly,
    Partial,
    Uncovered,
}

impl AcpDslDagCoverageStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Covered => "covered",
            Self::CoreOnly => "core_only",
            Self::Partial => "partial",
            Self::Uncovered => "uncovered",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AcpDslDagCoverageRow {
    pub surface_kind: AcpDslDagSurfaceKind,
    pub source_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dag_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_machine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_state: Option<String>,
    pub verb: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acp_provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_pack_boundary: Option<String>,
    pub acp_provider_exists: bool,
    pub language_pack_supported: bool,
    pub state_anchor_supported: bool,
    pub deterministic_dry_run_supported: bool,
    pub llm_revision_harness_supported: bool,
    pub structured_outcome_supported: bool,
    pub core_acp_covered: bool,
    pub full_loop_covered: bool,
    pub coverage_status: AcpDslDagCoverageStatus,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone)]
struct RawCoverageRow {
    surface_kind: AcpDslDagSurfaceKind,
    source_file: String,
    workspace: Option<String>,
    pack_id: Option<String>,
    dag_id: Option<String>,
    state_machine: Option<String>,
    subject_kind: Option<String>,
    transition_ref: Option<String>,
    from_state: Option<String>,
    to_state: Option<String>,
    verb: String,
    language_pack_readiness: Option<TransitionLanguagePackReadiness>,
}

#[derive(Debug, Default)]
struct InventoryCounts {
    pack_count: usize,
    dag_taxonomy_count: usize,
    state_machine_count: usize,
    domain_pack_count: usize,
    verb_config_count: usize,
}

pub fn build_acp_dsl_dag_coverage_report(
    config_root: impl AsRef<Path>,
) -> Result<AcpDslDagCoverageReport> {
    let config_root = config_root.as_ref();
    let mut rows = Vec::new();
    let mut counts = InventoryCounts::default();

    collect_pack_allowed_verbs(config_root, &mut rows, &mut counts)?;
    collect_verb_config_rows(config_root, &mut rows, &mut counts)?;
    collect_dag_taxonomy_rows(config_root, &mut rows, &mut counts)?;
    collect_state_machine_rows(config_root, &mut rows, &mut counts)?;
    collect_domain_pack_rows(config_root, &mut rows, &mut counts)?;

    let mut deduped = BTreeMap::new();
    for row in rows {
        deduped.entry(raw_row_key(&row)).or_insert(row);
    }

    let supported_transitions = supported_transition_index();
    let mut rows = deduped
        .into_values()
        .map(|row| annotate_row(row, &supported_transitions))
        .collect::<Vec<AcpDslDagCoverageRow>>();
    rows.sort_by(|left, right| {
        (
            left.surface_kind,
            left.source_file.as_str(),
            left.verb.as_str(),
            left.from_state.as_deref().unwrap_or_default(),
            left.to_state.as_deref().unwrap_or_default(),
        )
            .cmp(&(
                right.surface_kind,
                right.source_file.as_str(),
                right.verb.as_str(),
                right.from_state.as_deref().unwrap_or_default(),
                right.to_state.as_deref().unwrap_or_default(),
            ))
    });

    Ok(AcpDslDagCoverageReport {
        schema_version: ACP_DSL_DAG_COVERAGE_SCHEMA_VERSION,
        config_root: config_root.display().to_string(),
        summary: summarize(&rows, &counts),
        rows,
    })
}

pub fn write_acp_dsl_dag_coverage_artifacts(
    report: &AcpDslDagCoverageReport,
    out_dir: impl AsRef<Path>,
) -> Result<(PathBuf, PathBuf)> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)
        .with_context(|| format!("creating coverage output dir {}", out_dir.display()))?;
    let json_path = out_dir.join("acp_dsl_dag_coverage_report.json");
    let md_path = out_dir.join("acp_dsl_dag_coverage_report.md");
    fs::write(&json_path, serde_json::to_vec_pretty(report)?)
        .with_context(|| format!("writing {}", json_path.display()))?;
    fs::write(&md_path, render_markdown_report(report))
        .with_context(|| format!("writing {}", md_path.display()))?;
    Ok((json_path, md_path))
}

fn summarize(rows: &[AcpDslDagCoverageRow], counts: &InventoryCounts) -> AcpDslDagCoverageSummary {
    let total_rows = rows.len();
    let core_acp_covered_rows = rows.iter().filter(|row| row.core_acp_covered).count();
    let full_loop_covered_rows = rows.iter().filter(|row| row.full_loop_covered).count();
    let partial_rows = rows
        .iter()
        .filter(|row| row.coverage_status == AcpDslDagCoverageStatus::Partial)
        .count();
    let mut rows_by_surface = BTreeMap::new();
    let mut rows_by_status = BTreeMap::new();
    for row in rows {
        *rows_by_surface
            .entry(row.surface_kind.as_str().to_string())
            .or_insert(0) += 1;
        *rows_by_status
            .entry(row.coverage_status.as_str().to_string())
            .or_insert(0) += 1;
    }

    AcpDslDagCoverageSummary {
        total_rows,
        core_acp_covered_rows,
        full_loop_covered_rows,
        core_uncovered_rows: total_rows.saturating_sub(core_acp_covered_rows),
        full_loop_uncovered_rows: total_rows.saturating_sub(full_loop_covered_rows),
        partial_rows,
        prose_only_failure_count: 0,
        provider_count: provider_registry().len(),
        pack_count: counts.pack_count,
        dag_taxonomy_count: counts.dag_taxonomy_count,
        state_machine_count: counts.state_machine_count,
        domain_pack_count: counts.domain_pack_count,
        verb_config_count: counts.verb_config_count,
        core_coverage_percent: percent(core_acp_covered_rows, total_rows),
        full_loop_coverage_percent: percent(full_loop_covered_rows, total_rows),
        rows_by_surface,
        rows_by_status,
        providers: provider_registry().iter().map(provider_coverage).collect(),
    }
}

fn provider_coverage(provider: &AcpStateAnchorProviderDescriptor) -> AcpDslDagProviderCoverage {
    AcpDslDagProviderCoverage {
        provider_id: provider.provider_id.to_string(),
        task: provider.task.to_string(),
        subject_kind: provider.subject_kind.to_string(),
        language_pack_boundary: provider.language_pack_boundary.to_string(),
        dry_run_only: provider.dry_run_only,
        mutation_authority: provider.mutation_authority,
        supported_verbs: provider
            .supported_verbs
            .iter()
            .map(|verb| (*verb).to_string())
            .collect(),
    }
}

fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        100.0
    } else {
        ((numerator as f64 / denominator as f64) * 10_000.0).round() / 100.0
    }
}

fn annotate_row(
    row: RawCoverageRow,
    supported_transitions: &BTreeSet<String>,
) -> AcpDslDagCoverageRow {
    let provider = provider_for_verb(&row.verb);
    let acp_provider_exists = provider.is_some();
    let provider_is_dry_run_only = provider
        .map(|provider| provider.dry_run_only && !provider.mutation_authority)
        .unwrap_or(false);
    let update_status_like = row.verb.ends_with(".update-status");
    let domain_pack_ready = row
        .language_pack_readiness
        .as_ref()
        .map(|readiness| readiness.ready)
        .unwrap_or(false);
    let language_pack_supported = update_status_like && (acp_provider_exists || domain_pack_ready);
    let state_anchor_supported = acp_provider_exists;
    let provider_transition_binding_supported =
        transition_binding_supported(&row, supported_transitions);
    let deterministic_dry_run_supported = acp_provider_exists
        && update_status_like
        && provider_is_dry_run_only
        && provider_transition_binding_supported;
    let structured_outcome_supported = acp_provider_exists;
    let llm_revision_harness_supported = row.verb == "kyc-case.update-status";
    let core_acp_covered = acp_provider_exists
        && language_pack_supported
        && state_anchor_supported
        && deterministic_dry_run_supported
        && structured_outcome_supported;
    let full_loop_covered = core_acp_covered && llm_revision_harness_supported;

    let mut missing = Vec::new();
    if !acp_provider_exists {
        missing.push("acp_state_anchor_provider".to_string());
    }
    if !language_pack_supported {
        missing.push("language_pack_projection".to_string());
    }
    if !state_anchor_supported {
        missing.push("read_only_state_anchor".to_string());
    }
    if !deterministic_dry_run_supported {
        missing.push("deterministic_dry_run".to_string());
    }
    if acp_provider_exists && !provider_transition_binding_supported {
        missing.push("provider_transition_binding".to_string());
    }
    if !structured_outcome_supported {
        missing.push("structured_outcome_projection".to_string());
    }
    if !llm_revision_harness_supported {
        missing.push("llm_revision_harness".to_string());
    }

    let coverage_status = if full_loop_covered {
        AcpDslDagCoverageStatus::Covered
    } else if core_acp_covered {
        AcpDslDagCoverageStatus::CoreOnly
    } else if acp_provider_exists || language_pack_supported {
        AcpDslDagCoverageStatus::Partial
    } else {
        AcpDslDagCoverageStatus::Uncovered
    };

    AcpDslDagCoverageRow {
        surface_kind: row.surface_kind,
        source_file: row.source_file,
        workspace: row.workspace,
        pack_id: row.pack_id,
        dag_id: row.dag_id,
        state_machine: row.state_machine,
        subject_kind: row.subject_kind,
        transition_ref: row.transition_ref,
        from_state: row.from_state,
        to_state: row.to_state,
        verb: row.verb,
        acp_provider_id: provider.map(|provider| provider.provider_id.to_string()),
        language_pack_boundary: provider
            .map(|provider| provider.language_pack_boundary.to_string()),
        acp_provider_exists,
        language_pack_supported,
        state_anchor_supported,
        deterministic_dry_run_supported,
        llm_revision_harness_supported,
        structured_outcome_supported,
        core_acp_covered,
        full_loop_covered,
        coverage_status,
        missing,
    }
}

fn supported_transition_index() -> BTreeSet<String> {
    provider_supported_transition_registry()
        .into_iter()
        .map(|transition| {
            transition_key(
                &transition.task,
                Some(&transition.from_state),
                Some(&transition.to_state),
            )
        })
        .collect()
}

fn transition_binding_supported(
    row: &RawCoverageRow,
    supported_transitions: &BTreeSet<String>,
) -> bool {
    match (row.from_state.as_deref(), row.to_state.as_deref()) {
        (Some(from_state), Some(to_state)) => supported_transitions.contains(&transition_key(
            &row.verb,
            Some(from_state),
            Some(to_state),
        )),
        _ => true,
    }
}

fn transition_key(verb: &str, from_state: Option<&str>, to_state: Option<&str>) -> String {
    format!(
        "{}|{}|{}",
        verb,
        normalize_state_for_coverage(from_state.unwrap_or_default()),
        normalize_state_for_coverage(to_state.unwrap_or_default())
    )
}

fn normalize_state_for_coverage(value: &str) -> String {
    value.trim().replace(['-', ' '], "_").to_ascii_uppercase()
}

fn provider_for_verb(verb: &str) -> Option<&'static AcpStateAnchorProviderDescriptor> {
    provider_registry()
        .iter()
        .find(|provider| provider.task == verb || provider.supported_verbs.contains(&verb))
}

fn collect_pack_allowed_verbs(
    config_root: &Path,
    rows: &mut Vec<RawCoverageRow>,
    counts: &mut InventoryCounts,
) -> Result<()> {
    let pack_dir = config_root.join("packs");
    if !pack_dir.exists() {
        return Ok(());
    }

    for path in yaml_files(&pack_dir)? {
        counts.pack_count += 1;
        let value = read_yaml(&path)?;
        let Some(mapping) = value.as_mapping() else {
            continue;
        };
        let pack_id = string_at(mapping, "id");
        let workspace = string_list_at(mapping, "workspaces").join(",");
        let workspace = non_empty(workspace);
        let source_file = relative_path(config_root, &path);
        for verb in string_list_at(mapping, "allowed_verbs") {
            rows.push(RawCoverageRow {
                surface_kind: AcpDslDagSurfaceKind::PackAllowedVerb,
                source_file: source_file.clone(),
                workspace: workspace.clone(),
                pack_id: pack_id.clone(),
                dag_id: None,
                state_machine: None,
                subject_kind: None,
                transition_ref: None,
                from_state: None,
                to_state: None,
                verb,
                language_pack_readiness: None,
            });
        }
    }
    Ok(())
}

fn collect_verb_config_rows(
    config_root: &Path,
    rows: &mut Vec<RawCoverageRow>,
    counts: &mut InventoryCounts,
) -> Result<()> {
    let verbs_dir = config_root.join("verbs");
    if !verbs_dir.exists() {
        return Ok(());
    }

    for path in yaml_files(&verbs_dir)? {
        if should_skip_verb_config(&verbs_dir, &path) {
            continue;
        }
        let value = read_yaml(&path)?;
        let Some(root) = value.as_mapping() else {
            continue;
        };
        let Some(domains) = mapping_get(root, "domains").and_then(Value::as_mapping) else {
            continue;
        };
        counts.verb_config_count += 1;
        let source_file = relative_path(config_root, &path);
        for (domain_key, domain_value) in domains {
            let Some(domain) = scalar_string(domain_key) else {
                continue;
            };
            let Some(verbs) = domain_value
                .as_mapping()
                .and_then(|domain| mapping_get(domain, "verbs"))
                .and_then(Value::as_mapping)
            else {
                continue;
            };
            for verb_key in verbs.keys() {
                let Some(verb_name) = scalar_string(verb_key) else {
                    continue;
                };
                rows.push(RawCoverageRow {
                    surface_kind: AcpDslDagSurfaceKind::VerbConfig,
                    source_file: source_file.clone(),
                    workspace: None,
                    pack_id: None,
                    dag_id: None,
                    state_machine: None,
                    subject_kind: None,
                    transition_ref: None,
                    from_state: None,
                    to_state: None,
                    verb: format!("{domain}.{verb_name}"),
                    language_pack_readiness: None,
                });
            }
        }
    }
    Ok(())
}

fn should_skip_verb_config(verbs_dir: &Path, path: &Path) -> bool {
    let rel = path.strip_prefix(verbs_dir).unwrap_or(path);
    if rel.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("templates")
    }) {
        return true;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('_') || name.ends_with(".disabled"))
        .unwrap_or(false)
}

fn collect_dag_taxonomy_rows(
    config_root: &Path,
    rows: &mut Vec<RawCoverageRow>,
    counts: &mut InventoryCounts,
) -> Result<()> {
    let dag_dir = config_root.join("sem_os_seeds/dag_taxonomies");
    if !dag_dir.exists() {
        return Ok(());
    }

    for path in yaml_files(&dag_dir)? {
        counts.dag_taxonomy_count += 1;
        let value = read_yaml(&path)?;
        let workspace = value
            .as_mapping()
            .and_then(|mapping| string_at(mapping, "workspace"));
        let dag_id = value
            .as_mapping()
            .and_then(|mapping| string_at(mapping, "dag_id"));
        let source_file = relative_path(config_root, &path);
        collect_dag_rows_from_value(
            &value,
            rows,
            &source_file,
            workspace.as_deref(),
            dag_id.as_deref(),
            None,
        );
    }
    Ok(())
}

fn collect_dag_rows_from_value(
    value: &Value,
    rows: &mut Vec<RawCoverageRow>,
    source_file: &str,
    workspace: Option<&str>,
    dag_id: Option<&str>,
    inherited_state_machine: Option<&str>,
) {
    match value {
        Value::Mapping(mapping) => {
            let current_state_machine = string_at(mapping, "state_machine")
                .or_else(|| string_at(mapping, "state_machine_id"))
                .or_else(|| inherited_state_machine.map(ToOwned::to_owned));
            let from_state = string_at(mapping, "from");
            let to_state = string_at(mapping, "to");

            if let Some(verbs) = mapping_get(mapping, "progression_verbs").map(string_list) {
                for verb in verbs {
                    rows.push(RawCoverageRow {
                        surface_kind: AcpDslDagSurfaceKind::DagProgressionVerb,
                        source_file: source_file.to_string(),
                        workspace: workspace.map(ToOwned::to_owned),
                        pack_id: None,
                        dag_id: dag_id.map(ToOwned::to_owned),
                        state_machine: current_state_machine.clone(),
                        subject_kind: None,
                        transition_ref: None,
                        from_state: from_state.clone(),
                        to_state: to_state.clone(),
                        verb,
                        language_pack_readiness: None,
                    });
                }
            }

            if let Some(verbs) = mapping_get(mapping, "via").map(string_list) {
                for verb in verbs {
                    rows.push(RawCoverageRow {
                        surface_kind: AcpDslDagSurfaceKind::DagTransitionVia,
                        source_file: source_file.to_string(),
                        workspace: workspace.map(ToOwned::to_owned),
                        pack_id: None,
                        dag_id: dag_id.map(ToOwned::to_owned),
                        state_machine: current_state_machine.clone(),
                        subject_kind: None,
                        transition_ref: None,
                        from_state: from_state.clone(),
                        to_state: to_state.clone(),
                        verb,
                        language_pack_readiness: None,
                    });
                }
            }

            for child in mapping.values() {
                collect_dag_rows_from_value(
                    child,
                    rows,
                    source_file,
                    workspace,
                    dag_id,
                    current_state_machine.as_deref(),
                );
            }
        }
        Value::Sequence(items) => {
            for item in items {
                collect_dag_rows_from_value(
                    item,
                    rows,
                    source_file,
                    workspace,
                    dag_id,
                    inherited_state_machine,
                );
            }
        }
        _ => {}
    }
}

fn collect_state_machine_rows(
    config_root: &Path,
    rows: &mut Vec<RawCoverageRow>,
    counts: &mut InventoryCounts,
) -> Result<()> {
    let state_machine_dir = config_root.join("sem_os_seeds/state_machines");
    if !state_machine_dir.exists() {
        return Ok(());
    }

    for path in yaml_files(&state_machine_dir)? {
        counts.state_machine_count += 1;
        let value = read_yaml(&path)?;
        let Some(mapping) = value.as_mapping() else {
            continue;
        };
        let state_machine = string_at(mapping, "state_machine");
        let source_file = relative_path(config_root, &path);
        let Some(transitions) = mapping_get(mapping, "transitions").and_then(Value::as_sequence)
        else {
            continue;
        };
        for transition in transitions {
            let Some(transition) = transition.as_mapping() else {
                continue;
            };
            let from_state = string_at(transition, "from");
            let to_state = string_at(transition, "to");
            for verb in string_list_at(transition, "verbs") {
                rows.push(RawCoverageRow {
                    surface_kind: AcpDslDagSurfaceKind::StateMachineTransition,
                    source_file: source_file.clone(),
                    workspace: None,
                    pack_id: None,
                    dag_id: None,
                    state_machine: state_machine.clone(),
                    subject_kind: None,
                    transition_ref: None,
                    from_state: from_state.clone(),
                    to_state: to_state.clone(),
                    verb,
                    language_pack_readiness: None,
                });
            }
        }
    }
    Ok(())
}

fn collect_domain_pack_rows(
    config_root: &Path,
    rows: &mut Vec<RawCoverageRow>,
    counts: &mut InventoryCounts,
) -> Result<()> {
    let domain_pack_dir = config_root.join("sem_os_seeds/domain_packs");
    if !domain_pack_dir.exists() {
        return Ok(());
    }

    for path in yaml_files(&domain_pack_dir)? {
        counts.domain_pack_count += 1;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("reading domain pack {}", path.display()))?;
        let manifest: DomainPackManifest = serde_yaml::from_str(&source)
            .with_context(|| format!("parsing domain pack {}", path.display()))?;
        let source_file = relative_path(config_root, &path);
        for transition in &manifest.allowed_transitions {
            let readiness =
                transition_language_pack_readiness(&manifest, &transition.transition_ref);
            rows.push(RawCoverageRow {
                surface_kind: AcpDslDagSurfaceKind::DomainPackTransition,
                source_file: source_file.clone(),
                workspace: manifest.owned_constellations.first().cloned(),
                pack_id: Some(manifest.pack_id.clone()),
                dag_id: None,
                state_machine: Some(transition.state_machine.clone()),
                subject_kind: Some(transition.entity_type.clone()),
                transition_ref: Some(transition.transition_ref.clone()),
                from_state: Some(transition.from_state.clone()),
                to_state: Some(transition.to_state.clone()),
                verb: transition.verb.clone(),
                language_pack_readiness: Some(readiness),
            });
        }
    }
    Ok(())
}

fn yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_yaml_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_yaml_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading dir {}", dir.display()))? {
        let entry = entry.with_context(|| format!("reading entry in {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_files(&path, files)?;
        } else if is_yaml_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_yaml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension, "yaml" | "yml"))
        .unwrap_or(false)
}

fn read_yaml(path: &Path) -> Result<Value> {
    let source = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&source).with_context(|| format!("parsing {}", path.display()))
}

fn mapping_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_string()))
}

fn string_at(mapping: &Mapping, key: &str) -> Option<String> {
    mapping_get(mapping, key).and_then(scalar_string)
}

fn string_list_at(mapping: &Mapping, key: &str) -> Vec<String> {
    mapping_get(mapping, key)
        .map(string_list)
        .unwrap_or_default()
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => non_empty(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Tagged(tagged) => scalar_string(&tagged.value),
        _ => None,
    }
}

fn string_list(value: &Value) -> Vec<String> {
    match value {
        Value::Sequence(items) => items.iter().filter_map(scalar_string).collect(),
        Value::String(value) => split_inline_verb_string(value),
        Value::Tagged(tagged) => string_list(&tagged.value),
        _ => scalar_string(value).into_iter().collect(),
    }
}

fn split_inline_verb_string(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return trimmed
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .filter_map(|part| {
                non_empty(part.trim().trim_matches('"').trim_matches('\'').to_string())
            })
            .collect();
    }
    non_empty(trimmed.to_string()).into_iter().collect()
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn raw_row_key(row: &RawCoverageRow) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        row.surface_kind.as_str(),
        row.source_file,
        row.workspace.as_deref().unwrap_or_default(),
        row.pack_id.as_deref().unwrap_or_default(),
        row.dag_id.as_deref().unwrap_or_default(),
        row.state_machine.as_deref().unwrap_or_default(),
        row.from_state.as_deref().unwrap_or_default(),
        row.to_state.as_deref().unwrap_or_default(),
        row.verb
    )
}

fn render_markdown_report(report: &AcpDslDagCoverageReport) -> String {
    let mut out = String::new();
    out.push_str("# ACP DSL/DAG Coverage Report\n\n");
    out.push_str(&format!(
        "- Schema: `{}`\n- Rows: `{}`\n- Core ACP covered: `{}` / `{}` (`{:.2}%`)\n- Full LLM loop covered: `{}` / `{}` (`{:.2}%`)\n- Prose-only failures: `{}`\n- Providers: `{}`\n- Packs: `{}`\n- DAG taxonomies: `{}`\n- State machines: `{}`\n- Domain packs: `{}`\n- Verb configs: `{}`\n\n",
        report.schema_version,
        report.summary.total_rows,
        report.summary.core_acp_covered_rows,
        report.summary.total_rows,
        report.summary.core_coverage_percent,
        report.summary.full_loop_covered_rows,
        report.summary.total_rows,
        report.summary.full_loop_coverage_percent,
        report.summary.prose_only_failure_count,
        report.summary.provider_count,
        report.summary.pack_count,
        report.summary.dag_taxonomy_count,
        report.summary.state_machine_count,
        report.summary.domain_pack_count,
        report.summary.verb_config_count,
    ));

    out.push_str("## Providers\n\n");
    out.push_str("| Provider | Task | Boundary | Dry-run only | Mutation authority |\n");
    out.push_str("|---|---|---:|---:|---:|\n");
    for provider in &report.summary.providers {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            provider.provider_id,
            provider.task,
            provider.language_pack_boundary,
            provider.dry_run_only,
            provider.mutation_authority
        ));
    }

    out.push_str("\n## Uncovered Rows\n\n");
    out.push_str("| Surface | Source | Verb | From | To | Missing |\n");
    out.push_str("|---|---|---|---|---|---|\n");
    for row in report
        .rows
        .iter()
        .filter(|row| !row.full_loop_covered)
        .take(200)
    {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            row.surface_kind.as_str(),
            row.source_file,
            row.verb,
            row.from_state.as_deref().unwrap_or(""),
            row.to_state.as_deref().unwrap_or(""),
            row.missing.join(", ")
        ));
    }
    if report.summary.full_loop_uncovered_rows > 200 {
        out.push_str(&format!(
            "\nShowing first 200 of {} uncovered full-loop rows.\n",
            report.summary.full_loop_uncovered_rows
        ));
    }

    out
}
