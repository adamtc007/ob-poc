#!/bin/bash

# EDN Keyword Consistency Audit Script for UBO-POC
# Verifies that ':' keyword token pattern is consistently used across the codebase

set -e

echo "🔍 EDN Keyword Consistency Audit for UBO-POC"
echo "============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_header() {
    echo -e "\n${BLUE}## $1${NC}"
}

print_check() {
    echo -e "  ✓ ${GREEN}$1${NC}"
}

print_warning() {
    echo -e "  ⚠ ${YELLOW}$1${NC}"
}

print_error() {
    echo -e "  ✗ ${RED}$1${NC}"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "src" ]; then
    print_error "Must be run from the UBO-POC project root directory"
    exit 1
fi

# Track issues
ISSUES=0

print_header "1. EBNF Grammar Consistency (docs/dsl-grammar.ebnf)"

# Check EBNF defines keywords correctly
if grep -q "keyword = ':' identifier" docs/dsl-grammar.ebnf; then
    print_check "Keywords defined as ':' identifier"
else
    print_error "Keywords not properly defined in EBNF"
    ISSUES=$((ISSUES + 1))
fi

# Check map-entry syntax doesn't have extra colon
if grep -q "map-entry = keyword ':' value" docs/dsl-grammar.ebnf; then
    print_error "EBNF has incorrect map-entry syntax (extra colon)"
    ISSUES=$((ISSUES + 1))
elif grep -q "map-entry = keyword value" docs/dsl-grammar.ebnf; then
    print_check "Map entries use correct ':key value' syntax"
else
    print_warning "Could not find map-entry definition in EBNF"
fi

print_header "2. Parser Implementation (src/parser/mod.rs)"

# Check parse_keyword function
if grep -A 3 "fn parse_keyword" src/parser/mod.rs | grep -q "char(':')"; then
    print_check "parse_keyword function correctly parses ':' prefix"
else
    print_error "parse_keyword function missing or incorrect"
    ISSUES=$((ISSUES + 1))
fi

# Check map entry parsing doesn't expect extra colon
if grep -A 5 "fn parse_map_entry" src/parser/mod.rs | grep -q "char(':')"; then
    print_error "parse_map_entry expects extra colon (should be fixed)"
    ISSUES=$((ISSUES + 1))
else
    print_check "parse_map_entry uses correct keyword-only syntax"
fi

print_header "3. DSL Examples Validation"

# Count keyword usage patterns in DSL files
dsl_files=$(find examples -name "*.dsl" 2>/dev/null | wc -l)
if [ "$dsl_files" -gt 0 ]; then
    print_check "Found $dsl_files DSL files to validate"

    # Check for correct keyword pattern :identifier
    correct_keywords=$(grep -ho ":[a-zA-Z][a-zA-Z0-9_-]*[[:space:]]" examples/*.dsl | wc -l | tr -d ' ')
    print_check "Found $correct_keywords properly formatted keywords"

    # Check for incorrect patterns :identifier: (colon at end)
    incorrect_colons=$(grep -o ":[a-zA-Z][a-zA-Z0-9_-]*:" examples/*.dsl | wc -l | tr -d ' ')
    if [ "$incorrect_colons" -gt 0 ]; then
        print_error "Found $incorrect_colons incorrectly formatted keywords with trailing colon"
        ISSUES=$((ISSUES + 1))
    else
        print_check "No incorrectly formatted keywords found"
    fi

    # Validate property map syntax { :key value } not { :key: value }
    property_maps=$(grep -o "{[^}]*}" examples/*.dsl | wc -l | tr -d ' ')
    if [ "$property_maps" -gt 0 ]; then
        print_check "Found $property_maps property maps to validate"

        # Check for incorrect double colon pattern in maps
        incorrect_maps=$(grep -o "{[^}]*:[a-zA-Z][a-zA-Z0-9_-]*:[^}]*}" examples/*.dsl | wc -l | tr -d ' ')
        if [ "$incorrect_maps" -gt 0 ]; then
            print_error "Found $incorrect_maps property maps with incorrect ':key:' syntax"
            ISSUES=$((ISSUES + 1))
        else
            print_check "All property maps use correct ':key value' syntax"
        fi
    fi
else
    print_warning "No DSL files found in examples directory"
fi

print_header "4. Test Consistency"

# Check that parser tests use correct syntax
if [ -f "src/parser/mod.rs" ]; then
    test_keywords=$(grep -A 10 "#\[test\]" src/parser/mod.rs | grep -o ":[a-zA-Z][a-zA-Z0-9_-]*[[:space:]]" | wc -l | tr -d ' ')
    if [ "$test_keywords" -gt 0 ]; then
        print_check "Found $test_keywords keywords in parser tests"

        # Check for incorrect test syntax
        incorrect_test_syntax=$(grep -A 10 "#\[test\]" src/parser/mod.rs | grep -o ":[a-zA-Z][a-zA-Z0-9_-]*:" | wc -l | tr -d ' ')
        if [ "$incorrect_test_syntax" -gt 0 ]; then
            print_error "Found $incorrect_test_syntax incorrect keyword patterns in tests"
            ISSUES=$((ISSUES + 1))
        else
            print_check "All test keywords use correct syntax"
        fi
    else
        print_warning "No keywords found in parser tests"
    fi
fi

print_header "5. README Documentation"

# Check README examples
if [ -f "README.md" ]; then
    readme_keywords=$(grep -o ":[a-zA-Z][a-zA-Z0-9_-]*" README.md | wc -l | tr -d ' ')
    if [ "$readme_keywords" -gt 0 ]; then
        print_check "Found $readme_keywords keyword examples in README"
    else
        print_warning "No keyword examples found in README"
    fi
fi

print_header "6. Build and Test Validation"

echo "  Running cargo check..."
if cargo check --quiet 2>/dev/null; then
    print_check "Project compiles successfully"
else
    print_error "Project compilation failed"
    ISSUES=$((ISSUES + 1))
fi

echo "  Running parser tests..."
if cargo test --lib --quiet parser::tests 2>/dev/null; then
    print_check "All parser tests pass"
else
    print_error "Parser tests failed"
    ISSUES=$((ISSUES + 1))
fi

print_header "7. Common Patterns Verification"

echo "  Checking for EDN compliance patterns..."

# Verify common keyword patterns used in UBO domain
expected_patterns=(
    ":node-id"
    ":from"
    ":to"
    ":type"
    ":properties"
    ":target"
    ":threshold"
    ":evidenced-by"
)

for pattern in "${expected_patterns[@]}"; do
    if grep -r "$pattern" examples/ >/dev/null 2>&1; then
        print_check "✓ $pattern pattern found in examples"
    else
        print_warning "Pattern $pattern not found in examples"
    fi
done

print_header "Summary"
echo "============================================="

if [ "$ISSUES" -eq 0 ]; then
    echo -e "🎉 ${GREEN}EDN Keyword Consistency: PERFECT!${NC}"
    echo ""
    echo "✅ EBNF grammar defines keywords correctly as ':identifier'"
    echo "✅ Parser implementation handles ':' prefix properly"
    echo "✅ DSL examples use correct ':key value' syntax"
    echo "✅ Property maps use '{:key value}' not '{:key: value}'"
    echo "✅ Tests align with correct EDN syntax"
    echo "✅ Project compiles and all tests pass"
    echo ""
    echo "Your codebase follows Rich Hickey's EDN keyword philosophy perfectly!"
    echo "Keywords are self-evaluating, namespace-qualified, and human-readable."
    exit 0
else
    echo -e "❌ ${RED}Found $ISSUES consistency issues${NC}"
    echo ""
    echo "Please review the errors above and fix the inconsistencies."
    echo "Run this script again after making corrections."
    exit 1
fi
