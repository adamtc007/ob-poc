#!/usr/bin/env bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="${WORKSPACE_ROOT}/rust"
TARGET_DIR="${RUST_DIR}/target"
REPORTS_DIR="${TARGET_DIR}/housekeeping"

# Create reports directory
mkdir -p "${REPORTS_DIR}"

echo -e "${BLUE}🧹 Dead Code Comprehensive Sweep${NC}"
echo "========================================"
echo "Workspace: ${WORKSPACE_ROOT}"
echo "Reports: ${REPORTS_DIR}"
echo ""

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to install missing tools
install_missing_tools() {
    local missing_tools=()

    # Check for required tools
    if ! command_exists cargo-machete; then missing_tools+=("cargo-machete"); fi
    if ! command_exists cargo-udeps; then missing_tools+=("cargo-udeps"); fi
    if ! command_exists cargo-workspace-unused-pub; then missing_tools+=("cargo-workspace-unused-pub"); fi
    if ! command_exists cargo-llvm-cov; then missing_tools+=("cargo-llvm-cov"); fi
    # cargo-callgraph is optional - not available in registry
    # if ! command_exists cargo-callgraph; then missing_tools+=("cargo-callgraph"); fi
    if ! command_exists cargo-hack; then missing_tools+=("cargo-hack"); fi

    if [ ${#missing_tools[@]} -ne 0 ]; then
        echo -e "${YELLOW}⚠️  Installing missing tools: ${missing_tools[*]}${NC}"
        for tool in "${missing_tools[@]}"; do
            echo "Installing ${tool}..."
            if cargo install "${tool}"; then
                echo -e "${GREEN}✅ ${tool} installed successfully${NC}"
            else
                echo -e "${RED}❌ Failed to install ${tool}${NC}"
                echo "Please install manually: cargo install ${tool}"
                exit 1
            fi
        done
        echo ""
    fi
}

# Install missing tools
install_missing_tools

# Function to print status messages (matching project patterns)
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_check() {
    echo -e "  ${BLUE}✓${NC} $1"
}

# Change to rust directory where Cargo.toml exists
cd "${RUST_DIR}"

# Ensure we're using the correct Rust version
echo -e "${BLUE}[INFO]${NC} Using Rust version: $(rustc --version)"
if ! rustc --version | grep -q "1.91"; then
    echo -e "${YELLOW}[WARNING]${NC} Expected Rust 1.91.x, but found: $(rustc --version)"
    echo -e "${BLUE}[INFO]${NC} Consider running: rustup default 1.91"
fi

# Phase 1: Dependencies Analysis
echo -e "${BLUE}📦 Phase 1: Dependencies Analysis${NC}"
echo "----------------------------------------"

print_status "Quick dependency scan (cargo-machete)..."
if cargo machete > "${REPORTS_DIR}/machete-report.txt" 2>&1; then
    print_success "Machete scan complete"
    if [ -s "${REPORTS_DIR}/machete-report.txt" ]; then
        print_warning "Potentially unused dependencies found (see machete-report.txt)"
        head -10 "${REPORTS_DIR}/machete-report.txt"
    else
        print_success "No unused dependencies detected by machete"
    fi
else
    print_error "Machete scan failed"
fi

echo ""
print_status "Precise dependency analysis (cargo-udeps)..."
if RUSTC_BOOTSTRAP=1 cargo +1.91 udeps --all-targets --workspace > "${REPORTS_DIR}/udeps-report.txt" 2>&1; then
    print_success "Udeps analysis complete"
    if grep -q "unused dependencies" "${REPORTS_DIR}/udeps-report.txt"; then
        print_warning "Unused dependencies confirmed (see udeps-report.txt)"
        grep -A 5 "unused dependencies" "${REPORTS_DIR}/udeps-report.txt" || true
    else
        print_success "No unused dependencies confirmed by udeps"
    fi
else
    print_warning "Udeps analysis had issues (see udeps-report.txt)"
fi

echo ""

# Phase 2: Cross-Crate Public API Analysis
echo -e "${BLUE}🔍 Phase 2: Cross-Crate Public API Analysis${NC}"
echo "------------------------------------------------"

print_status "Analyzing workspace-wide unused public API..."
echo "🔍 Analyzing workspace-wide unused public API..."
# Use cargo-workspace-unused-pub (working) as primary, warnalyzer as fallback
if command_exists cargo-workspace-unused-pub; then
    if cargo +1.91 workspace-unused-pub > "${REPORTS_DIR}/warnalyzer.txt" 2>&1; then
        echo -e "${GREEN}✅ cargo-workspace-unused-pub analysis complete${NC}"
        if [ -s "${REPORTS_DIR}/warnalyzer.txt" ]; then
            unused_count=$(grep -c "pub fn\|pub struct\|pub enum\|pub mod" "${REPORTS_DIR}/warnalyzer.txt" 2>/dev/null || echo "0")
            total_lines=$(wc -l < "${REPORTS_DIR}/warnalyzer.txt" 2>/dev/null || echo "0")
            echo "📊 Found ${unused_count} potentially unused public items (${total_lines} total lines)"
            # Show preview
            echo "Preview (first 10 lines):"
            head -10 "${REPORTS_DIR}/warnalyzer.txt" | grep -v "INFO\|WARN" | sed 's/^/  /'
        fi
    else
        echo -e "${YELLOW}⚠️  cargo-workspace-unused-pub analysis had issues${NC}"
    fi
elif command_exists warnalyzer; then
    if warnalyzer --workspace --all-features --all-targets > "${REPORTS_DIR}/warnalyzer.txt" 2>&1; then
        echo -e "${GREEN}✅ Warnalyzer analysis complete${NC}"
    else
        echo -e "${YELLOW}⚠️  Warnalyzer analysis had issues${NC}"
    fi
else
    echo "WARN: warnalyzer and cargo-workspace-unused-pub not installed." > "${REPORTS_DIR}/warnalyzer.txt"
    echo -e "${YELLOW}⚠️  No workspace unused pub tool available${NC}"
fi

echo ""

# Phase 3: Call Graph Analysis
echo -e "${BLUE}📊 Phase 3: Call Graph Analysis${NC}"
echo "------------------------------------"

print_status "Generating call graph..."
if cargo +1.91 callgraph --lib > "${REPORTS_DIR}/callgraph.dot" 2>/dev/null; then
    print_success "Call graph generated"
    dot_lines=$(wc -l < "${REPORTS_DIR}/callgraph.dot" 2>/dev/null || echo "0")
    print_check "Call graph contains ${dot_lines} lines"

    # Try to generate SVG if dot is available
    if command_exists dot; then
        print_status "Converting to SVG..."
        if dot -Tsvg "${REPORTS_DIR}/callgraph.dot" -o "${REPORTS_DIR}/callgraph.svg" 2>/dev/null; then
            print_success "SVG visualization created: callgraph.svg"
        fi
    fi
else
    print_warning "Call graph generation had issues"
fi

echo ""

# Phase 4: Coverage Analysis
echo -e "${BLUE}📈 Phase 4: Coverage Analysis${NC}"
echo "----------------------------------"

print_status "Generating coverage report..."
if cargo +1.91 llvm-cov --workspace --all-features --html --output-dir "${REPORTS_DIR}/coverage" > "${REPORTS_DIR}/coverage_summary.txt" 2>&1; then
    print_success "Coverage analysis complete"

    # Extract summary if available
    if grep -q "TOTAL" "${REPORTS_DIR}/coverage_summary.txt"; then
        echo "📊 Coverage Summary:"
        grep "TOTAL" "${REPORTS_DIR}/coverage_summary.txt" || true
    fi

    echo "📊 HTML report: ${REPORTS_DIR}/coverage/index.html"

    # Also generate LCOV for tooling
    if cargo +1.91 llvm-cov --workspace --all-features --lcov --output-path "${REPORTS_DIR}/lcov.info" >/dev/null 2>&1; then
        echo "📊 LCOV report: ${REPORTS_DIR}/lcov.info"
    fi
else
    echo -e "${YELLOW}⚠️  Coverage analysis had issues (see coverage_summary.txt)${NC}"
    print_warning "Coverage analysis had issues (see coverage_summary.txt)"
fi

echo ""

# Phase 5: Auto-Fix Safe Issues
echo -e "${BLUE}🔧 Phase 5: Auto-Fix Safe Issues${NC}"
echo "------------------------------------"

print_status "Applying safe clippy fixes..."
if cargo +1.91 clippy --fix --all-targets --all-features --allow-dirty --allow-staged > "${REPORTS_DIR}/clippy-fix.log" 2>&1; then
    print_success "Clippy auto-fixes applied"
else
    print_warning "Some clippy fixes couldn't be applied (see clippy-fix.log)"
fi

print_status "Applying rustc fixes..."
if cargo +1.91 fix --all-targets --all-features --allow-dirty --allow-staged > "${REPORTS_DIR}/rustc-fix.log" 2>&1; then
    print_success "Rustc auto-fixes applied"
else
    print_warning "Some rustc fixes couldn't be applied (see rustc-fix.log)"
fi

echo ""

# Phase 6: Final Validation
echo -e "${BLUE}✅ Phase 6: Final Validation${NC}"
echo "-------------------------------"

print_status "Validating workspace builds..."
if cargo +1.91 check --workspace --all-targets --all-features > "${REPORTS_DIR}/final-check.log" 2>&1; then
    print_success "Workspace builds successfully"
else
    print_error "Workspace build failed after fixes"
    echo "See final-check.log for details"
    exit 1
fi

print_status "Running basic test suite..."
if cargo +1.91 test --workspace --lib > "${REPORTS_DIR}/final-test.log" 2>&1; then
    print_success "Core tests pass"
else
    print_warning "Some tests failed (see final-test.log)"
fi

# Feature matrix validation (optional, can be slow)
if [ "${SKIP_FEATURE_MATRIX:-}" != "1" ]; then
    print_status "Validating feature combinations..."
    if cargo +1.91 hack check --workspace --each-feature > "${REPORTS_DIR}/feature-matrix.log" 2>&1; then
        print_success "All feature combinations build"
    else
        print_warning "Some feature combinations failed (see feature-matrix.log)"
    fi
fi

echo ""

# Summary Report
echo -e "${BLUE}📋 Summary Report${NC}"
echo "=================="

print_check "All reports saved to: ${REPORTS_DIR}/"
echo "📁 All reports saved to: ${REPORTS_DIR}/"
echo ""
echo "📊 Key Findings:"

# Dependencies
if [ -s "${REPORTS_DIR}/udeps.json" ] && [ "$(cat "${REPORTS_DIR}/udeps.json")" != "[]" ]; then
    echo -e "${YELLOW}  🔸 Unused dependencies detected (udeps.json)${NC}"
elif [ -s "${REPORTS_DIR}/machete.txt" ] && grep -q "unused dependencies" "${REPORTS_DIR}/machete.txt"; then
    echo -e "${YELLOW}  🔸 Unused dependencies detected (machete.txt)${NC}"
else
    echo -e "${GREEN}  ✅ No unused dependencies${NC}"
fi

# Public API
if [ -s "${REPORTS_DIR}/warnalyzer.txt" ]; then
    unused_count=$(grep -c "pub fn\|pub struct\|pub enum\|pub mod" "${REPORTS_DIR}/warnalyzer.txt" 2>/dev/null || echo "0")
    if [ "$unused_count" -gt 0 ]; then
        echo -e "${YELLOW}  🔸 ${unused_count} potentially unused public API items${NC}"
    else
        echo -e "${GREEN}  ✅ Clean public API analysis${NC}"
    fi
fi

# Coverage
if [ -s "${REPORTS_DIR}/coverage_summary.txt" ]; then
    echo -e "${GREEN}  ✅ Coverage report generated${NC}"
fi

echo ""
echo -e "${BLUE}🎯 Next Steps:${NC}"
echo "1. Generate ranked report: python3 scripts/generate-report.py"
echo "2. Review reports in ${REPORTS_DIR}/"
echo "3. Address unused dependencies (udeps.json)"
echo "4. Clean up orphaned public API (warnalyzer.txt)"
echo "5. Examine 0% coverage items for deletion candidates"
echo ""

# Generate action items file
# Generate workspace metadata for the report
echo "== Rust version & workspace ==" | tee "${REPORTS_DIR}/workspace.txt"
rustc -V >> "${REPORTS_DIR}/workspace.txt" 2>&1 || true
cargo -V >> "${REPORTS_DIR}/workspace.txt" 2>&1 || true
echo "workspace members:" >> "${REPORTS_DIR}/workspace.txt"
cargo metadata --no-deps --format-version 1 >> "${REPORTS_DIR}/workspace.txt" 2>&1 || true

cat > "${REPORTS_DIR}/ACTION_ITEMS.md" << EOF
# Dead Code Cleanup Action Items

Generated: $(date)

## Priority 1: Generate Ranked Report
- [ ] Run: \`python3 scripts/generate-report.py\`
- [ ] Review: \`target/housekeeping/housekeeping_report.md\`

## Priority 2: Dependencies
- [ ] Review unused dependencies in \`udeps.json\` and \`machete.txt\`
- [ ] Remove confirmed unused deps from Cargo.toml files
- [ ] Clean up unused \`use\` statements

## Priority 3: Public API Cleanup
- [ ] Review unused public items in \`warnalyzer.txt\`
- [ ] Convert unused \`pub\` to \`pub(crate)\` where appropriate
- [ ] Delete truly unused public functions/structs

## Priority 4: Coverage-Based Cleanup
- [ ] Open \`coverage/index.html\` and identify 0% coverage items
- [ ] Cross-reference with warnalyzer results
- [ ] Archive or delete never-executed code paths

## Priority 5: Call Graph Analysis
- [ ] Review \`callgraph.dot\` for disconnected subgraphs
- [ ] Investigate orphaned modules/functions
- [ ] Consider archiving entire disconnected subsystems

## Validation Checklist
- [ ] Run \`cargo check --workspace --all-targets --all-features\`
- [ ] Run \`cargo test --workspace\`
- [ ] Run \`cargo hack check --workspace --each-feature\`
- [ ] Verify all examples still build: \`cargo build --examples\`

## Files Generated
- \`machete.txt\`: Fast unused dependency scan
- \`udeps.json\`: Precise unused dependency analysis (JSON format)
- \`warnalyzer.txt\`: Unused public API items
- \`lcov.info\`: Coverage data in LCOV format
- \`coverage_summary.txt\`: Coverage summary
- \`callgraph.dot\`: Call graph (if available)
- \`workspace.txt\`: Rust version and workspace info
EOF

echo -e "${GREEN}📝 Action items checklist: ${REPORTS_DIR}/ACTION_ITEMS.md${NC}"
echo ""
echo -e "${BLUE}🚀 Generate Ranked Report:${NC}"
echo "python3 scripts/generate-report.py"
echo ""
echo -e "${GREEN}🎉 Dead code sweep complete!${NC}"
