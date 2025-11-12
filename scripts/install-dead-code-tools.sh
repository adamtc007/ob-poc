#!/usr/bin/env bash
set -euo pipefail

# Dead Code Analysis Tools Installation Script
# Enhanced version combining patterns from existing workflow scripts

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "\n${BLUE}## $1${NC}"
}

print_success() {
    echo -e "  ${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "  ${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "  ${RED}✗${NC} $1"
}

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to install a tool with error handling
install_tool() {
    local tool="$1"
    local description="$2"

    echo -n "Installing $tool ($description)..."
    if cargo install "$tool" >/dev/null 2>&1; then
        print_success "$tool installed successfully"
        return 0
    else
        print_error "$tool installation failed"
        return 1
    fi
}

# Check prerequisites
print_header "Prerequisites Check"

if ! command_exists cargo; then
    print_error "cargo not found in PATH. Please install Rust toolchain first."
    print_status "Visit: https://rustup.rs/"
    exit 1
fi

if ! command_exists rustc; then
    print_error "rustc not found in PATH. Please install Rust toolchain first."
    exit 1
fi

print_success "cargo $(cargo --version | cut -d' ' -f2)"
print_success "rustc $(rustc --version | cut -d' ' -f2)"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] && [ ! -f "rust/Cargo.toml" ]; then
    print_warning "No Cargo.toml found. Make sure you're in a Rust workspace."
fi

print_header "Installing Core Dead Code Analysis Tools"

# Track installation results
INSTALLED=0
FAILED=0

# Core tools for dead code analysis
CORE_TOOLS=(
    "cargo-machete:Fast unused dependency detection"
    "cargo-udeps:Precise unused dependency analysis"
    "cargo-hack:Feature matrix validation"
    "cargo-llvm-cov:Coverage analysis"
    "cargo-callgraph:Call graph generation"
    "warnalyzer:Workspace-wide unused public API detection"
)

for tool_desc in "${CORE_TOOLS[@]}"; do
    tool="${tool_desc%%:*}"
    desc="${tool_desc##*:}"

    if command_exists "$tool"; then
        print_success "$tool already installed"
        INSTALLED=$((INSTALLED + 1))
    else
        if install_tool "$tool" "$desc"; then
            INSTALLED=$((INSTALLED + 1))
        else
            FAILED=$((FAILED + 1))
        fi
    fi
done

print_header "Installing Optional Enhancement Tools"

# Optional tools for enhanced analysis
OPTIONAL_TOOLS=(
    "cargo-public-api:Public API surface monitoring"
    "cargo-semver-checks:Breaking change detection"
    "cargo-unused-features:Feature flag cleanup"
    "cargo-workspace-unused-pub:Alternative unused public API tool"
)

for tool_desc in "${OPTIONAL_TOOLS[@]}"; do
    tool="${tool_desc%%:*}"
    desc="${tool_desc##*:}"

    if command_exists "$tool"; then
        print_success "$tool already installed"
        INSTALLED=$((INSTALLED + 1))
    else
        echo -n "Installing $tool ($desc)..."
        if cargo install "$tool" >/dev/null 2>&1; then
            print_success "$tool installed successfully"
            INSTALLED=$((INSTALLED + 1))
        else
            print_warning "$tool installation failed (optional)"
        fi
    fi
done

print_header "Additional Dependencies Check"

# Check for additional system dependencies
if command_exists dot; then
    print_success "graphviz (dot) available for call graph visualization"
else
    print_warning "graphviz not found - call graph SVG generation will be skipped"
    print_status "Install with: brew install graphviz (macOS) or apt install graphviz (Ubuntu)"
fi

if command_exists python3; then
    print_success "python3 available for report generation"

    # Check if required Python modules are available
    if python3 -c "import json, re, datetime" >/dev/null 2>&1; then
        print_success "Python modules (json, re, datetime) available"
    else
        print_warning "Some Python modules missing (should be built-in)"
    fi
else
    print_warning "python3 not found - report generation will be limited"
fi

if command_exists jq; then
    print_success "jq available for JSON processing"
else
    print_warning "jq not found - JSON output parsing will be limited"
    print_status "Install with: brew install jq (macOS) or apt install jq (Ubuntu)"
fi

print_header "Installation Summary"

echo "Core tools installed: $INSTALLED"
if [ $FAILED -gt 0 ]; then
    echo "Failed installations: $FAILED"
fi

print_header "Next Steps"

echo "✅ Tools ready! You can now run:"
echo ""
echo "   ./scripts/dead-code-sweep.sh      # Full analysis"
echo "   python3 scripts/generate-report.py # Generate report"
echo ""

# Test basic tool availability
print_header "Tool Verification"

VERIFICATION_COMMANDS=(
    "cargo-machete:cargo machete --help"
    "cargo-udeps:cargo udeps --help"
    "warnalyzer:warnalyzer --help"
    "cargo-llvm-cov:cargo llvm-cov --help"
    "cargo-hack:cargo hack --help"
)

ALL_WORKING=true

for cmd_desc in "${VERIFICATION_COMMANDS[@]}"; do
    tool="${cmd_desc%%:*}"
    cmd="${cmd_desc##*:}"

    if $cmd >/dev/null 2>&1; then
        print_success "$tool working"
    else
        print_error "$tool not working properly"
        ALL_WORKING=false
    fi
done

if $ALL_WORKING; then
    print_success "All core tools verified and working!"
    echo ""
    echo -e "${GREEN}🎉 Installation complete! Ready to analyze dead code.${NC}"
else
    print_warning "Some tools may need manual troubleshooting"
    echo ""
    echo "Common issues:"
    echo "- Some tools require nightly Rust: rustup install nightly"
    echo "- Some tools need LLVM: install via system package manager"
    echo "- Network issues: check internet connection"
fi

echo ""
echo "For help with any tool, run: <tool-name> --help"
echo "Full documentation: https://github.com/your-repo/ob-poc/blob/main/COMPREHENSIVE_DEAD_CODE_WORKFLOW.md"
