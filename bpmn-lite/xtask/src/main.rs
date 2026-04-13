use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_help();
        bail!("missing xtask subcommand");
    }

    match args[0].as_str() {
        "smoke" => run_profile("smoke", &args[1..]),
        "stress" => run_profile("stress", &args[1..]),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => bail!("unknown xtask subcommand '{}'", other),
    }
}

fn run_profile(profile: &str, extra_args: &[String]) -> Result<()> {
    let workspace_root = workspace_root()?;
    let mut parsed = ParsedArgs::default();
    let mut forward_args = Vec::new();

    let mut i = 0usize;
    while i < extra_args.len() {
        let arg = &extra_args[i];
        match arg.as_str() {
            "--server-url" => {
                let value = extra_args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--server-url requires a value"))?;
                parsed.server_url = Some(value.clone());
                forward_args.push(arg.clone());
                forward_args.push(value.clone());
                i += 2;
            }
            "--database-url" => {
                let value = extra_args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--database-url requires a value"))?;
                parsed.database_url = Some(value.clone());
                i += 2;
            }
            "--spawn-server" => {
                parsed.spawn_server = true;
                i += 1;
            }
            other => {
                forward_args.push(other.to_string());
                i += 1;
            }
        }
    }

    let server_url = parsed.server_url.clone().unwrap_or_else(|| {
        if parsed.spawn_server {
            "http://127.0.0.1:50061".to_string()
        } else {
            "http://127.0.0.1:50051".to_string()
        }
    });

    let server_child = if parsed.spawn_server {
        Some(spawn_server(
            &workspace_root,
            &server_url,
            parsed.database_url.as_deref(),
        )?)
    } else {
        None
    };

    if parsed.spawn_server {
        wait_for_server(&server_url, Duration::from_secs(20))?;
    }

    let status = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("bpmn-lite-server")
        .arg("--bin")
        .arg("load_harness")
        .arg("--")
        .arg("--profile")
        .arg(profile)
        .arg("--server-url")
        .arg(&server_url)
        .args(&forward_args)
        .current_dir(&workspace_root)
        .status()
        .context("failed to run load harness")?;

    if let Some(mut child) = server_child {
        let _ = child.kill();
        let _ = child.wait();
    }

    if !status.success() {
        bail!("load harness exited with {}", status);
    }

    Ok(())
}

fn spawn_server(
    workspace_root: &PathBuf,
    server_url: &str,
    database_url: Option<&str>,
) -> Result<Child> {
    let bind_addr = extract_bind_addr(server_url)?;
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("-p")
        .arg("bpmn-lite-server")
        .arg("--bin")
        .arg("bpmn-lite-server")
        .current_dir(workspace_root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .env("RUST_LOG", "info")
        .env("BPMN_LITE_BIND", &bind_addr);

    if let Some(url) = database_url {
        command.env("DATABASE_URL", url);
    }

    let child = command.spawn().with_context(|| {
        format!(
            "failed to spawn bpmn-lite-server for load harness on {}",
            bind_addr
        )
    })?;
    Ok(child)
}

fn wait_for_server(server_url: &str, timeout: Duration) -> Result<()> {
    let addr = extract_host_port(server_url)?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(&addr).is_ok() {
            return Ok(());
        }
        sleep(Duration::from_millis(200));
    }
    bail!("timed out waiting for BPMN-Lite server at {}", server_url);
}

fn extract_host_port(server_url: &str) -> Result<String> {
    let stripped = server_url
        .strip_prefix("http://")
        .or_else(|| server_url.strip_prefix("https://"))
        .ok_or_else(|| anyhow!("server url must start with http:// or https://"))?;
    Ok(stripped.to_string())
}

fn extract_port(server_url: &str) -> Result<u16> {
    let host_port = extract_host_port(server_url)?;
    let port = host_port
        .rsplit(':')
        .next()
        .ok_or_else(|| anyhow!("missing port in server url"))?;
    port.parse::<u16>()
        .with_context(|| format!("invalid port in {}", server_url))
}

fn extract_bind_addr(server_url: &str) -> Result<String> {
    Ok(format!("0.0.0.0:{}", extract_port(server_url)?))
}

fn workspace_root() -> Result<PathBuf> {
    std::env::current_dir().context("failed to read current directory")
}

fn print_help() {
    eprintln!(
        "Usage:
  cargo run -p xtask -- smoke [--spawn-server] [--database-url URL] [harness args...]
  cargo run -p xtask -- stress [--spawn-server] [--database-url URL] [harness args...]

Examples:
  cargo run -p xtask -- smoke --spawn-server
  cargo run -p xtask -- stress --spawn-server --instances 500 --workers 24
  cargo run -p xtask -- stress --server-url http://127.0.0.1:50051 --timeout-secs 180"
    );
}

#[derive(Default)]
struct ParsedArgs {
    server_url: Option<String>,
    database_url: Option<String>,
    spawn_server: bool,
}
