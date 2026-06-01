# AI Policy

Agate is developed with substantial help from AI coding assistants, and it is
itself a gateway for AI agents. This policy sets expectations for AI-assisted
contributions.

## Principles

- **Human accountability.** The contributor who opens a PR is fully responsible
  for every line, regardless of whether an AI generated it. "The AI wrote it" is
  not an explanation for a bug or a license/security issue.
- **Understand before you submit.** Do not submit AI-generated code you cannot
  explain. Reviews assume the author understands the change.
- **Same bar as any code.** AI-generated changes must pass the full gate
  (`just ci`: format, strict clippy, tests, cargo-deny) and follow the
  conventions in [`AGENTS.md`](../AGENTS.md). Domain purity, factories, and the
  dependency rule are not negotiable.
- **No secrets to third parties.** Do not paste credentials, private keys, or
  non-public data into AI tools.
- **Provenance & licensing.** Do not submit AI output that reproduces
  copyrighted code or incompatible licenses. When in doubt, rewrite.
- **Tests are not AI-only.** New behavior needs real tests (unit + proptest for
  invariants); do not rely on AI assertions you have not verified.

## For AI agents working in this repo

`AGENTS.md` is the contract; `.claude/skills/` encodes repeatable tasks
(`gate`, `add-domain-object`, `add-bounded-context`, `check-architecture`).
Prefer those conventions over generic patterns, and verify with `just ci`.
