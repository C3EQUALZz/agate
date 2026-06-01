# Agate — dev task runner. Run `just` to list recipes.
# Single source of truth for commands: CI and git hooks both call these recipes.

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

[doc("Type-check the workspace (used for the MSRV gate)")]
check:
    cargo check --workspace --all-features

[doc("Run the workspace test suite")]
test:
    cargo test --workspace

[doc("Run tests with all features (GOST/Streebog, Ed25519, ...)")]
test-all:
    cargo test --workspace --all-features

[doc("Dependency, license and advisory audit (cargo-deny)")]
deny:
    cargo deny check

[doc("Build API docs, denying doc warnings")]
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

[doc("Run every git hook over all files (prek)")]
hooks:
    prek run --all-files

[doc("Full local gate: all hooks (fmt, clippy, deny, typos, hygiene, secrets) + tests")]
ci: hooks test
