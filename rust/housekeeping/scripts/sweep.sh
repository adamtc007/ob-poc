
#!/usr/bin/env bash
set -euo pipefail

OUT="target/housekeeping"
mkdir -p "$OUT"

echo "== Rust version & workspace =="
rustc -V || true
cargo -V || true
echo "workspace members:" | tee "$OUT/workspace.txt"
cargo metadata --no-deps --format-version 1 >> "$OUT/workspace.txt" || true

echo "== Unused deps (machete) =="
# Fast heuristic. May report false positives.
cargo machete > "$OUT/machete.txt" || true

echo "== Unused deps (udeps, JSON) =="
# More precise; may need nightly depending on project.
cargo udeps --workspace --all-targets --output json > "$OUT/udeps.json" || true

echo "== Workspace unused pub items =="
# Prefer warnalyzer; fallback to cargo-workspace-unused-pub if warnalyzer missing.
if command -v warnalyzer >/dev/null 2>&1; then
  warnalyzer --workspace --all-features --all-targets > "$OUT/warnalyzer.txt" || true
elif command -v cargo-workspace-unused-pub >/dev/null 2>&1; then
  cargo workspace-unused-pub --workspace > "$OUT/warnalyzer.txt" || true
else
  echo "WARN: warnalyzer and cargo-workspace-unused-pub not installed." | tee "$OUT/warnalyzer.txt"
fi

echo "== Callgraph (dot) =="
cargo callgraph --dev-deps --bin --test --bench --example --output "$OUT/callgraph.dot" || true

echo "== Coverage (llvm-cov) =="
cargo llvm-cov --workspace --all-features --lcov --output-path "$OUT/lcov.info" || true
cargo llvm-cov --workspace --all-features --summary-only | tee "$OUT/coverage_summary.txt" || true

echo "== Feature matrix (hack) =="
cargo hack check --workspace --each-feature | tee "$OUT/feature_matrix.txt" || true

echo "Sweep complete. Outputs in $OUT"
