//! BPMN-Lite service build/test/deploy automation
//!
//! bpmn-lite is a separate repo (github.com/adamtc007/bpmn-lite), not a
//! subdirectory of ob-poc. Every command here operates against a local
//! checkout located via the `BPMN_LITE_DIR` env var — see
//! `resolve_bpmn_lite_dir`.

use anyhow::{Context, Result};
use xshell::{cmd, Shell};

use super::project_root;

/// Locate the local bpmn-lite checkout via `BPMN_LITE_DIR`. Fails loud
/// (never guesses a path) since bpmn-lite is an external sibling repo with
/// no fixed location relative to ob-poc.
fn resolve_bpmn_lite_dir() -> Result<std::path::PathBuf> {
    let dir = std::env::var("BPMN_LITE_DIR").context(
        "BPMN_LITE_DIR is not set. bpmn-lite is a separate repo \
         (github.com/adamtc007/bpmn-lite) — clone it, then \
         `export BPMN_LITE_DIR=/path/to/your/bpmn-lite/checkout`.",
    )?;
    let path = std::path::PathBuf::from(dir);
    if !path.join("Cargo.toml").is_file() {
        anyhow::bail!(
            "BPMN_LITE_DIR={:?} does not look like a bpmn-lite checkout (no Cargo.toml there).",
            path
        );
    }
    Ok(path)
}

/// Build the bpmn-lite workspace.
pub(crate) fn build(sh: &Shell, release: bool) -> Result<()> {
    let bpmn_dir = resolve_bpmn_lite_dir()?;
    sh.change_dir(&bpmn_dir);

    println!("Building bpmn-lite...");
    if release {
        cmd!(sh, "cargo build --release --workspace")
            .run()
            .context("Failed to build bpmn-lite (release)")?;
    } else {
        cmd!(sh, "cargo build --workspace")
            .run()
            .context("Failed to build bpmn-lite")?;
    }
    println!("bpmn-lite build complete.");
    Ok(())
}

/// Run all tests in the bpmn-lite workspace.
pub(crate) fn test(sh: &Shell, filter: Option<&str>) -> Result<()> {
    let bpmn_dir = resolve_bpmn_lite_dir()?;
    sh.change_dir(&bpmn_dir);

    println!("Running bpmn-lite tests...");
    if let Some(f) = filter {
        cmd!(sh, "cargo test --workspace -- {f}")
            .run()
            .context("bpmn-lite tests failed")?;
    } else {
        cmd!(sh, "cargo test --workspace")
            .run()
            .context("bpmn-lite tests failed")?;
    }
    println!("All bpmn-lite tests passed.");
    Ok(())
}

/// Run clippy on the bpmn-lite workspace.
pub(crate) fn clippy(sh: &Shell) -> Result<()> {
    let bpmn_dir = resolve_bpmn_lite_dir()?;
    sh.change_dir(&bpmn_dir);

    println!("Running clippy on bpmn-lite...");
    cmd!(sh, "cargo clippy --workspace -- -D warnings")
        .run()
        .context("bpmn-lite clippy failed")?;
    println!("bpmn-lite clippy passed.");
    Ok(())
}

/// Build the Docker image for bpmn-lite.
pub(crate) fn docker_build(sh: &Shell) -> Result<()> {
    let bpmn_dir = resolve_bpmn_lite_dir()?;
    let root = project_root()?;
    sh.change_dir(&root);

    println!("Building bpmn-lite Docker image from {:?}...", bpmn_dir);
    let bpmn_dir_str = bpmn_dir.to_string_lossy().to_string();
    cmd!(sh, "docker build -t bpmn-lite {bpmn_dir_str}")
        .run()
        .context("Failed to build bpmn-lite Docker image")?;
    println!("bpmn-lite Docker image built successfully.");
    Ok(())
}

/// Deploy bpmn-lite via docker compose. Requires `BPMN_LITE_DIR` — passed
/// through to docker-compose.yml's `bpmn-lite` service, whose build context
/// is `${BPMN_LITE_DIR}` (it can no longer be a fixed relative path since
/// bpmn-lite isn't a subdirectory of this repo).
pub(crate) fn deploy(sh: &Shell, build_image: bool) -> Result<()> {
    let bpmn_dir = resolve_bpmn_lite_dir()?;
    let root = project_root()?;
    sh.change_dir(&root);

    if build_image {
        docker_build(sh)?;
    }

    println!("Starting bpmn-lite via docker compose...");
    let bpmn_dir_str = bpmn_dir.to_string_lossy().to_string();
    let bash_cmd = format!("BPMN_LITE_DIR={bpmn_dir_str} docker compose up -d bpmn-lite");
    cmd!(sh, "bash -c {bash_cmd}")
        .run()
        .context("Failed to start bpmn-lite via docker compose")?;
    println!("bpmn-lite is running on port 50053 (gRPC).");
    Ok(())
}

/// Start the bpmn-lite gRPC server natively (release build, background process).
pub(crate) fn start(sh: &Shell, port: u16) -> Result<()> {
    let bpmn_dir = resolve_bpmn_lite_dir()?;

    // Stop any existing instance first
    stop_inner(sh, port);

    // Build release. Package is `bpmn-lite-server-runner`; it produces the
    // `bpmn-lite-server` binary this function starts below.
    sh.change_dir(&bpmn_dir);
    println!("Building bpmn-lite (release)...");
    cmd!(sh, "cargo build --release -p bpmn-lite-server-runner")
        .run()
        .context("Failed to build bpmn-lite-server-runner")?;

    let binary = bpmn_dir.join("target/release/bpmn-lite-server");
    if !binary.exists() {
        anyhow::bail!("Binary not found at {:?}", binary);
    }

    let binary_str = binary.to_string_lossy().to_string();

    println!("Starting bpmn-lite on port {}...", port);
    // Start as background process, redirect output to log file
    let log_file = bpmn_dir.join("bpmn-lite.log");
    let log_str = log_file.to_string_lossy().to_string();
    let bash_cmd = format!(
        "BPMN_LITE_PORT={} RUST_LOG=info nohup {} > {} 2>&1 &",
        port, binary_str, log_str
    );
    cmd!(sh, "bash -c {bash_cmd}")
        .run()
        .context("Failed to start bpmn-lite server")?;

    // Wait briefly and check it started
    std::thread::sleep(std::time::Duration::from_millis(500));

    let port_str = port.to_string();
    let check = cmd!(sh, "lsof -ti:{port_str}").read();
    match check {
        Ok(pids) if !pids.trim().is_empty() => {
            println!(
                "bpmn-lite gRPC server running on port {} (PID: {})",
                port,
                pids.trim()
            );
            println!("Log file: {}", log_str);
        }
        _ => {
            // Show log tail for diagnostics
            let log_tail = std::fs::read_to_string(&log_file).unwrap_or_default();
            let last_lines: Vec<&str> = log_tail.lines().rev().take(10).collect();
            println!("Warning: server may not have started. Log tail:");
            for line in last_lines.iter().rev() {
                println!("  {}", line);
            }
        }
    }
    Ok(())
}

/// Stop the bpmn-lite gRPC server.
pub(crate) fn stop(sh: &Shell, port: u16) -> Result<()> {
    stop_inner(sh, port);
    Ok(())
}

fn stop_inner(sh: &Shell, port: u16) {
    // Kill by process name
    let _ = cmd!(sh, "pkill -f bpmn-lite-server").run();

    // Also kill anything on the target port
    let port_str = port.to_string();
    let _ = cmd!(sh, "lsof -ti:{port_str}").read().map(|pids| {
        for pid in pids.lines() {
            if !pid.trim().is_empty() {
                let _ = cmd!(sh, "kill {pid}").run();
            }
        }
    });

    std::thread::sleep(std::time::Duration::from_millis(300));
    println!("bpmn-lite stopped.");
}

/// Show status of the bpmn-lite service (native and Docker).
pub(crate) fn status(sh: &Shell, port: u16) -> Result<()> {
    let port_str = port.to_string();

    println!("=== bpmn-lite service status ===\n");

    // Check native process
    let native = cmd!(sh, "lsof -ti:{port_str}").read();
    match native {
        Ok(pids) if !pids.trim().is_empty() => {
            println!("Native:  RUNNING on port {} (PID: {})", port, pids.trim());
        }
        _ => {
            println!("Native:  NOT RUNNING on port {}", port);
        }
    }

    // Check Docker container
    let docker_fmt = "{{.Status}}";
    let docker = cmd!(
        sh,
        "docker ps --filter name=bpmn-lite --format {docker_fmt}"
    )
    .read();
    match docker {
        Ok(status) if !status.trim().is_empty() => {
            println!("Docker:  {} (port 50053 → 50051)", status.trim());
        }
        _ => {
            println!("Docker:  NOT RUNNING");
        }
    }

    Ok(())
}
