# Contributing to CARNELIAN

Thank you for your interest in contributing to CARNELIAN! We welcome contributions from the community and are excited to work with you.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Features](#suggesting-features)
  - [Pull Requests](#pull-requests)
- [Development Guidelines](#development-guidelines)
  - [Code Style](#code-style)
  - [Testing](#testing)
  - [Documentation](#documentation)
- [Community](#community)

## Code of Conduct

This project and everyone participating in it is governed by our [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally
3. Set up the development environment
4. Create a branch for your changes

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable version)
- [Node.js](https://nodejs.org/) (for E2E tests and skill development)
- [Docker](https://docs.docker.com/get-docker/) and Docker Compose
- [PostgreSQL](https://www.postgresql.org/) (or use Docker)

### Installation

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/carnelian.git
cd carnelian

# Install Rust dependencies
cargo build

# Set up environment
cp .env.example .env
# Edit .env with your configuration

# Start development services
docker-compose up -d postgres ollama

# Run database migrations
cargo sqlx migrate run

# Run tests
cargo test --all
```

## How to Contribute

### Reporting Bugs

Before creating a bug report, please:

1. Check the [existing issues](https://github.com/kordspace/carnelian/issues) to see if the problem has already been reported
2. Try to reproduce the issue with the latest `main` branch
3. Collect information about the bug:
   - Stack traces
   - Error messages
   - Steps to reproduce
   - Expected vs actual behavior

When creating a bug report, please include:

- **Title**: Clear and descriptive
- **Description**: Detailed explanation of the issue
- **Environment**: OS, Rust version, CARNELIAN version
- **Reproduction steps**: Step-by-step instructions
- **Expected behavior**: What you expected to happen
- **Actual behavior**: What actually happened
- **Screenshots/Logs**: If applicable

### Suggesting Features

We welcome feature suggestions! Please:

1. Check if the feature has already been suggested
2. Provide a clear use case
3. Explain why this feature would be useful
4. Consider how it fits with the project's goals

Feature requests should include:

- **Title**: Clear and concise
- **Description**: What you want to achieve
- **Motivation**: Why this feature is needed
- **Proposed solution**: How you think it should work
- **Alternatives**: Other approaches you've considered

### Pull Requests

1. **Create a branch**: `git checkout -b feature/your-feature-name`
2. **Make your changes**: Follow our development guidelines
3. **Test your changes**: Run the full test suite
4. **Commit your changes**: Use clear, descriptive commit messages
5. **Push to your fork**: `git push origin feature/your-feature-name`
6. **Open a Pull Request**: Include a clear description of changes

#### Pull Request Process

1. Update the README.md if needed
2. Update documentation for any API changes
3. Ensure all tests pass
4. Ensure code is properly formatted (`cargo fmt`)
5. Address any review feedback
6. Squash commits if requested

## Development Guidelines

### Code Style

We follow standard Rust conventions:

- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common mistakes
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Write descriptive variable and function names
- Add comments for complex logic
- Keep functions focused and small

#### Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting without making changes
cargo fmt --all -- --check
```

#### Linting

```bash
# Run clippy
cargo clippy --all -- -D warnings
```

### Testing

All code should be tested:

- Write unit tests for new functionality
- Add integration tests for API changes
- Ensure E2E tests pass for user-facing changes
- Aim for high test coverage

```bash
# Run all tests
cargo test --all

# Run specific test
cargo test test_name

# Run with output
cargo test --all -- --nocapture

# Run benchmarks
cargo bench

# Run E2E tests
cd tests/e2e && npm test
```

### Documentation

- Update README.md for user-facing changes
- Add rustdoc comments for public APIs
- Update TESTING_GUIDE.md for test changes
- Update SECURITY_CHECKLIST.md for security changes
- Write clear commit messages

## Project Structure

```
carnelian/
├── crates/              # Rust crates
│   ├── carnelian-core/  # Core library
│   └── carnelian-common/# Shared types
├── skills/              # WASM skills
├── tests/               # Test suites
├── docs/                # Documentation
├── scripts/             # Utility scripts
└── monitoring/          # Observability configs
```

## Community

- **Discussions**: Use GitHub Discussions for questions
- **Issues**: Report bugs and request features via GitHub Issues
- **Security**: Report security issues privately (see SECURITY.md)

## Questions?

If you have questions, feel free to:

- Open a [GitHub Discussion](https://github.com/kordspace/carnelian/discussions)
- Check existing documentation in `docs/`
- Ask in an issue (if related to existing work)

## License

By contributing to CARNELIAN, you agree that your contributions will be licensed under the [LICENSE](LICENSE) file in this repository.

---

Thank you for contributing to CARNELIAN! 🎉
