# Contributing to Agate

Thanks for your interest! This guide covers local setup and the contribution
flow. Project conventions live in [`AGENTS.md`](../AGENTS.md) — read it first.

## Prerequisites

- Rust (edition 2024 / resolver 3; toolchain ≥ 1.94) via `rustup`
- [`just`](https://github.com/casey/just), [`prek`](https://github.com/j178/prek),
  [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny)

```sh
cargo install prek cargo-deny      # just is usually available via your package manager
prek install && prek install --hook-type commit-msg
```

## Workflow

1. Create a feature branch from `main`.
2. Make your change following `AGENTS.md` (Clean Architecture + DDD; pure domain;
   factories; one concept per file).
3. Run the gate: `just ci` (hooks + tests). Keep it green.
4. Open a PR. Use a **Conventional Commit** title (`feat:`, `fix:`, `refactor:`,
   `test:`, `docs:`, `chore:`, `ci:`) and fill in the PR template.

## Expectations

- New behavior is tested (unit + `proptest` for invariants; scenario tests in
  `<crate>/tests/` for cross-layer flows).
- The domain layer stays pure (no async/I/O/framework deps).
- CI (multi-OS tests, clippy `-D warnings`, cargo-deny, typos, secret scan,
  workflow security) must pass.

See also: [`AI_POLICY.md`](AI_POLICY.md), [`SECURITY.md`](SECURITY.md),
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
