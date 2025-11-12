# Dead Code Workflow - Quick Start Guide

## 🚀 TL;DR - Get Started in 2 Minutes

```bash
# 1. Install tools (one-time setup)
./scripts/install-dead-code-tools.sh

# 2. Run comprehensive analysis
./scripts/dead-code-sweep.sh

# 3. Generate ranked report
python3 scripts/generate-report.py

# 4. Review results
open target/housekeeping/housekeeping_report.md
```

## 📋 What This Workflow Finds

| Issue Type | Traditional `cargo clippy` | This Workflow |
|------------|---------------------------|---------------|
| Private dead code within crate | ✅ | ✅ |
| **Orphaned `pub` items across crates** | ❌ | ✅ **NEW** |
| **Unused dependencies after refactors** | ❌ | ✅ **NEW** |
| **Never-executed code paths** | ❌ | ✅ **NEW** |
| **Disconnected code islands** | ❌ | ✅ **NEW** |

## 🎯 Expected Results

For a heavily refactored 50k+ LOC Rust workspace like ob-poc:

- **5-15 unused dependencies** (axum, tonic, prost after rewrites)
- **10-30% of public API items** are workspace-orphaned
- **2-5 disconnected code islands** (old subsystems never removed)
- **15-25% total LOC reduction** after systematic cleanup

## 📁 Files Created

```
ob-poc/
├── COMPREHENSIVE_DEAD_CODE_WORKFLOW.md  # Full documentation
├── DEAD_CODE_WORKFLOW_QUICKSTART.md    # This file
├── scripts/
│   ├── dead-code-sweep.sh              # Main analysis script
│   ├── generate-report.py              # Python report generator
│   └── install-dead-code-tools.sh      # Tool installation
├── .github/workflows/
│   └── dead-code-housekeeping.yml      # CI automation
└── .zed/
    └── tasks.json                       # Agent-friendly tasks
```

## 🛠️ Tool Installation

### Core Tools (Required)
```bash
cargo install cargo-udeps        # Precise unused dependency detection
cargo install cargo-machete      # Fast unused dependency scan
cargo install warnalyzer         # Cross-crate unused public API
cargo install cargo-llvm-cov     # Coverage analysis
cargo install cargo-hack         # Feature matrix validation
cargo install cargo-callgraph    # Call graph generation
```

### Optional Tools (Enhanced Analysis)
```bash
cargo install cargo-public-api   # API surface monitoring
cargo install cargo-semver-checks # Breaking change detection
cargo install cargo-unused-features # Feature flag cleanup
```

## 🏃‍♂️ Quick Commands

| Task | Command | Output |
|------|---------|--------|
| **Full workflow** | `./scripts/dead-code-sweep.sh && python3 scripts/generate-report.py` | `target/housekeeping/` |
| **Ranked report** | `python3 scripts/generate-report.py` | `housekeeping_report.md` |
| Fast dep scan | `cargo machete` | Terminal output |
| Precise deps | `cargo udeps --workspace` | JSON output |
| Unused pub API | `warnalyzer --workspace` | Text report |
| Coverage | `cargo llvm-cov --html` | HTML report |
| Call graph | `cargo callgraph --lib` | DOT file |

## 📊 Understanding the Reports

### 1. Ranked Report (`housekeeping_report.md`)
**The main output** - professionally formatted with prioritized recommendations:
```markdown
## Action Buckets
### Delete / Demote Candidates (ranked)
| Item | Score | Recommendation | Evidence |
|---|---:|---|---|
| `some::orphaned_function` | 3 | Delete | unused pub, zero coverage |
| `another::Module` | 2 | Demote to pub(crate) | unused pub |
```

### 2. Dependencies (`udeps.json`)
JSON format with precise unused dependency information.
**Action**: Remove from `Cargo.toml`, clean up `use` statements

### 3. Public API (`warnalyzer.txt`)
Text format listing unused public items across workspace.
**Action**: Change `pub` → `pub(crate)` or delete if unused

### 4. Coverage (`lcov.info` + HTML)
- **0% line + 0% function coverage** = deletion candidates
- Cross-reference with warnalyzer results
- Focus on public items with zero coverage

### 5. Call Graph (`callgraph.dot`)
- Look for disconnected subgraphs
- Identify orphaned modules/functions
- Consider archiving entire disconnected subsystems

## 🎯 Zed Agent Integration

If using Zed editor with agents, you now have these tasks:

```
🧹 Dead Code Comprehensive Sweep  # Full analysis
📊 Generate Ranked Report         # Python report generation
🎯 Complete Dead Code Analysis    # Full workflow (sweep + report)
📦 Check Unused Dependencies       # Quick dep check
🔍 Find Orphaned Public API       # API analysis
📈 Generate Coverage Report        # Coverage
🔧 Apply Safe Clippy Fixes        # Auto-fixes
🛠️ Install Dead Code Analysis Tools # One-time setup
```

### Agent Prompts
```
"Run the full-workflow task to generate a comprehensive dead code analysis with ranked recommendations. Review the housekeeping_report.md and implement the highest-scoring deletion candidates."

"Execute the dead-code-sweep task followed by generate-report task. Focus on items with score ≥3 in the ranked report for immediate cleanup."

"Run install-tools task first if tools are missing, then execute the full dead code workflow and summarize the top 10 deletion candidates."
```

## ⚠️ Important Guardrails

**Always run before cleanup:**
```bash
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
cargo hack check --workspace --each-feature
```

**Always run after cleanup:**
- Same commands to ensure nothing broke
- All examples build: `cargo build --examples`
- All benchmarks build: `cargo build --benches`

## 🚨 Common Pitfalls

1. **Don't trust cargo-machete alone** - use cargo-udeps for confirmation
2. **Feature-gated code appears unused** - validate with `cargo hack --each-feature`
3. **Examples/benches seem unused** - ensure `--all-targets` in analysis
4. **Dynamic dispatch hides call edges** - use coverage as supplementary signal

## 📈 Success Metrics

### Before Cleanup (ob-poc current state)
- ✅ 17 compilation errors → 0 errors *(already fixed)*
- ✅ 8 dead code warnings → 0 warnings *(already fixed)*
- 🟡 ~18,583 public API items → **needs analysis**
- 🟡 Unknown unused dependencies → **needs analysis**

### After Full Cleanup (targets)
- ✅ Zero `cargo udeps` warnings
- ✅ Zero `warnalyzer` unused pub items
- ✅ >90% test coverage on remaining code
- ✅ Clean `cargo clippy --all-targets --all-features`
- ✅ All features build via `cargo hack --each-feature`

## 🏁 Next Steps

1. **Install tools**: `./scripts/install-dead-code-tools.sh`
2. **Run the analysis**: `./scripts/dead-code-sweep.sh`
3. **Generate ranked report**: `python3 scripts/generate-report.py`
4. **Review findings**: Open `target/housekeeping/housekeeping_report.md`
5. **Start with high-scoring items**: Address items with score ≥3 first
6. **Dependencies next**: Clean up unused deps (lowest risk)
7. **Coverage-guided cleanup**: Remove 0% coverage orphaned code
8. **Validate thoroughly**: Run full test suite after each batch

## 🤖 CI Integration

The workflow automatically runs on:
- Pull requests touching Rust code
- Pushes to main/master
- Manual workflow dispatch

Reports are uploaded as CI artifacts and PR comments highlight significant findings.

---

**Status**: Ready for execution  
**Est. Time**: 30 minutes analysis + 2-4 hours systematic cleanup  
**Risk Level**: Low (comprehensive validation at each step)