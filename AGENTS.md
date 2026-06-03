# AGENTS.md — Agate

**Agate** is a high-performance **security gateway for LLM agents** (AG2 / AG-UI protocol),
implemented in Rust as a reverse proxy. It adds policy enforcement, tamper-evident auditing,
and observability **without changing agent code**. The AG-UI event model is treated as one
adapter over a protocol-agnostic core, and the audit trail is a verifiable RFC 6962
transparency log rather than a naive hash chain.

This file is the contract for anyone (human or agent) working in this repository. Repeatable,
convention-heavy tasks are encoded as invocable skills under [`.claude/skills/`](.claude/skills).

---

## Project Structure

Cargo workspace; **one crate per bounded context**. Within a context, Clean Architecture
layers are modules, and files are grouped by DDD object type.

```
crates/
  agate-crypto/                 # generic subdomain (library): crypto agility — hash + signature
  agate-audit/                  # bounded context: append-only RFC 6962 transparency log
    src/
      lib.rs
      domain/                   # pure: no async, no I/O, no framework deps
        common/                 # seedwork (base building blocks)
          entities/  (base_entity.rs: Entity, base_aggregate.rs: AggregateRoot)
          values/    (base.rs: ValueObject, timestamp.rs, timestamps.rs)
          services/  (base.rs: DomainService)
          factories/ (base.rs: Factory)
          errors/    (base.rs: DomainError, time_errors.rs)
          events/    (base.rs: DomainEvent/EventMeta, event_id.rs, events_collection.rs)
        merkle/                 # transparency-log subdomain
          values/  entities/  services/  factories/  events.rs
        ports/                  # domain ports: Clock, IdGenerator
      application/              # CQRS use cases over a mediator pipeline
        common/                 # shared: messaging (mediator), ports, query_models
          messaging/            # Request/Command/Query, RequestHandler, Behavior+Next, Mediator
          ports/                # log/{command_gateway, query_gateway} (CQRS), KeyStore, CheckpointAnchor, EventOutbox, TransactionManager
          query_models/         # read models (DTOs) returned by query gateways
        errors/                 # base.rs: AuditError
        usecases/<name>/        # command.rs|query.rs + handler.rs
      infrastructure/           # adapters
        clock.rs  id_generator.rs               # SystemClock, UuidLogIdGenerator
        persistence/log/postgres/               # PostgresLog{Command,Query}Gateway, run_migrations
    migrations/                 # sqlx migrations (per-context Postgres schema)
    tests/                      # integration tests (incl. testcontainers Postgres under persistence/)
Cargo.toml                      # [workspace], [workspace.dependencies], [workspace.lints]
deny.toml  justfile  rustfmt.toml  .pre-commit-config.yaml
```

Per bounded context, layers are added inward-out as the context grows:
`domain → application → infrastructure → presentation`.

---

## Architecture Principles

- **Clean Architecture + DDD.** Dependencies flow **inward only**.
- **Crate = bounded context.** Each crate owns its aggregates and domain. There is **no shared
  kernel**. Cross-context *technical* capabilities (e.g. crypto) are generic-subdomain
  *libraries* with a published interface, not shared domain models.
- **The `domain` layer is pure:** no `async`, no I/O, no framework dependencies. This is
  enforced structurally by the acyclic crate graph (the compiler) — a domain crate cannot use
  what is not in its `Cargo.toml`.
- **Depend on abstractions, not implementations.** Ports are traits; concrete adapters are
  injected at the composition root (DI framework stays out of domain/application).
- **Composition over inheritance.** Rust has no inheritance; reuse via traits + default
  methods (shared behavior), struct embedding (shared data), generics, and derives.
- **Small modules, single responsibility, one concept per file.** Avoid grab-bag names
  (`Meta`, `Info`, `Data`, `Manager`) — compose meaningful parts.

### DDD building blocks (Rust)

- **Value object** — `#[derive(Clone, PartialEq, Eq, Hash)]` + `impl ValueObject`, private
  fields, validating smart constructor `new(..) -> Result<Self, DomainError>` (parse, don't
  validate); immutable (mutators return a new value).
- **Entity** — implements `Entity` (identity-based equality). Identity and lifecycle are
  composed from explicit parts (e.g. an `id` field + a `Timestamps` value object), never a
  `Meta` bag.
- **Aggregate root** — embeds `EventCollection<E>`, implements `AggregateRoot`. Construction is
  exposed **only** through a `Factory` that injects collaborators; `new`/`reconstitute` are
  `pub(crate)`.
- **Domain service** — stateless unit struct + `impl DomainService`.
- **Errors** — hierarchy modeled as nested `enum` (`DomainError::Time(TimeError)`) with
  `Error::source()`.
- **Ports** — `Clock` and `IdGenerator` are **domain ports**; persistence and external systems
  are **application ports**. Persistence is split CQRS-style: a **command gateway** loads/saves
  the aggregate (write side), a **query gateway** returns read models/DTOs (read side, possibly
  from a projection/cache) — not the aggregate. Obtaining "now"/new ids is I/O behind a port.

---

## Tooling & Standards (Mandatory)

- **Rust** edition 2024, resolver 3.
- Task runner: **just**
- Formatting: **rustfmt** (`rustfmt.toml`)
- Linting: **clippy** — `clippy::all = deny`, `clippy::pedantic = warn`; `unsafe_code = forbid`.
  The gate `cargo clippy --workspace --all-targets --all-features -- -D warnings` must be clean.
- Dependencies / licenses / advisories: **cargo-deny** (`deny.toml`)
- Git hooks: **prek** (`.pre-commit-config.yaml`)
- Time: **jiff** · Property testing: **proptest** · DI/IoC: **froodi** (wired only at the composition root)

CI and git hooks both call the **same `just` recipes** (single source of truth).

```sh
just            # list recipes
just fmt        # format
just lint       # strict clippy (-D warnings)
just test-all   # tests with all features
just deny       # cargo-deny audit
just hooks      # run all pre-commit hooks via prek
just ci         # full local gate: hooks (fmt, clippy, deny, typos, hygiene, secrets) + tests
```

Enable hooks once: `prek install && prek install --hook-type commit-msg`.

---

## Testing Rules (Mandatory)

- **Unit tests** live in-file in `#[cfg(test)] mod tests` (idiomatic Rust; whitebox access to
  internals). Do **not** extract them via `#[path]`.
- **Integration / scenario tests** live in `<crate>/tests/` and exercise the public API only.
- Cover **domain invariants** with **proptest** (e.g. Merkle proof round-trips, tamper rejection).
- Use **Arrange / Act / Assert**. No comments in tests except parametrization case descriptions.
- Name tests by behavior and expected outcome.
- Keep tests deterministic, fast, and isolated from network/IO — inject timestamps and use
  fixed key seeds; never call wall-clock or RNG directly.
- Group an integration suite as one binary (`tests/<area>.rs`) with module folders, sharing
  helpers from `tests/common/` (`fakes.rs`, `factories.rs`; `#![allow(dead_code)]`).
- Application tests run handlers over **in-memory fakes**. Database-backed gateway adapters get
  their own integration tests via **testcontainers** in the infrastructure layer; full wired
  end-to-end tests use **froodi** later. Don't add a DB/testcontainers/froodi before there's an
  adapter to test.

---

## Cryptography

- **Crypto agility:** algorithms are pluggable and self-describing (the algorithm tag travels
  with every `Digest`/`Signature`).
- Hashes: SHA-2, SHA-3, **Streebog (GOST R 34.11-2012)** — feature-gated.
  Signatures: **Ed25519** (GOST R 34.10-2012 planned).
- Hashing and signing are **pure strategies** (traits). Key loading is I/O and lives behind a
  port. A single Merkle tree uses one hash algorithm (an epoch); switching algorithms starts a
  new epoch, recorded in the signed tree head.

---

## Commit & Pull Request Rules

- **Conventional Commits**: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`, `ci:`.
- PRs include a concise description and motivation.
- CI must pass: format check, strict clippy, tests on Linux/macOS/Windows, and cargo-deny.

---

## Definition of Done

A change is complete when: `just ci` is green; new behavior has tests (unit + proptest for
invariants, scenario tests for cross-layer flows); the domain layer stays pure; public types are
constructed through their factories; and the dependency rule is preserved.
