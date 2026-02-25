//! BootstrapReport formatting utilities.
//!
//! Produces a human-readable summary of what the bootstrap seed wrote.

use super::manifest::OnboardingManifest;
use super::seed::BootstrapReport;
use super::xref::ColumnClassification;

/// Format a BootstrapReport as a multi-line string for terminal output.
pub fn format_bootstrap_report(report: &BootstrapReport) -> String {
    let mut lines = Vec::new();

    lines.push("── Bootstrap Report ────────────────────────────────".into());
    lines.push(format!(
        "  AttributeDefs written:          {}",
        report.attribute_defs_written
    ));
    lines.push(format!(
        "  AttributeDefs skipped:          {}",
        report.attribute_defs_skipped
    ));
    lines.push(format!(
        "  VerbContracts written:          {}",
        report.verb_contracts_written
    ));
    lines.push(format!(
        "  VerbContracts skipped:          {}",
        report.verb_contracts_skipped
    ));
    lines.push(format!(
        "  EntityTypeDefs written:         {}",
        report.entity_type_defs_written
    ));
    lines.push(format!(
        "  EntityTypeDefs skipped:         {}",
        report.entity_type_defs_skipped
    ));
    lines.push(format!(
        "  RelationshipTypeDefs written:   {}",
        report.relationship_type_defs_written
    ));
    lines.push(format!(
        "  RelationshipTypeDefs skipped:   {}",
        report.relationship_type_defs_skipped
    ));
    lines.push(format!(
        "  Total snapshots:                {}",
        report.total_written()
    ));

    lines.join("\n")
}

/// Format a manifest summary showing what WOULD be seeded.
pub fn format_seed_preview(manifest: &OnboardingManifest) -> String {
    let mut lines = Vec::new();

    let seedable_attrs = manifest
        .attribute_candidates
        .iter()
        .filter(|c| {
            matches!(
                c.classification,
                ColumnClassification::VerbConnected | ColumnClassification::OperationalOrphan
            )
        })
        .count();

    lines.push("── Bootstrap Seed Preview ──────────────────────────".into());
    lines.push(format!(
        "  AttributeDefs to seed:          {}",
        seedable_attrs
    ));
    lines.push(format!(
        "  VerbContracts to seed:          {}",
        manifest.verb_extracts.len()
    ));
    lines.push(format!(
        "  EntityTypeDefs to seed:         {}",
        manifest.entity_type_candidates.len()
    ));
    lines.push(format!(
        "  RelationshipTypeDefs to seed:   {}",
        manifest.relationship_candidates.len()
    ));

    lines.push(String::new());
    lines.push("  NOT seeded (Phase B2+):".into());
    lines.push(
        "    PolicyRules, EvidenceRequirements, TaxonomyMemberships,".into(),
    );
    lines.push("    SecurityLabels, Templates, VerbBindings".into());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bootstrap_report() {
        let report = BootstrapReport {
            attribute_defs_written: 10,
            attribute_defs_skipped: 5,
            verb_contracts_written: 20,
            verb_contracts_skipped: 3,
            entity_type_defs_written: 8,
            entity_type_defs_skipped: 2,
            relationship_type_defs_written: 15,
            relationship_type_defs_skipped: 1,
        };

        let output = format_bootstrap_report(&report);
        assert!(output.contains("AttributeDefs written:          10"));
        assert!(output.contains("Total snapshots:                53"));
    }
}
