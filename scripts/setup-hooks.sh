#!/bin/bash
# Carnelian OS - Development Environment Setup
# This script sets up Git hooks and validates the development environment.

set -e

echo "=== Carnelian OS Development Setup ==="
echo ""

# Check if prek is installed
if ! command -v prek &> /dev/null; then
    echo "❌ prek is not installed."
    echo ""
    echo "Install prek with:"
    echo "  cargo install prek"
    echo ""
    echo "Then run this script again."
    exit 1
fi

echo "✓ prek is installed"

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "❌ cargo is not installed."
    echo ""
    echo "Install Rust from https://rustup.rs"
    exit 1
fi

echo "✓ cargo is installed"

# Check Rust version
RUST_VERSION=$(rustc --version | cut -d' ' -f2)
echo "✓ Rust version: $RUST_VERSION"

# Install Git hooks
echo ""
echo "Installing Git hooks..."
prek install
echo "✓ Git hooks installed"

# Format existing code
echo ""
echo "Formatting code with cargo fmt..."
cargo fmt --all
echo "✓ Code formatted"

# Run Clippy
echo ""
echo "Running cargo clippy..."
if cargo clippy --workspace --all-targets -- -D warnings; then
    echo "✓ Clippy checks passed"
else
    echo "⚠ Clippy found issues (see above)"
    echo "  Fix issues before committing."
fi

# Build to verify everything works
echo ""
echo "Building workspace..."
if cargo build --workspace; then
    echo "✓ Build successful"
else
    echo "❌ Build failed"
    exit 1
fi

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Next steps:"
echo "  1. Make changes to the codebase"
echo "  2. Run 'cargo fmt --all' to format code"
echo "  3. Run 'cargo clippy --workspace --all-targets' to check for issues"
echo "  4. Run 'cargo test --workspace' to run tests"
echo "  5. Commit your changes (hooks will run automatically)"
echo ""
echo "To run all hooks manually:"
echo "  prek run --all-files"
echo ""
