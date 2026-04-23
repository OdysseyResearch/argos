# Argos — development task runner
# Prerequisites:
#   1. cargo install just --version 1.50.0
#   2. install uv: https://docs.astral.sh/uv/getting-started/installation/
# Then run: just setup

# List available recipes
default:
    @just --list

# Install all dev dependencies (run once after cloning)
setup: _cargo-tools
    uv sync --dev
    uv run pre-commit install

# Read [package.metadata.tools] from Cargo.toml and cargo install each pinned version
_cargo-tools:
    uv run python3 -c "import subprocess,tomllib;tools=tomllib.load(open('Cargo.toml','rb')).get('package',{}).get('metadata',{}).get('tools',{});[subprocess.run(['cargo','install',n,'--version',v],check=True) for n,v in tools.items()]"

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
