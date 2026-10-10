# Common development tasks. Run `just` to list recipes.
set shell := ["zsh", "-c"]

default:
    @just --list

# Format the entire workspace.
fmt:
    cargo fmt --all

# Fail if the workspace is not formatted.
fmt-check:
    cargo fmt --all -- --check

# Lint the workspace with clippy, denying warnings.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run the unit and integration test suite.
test:
    cargo test --workspace --all-features

# Run the editor.
run *args:
    cargo run -p stet {{args}}

# Build a release binary.
build:
    cargo build --release -p stet

# Check dependency licenses and advisories.
deny:
    cargo deny check

# Generate and open the API documentation.
doc:
    cargo doc --workspace --no-deps --open

# Everything CI runs.
ci: fmt-check lint test
