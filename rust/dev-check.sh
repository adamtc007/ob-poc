#!/bin/bash

# Quick development check script for UBO-POC
# Runs fast checks without committing

set -e

echo "🔍 Quick UBO-POC development check..."

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_check() {
    echo -e "  Checking $1..."
}

print_ok() {
    echo -e "  ${GREEN}✓${NC} $1"
}

print_fail() {
    echo -e "  ${RED}✗${NC} $1"
}

print_warn() {
    echo -e "  ${YELLOW}!${NC} $1"
}

# Quick compile check
print_check "compilation"
if cargo check --quiet; then
    print_ok "Code compiles"
else
    print_fail "Compilation errors found"
    exit 1
fi

# Quick clippy check
print_check "clippy (warnings only)"
clippy_output=$(cargo clippy --all-targets --all-features 2>&1)
warning_count=$(echo "$clippy_output" | grep -c "warning:" || true)

if [ -z "$warning_count" ] || [ "$warning_count" -eq 0 ]; then
    print_ok "No clippy warnings"
else
    print_warn "Found $warning_count clippy warnings"
fi

# Quick test check
print_check "tests"
if cargo test --quiet; then
    print_ok "Tests pass"
else
    print_fail "Tests failing"
    exit 1
fi

# Check if CLI binary works
print_check "CLI binary"
if cargo run --bin cli --quiet 2>&1 | grep -q "Usage:"; then
    print_ok "CLI binary runs"
else
    print_fail "CLI binary has issues"
fi

# Git status
if [ -d ".git" ]; then
    changes=$(git diff --name-only | wc -l)
    if [ "$changes" -gt 0 ]; then
        print_warn "$changes files have uncommitted changes"
    else
        print_ok "Working directory clean"
    fi
fi

echo ""
echo "Quick check completed! Use ./dev-commit.sh to run full workflow and commit."
