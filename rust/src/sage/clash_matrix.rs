//! Export deterministic Coder clash diagnostics from the verb metadata index.

use std::fmt::Write;

use anyhow::Result;
use dsl_core::config::types::HarmClass;

use super::verb_index::{VerbMeta, VerbMetadataIndex};

/// One candidate clash pair in the Coder surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClashRow {
    pub verb_a: String,
    pub verb_b: String,
    pub domain: String,
    pub planes: Vec<String>,
    pub required_param_signature: Vec<String>,
    pub action_overlap: Vec<String>,
    pub clash_kind: ClashKind,
    pub harm_a: HarmClass,
    pub harm_b: HarmClass,
    pub side_effects_a: Option<String>,
    pub side_effects_b: Option<String>,
}

/// Heuristic discriminator bucket for a clash pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClashKind {
    ActionDifferentiable,
    StateDifferentiable,
    Synonymous,
    ContextDifferentiable,
}

/// Build a deterministic clash matrix from the current verb metadata index.
///
/// # Examples
/// ```ignore
/// use ob_poc::sage::clash_matrix::build_clash_matrix;
/// use ob_poc::sage::VerbMetadataIndex;
///
/// let index = VerbMetadataIndex::load()?;
/// let rows = build_clash_matrix(&index);
/// assert!(rows.iter().all(|row| row.domain == row.verb_a.split('.').next().unwrap()));
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn build_clash_matrix(index: &VerbMetadataIndex) -> Vec<ClashRow> {
    let mut metas = index.iter().collect::<Vec<_>>();
    metas.sort_by(|a, b| a.fqn.cmp(&b.fqn));

    let mut rows = Vec::new();
    for (left_idx, left) in metas.iter().enumerate() {
        for right in metas.iter().skip(left_idx + 1) {
            if !is_clash_candidate(left, right) {
                continue;
            }

            let mut planes = intersect_strings(
                left.planes.iter().map(|plane| format!("{plane:?}")),
                right.planes.iter().map(|plane| format!("{plane:?}")),
            );
            planes.sort();

            let mut required_param_signature = left.required_params.clone();
            required_param_signature.sort();

            let mut action_overlap = intersect_strings(
                left.action_tags.iter().cloned(),
                right.action_tags.iter().cloned(),
            );
            action_overlap.retain(|tag| !tag.is_empty());
            action_overlap.sort();

            rows.push(ClashRow {
                verb_a: left.fqn.clone(),
                verb_b: right.fqn.clone(),
                domain: left.domain.clone(),
                planes,
                required_param_signature,
                action_overlap,
                clash_kind: classify_clash_kind(left, right),
                harm_a: left.harm_class,
                harm_b: right.harm_class,
                side_effects_a: left.side_effects.clone(),
                side_effects_b: right.side_effects.clone(),
            });
        }
    }

    rows
}

/// Render clash rows to CSV and Markdown diagnostics.
///
/// # Examples
/// ```ignore
/// use ob_poc::sage::clash_matrix::{build_clash_matrix, render_clash_reports};
/// use ob_poc::sage::VerbMetadataIndex;
///
/// let index = VerbMetadataIndex::load()?;
/// let rows = build_clash_matrix(&index);
/// let (_csv, markdown) = render_clash_reports(&rows)?;
/// assert!(markdown.starts_with("# Coder Clash Matrix"));
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn render_clash_reports(rows: &[ClashRow]) -> Result<(String, String)> {
    let mut csv = String::from(
        "verb_a,verb_b,domain,planes,required_param_signature,action_overlap,clash_kind,harm_a,harm_b,side_effects_a,side_effects_b\n",
    );
    let mut markdown = String::from("# Coder Clash Matrix\n\n");
    writeln!(&mut markdown, "- clash pairs: {}", rows.len())?;
    let mut by_kind = std::collections::BTreeMap::<String, usize>::new();
    for row in rows {
        *by_kind.entry(format!("{:?}", row.clash_kind)).or_default() += 1;
    }
    markdown.push_str("## Clash Kinds\n\n");
    for (kind, count) in by_kind {
        writeln!(&mut markdown, "- {}: {}", kind, count)?;
    }
    markdown.push('\n');
    markdown.push_str(
        "| Verb A | Verb B | Domain | Planes | Required Params | Action Overlap | Kind | Harm |\n",
    );
    markdown.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");

    for row in rows {
        writeln!(
            &mut csv,
            "{},{},{},{},{},{},{:?},{:?},{:?},{},{}",
            csv_escape(&row.verb_a),
            csv_escape(&row.verb_b),
            csv_escape(&row.domain),
            csv_escape(&row.planes.join("|")),
            csv_escape(&row.required_param_signature.join("|")),
            csv_escape(&row.action_overlap.join("|")),
            row.clash_kind,
            row.harm_a,
            row.harm_b,
            csv_escape(row.side_effects_a.as_deref().unwrap_or("")),
            csv_escape(row.side_effects_b.as_deref().unwrap_or("")),
        )?;
        writeln!(
            &mut markdown,
            "| {} | {} | {} | {} | {} | {} | `{:?}` | `{:?}` / `{:?}` |",
            row.verb_a,
            row.verb_b,
            row.domain,
            row.planes.join(", "),
            row.required_param_signature.join(", "),
            row.action_overlap.join(", "),
            row.clash_kind,
            row.harm_a,
            row.harm_b,
        )?;
    }

    Ok((csv, markdown))
}

fn is_clash_candidate(left: &VerbMeta, right: &VerbMeta) -> bool {
    left.domain == right.domain
        && same_required_signature(left, right)
        && has_plane_overlap(left, right)
        && has_action_overlap(left, right)
}

fn same_required_signature(left: &VerbMeta, right: &VerbMeta) -> bool {
    let mut left_sig = left.required_params.clone();
    let mut right_sig = right.required_params.clone();
    left_sig.sort();
    right_sig.sort();
    left_sig == right_sig
}

fn has_plane_overlap(left: &VerbMeta, right: &VerbMeta) -> bool {
    left.planes
        .iter()
        .any(|plane| right.planes.iter().any(|candidate| candidate == plane))
}

fn has_action_overlap(left: &VerbMeta, right: &VerbMeta) -> bool {
    left.action_tags
        .iter()
        .any(|tag| right.action_tags.iter().any(|candidate| candidate == tag))
}

fn classify_clash_kind(left: &VerbMeta, right: &VerbMeta) -> ClashKind {
    let left_action = primary_action(left);
    let right_action = primary_action(right);
    if left_action != right_action {
        return ClashKind::ActionDifferentiable;
    }

    if is_stateful_action(left_action) || left.harm_class != right.harm_class {
        return ClashKind::StateDifferentiable;
    }

    let same_tags = left.action_tags == right.action_tags;
    let same_harm = left.harm_class == right.harm_class;
    if same_tags && same_harm {
        return ClashKind::Synonymous;
    }

    ClashKind::ContextDifferentiable
}

fn primary_action(meta: &VerbMeta) -> &str {
    meta.verb_name
        .split(['-', '.'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(meta.verb_name.as_str())
}

fn is_stateful_action(action: &str) -> bool {
    matches!(
        action,
        "submit"
            | "amend"
            | "approve"
            | "reject"
            | "verify"
            | "activate"
            | "deactivate"
            | "archive"
            | "publish"
            | "reopen"
            | "close"
    )
}

fn intersect_strings(
    left: impl IntoIterator<Item = String>,
    right: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let right = right.into_iter().collect::<std::collections::BTreeSet<_>>();
    left.into_iter()
        .filter(|value| right.contains(value))
        .collect()
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sage::{IntentPolarity, ObservationPlane, VerbMeta};
    use dsl_core::config::types::ActionClass;

    fn sample_meta(fqn: &str, action_tags: &[&str], required_params: &[&str]) -> VerbMeta {
        let (domain, verb_name) = fqn.split_once('.').unwrap();
        VerbMeta {
            fqn: fqn.to_string(),
            domain: domain.to_string(),
            verb_name: verb_name.to_string(),
            polarity: IntentPolarity::Read,
            side_effects: Some("facts_only".to_string()),
            harm_class: HarmClass::ReadOnly,
            action_class: ActionClass::Read,
            subject_kinds: vec![],
            phase_tags: vec![],
            requires_subject: true,
            planes: vec![ObservationPlane::Instance],
            action_tags: action_tags.iter().map(|value| value.to_string()).collect(),
            param_names: required_params
                .iter()
                .map(|value| value.to_string())
                .collect(),
            required_params: required_params
                .iter()
                .map(|value| value.to_string())
                .collect(),
            description: "sample".to_string(),
        }
    }

    #[test]
    fn clash_matrix_only_includes_matching_signatures() {
        let index = VerbMetadataIndex::from_test_map(
            [
                sample_meta("deal.list", &["deal", "list"], &["client-id"]),
                sample_meta("deal.search-records", &["deal", "search"], &["query"]),
                sample_meta("deal.report", &["deal", "report"], &["client-id"]),
            ]
            .into_iter()
            .map(|meta| (meta.fqn.clone(), meta))
            .collect(),
        );

        let rows = build_clash_matrix(&index);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].verb_a, "deal.list");
        assert_eq!(rows[0].verb_b, "deal.report");
        assert_eq!(rows[0].clash_kind, ClashKind::ActionDifferentiable);
    }
}
