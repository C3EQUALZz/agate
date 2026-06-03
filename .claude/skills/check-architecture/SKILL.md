---
name: check-architecture
description: Audit Agate's architectural boundaries — Clean Architecture dependency rule, domain purity (no async/I/O/framework deps in the domain), crate-graph direction, and dependency hygiene. Use after structural changes, before merging, or when asked to verify layering / "import-linter"-style boundaries.
---

# Architecture audit

Rust has no dedicated import-linter; boundaries are enforced primarily by the **acyclic crate
graph** (the compiler) plus the checks below. Report findings as pass/fail with file:line
evidence; do not "fix" by loosening a rule without flagging the trade-off.

## 1. Crate-graph direction
- `cargo metadata` / `cargo tree -e normal` — confirm dependencies flow inward only and there
  are no cycles (Cargo rejects cycles, but verify intent: domain crates depend on nothing
  internal; application on domain; infrastructure on application+domain).

## 2. Domain purity
A crate exposing a pure domain must not pull async runtimes, transport, or sandbox deps.
- Check each domain crate's `Cargo.toml` has **none** of: `tokio`, `hyper`, `axum`, `reqwest`,
  `extism`, `async-std`.
- Check `src/domain/` for forbidden constructs:
  ```sh
  rg -n "async fn|tokio::|reqwest::|std::fs|std::net" crates/*/src/domain && echo "VIOLATION" || echo "clean"
  ```
  The domain is pure: time and id generation go through `Clock` / `IdGenerator` ports; signing
  through injected strategies; persistence through application ports.

## 3. Construction & encapsulation
- Aggregates are created only via factories: aggregate `new`/`reconstitute` should be
  `pub(crate)`, the `Factory` `pub`. Flag any external direct construction.
- No grab-bag types (`Meta`, `Info`, `Data`, `Manager`) in the domain.

## 4. Dependency hygiene
- `cargo deny check bans` — no wildcards (except workspace path crates), no banned/duplicate deps.
- `cargo deny check advisories licenses sources`.

## 5. Lints as guardrails
- Confirm every crate has `[lints] workspace = true`, and `unsafe_code = "forbid"` holds.

Summarize: which checks passed, any violations with evidence, and a concrete remediation for
each. Re-run after fixes.
