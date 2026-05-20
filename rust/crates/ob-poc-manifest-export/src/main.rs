//! Manifest export CLI — emits the v0.6 §7 manifest for ob-poc.
//!
//! Default invocation (run from the ob-poc repo root):
//!
//! ```text
//! cargo run -p ob-poc-manifest-export -- \
//!     --verbs-dir rust/config/verbs \
//!     --allowlist rust/config/manifest-allowlist.yaml \
//!     --output bpmn-lite/manifests/ob-poc-v1.0.0.yaml \
//!     --catalogue-version v1.0.0
//! ```

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about = "Emit ob-poc catalogue manifest (v0.6 §7)")]
struct Cli {
    /// Directory containing per-domain verb YAML files
    /// (typically `rust/config/verbs`).
    #[arg(long)]
    verbs_dir: PathBuf,

    /// Publication allowlist YAML (per v0.6 §7.5).
    #[arg(long)]
    allowlist: PathBuf,

    /// Output manifest path.
    #[arg(long)]
    output: PathBuf,

    /// Domain id this manifest publishes — typically `ob-poc`.
    #[arg(long, default_value = "ob-poc")]
    domain: String,

    /// Catalogue version stamped into the manifest.
    #[arg(long, default_value = "v1.0.0")]
    catalogue_version: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let written = ob_poc_manifest_export::export_to_path(
        &cli.verbs_dir,
        &cli.allowlist,
        &cli.output,
        &cli.domain,
        &cli.catalogue_version,
    )?;
    println!("wrote manifest to {}", written.display());
    Ok(())
}
