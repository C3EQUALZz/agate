# Design (proposal): config-driven multi-backend persistence

> Status: brainstorm / not implemented. Decisions taken: build the **seam now,
> Postgres-only**; backend chosen by an **explicit TOML enum** `[audit].backend`.

## Goal

`[audit].backend = "postgres" | "redis" | "mongodb"` in TOML drives which set of
adapters the froodi IoC container assembles at startup, all behind the same
application ports. Postgres is the only implemented backend; selecting an
unimplemented one fails clearly at startup. Adding a backend later = "new adapter
module + a `match` arm + a config variant".

## 1. Config schema

Keep `AuditSection` a **flat struct** (preserves `Default` + figment layering +
`AGATE__AUDIT__*` env overrides) with the discriminant as a unit enum field —
matching the existing `PolicyFailMode`/`ToolMode` style. Do variant validation in
`validate()`/a mapping fn, not in serde.

```rust
// agate-server: setup/configs/app_config.rs
#[serde(default)]
pub struct AuditSection { pub backend: AuditBackend, pub database_url: String }

#[serde(rename_all = "lowercase")]
pub enum AuditBackend { #[default] Postgres, Redis, Mongodb }
```

agate-audit owns a backend-neutral descriptor; both config layers map onto it:

```rust
// agate-audit: setup/configs/storage_config.rs
pub enum StorageConfig { Postgres(PostgresConfig) /* , Redis(..), Mongodb(..) */ }
```

Unknown backend → figment "unknown variant" at load. Known-but-unimplemented →
clear `Err` from `storage_config()`/`validate()` before any I/O.

## 2. The IoC seam

A connected-backend enum owns live resources and centralizes connect+migrate:

```rust
// agate-audit: infrastructure/persistence/storage.rs
pub enum Storage { Postgres(PgPool) /* , Redis(..), Mongodb(..) */ }
impl Storage {
    pub async fn connect(c: &StorageConfig) -> Result<Self, AuditError>; // connect + run_migrations
    pub fn health_check(&self) -> Arc<dyn HealthCheck>;                  // §3
}
```

`build_container(&Storage)` matches and registers a **per-backend infrastructure
provider module**. Split today's `infrastructure_providers` into:
- `providers/agnostic.rs` — clock, id-gen, factory, hasher, key-store, anchor (backend-free).
- `providers/postgres.rs` — tx slot + manager (+rollback finalizer) + the 2 gateways.

## 3. HealthCheck resolution → method on `Storage` (option a)

`build_server` is sync; froodi resolution is async and only returns concrete
types. The readiness probe is an operational concern, not a pipeline dependency,
so resolving it from the container would force `build_server` async for no gain.
Use `storage.health_check()` — the only place naming a concrete adapter is that
one match arm in agate-audit.

## 4. The hard part — do the ports abstract non-SQL stores?

| Port | Abstracts non-SQL? | Action |
| --- | --- | --- |
| `LogCommandGateway` | Yes | none (note: `save` rewrites full leaf set → O(n) without cheap upsert) |
| `LogQueryGateway` | Yes | none (shared full-scan-to-rebuild-proof cost; cache is per-adapter) |
| `HealthCheck` | Yes | none |
| `TransactionManager` | **No — assumes ACID** | relax contract to "best-effort; correctness via append-only idempotency" + document |
| `run_migrations` | n/a (not a port) | fold per-backend setup into `Storage::connect` |

**Two headline findings:**

1. **`handlers.rs` already leaks Postgres.** Handler providers `Inject` the
   *concrete* `PostgresLogCommandGateway` / `PostgresLogQueryGateway` /
   `PgTransactionManager` and coerce to `Arc<dyn Port>`. froodi resolves by
   concrete type, so **the seam is cosmetic until handlers stop naming Postgres.**
   Fix: register trait-object **newtypes** (`SharedCommandGateway(Arc<dyn …>)`,
   …) in the backend module; handlers `Inject` the newtype and read `.0`.
2. **`TransactionManager` assumes ACID begin/commit/rollback.** Redis has no
   rollback of side effects and can't read-its-writes inside `MULTI`; Mongo needs
   a replica-set `ClientSession`. The honest fix: keep the 3-method trait but
   **relax its contract** — "rollback is best-effort; non-ACID backends must be
   append-only + idempotent." The Merkle log already *is* (idempotent
   `ON CONFLICT` upserts, append-only), so Redis can implement begin=open
   pipeline / commit=atomic Lua CAS / rollback=no-op. Document it as an explicit
   decision.

## 5. Feature-flags + runtime selection

Gate each backend behind a **Cargo feature** (`default = ["postgres"]`) so unused
driver crates (redis/mongodb → BSON, TLS stacks) aren't compiled — keeps the
default build lean and `cargo deny` (`multiple-versions`) clean. Keep all
`AuditBackend` variants compiled (tiny); the `#[cfg(not(feature="redis"))]` arm of
`storage_config()`/`connect` returns an actionable error ("built without the
`redis` feature; rebuild with --features redis"). Add `deny.toml` `skip-tree`
entries only *when* a second backend lands.

## 6. Rollout

**Phase 1 (Postgres-only seam, shippable, no behavior change):** StorageConfig →
Storage + connect/health_check → newtypes → split providers + de-leak handlers →
`build_container(&Storage)` → server `AuditSection{backend,…}` + `storage_config()`
+ `build_server(&Storage)` + `main` uses `Storage::connect` → tests fixture uses
`Storage::connect` → docs + `agate.example.toml`.

**Phase 2 (add a backend, later):** optional dep + feature → implement 4 ports
(relaxed tx per §4) → config/Storage/container arms (cfg-gated) → testcontainers
fixture, run the **same** port-level integration suite against it → `deny.toml`.

## 7. Open questions for the owner

- Accept the **relaxed `TransactionManager` contract** (advisory for non-ACID),
  or split into `AcidTransactionManager` vs `BatchWriter`? (Recommend relax+doc.)
- Mongo multi-doc ACID **requires a replica set** — document as an op requirement
  or lean on the append-only escape hatch?
- `save` full-leaf rewrite is O(n) on non-upsert stores — add an incremental
  `append_leaf` to the gateway later (port change)?
