## Summary

<!-- What does this PR do, and why? -->

## Motivation / context

<!-- Link issues; explain the design choice if non-obvious. -->

## Changes

-

## Checklist

- [ ] `just ci` is green (fmt, clippy `-D warnings`, tests, cargo-deny)
- [ ] New behavior is tested (unit + proptest for invariants; scenario tests in `tests/` for cross-layer flows)
- [ ] Domain layer stays pure (no async / I/O / framework deps)
- [ ] Public types are constructed via their factories; dependency rule preserved
- [ ] PR title follows Conventional Commits
