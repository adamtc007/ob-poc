#!/usr/bin/env bash
# ensure-rust-version.sh - Enforce Rust 1.91 usage for ob-poc project
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

REQUIRED_VERSION="1.91"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

print_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}========================================${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

# Function to get current Rust version
get_rust_version() {
    rustc --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || echo "unknown"
}

# Function to get major.minor version
get_rust_major_minor() {
    get_rust_version | grep -oE '[0-9]+\.[0-9]+' || echo "unknown"
}

# Function to check if rustup is available
has_rustup() {
    command -v rustup >/dev/null 2>&1
}

# Function to install Rust 1.91 if needed
install_rust_1_91() {
    if has_rustup; then
        print_info "Installing Rust 1.91 via rustup..."
        rustup install 1.91
        rustup default 1.91
        print_success "Rust 1.91 installed and set as default"
    else
        print_error "rustup not found. Please install Rust 1.91 manually"
        print_info "Visit: https://rustup.rs/"
        exit 1
    fi
}

# Function to verify toolchain components
verify_components() {
    local components=("rustfmt" "clippy" "rust-src")
    local missing_components=()

    for component in "${components[@]}"; do
        if ! rustup component list --toolchain 1.91 --installed | grep -q "^${component}"; then
            missing_components+=("$component")
        fi
    done

    if [ ${#missing_components[@]} -ne 0 ]; then
        print_warning "Missing components: ${missing_components[*]}"
        print_info "Installing missing components..."
        for component in "${missing_components[@]}"; do
            rustup component add "$component" --toolchain 1.91
        done
        print_success "All required components installed"
    else
        print_success "All required components are present"
    fi
}

# Function to update project files for Rust 1.91
update_project_files() {
    print_info "Verifying project configuration..."

    # Check rust-toolchain.toml
    if [ -f "$PROJECT_ROOT/rust-toolchain.toml" ]; then
        if grep -q 'channel = "1.91"' "$PROJECT_ROOT/rust-toolchain.toml"; then
            print_success "rust-toolchain.toml is correctly configured"
        else
            print_warning "rust-toolchain.toml needs update"
        fi
    else
        print_warning "rust-toolchain.toml not found - should be created"
    fi

    # Check Cargo.toml
    if [ -f "$PROJECT_ROOT/rust/Cargo.toml" ]; then
        if grep -q 'rust-version = "1.91"' "$PROJECT_ROOT/rust/Cargo.toml"; then
            print_success "Cargo.toml rust-version is correctly set"
        else
            print_warning "Cargo.toml should specify rust-version = \"1.91\""
        fi
    fi
}

# Function to test basic functionality
test_rust_functionality() {
    print_info "Testing Rust 1.91 functionality..."

    cd "$PROJECT_ROOT/rust"

    # Test basic compilation
    if cargo +1.91 check --workspace --lib >/dev/null 2>&1; then
        print_success "Basic compilation works"
    else
        print_error "Basic compilation failed"
        return 1
    fi

    # Test clippy
    if cargo +1.91 clippy --workspace --lib -- --cap-lints warn >/dev/null 2>&1; then
        print_success "Clippy analysis works"
    else
        print_warning "Clippy analysis had issues"
    fi

    # Test formatting
    if cargo +1.91 fmt --check >/dev/null 2>&1; then
        print_success "Code formatting check passed"
    else
        print_info "Code may need formatting (run: cargo +1.91 fmt)"
    fi
}

# Function to setup development aliases
setup_aliases() {
    print_info "Setting up development aliases for this session..."

    # Create aliases that use +1.91
    alias cargo-1.91='cargo +1.91'
    alias c91='cargo +1.91'
    alias cbuild91='cargo +1.91 build'
    alias ctest91='cargo +1.91 test'
    alias ccheck91='cargo +1.91 check'
    alias cclippy91='cargo +1.91 clippy'
    alias cfmt91='cargo +1.91 fmt'

    echo "Aliases created for this session:"
    echo "  cargo-1.91, c91, cbuild91, ctest91, ccheck91, cclippy91, cfmt91"
}

# Main execution
main() {
    print_header "Rust 1.91 Version Enforcement"

    # Check current Rust version
    current_version=$(get_rust_version)
    current_major_minor=$(get_rust_major_minor)

    echo "Current Rust version: $current_version"
    echo "Required version: $REQUIRED_VERSION.x"
    echo

    if [ "$current_major_minor" = "$REQUIRED_VERSION" ]; then
        print_success "Correct Rust version ($current_version) is active"
    else
        print_warning "Current version ($current_version) does not match required ($REQUIRED_VERSION.x)"

        if has_rustup; then
            # Check if 1.91 is installed
            if rustup toolchain list | grep -q "1.91"; then
                print_info "Rust 1.91 is installed but not active. Setting as default..."
                rustup default 1.91
                print_success "Rust 1.91 is now the default toolchain"
            else
                read -p "Install Rust 1.91? (y/N): " -n 1 -r
                echo
                if [[ $REPLY =~ ^[Yy]$ ]]; then
                    install_rust_1_91
                else
                    print_error "Rust 1.91 is required for this project"
                    exit 1
                fi
            fi
        else
            print_error "rustup not available. Please install Rust 1.91 manually"
            exit 1
        fi
    fi

    # Verify components
    verify_components

    # Update and verify project files
    update_project_files

    # Test functionality
    if ! test_rust_functionality; then
        print_error "Rust 1.91 functionality test failed"
        exit 1
    fi

    # Setup development aliases
    setup_aliases

    print_header "Setup Complete"
    print_success "Rust 1.91 is properly configured and working"
    print_info "You can now use cargo commands with +1.91 or use the aliases"
    print_info "Example: cargo +1.91 build or cbuild91"
    echo
    print_info "To run the dead code analysis with correct version:"
    print_info "  ./scripts/dead-code-sweep.sh"
    echo
    print_info "Common development commands:"
    print_info "  cargo +1.91 check --workspace"
    print_info "  cargo +1.91 test --workspace"
    print_info "  cargo +1.91 clippy --workspace"
    print_info "  cargo +1.91 fmt"
}

# Handle command line arguments
case "${1:-}" in
    --check)
        current_major_minor=$(get_rust_major_minor)
        if [ "$current_major_minor" = "$REQUIRED_VERSION" ]; then
            print_success "Rust $REQUIRED_VERSION is active"
            exit 0
        else
            print_error "Rust $REQUIRED_VERSION is not active (found: $(get_rust_version))"
            exit 1
        fi
        ;;
    --install)
        install_rust_1_91
        verify_components
        ;;
    --test)
        test_rust_functionality
        ;;
    --help|-h)
        echo "Usage: $0 [OPTION]"
        echo ""
        echo "Ensure Rust 1.91 is properly configured for ob-poc project"
        echo ""
        echo "Options:"
        echo "  --check    Check if Rust 1.91 is active"
        echo "  --install  Install Rust 1.91 and set as default"
        echo "  --test     Test Rust 1.91 functionality"
        echo "  --help     Show this help message"
        echo ""
        echo "Run without arguments for interactive setup"
        exit 0
        ;;
    "")
        main
        ;;
    *)
        print_error "Unknown option: $1"
        print_info "Use --help for usage information"
        exit 1
        ;;
esac
