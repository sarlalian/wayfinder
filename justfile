# Default recipe - show available commands
default:
    @just --list

# Build the project
build:
    cargo build

# Build in release mode
build-release:
    cargo build --release

# Run all tests
test:
    cargo test

# Run only library tests
test-lib:
    cargo test --lib

# Run integration tests
test-integration:
    cargo test --test integration_tests

# Run parser tests
test-parser:
    cargo test --test parser_tests

# Run engine tests
test-engine:
    cargo test --test engine_tests

# Run reporting tests
test-reporting:
    cargo test --test reporting_tests

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run a specific test
test-one TEST:
    cargo test {{TEST}}

# Run tests and show only failures
test-failures:
    cargo test 2>&1 | grep -E "(FAILED|failed|error)"

# Check code without building
check:
    cargo check

# Check all targets
check-all:
    cargo check --all-targets

# Run clippy linter
lint:
    cargo clippy

# Run clippy with all features and strict settings
lint-strict:
    cargo clippy --all-targets --all-features -- -D warnings

lint-quality:
    cargo audit

lint-outdated:
    cargo outdated

# Format code
fmt:
    cargo fmt

# Check if code is formatted
fmt-check:
    cargo fmt -- --check

# Clean build artifacts
clean:
    cargo clean

# Update dependencies
update:
    cargo update

# Generate documentation
doc:
    cargo doc

# Generate and open documentation
doc-open:
    cargo doc --open

# Run the CLI (requires building first)
run *ARGS:
    cargo run -- {{ARGS}}

# Run with release optimizations
run-release *ARGS:
    cargo run --release -- {{ARGS}}

# Execute a workflow file
execute WORKFLOW:
    cargo run -- execute {{WORKFLOW}}

# Execute with verbose output
execute-verbose WORKFLOW:
    cargo run -- execute {{WORKFLOW}} --verbose

# Validate a workflow file
validate WORKFLOW:
    cargo run -- validate {{WORKFLOW}}

# Show workflow info
info WORKFLOW:
    cargo run -- info {{WORKFLOW}}

# Development workflow - format, check, test
dev:
    just fmt
    just check
    just test-lib

# Full CI workflow - format, lint, test everything
ci:
    just fmt-check
    just lint-strict
    just test

# Quick development check (fast)
quick:
    just check
    just test-lib

# Install from source
install:
    cargo install --path .

# Benchmark performance (if benchmarks exist)
bench:
    cargo bench

# Run with logging enabled
run-debug *ARGS:
    RUST_LOG=debug cargo run -- {{ARGS}}

# Run with trace logging
run-trace *ARGS:
    RUST_LOG=trace cargo run -- {{ARGS}}

# Show dependency tree
deps:
    cargo tree

# Check for security vulnerabilities
audit:
    cargo audit

# Show outdated dependencies
outdated:
    cargo outdated

# Run tests with coverage (requires cargo-tarpaulin)
coverage:
    cargo tarpaulin --out html

# Fix code issues automatically
fix:
    cargo fix --allow-dirty
    cargo clippy --fix --allow-dirty

# Create a new workflow template
new-workflow NAME:
    @echo "Creating new workflow template: {{NAME}}"
    @mkdir -p examples
    @echo 'name: {{NAME}}' > examples/{{NAME}}.yaml
    @echo 'description: "Generated workflow template"' >> examples/{{NAME}}.yaml
    @echo 'version: "1.0"' >> examples/{{NAME}}.yaml
    @echo '' >> examples/{{NAME}}.yaml
    @echo 'variables:' >> examples/{{NAME}}.yaml
    @echo '  environment: "development"' >> examples/{{NAME}}.yaml
    @echo '' >> examples/{{NAME}}.yaml
    @echo 'tasks:' >> examples/{{NAME}}.yaml
    @echo '  example_task:' >> examples/{{NAME}}.yaml
    @echo '    type: command' >> examples/{{NAME}}.yaml
    @echo '    config:' >> examples/{{NAME}}.yaml
    @echo '      command: echo' >> examples/{{NAME}}.yaml
    @echo '      args: ["Hello from {{NAME}}"]' >> examples/{{NAME}}.yaml
    @echo '    timeout: 30s' >> examples/{{NAME}}.yaml
    @echo '' >> examples/{{NAME}}.yaml
    @echo 'output:' >> examples/{{NAME}}.yaml
    @echo '  destination: "file://./{{NAME}}_output.json"' >> examples/{{NAME}}.yaml
    @echo "Created examples/{{NAME}}.yaml"

# Watch for file changes and run tests (requires cargo-watch)
watch:
    cargo watch -x "test --lib"

# Watch and run specific test
watch-test TEST:
    cargo watch -x "test {{TEST}}"

# Profile the application (requires cargo-profiler)
profile WORKFLOW:
    cargo build --release
    perf record --call-graph dwarf target/release/wayfinder execute {{WORKFLOW}}
    perf report

# Check binary size
size:
    cargo build --release
    @echo "Binary size:"
    @ls -lh target/release/wayfinder | awk '{print $5, $9}'

# Run examples
example EXAMPLE:
    cargo run -- execute examples/{{EXAMPLE}}.yaml

# List all examples
examples:
    @echo "Available examples:"
    @ls examples/*.yaml 2>/dev/null | sed 's/examples\///; s/\.yaml//' | sort || echo "No examples found"

# Package for distribution
package:
    cargo build --release
    @mkdir -p dist
    cp target/release/wayfinder dist/
    cp -r examples dist/ 2>/dev/null || true
    cp README.md dist/ 2>/dev/null || true
    @echo "Package created in dist/"

# Setup development environment
setup:
    @echo "Setting up development environment..."
    rustup component add rustfmt clippy
    cargo install cargo-watch cargo-audit cargo-outdated cargo-tarpaulin
    @echo "Development tools installed!"

# Show project statistics
stats:
    @echo "=== Project Statistics ==="
    @echo "Lines of code:"
    @find src -name "*.rs" | xargs wc -l | tail -n1
    @echo ""
    @echo "Test files:"
    @find tests -name "*.rs" | wc -l
    @echo ""
    @echo "Example workflows:"
    @ls examples/*.yaml 2>/dev/null | wc -l || echo "0"
    @echo ""
    @echo "Dependencies:"
    @grep -c "^[a-zA-Z]" Cargo.toml | head -n1

# Release preparation checklist
release:
    @echo "=== Release Checklist ==="
    just fmt-check
    just lint-strict
    just test
    just doc
    @echo "✓ All checks passed!"
    @echo ""
    @echo "Manual steps:"
    @echo "1. Update version in Cargo.toml"
    @echo "2. Update CHANGELOG.md"
    @echo "3. Create git tag"
    @echo "4. Run: cargo publish"
