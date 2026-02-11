//! BPMN-Lite service build/test/deploy automation
//!
//! The bpmn-lite service is a standalone workspace at `bpmn-lite/` (repo root),
//! NOT inside the `rust/` workspace. Commands change directory accordingly.

use anyhow::{Context, Result};
use xshell::{cmd, Shell};

use super::project_root;

/// Build the bpmn-lite workspace.
pub fn build(sh: &Shell, release: bool) -> Result<()> {
    let bpmn_dir = project_root()?.join("bpmn-lite");
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
pub fn test(sh: &Shell, filter: Option<&str>) -> Result<()> {
    let bpmn_dir = project_root()?.join("bpmn-lite");
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
pub fn clippy(sh: &Shell) -> Result<()> {
    let bpmn_dir = project_root()?.join("bpmn-lite");
    sh.change_dir(&bpmn_dir);

    println!("Running clippy on bpmn-lite...");
    cmd!(sh, "cargo clippy --workspace -- -D warnings")
        .run()
        .context("bpmn-lite clippy failed")?;
    println!("bpmn-lite clippy passed.");
    Ok(())
}

/// Build the Docker image for bpmn-lite.
pub fn docker_build(sh: &Shell) -> Result<()> {
    let root = project_root()?;
    sh.change_dir(&root);

    println!("Building bpmn-lite Docker image...");
    cmd!(sh, "docker build -t bpmn-lite ./bpmn-lite")
        .run()
        .context("Failed to build bpmn-lite Docker image")?;
    println!("bpmn-lite Docker image built successfully.");
    Ok(())
}

/// Deploy bpmn-lite via docker compose.
pub fn deploy(sh: &Shell, build_image: bool) -> Result<()> {
    let root = project_root()?;
    sh.change_dir(&root);

    if build_image {
        docker_build(sh)?;
    }

    println!("Starting bpmn-lite via docker compose...");
    cmd!(sh, "docker compose up -d bpmn-lite")
        .run()
        .context("Failed to start bpmn-lite via docker compose")?;
    println!("bpmn-lite is running on port 50053 (gRPC).");
    Ok(())
}

/// Start the bpmn-lite gRPC server natively (release build, background process).
pub fn start(sh: &Shell, port: u16) -> Result<()> {
    let bpmn_dir = project_root()?.join("bpmn-lite");

    // Stop any existing instance first
    stop_inner(sh, port);

    // Build release
    sh.change_dir(&bpmn_dir);
    println!("Building bpmn-lite (release)...");
    cmd!(sh, "cargo build --release -p bpmn-lite-server")
        .run()
        .context("Failed to build bpmn-lite-server")?;

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
pub fn stop(sh: &Shell, port: u16) -> Result<()> {
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
pub fn status(sh: &Shell, port: u16) -> Result<()> {
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
