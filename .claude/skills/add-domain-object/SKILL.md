---
name: add-domain-object
description: Scaffold a new DDD domain object — value object, entity, aggregate, domain service, or factory — inside a bounded-context crate, following Agate's exact conventions. Use when adding domain types under crates/agate-*/src/domain (e.g. "add a value object", "create an aggregate", "add a domain service").
---

# Add a domain object

The `domain` layer is **pure**: no `async`, no I/O, no framework deps. One concept per file,
grouped by type folder (`values/`, `entities/`, `services/`, `factories/`, `events/`, `ports/`).
After adding a file, declare it (`pub mod <name>;`) in the parent `mod.rs` and re-export the
type. Never use grab-bag names (`Meta`, `Info`, `Data`, `Manager`).

## Value object (`values/<name>.rs`)
- `#[derive(Clone, PartialEq, Eq, Hash)]` (add `Copy`, `PartialOrd`, `Ord` when natural).
- `impl ValueObject for T {}`.
- Private fields; expose a validating smart constructor `new(..) -> Result<Self, DomainError>`
  (parse, don't validate). Immutable: "mutators" return a new value.

## Entity (`entities/<name>.rs`)
- `impl Entity` with an associated `Id` type; equality is identity-based.
- Compose identity + lifecycle from explicit parts: an `id` field plus a `Timestamps` value
  object. Do **not** introduce an `EntityMeta`/`Meta` bag.

## Aggregate root (`entities/<name>.rs`)
- Embed `EventCollection<E>` for its event enum; `impl AggregateRoot`.
- `new`/`reconstitute` are `pub(crate)` and take already-validated VOs + ready collaborators.
- Public construction goes **only** through a `Factory` (see below). Record domain events on
  state-changing commands.

## Domain service (`services/<name>.rs`)
- Stateless unit struct, associated functions, `impl DomainService`. Inject strategies/ports by
  reference (`&dyn Trait`). Pure computation only.

## Factory (`factories/<name>_factory.rs`)
- Holds injected collaborators (e.g. `Arc<dyn Hasher>`); `create(..)` / `reconstitute(..)`
  assemble the aggregate (build `Timestamps`, fresh `EventCollection`). `impl Factory`.

## Events (`events.rs`)
- An `enum` per context implementing `DomainEvent` (`event_type(&self) -> &'static str`).

## Tests
- Add **in-file** `#[cfg(test)] mod tests`. Cover invariants with **proptest**. Cross-layer or
  public-API scenarios go in `<crate>/tests/`.

Finish with `just ci` (see the `gate` skill).
