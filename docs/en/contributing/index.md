# Contributing

Agate is a Rust workspace following Domain-Driven Design and Clean Architecture.
The full contract for contributors — human or agent — lives in `AGENTS.md` at
the repository root. This page summarizes the essentials and links to the
[Documentation Guide](documentation.md).

## Conventions

- **Conventional Commits** for commit messages and PR titles:
  `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`, `ci:`.
- **Code and identifiers are English.** Documentation *content* is bilingual
  (English primary, Russian secondary) — see the [Documentation Guide](documentation.md).
- **The domain layer stays pure** (no async, no I/O, no frameworks); public
  types are constructed through their factories; the dependency rule is
  preserved.

## The local quality gate

CI and the git hooks call the **same `just` recipes** (single source of truth):

```bash
just            # list recipes
just fmt        # format (rustfmt)
just lint       # strict clippy (-D warnings)
just test-all   # tests with all features
just deny       # cargo-deny audit
just doc        # build rustdoc (fails on broken doc links)
just ci         # full local gate: hooks (fmt, clippy, deny, typos, hygiene, secrets) + tests
```

Enable the hooks once:

```bash
prek install && prek install --hook-type commit-msg
```

## Definition of Done

A change is complete when `just ci` is green; new behavior has tests (unit +
**proptest** for invariants, scenario tests for cross-layer flows); the domain
layer stays pure; public types are constructed through their factories; the
dependency rule is preserved; **and the change is documented** (see the
[Documentation Guide](documentation.md)).

## Testing rules (essentials)

- Unit tests live in-file in `#[cfg(test)] mod tests`.
- Integration / scenario tests live in `<crate>/tests/` and exercise the public
  API only.
- Cover domain invariants with **proptest**.
- Keep tests deterministic and isolated from network/IO — inject timestamps and
  fixed key seeds; never call the wall clock or RNG directly.

See `AGENTS.md` for the complete rules.
