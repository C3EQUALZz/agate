# Agate — dev task runner. Run `just` to list recipes.

[doc("List available recipes")]
default:
    @just --list

[doc("Format all code")]
fmt:
    cargo fmt --all

[doc("Check formatting without writing changes")]
fmt-check:
    cargo fmt --all -- --check

[doc("Strict lint: clippy with every warning denied")]
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

[doc("Run the workspace test suite")]
test:
    cargo test --workspace

[doc("Run tests with all features (GOST/Streebog, Ed25519, ...)")]
test-all:
    cargo test --workspace --all-features

[doc("Dependency, license and advisory audit (cargo-deny)")]
deny:
    cargo deny check

[doc("Run every git hook over all files (prek)")]
hooks:
    prek run --all-files

[doc("Build API docs without dependencies")]
doc:
    cargo doc --workspace --no-deps

[doc("Full local gate: fmt-check, lint, test, deny")]
ci: fmt-check lint test deny
