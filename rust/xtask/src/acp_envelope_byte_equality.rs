//! R6 — PR byte-equality CI gate for ACP pack context envelopes.
//!
//! Rebuilds the Slice 1 envelopes via the deterministic CLI
//! (`cargo run --bin acp_pack_context_envelope_v2 -- config <pack>`),
//! compares the byte count and SHA-256 against the persisted baseline in
//! `tools/acp_envelope_baseline_v3.json`, and fails the build on drift.
//!
//! Bump the baseline via `cargo run -p xtask -- acp-envelope-byte-equality-check
//! --bless` after reviewing the intentional envelope change.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;

const BASELINE_PATH: &str = "tools/acp_envelope_baseline_v3.json";
const SCHEMA_VERSION: &str = "acp_pack_context_envelope_v3";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Baseline {
    #[serde(rename = "_comment", default, skip_serializing_if = "Option::is_none")]
    comment: Option<String>,
    schema_version: String,
    captured_at: String,
    captured_under: String,
    entries: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaselineEntry {
    config: String,
    bytes: usize,
    sha256: String,
}

pub(crate) fn run(bless: bool) -> Result<()> {
    let workspace_root = workspace_root()?;
    let baseline_path = workspace_root.join(BASELINE_PATH);

    // Rebuild current envelopes.
    let configs = ["all", "onboarding-request", "cbu-maintenance", "product-service-taxonomy"];
    let mut current_entries: Vec<BaselineEntry> = Vec::new();
    for config in configs {
        let output = run_envelope_cli(&workspace_root, config)?;
        let bytes = output.len();
        let mut hasher = Sha256::new();
        hasher.update(output.as_bytes());
        let sha256 = format!("{:x}", hasher.finalize());
        current_entries.push(BaselineEntry {
            config: config.to_string(),
            bytes,
            sha256,
        });
    }

    if bless {
        let new_baseline = Baseline {
            comment: Some(
                "R6 byte-equality baseline for ACP pack context envelopes. \
                 Bump via `cargo run -p xtask -- acp-envelope-byte-equality-check --bless` \
                 after reviewing intentional envelope changes."
                    .to_string(),
            ),
            schema_version: SCHEMA_VERSION.to_string(),
            captured_at: today_iso(),
            captured_under: "blessed via xtask --bless".to_string(),
            entries: current_entries,
        };
        let serialized = serde_json::to_string_pretty(&new_baseline)
            .context("serializing refreshed baseline")?;
        std::fs::write(&baseline_path, serialized)
            .with_context(|| format!("writing baseline to {}", baseline_path.display()))?;
        println!(
            "ACP envelope byte-equality baseline refreshed at {}",
            baseline_path.display()
        );
        return Ok(());
    }

    // Load + compare.
    let baseline_bytes = std::fs::read(&baseline_path)
        .with_context(|| format!("reading baseline {}", baseline_path.display()))?;
    let baseline: Baseline = serde_json::from_slice(&baseline_bytes)
        .with_context(|| format!("parsing baseline {}", baseline_path.display()))?;

    if baseline.schema_version != SCHEMA_VERSION {
        bail!(
            "baseline schema_version mismatch: expected {} got {}. \
             Re-bless after a deliberate schema bump.",
            SCHEMA_VERSION,
            baseline.schema_version,
        );
    }

    let mut drifted: Vec<String> = Vec::new();
    for entry in &current_entries {
        let baseline_entry = baseline.entries.iter().find(|e| e.config == entry.config);
        match baseline_entry {
            Some(b) if b.bytes == entry.bytes && b.sha256 == entry.sha256 => {
                // Match — no drift.
            }
            Some(b) => {
                drifted.push(format!(
                    "  {}\n    expected: bytes={} sha256={}\n    actual:   bytes={} sha256={}",
                    entry.config, b.bytes, b.sha256, entry.bytes, entry.sha256
                ));
            }
            None => {
                drifted.push(format!(
                    "  {} — missing from baseline (current: bytes={} sha256={})",
                    entry.config, entry.bytes, entry.sha256
                ));
            }
        }
    }

    // Detect entries in baseline but missing from current build (e.g. pack removed).
    for baseline_entry in &baseline.entries {
        if !current_entries.iter().any(|c| c.config == baseline_entry.config) {
            drifted.push(format!(
                "  {} — present in baseline but not in current build",
                baseline_entry.config
            ));
        }
    }

    if drifted.is_empty() {
        println!(
            "ACP envelope byte-equality clean: {} entries match baseline at {}",
            current_entries.len(),
            baseline_path.display()
        );
        Ok(())
    } else {
        bail!(
            "ACP envelope byte-equality drift detected ({} entr{}):\n{}\n\
             If the drift is intentional, review the change and run:\n  \
             cargo run -p xtask -- acp-envelope-byte-equality-check --bless",
            drifted.len(),
            if drifted.len() == 1 { "y" } else { "ies" },
            drifted.join("\n"),
        )
    }
}

fn run_envelope_cli(workspace_root: &std::path::Path, config: &str) -> Result<String> {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--release")
        .arg("--bin")
        .arg("acp_pack_context_envelope_v2")
        .arg("--")
        .arg("config")
        .arg(config)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("invoking envelope CLI for config={}", config))?;
    if !output.status.success() {
        bail!(
            "envelope CLI failed for config={}: status={:?} stderr={}",
            config,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
        );
    }
    String::from_utf8(output.stdout)
        .with_context(|| format!("envelope CLI stdout not UTF-8 for config={}", config))
}

fn workspace_root() -> Result<PathBuf> {
    // Resolve via Cargo manifest dir of xtask, then ascend one level.
    let xtask_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    xtask_manifest
        .parent()
        .map(PathBuf::from)
        .context("xtask CARGO_MANIFEST_DIR has no parent — unexpected workspace layout")
}

fn today_iso() -> String {
    // Avoid pulling in chrono just for this — use a coarse format.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    // Approximate YYYY-MM-DD from Unix days since 1970-01-01.
    // This is a coarse placeholder; reviewers update the captured_at
    // field if precision matters.
    let (y, m, d) = unix_days_to_ymd(days as i64);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn unix_days_to_ymd(mut days: i64) -> (i32, u32, u32) {
    // Civil-from-days algorithm by Howard Hinnant.
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}
