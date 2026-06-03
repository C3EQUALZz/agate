---
name: gate
description: Run Agate's full local quality gate (pre-commit hooks + tests) and report each step. Use before committing or pushing, when asked to "verify the build" or "check everything passes", or after a non-trivial change to confirm nothing regressed.
---

# Quality gate

Run the gate and report the outcome of each stage:

```sh
just ci
```

`just ci` runs `just hooks` (all pre-commit hooks via prek) then `just test`:

1. **Hooks** (`prek run --all-files`): file hygiene (trailing whitespace, EOF, line endings,
   large files, YAML/TOML, merge conflicts), **gitleaks** (secrets), **typos**, and the cargo
   hooks — `just fmt-check`, `just lint` (clippy `-D warnings`), `just deny` (cargo-deny).
2. **Tests** (`just test`): unit, proptest, and integration tests.

CI runs the equivalent via the same `just` recipes (see `.github/workflows/ci.yml`); the test
matrix uses `just test-all` across Linux/macOS/Windows.

## Interpreting failures

- **fmt** → `just fmt`, re-run.
- **clippy** → fix the real issue; avoid `#[allow]` unless it is a known false positive (then
  curate it in `[workspace.lints.clippy]` with a one-line rationale).
- **typos / hygiene / gitleaks** → fix the flagged content; never commit a real secret — rotate it.
- **deny** → see `deny.toml`; workspace path crates need a `version` and `allow-wildcard-paths`.
- **tests** → show the failing output; fix the cause, not the assertion.

Prerequisites: `just`, `prek`, and `cargo-deny` installed. Report results plainly and do not
claim success unless every stage is green.
