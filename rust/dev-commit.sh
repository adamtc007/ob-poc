#!/bin/bash

# UBO-POC Development Workflow Script
# Runs tests, clippy, and commits changes if everything passes

set -e  # Exit on any error

echo "🚀 Starting UBO-POC development workflow..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
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

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    print_error "Not in a git repository!"
    exit 1
fi

# Format code with rustfmt
print_status "Running rustfmt..."
if cargo fmt --check; then
    print_success "Code is properly formatted"
else
    print_warning "Formatting code..."
    cargo fmt
    print_success "Code formatted"
fi

# Run clippy
print_status "Running clippy..."
if cargo clippy --all-targets --all-features -- -D warnings; then
    print_success "Clippy passed with no warnings"
else
    print_error "Clippy found issues. Please fix them before committing."
    exit 1
fi

# Run tests
print_status "Running tests..."
if cargo test; then
    print_success "All tests passed"
else
    print_error "Tests failed. Please fix them before committing."
    exit 1
fi

# Build project
print_status "Building project..."
if cargo build; then
    print_success "Build successful"
else
    print_error "Build failed. Please fix compilation errors."
    exit 1
fi

# Check for uncommitted changes
if git rev-parse --verify HEAD >/dev/null 2>&1; then
    # Not initial commit
    if git diff-index --quiet HEAD --; then
        print_warning "No changes to commit"
        exit 0
    fi

    # Show what will be committed
    print_status "Changes to be committed:"
    git diff --name-only HEAD
else
    # Initial commit - check if there are any staged or unstaged files
    if [ -z "$(git status --porcelain)" ]; then
        print_warning "No changes to commit"
        exit 0
    fi

    # Show what will be committed for initial commit
    print_status "Initial commit - files to be added:"
    git status --porcelain | cut -c4-
fi

# Add all changes
git add .

# Get commit message
if [ -z "$1" ]; then
    echo ""
    echo "Enter commit message (press Enter for auto-generated message):"
    read -r commit_msg

    if [ -z "$commit_msg" ]; then
        # Generate automatic commit message based on changes
        commit_msg="chore: automated commit after successful clippy and tests"

        # Try to be more specific based on file changes
        if git diff --cached --name-only | grep -q "src/"; then
            commit_msg="feat: update source code - clippy clean"
        fi

        if git diff --cached --name-only | grep -q "test"; then
            commit_msg="test: update tests - all passing"
        fi

        if git diff --cached --name-only | grep -q "Cargo.toml"; then
            commit_msg="chore: update dependencies"
        fi

        if git diff --cached --name-only | grep -q "README\|docs/"; then
            commit_msg="docs: update documentation"
        fi
    fi
else
    commit_msg="$1"
fi

# Commit changes
print_status "Committing with message: '$commit_msg'"
git commit -m "$commit_msg"

# Push to origin if remote exists
if git remote | grep -q "origin"; then
    print_status "Pushing to origin..."
    if git push origin $(git branch --show-current) 2>/dev/null; then
        print_success "Successfully pushed to origin"
    else
        print_warning "Push to origin failed (this might be the first push)"
        print_status "Try: git push -u origin $(git branch --show-current)"
    fi
else
    print_warning "No origin remote configured"
fi

print_success "Development workflow completed successfully! ✅"
echo ""
echo "Summary:"
echo "  ✅ Code formatted"
echo "  ✅ Clippy passed"
echo "  ✅ Tests passed"
echo "  ✅ Build successful"
echo "  ✅ Changes committed"
echo ""
echo "Your UBO-POC project is ready for development! 🎉"
