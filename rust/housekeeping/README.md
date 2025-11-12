
# Rust Housekeeping Sweep — Dead Code & Debris Cleanup

This bundle wires up a repeatable **dead-code sweep** for a large Rust workspace:
- Unused dependencies (fast+precise)
- Unused **pub** items across the workspace
- Callgraph snapshot (for orphan islands)
- Coverage summary (flag 0% modules)
- A ranked **Housekeeping Report** with delete/demote/keep recommendations

> Run the sweep locally, then generate the report. Drop both steps into CI for drift control.

## Quickstart

```bash
# 1) (Optional) Install tools once
bash housekeeping/scripts/install_tools.sh

# 2) Run the sweep (writes outputs under target/housekeeping/)
bash housekeeping/scripts/sweep.sh

# 3) Generate the markdown report
python3 housekeeping/scripts/report.py

# 4) Open the report
$EDITOR target/housekeeping/housekeeping_report.md
```

## Outputs (under `target/housekeeping/`)
- `machete.txt`  — cargo-machete output
- `udeps.json`   — cargo-udeps (JSON)
- `warnalyzer.txt` — workspace-wide unused `pub` signals (warnalyzer or fallback tool)
- `callgraph.dot`  — cargo-callgraph DOT graph (optional)
- `lcov.info`      — coverage (cargo-llvm-cov)
- `coverage_summary.txt` — llvm-cov summary-only output
- `feature_matrix.txt`   — cargo-hack per-feature check output
- `housekeeping_report.md` — the ranked report (generated)

## Notes
- The scripts try to be resilient to missing tools: they’ll skip a section and mark it as “not available” in the report.
- For accuracy, run with **all targets & features**. The sweep does so by default.
- For CI, see `.github/workflows/housekeeping.yml`.

## Tools referenced
- cargo-machete, cargo-udeps, warnalyzer (or cargo-workspace-unused-pub), cargo-callgraph, cargo-llvm-cov, cargo-hack
