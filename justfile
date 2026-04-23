# Argos — development task runner
# Prerequisite: cargo install just --version 1.50.0
# Then run:     just setup

# List available recipes
default:
    @just --list

# Install all dev dependencies (run once after cloning)
setup: _cargo-tools
    uv sync --dev
    uv run pre-commit install

# Read [package.metadata.tools] from Cargo.toml and cargo install each pinned version
_cargo-tools:
    #!/usr/bin/env python3
    import subprocess, tomllib
    with open("Cargo.toml", "rb") as f:
        tools = tomllib.load(f).get("package", {}).get("metadata", {}).get("tools", {})
    for name, version in tools.items():
        print(f"  cargo install {name} --version {version}")
        subprocess.run(["cargo", "install", name, "--version", version], check=True)

# Format all code (markdown + Rust)
fmt:
    dprint fmt
    cargo fmt

# Run all pre-commit checks
check:
    uv run pre-commit run --all-files

# Run Clippy (warnings are errors)
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
    cargo test --all-targets

# Build release binary
build:
    cargo build --release

# Full local CI pass: format, lint, test
ci: fmt lint test
