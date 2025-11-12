# Rust 1.91 Setup Guide for ob-poc

This document provides comprehensive instructions for setting up and using Rust 1.91 with the ob-poc project.

## 🎯 Overview

The ob-poc project has been configured to use **Rust 1.91** across all build tools, CI/CD pipelines, and development workflows. This ensures consistent behavior and takes advantage of the latest stable features.

## 🚀 Quick Setup

### Automatic Setup (Recommended)
```bash
./scripts/ensure-rust-version.sh
```

This script will:
- Check your current Rust version
- Install Rust 1.91 if needed
- Verify all required components
- Test basic functionality
- Set up development aliases

### Manual Setup
```bash
# Install Rust 1.91
rustup install 1.91
rustup default 1.91

# Add required components
rustup component add rustfmt clippy rust-src

# Verify installation
rustc --version  # Should show 1.91.x
```

## 📁 Project Configuration

### Files Updated for Rust 1.91

1. **`rust-toolchain.toml`** - Pins the project to Rust 1.91
2. **`rust/Cargo.toml`** - Specifies `rust-version = "1.91"`
3. **`.github/workflows/dead-code-housekeeping.yml`** - CI uses Rust 1.91
4. **`scripts/dead-code-sweep.sh`** - Analysis tools use `+1.91` toolchain
5. **`scripts/install-dead-code-tools.sh`** - Tool installation for 1.91
6. **`rust/dev-*.sh`** - Development scripts use `+1.91`

### Key Configuration Details

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.91"
components = ["rustfmt", "clippy", "rust-src", "rust-analyzer"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-pc-windows-msvc"]
profile = "default"
```

```toml
# rust/Cargo.toml
[package]
rust-version = "1.91"
```

## 🛠️ Development Commands

### Standard Commands (with explicit toolchain)
```bash
# Build
cargo +1.91 build
cargo +1.91 build --workspace

# Test
cargo +1.91 test
cargo +1.91 test --workspace --lib

# Check
cargo +1.91 check
cargo +1.91 check --workspace --all-targets --all-features

# Clippy
cargo +1.91 clippy
cargo +1.91 clippy --workspace --all-targets --all-features

# Format
cargo +1.91 fmt
cargo +1.91 fmt --check
```

### Development Aliases (after running setup script)
```bash
# Short aliases for faster development
cbuild91    # cargo +1.91 build
ctest91     # cargo +1.91 test
ccheck91    # cargo +1.91 check
cclippy91   # cargo +1.91 clippy
cfmt91      # cargo +1.91 fmt
```

### Quick Development Workflow
```bash
# Quick checks
./rust/dev-check.sh

# Full development workflow with commit
./rust/dev-commit.sh "your commit message"

# Dead code analysis
./scripts/dead-code-sweep.sh
```

## 🔧 Advanced Usage

### Feature Development
```bash
# Check all features work with Rust 1.91
cargo +1.91 hack check --workspace --each-feature

# Build with database features
cargo +1.91 build --features database

# Test with specific features
cargo +1.91 test --features database
```

### Analysis Tools
```bash
# Install analysis tools for Rust 1.91
./scripts/install-dead-code-tools.sh

# Run comprehensive analysis
./scripts/dead-code-sweep.sh

# Coverage analysis
cargo +1.91 llvm-cov --workspace --html
```

## 🐛 Troubleshooting

### Version Mismatch Issues

**Problem**: Commands fail with version-related errors
```bash
# Check current version
rustc --version

# Verify 1.91 is installed
rustup toolchain list | grep 1.91

# Set 1.91 as default if needed
rustup default 1.91
```

### Component Missing Errors

**Problem**: `clippy` or `rustfmt` not found
```bash
# Add missing components
rustup component add clippy rustfmt rust-src --toolchain 1.91

# Verify components
rustup component list --toolchain 1.91 --installed
```

### CI/CD Issues

**Problem**: CI fails with Rust version errors
- Ensure `.github/workflows/dead-code-housekeeping.yml` specifies `toolchain: 1.91`
- Check that all `cargo` commands in CI use `+1.91`

### Tool Installation Issues

**Problem**: Analysis tools don't work with Rust 1.91
```bash
# Reinstall tools for current toolchain
cargo install cargo-udeps --force
cargo install cargo-machete --force
cargo install warnalyzer --force

# Or use the automated script
./scripts/install-dead-code-tools.sh
```

## 🔍 Verification

### Check Setup Status
```bash
# Quick check
./scripts/ensure-rust-version.sh --check

# Full verification
./scripts/ensure-rust-version.sh --test
```

### Manual Verification
```bash
# Verify versions
rustc --version         # Should be 1.91.x
cargo --version         # Should use rustc 1.91.x

# Test compilation
cd rust/
cargo +1.91 check --workspace

# Test tools
cargo +1.91 clippy --version
cargo +1.91 fmt --version
```

## 📋 Common Workflows

### Daily Development
```bash
# Morning setup check
./scripts/ensure-rust-version.sh --check

# Regular development cycle
cd rust/
cargo +1.91 check      # Quick compile check
cargo +1.91 test       # Run tests
cargo +1.91 clippy     # Check for issues
cargo +1.91 fmt        # Format code

# Or use the dev script
./dev-check.sh          # Quick check
./dev-commit.sh "fix: update feature X"  # Full workflow + commit
```

### Code Quality Analysis
```bash
# Comprehensive dead code analysis
./scripts/dead-code-sweep.sh

# Coverage analysis
cd rust/
cargo +1.91 llvm-cov --workspace --html --output-dir target/coverage

# Dependency analysis
cargo +1.91 udeps --workspace
cargo +1.91 machete
```

### CI/CD Integration
The project is configured to automatically use Rust 1.91 in:
- GitHub Actions workflows
- Dead code analysis scripts
- Development automation

## 🎯 Benefits of Rust 1.91

### Performance Improvements
- Better compile times
- Optimized code generation
- Improved incremental compilation

### Language Features
- Enhanced pattern matching
- Better async/await support
- Improved error messages

### Tooling Enhancements
- Better IDE integration
- Improved `clippy` lints
- Enhanced `rustfmt` formatting

## 📚 Additional Resources

### Official Documentation
- [Rust 1.91 Release Notes](https://forge.rust-lang.org/channel-releases.html#191)
- [Rustup Documentation](https://rust-lang.github.io/rustup/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)

### Project-Specific Scripts
- `./scripts/ensure-rust-version.sh` - Version management
- `./scripts/dead-code-sweep.sh` - Code analysis
- `./scripts/install-dead-code-tools.sh` - Tool setup
- `./rust/dev-check.sh` - Quick development checks
- `./rust/dev-commit.sh` - Full development workflow

## 🚨 Important Notes

1. **Always use `+1.91`** when running cargo commands manually
2. **CI/CD pipelines** are configured to use Rust 1.91 automatically
3. **Development scripts** handle version selection automatically
4. **Tool installations** should be done with the setup script
5. **Project files** are configured to enforce Rust 1.91 usage

## 📞 Support

If you encounter issues with Rust 1.91 setup:

1. Run `./scripts/ensure-rust-version.sh` for automated fixes
2. Check this documentation for common issues
3. Verify your `rustup` installation is current
4. Ensure all project configuration files are up to date

---

**Status**: ✅ Rust 1.91 setup complete and operational
**Last Updated**: 2025-11-12
**Compatibility**: All ob-poc features and tools verified with Rust 1.91