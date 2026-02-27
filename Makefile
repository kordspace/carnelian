# Makefile for CARNELIAN
# Common development tasks and workflows

.PHONY: help test test-coverage test-unit test-integration test-e2e lint fmt fmt-check build build-release security-audit validate deploy ci-check clean install-dev

# Default target
help:
	@echo "CARNELIAN Development Commands"
	@echo "=============================="
	@echo ""
	@echo "Build Commands:"
	@echo "  make build          - Build debug version"
	@echo "  make build-release  - Build optimized release"
	@echo ""
	@echo "Test Commands:"
	@echo "  make test           - Run all tests"
	@echo "  make test-unit      - Run unit tests only"
	@echo "  make test-integration - Run integration tests only"
	@echo "  make test-e2e       - Run E2E tests (requires running server)"
	@echo "  make test-coverage  - Run tests with coverage report"
	@echo "  make bench          - Run performance benchmarks"
	@echo ""
	@echo "Code Quality:"
	@echo "  make lint           - Run clippy lints"
	@echo "  make fmt            - Format all code"
	@echo "  make fmt-check      - Check code formatting"
	@echo "  make security-audit - Run security audit"
	@echo ""
	@echo "Development:"
	@echo "  make install-dev    - Install development dependencies"
	@echo "  make clean          - Clean build artifacts"
	@echo "  make validate       - Validate deployment"
	@echo "  make ci-check       - Full CI check (run before commit)"
	@echo ""
	@echo "Docker:"
	@echo "  make docker-up      - Start Docker services"
	@echo "  make docker-down    - Stop Docker services"
	@echo "  make docker-logs    - View Docker logs"

# Build Commands
build:
	cargo build --all

build-release:
	cargo build --release --all

# Test Commands
test:
	@echo "Running all tests..."
	cargo test --all
	cd tests/e2e && npm test

test-unit:
	@echo "Running unit tests..."
	cargo test --lib --all

test-integration:
	@echo "Running integration tests..."
	cargo test --test integration --all

test-e2e:
	@echo "Running E2E tests..."
	cd tests/e2e && npm test

test-coverage:
	@echo "Running tests with coverage..."
	cargo tarpaulin --out Html --output-dir coverage --all
	@echo "Coverage report generated: coverage/tarpaulin-report.html"

bench:
	@echo "Running benchmarks..."
	cargo bench

# Code Quality
lint:
	cargo clippy --all -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

security-audit:
	@echo "Running security audit..."
	./scripts/security-audit.sh

# Development
install-dev:
	@echo "Installing development dependencies..."
	cd tests/e2e && npm install
	cargo install cargo-tarpaulin cargo-audit

clean:
	cargo clean
	rm -rf coverage/
	rm -rf target/

validate:
	@echo "Validating deployment..."
	./scripts/validate-deployment.sh

ci-check: fmt-check lint test security-audit
	@echo "✅ All CI checks passed!"

# Docker Commands
docker-up:
	docker-compose up -d

docker-down:
	docker-compose down

docker-logs:
	docker-compose logs -f

# Database
db-migrate:
	cargo sqlx migrate run

db-prepare:
	cargo sqlx prepare

# Release
check-release: ci-check test-coverage
	@echo "✅ Release validation complete!"
