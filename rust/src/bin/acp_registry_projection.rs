use anyhow::{Context, Result};
use ob_poc_boundary::acp_registry_projection::build_slice1_acp_registry_projection;
use std::io::Write;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Phase 3D of capability-crate restructure (2026-05-13): the
    // disk-loading hook lives behind a boundary-side provider; register
    // both pack provider hooks via the shared helper.
    ob_poc::journey::providers::register_pack_providers();
    let config_root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config"));
    let projection = build_slice1_acp_registry_projection(&config_root)
        .with_context(|| format!("building projection from {}", config_root.display()))?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, &projection)
        .context("serializing ACP registry projection")?;
    writeln!(handle).context("writing trailing newline")?;
    Ok(())
}
