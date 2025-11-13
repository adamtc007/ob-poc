#!/bin/bash

# OB-POC API Key Loader and Example Runner
# This script loads API keys from macOS Keychain and runs examples

set -e  # Exit on any error

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔑 OB-POC Example Runner${NC}"
echo -e "${BLUE}Loading API keys from macOS Keychain...${NC}"

# Load API keys from keychain
export OPENAI_API_KEY="$(security find-generic-password -w -s "OPENAI_API_KEY" 2>/dev/null || echo "")"
export GEMINI_API_KEY="$(security find-generic-password -w -s "GEMINI_API_KEY" 2>/dev/null || echo "")"

# Check if keys were loaded
openai_loaded="false"
gemini_loaded="false"

if [ -n "$OPENAI_API_KEY" ]; then
    echo -e "${GREEN}✅ OpenAI API key loaded${NC}"
    openai_loaded="true"
else
    echo -e "${YELLOW}⚠️  OpenAI API key not found in keychain${NC}"
fi

if [ -n "$GEMINI_API_KEY" ]; then
    echo -e "${GREEN}✅ Gemini API key loaded${NC}"
    gemini_loaded="true"
else
    echo -e "${YELLOW}⚠️  Gemini API key not found in keychain${NC}"
fi

# Function to show usage
show_usage() {
    echo ""
    echo -e "${BLUE}Usage:${NC}"
    echo "  $0 <example_name>                    # Run specific example"
    echo "  $0 test                             # Test API key setup"
    echo "  $0 list                             # List available examples"
    echo "  $0                                  # Show this help"
    echo ""
    echo -e "${BLUE}Available Examples:${NC}"
    echo "  ai_dsl_onboarding_demo              # Full AI workflow demo"
    echo "  simple_openai_dsl_demo              # Basic OpenAI integration"
    echo "  mock_openai_demo                    # Architecture demo (no API needed)"
    echo "  parse_zenith                        # DSL parsing with UBO case study"
    echo "  minimal_orchestration_demo          # Core DSL orchestration"
    echo "  test_api_keys                       # Test API key setup"
    echo ""
    echo -e "${BLUE}Examples:${NC}"
    echo "  $0 test"
    echo "  $0 mock_openai_demo"
    echo "  $0 ai_dsl_onboarding_demo"
    echo ""
}

# Function to run example
run_example() {
    local example_name="$1"
    echo -e "${BLUE}🚀 Running example: ${example_name}${NC}"
    echo ""

    # Change to rust directory if it exists
    if [ -d "rust" ]; then
        cd rust
    fi

    # Run the example
    cargo run --example "$example_name"
}

# Function to list examples
list_examples() {
    echo -e "${BLUE}📋 Available Examples:${NC}"

    # Change to rust directory if it exists
    if [ -d "rust" ]; then
        cd rust
    fi

    if [ -d "examples" ]; then
        echo ""
        for file in examples/*.rs; do
            if [ -f "$file" ]; then
                basename "$file" .rs
            fi
        done | sort
    else
        echo "No examples directory found"
    fi
}

# Main logic
case "${1:-}" in
    "test")
        run_example "test_api_keys"
        ;;
    "list")
        list_examples
        ;;
    "")
        show_usage
        ;;
    *)
        # Check if it's a valid example name
        example_name="$1"

        # Change to rust directory to check for example
        example_dir="."
        if [ -d "rust" ]; then
            example_dir="rust"
        fi

        if [ -f "$example_dir/examples/${example_name}.rs" ]; then
            run_example "$example_name"
        else
            echo -e "${RED}❌ Example '${example_name}' not found${NC}"
            echo ""
            list_examples
            echo ""
            show_usage
            exit 1
        fi
        ;;
esac
