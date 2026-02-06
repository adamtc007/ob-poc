//! Pack Playback — Pack-level summary and chapter view generation
//!
//! Produces human-readable summaries of a runbook, grouped by pack section
//! layout when available.

use std::collections::HashMap;

use crate::journey::pack::PackManifest;
use crate::repl::runbook::Runbook;

// ---------------------------------------------------------------------------
// ChapterView
// ---------------------------------------------------------------------------

/// A chapter in a runbook summary — groups related steps for display.
#[derive(Debug, Clone)]
pub struct ChapterView {
    pub chapter: String,
    pub steps: Vec<(i32, String)>, // (sequence, sentence)
}

// ---------------------------------------------------------------------------
// PackPlayback
// ---------------------------------------------------------------------------

/// Generates pack-level playback summaries.
pub struct PackPlayback;

impl PackPlayback {
    /// Produce a one-paragraph summary of the runbook within a pack context.
    pub fn summarize(
        pack: &PackManifest,
        runbook: &Runbook,
        answers: &HashMap<String, serde_json::Value>,
    ) -> String {
        // Use pack_summary_template if available.
        if let Some(ref template) = pack.pack_summary_template {
            let mut summary = template.clone();
            for (key, value) in answers {
                let placeholder = format!("{{{}}}", key);
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Array(arr) => arr
                        .iter()
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    other => other.to_string(),
                };
                summary = summary.replace(&placeholder, &value_str);
            }
            summary = summary.replace("{step_count}", &runbook.entries.len().to_string());
            return summary;
        }

        // Fallback: auto-generate summary.
        let step_count = runbook.entries.len();
        let verbs: Vec<_> = runbook
            .entries
            .iter()
            .map(|e| e.verb.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        format!(
            "{} — {} steps across {} verbs: {}",
            pack.name,
            step_count,
            verbs.len(),
            verbs.join(", ")
        )
    }

    /// Group runbook entries into chapters based on pack section layout.
    pub fn chapter_view(pack: &PackManifest, runbook: &Runbook) -> Vec<ChapterView> {
        if pack.section_layout.is_empty() {
            // No section layout — single "Steps" chapter.
            return vec![ChapterView {
                chapter: "Steps".to_string(),
                steps: runbook
                    .entries
                    .iter()
                    .map(|e| (e.sequence, e.sentence.clone()))
                    .collect(),
            }];
        }

        let mut chapters: Vec<ChapterView> = Vec::new();
        let mut assigned: Vec<bool> = vec![false; runbook.entries.len()];

        // Assign entries to sections by verb prefix.
        for section in &pack.section_layout {
            let mut steps = Vec::new();
            for (i, entry) in runbook.entries.iter().enumerate() {
                if !assigned[i]
                    && section
                        .verb_prefixes
                        .iter()
                        .any(|prefix| entry.verb.starts_with(prefix))
                {
                    steps.push((entry.sequence, entry.sentence.clone()));
                    assigned[i] = true;
                }
            }
            if !steps.is_empty() {
                chapters.push(ChapterView {
                    chapter: section.title.clone(),
                    steps,
                });
            }
        }

        // Collect unassigned entries into "Other" chapter.
        let other_steps: Vec<_> = runbook
            .entries
            .iter()
            .enumerate()
            .filter(|(i, _)| !assigned[*i])
            .map(|(_, e)| (e.sequence, e.sentence.clone()))
            .collect();

        if !other_steps.is_empty() {
            chapters.push(ChapterView {
                chapter: "Other".to_string(),
                steps: other_steps,
            });
        }

        chapters
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey::pack::load_pack_from_bytes;
    use crate::repl::runbook::{Runbook, RunbookEntry};
    use uuid::Uuid;

    fn make_pack_with_sections() -> PackManifest {
        let yaml = r#"
id: test-pack
name: Test Pack
version: "1.0"
description: Test pack with sections
section_layout:
  - title: "Setup"
    verb_prefixes: ["cbu.create", "cbu.assign"]
  - title: "Trading"
    verb_prefixes: ["trading-profile"]
  - title: "KYC"
    verb_prefixes: ["kyc"]
"#;
        let (manifest, _) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
        manifest
    }

    fn make_pack_with_summary_template() -> PackManifest {
        let yaml = r#"
id: test-pack
name: Test Pack
version: "1.0"
description: Test pack
pack_summary_template: "Onboarding {client} in {jurisdiction} with {step_count} steps, products: {products}"
"#;
        let (manifest, _) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
        manifest
    }

    fn sample_runbook() -> Runbook {
        let mut rb = Runbook::new(Uuid::new_v4());
        rb.add_entry(RunbookEntry::new(
            "cbu.create".to_string(),
            "Create Allianz Lux CBU".to_string(),
            "(cbu.create :name \"Allianz Lux\")".to_string(),
        ));
        rb.add_entry(RunbookEntry::new(
            "cbu.assign-product".to_string(),
            "Add IRS product".to_string(),
            "(cbu.assign-product :product \"IRS\")".to_string(),
        ));
        rb.add_entry(RunbookEntry::new(
            "trading-profile.create".to_string(),
            "Create trading profile".to_string(),
            "(trading-profile.create)".to_string(),
        ));
        rb.add_entry(RunbookEntry::new(
            "kyc.open-case".to_string(),
            "Open KYC case".to_string(),
            "(kyc.open-case)".to_string(),
        ));
        rb
    }

    #[test]
    fn test_chapter_view_with_sections() {
        let pack = make_pack_with_sections();
        let runbook = sample_runbook();
        let chapters = PackPlayback::chapter_view(&pack, &runbook);

        assert_eq!(chapters.len(), 3);
        assert_eq!(chapters[0].chapter, "Setup");
        assert_eq!(chapters[0].steps.len(), 2); // cbu.create + cbu.assign-product
        assert_eq!(chapters[1].chapter, "Trading");
        assert_eq!(chapters[1].steps.len(), 1); // trading-profile.create
        assert_eq!(chapters[2].chapter, "KYC");
        assert_eq!(chapters[2].steps.len(), 1); // kyc.open-case
    }

    #[test]
    fn test_chapter_view_without_sections() {
        let yaml = b"id: bare\nname: Bare\nversion: '1'\ndescription: d\n";
        let (pack, _) = load_pack_from_bytes(yaml).unwrap();
        let runbook = sample_runbook();
        let chapters = PackPlayback::chapter_view(&pack, &runbook);

        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].chapter, "Steps");
        assert_eq!(chapters[0].steps.len(), 4);
    }

    #[test]
    fn test_chapter_view_unassigned_go_to_other() {
        let yaml = r#"
id: partial
name: Partial
version: "1"
description: d
section_layout:
  - title: "Setup"
    verb_prefixes: ["cbu"]
"#;
        let (pack, _) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
        let runbook = sample_runbook();
        let chapters = PackPlayback::chapter_view(&pack, &runbook);

        // "Setup" gets cbu.* entries, "Other" gets the rest.
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].chapter, "Setup");
        assert_eq!(chapters[0].steps.len(), 2);
        assert_eq!(chapters[1].chapter, "Other");
        assert_eq!(chapters[1].steps.len(), 2);
    }

    #[test]
    fn test_summarize_with_template() {
        let pack = make_pack_with_summary_template();
        let runbook = sample_runbook();
        let answers = HashMap::from([
            ("client".to_string(), serde_json::json!("Allianz")),
            ("jurisdiction".to_string(), serde_json::json!("LU")),
            ("products".to_string(), serde_json::json!(["IRS", "EQUITY"])),
        ]);

        let summary = PackPlayback::summarize(&pack, &runbook, &answers);
        assert!(summary.contains("Allianz"));
        assert!(summary.contains("LU"));
        assert!(summary.contains("4")); // step_count
        assert!(summary.contains("IRS"));
    }

    #[test]
    fn test_summarize_fallback() {
        let yaml = b"id: bare\nname: Bare Pack\nversion: '1'\ndescription: d\n";
        let (pack, _) = load_pack_from_bytes(yaml).unwrap();
        let runbook = sample_runbook();

        let summary = PackPlayback::summarize(&pack, &runbook, &HashMap::new());
        assert!(summary.contains("Bare Pack"));
        assert!(summary.contains("4 steps"));
    }

    #[test]
    fn test_empty_runbook_summary() {
        let yaml = b"id: bare\nname: Bare\nversion: '1'\ndescription: d\n";
        let (pack, _) = load_pack_from_bytes(yaml).unwrap();
        let runbook = Runbook::new(Uuid::new_v4());

        let summary = PackPlayback::summarize(&pack, &runbook, &HashMap::new());
        assert!(summary.contains("0 steps"));
    }
}
