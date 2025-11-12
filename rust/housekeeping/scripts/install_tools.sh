
#!/usr/bin/env bash
set -euo pipefail

# Install commonly used sweep tools. Adjust versions as needed.
# Note: Some tools may require a recent nightly toolchain.

echo "Installing cargo-machete (unused deps, fast heuristic)..."
cargo install cargo-machete || true

echo "Installing cargo-udeps (unused deps, precise)..."
cargo install cargo-udeps || true

echo "Installing cargo-hack (feature matrix checks)..."
cargo install cargo-hack || true

echo "Installing cargo-llvm-cov (coverage)..."
cargo install cargo-llvm-cov || true

echo "Installing cargo-callgraph (call graph)..."
cargo install cargo-callgraph || true

# Unused public items across workspace — choose one (or both) depending on your preference:
echo "Installing warnalyzer (workspace-wide unused pub analysis)..."
cargo install warnalyzer || true

echo "Installing cargo-workspace-unused-pub (alternative) ..."
cargo install cargo-workspace-unused-pub || true

echo "Done."
