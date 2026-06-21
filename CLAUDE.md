@AGENTS.md

## Working with Claude Code in this repo

- **Navigate via CodeGraph first.** A `.codegraph/` index exists — use `codegraph_explore`
  / `codegraph_node` (or `codegraph explore "<question>"`) before grep/Read when you need to
  understand or locate code. One call returns the verbatim source of the relevant symbols
  plus who calls them, in far fewer tokens than reading files yourself.
- **Delegate to the project subagents** in [`.claude/agents/`](.claude/agents):
  - `architecture-guardian` — audit Clean Architecture boundaries, domain purity, factories.
  - `ddd-builder` — scaffold value objects / entities / aggregates / contexts to convention.
  - `rust-test-author` — write AAA unit tests and proptest invariants.
  - `crypto-reviewer` — review crypto-agility code (algorithm tags, pure strategies, key ports).

  Spawn them in parallel for independent work; you coordinate and adjudicate the results.
- **Skills encode the repeatable conventions** — prefer `/add-domain-object`,
  `/add-bounded-context`, `/check-architecture`, and `/gate` over reinventing the steps.
- **The gate is the source of truth.** Run `just ci` (or the `gate` skill) before declaring a
  change done. The `domain` layer stays pure; public types are constructed through factories;
  the inward-only dependency rule holds.
- **Conventional Commits**, branch off `main`, and never commit `target/` or `.codegraph/`.

> The `codegraph` MCP server (see `.mcp.json`) requires the `codegraph` binary on PATH; if it is
> not installed, the MCP tools are simply absent and grep/Read still work.
