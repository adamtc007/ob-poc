//! Export the deterministic Coder clash matrix.
//!
//! Usage:
//!   RUSTC_WRAPPER= cargo test -p ob-poc --test coder_clash_matrix -- --ignored --nocapture

use std::fs;
use std::path::{Path, PathBuf};

use ob_poc::sage::{build_clash_matrix, render_clash_reports, VerbMetadataIndex};

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/coder-clash-matrix")
}

#[test]
#[ignore]
fn export_coder_clash_matrix() -> anyhow::Result<()> {
    let index = VerbMetadataIndex::load()?;
    let rows = build_clash_matrix(&index);
    let (csv, markdown) = render_clash_reports(&rows)?;

    fs::create_dir_all(output_dir())?;
    fs::write(output_dir().join("clash_matrix.csv"), csv)?;
    fs::write(output_dir().join("clash_matrix.md"), markdown)?;

    let mut by_domain = std::collections::BTreeMap::<String, usize>::new();
    let mut by_kind = std::collections::BTreeMap::<String, usize>::new();
    for row in &rows {
        *by_domain.entry(row.domain.clone()).or_default() += 1;
        *by_kind.entry(format!("{:?}", row.clash_kind)).or_default() += 1;
    }

    println!("clash_pairs={}", rows.len());
    for (kind, count) in by_kind {
        println!("kind.{kind}: {count}");
    }
    for (domain, count) in by_domain {
        println!("{domain}: {count}");
    }
    println!("csv: {}", output_dir().join("clash_matrix.csv").display());
    println!("md: {}", output_dir().join("clash_matrix.md").display());
    Ok(())
}
