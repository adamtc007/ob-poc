use anyhow::{Context, Result};
use ob_poc::acp_pack_context_envelope_v2::build_acp_pack_context_artifact_bytes_v2;
use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
use std::io::Write;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Phase 3D of capability-crate restructure (2026-05-13): register
    // both pack provider hooks via the shared helper.
    ob_poc::journey::providers::register_pack_providers();
    let mut args = std::env::args_os().skip(1);
    let config_root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config"));
    let pack_id = args
        .next()
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "all".to_string());
    let projection = build_slice1_acp_registry_projection(&config_root)
        .with_context(|| format!("building projection from {}", config_root.display()))?;
    let output = build_acp_pack_context_artifact_bytes_v2(&projection, &config_root, &pack_id)
        .with_context(|| {
            format!("building deterministic ACP pack context artifact for {pack_id}")
        })?;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle
        .write_all(&output)
        .context("writing ACP pack context envelope output")?;
    Ok(())
}
