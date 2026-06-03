---
name: add-bounded-context
description: Create a new bounded-context crate in the Agate workspace (e.g. agate-policy, agate-proxy) with the correct layout, lints, dependencies, and dependency direction. Use when introducing a new domain area as its own crate.
---

# Add a bounded context (crate)

Each bounded context is its **own crate** under `crates/`. It owns its aggregates and domain.
There is **no shared kernel**; if two contexts need the same *technical* capability, publish it
as a generic-subdomain library crate (like `agate-crypto`), not a shared domain model.

## Steps

1. **Create the crate** `crates/agate-<context>/` with `Cargo.toml`:
   ```toml
   [package]
   name = "agate-<context>"
   version.workspace = true
   edition.workspace = true
   license.workspace = true
   description = "<one line>"

   [dependencies]
   # only inward deps: other domain/generic-subdomain crates. NO async runtimes,
   # NO hyper/axum/reqwest/extism in a crate that exposes a pure domain.

   [lints]
   workspace = true
   ```
   If other crates depend on it, register it in root `[workspace.dependencies]` **with a
   version** (`{ path = "...", version = "0.1.0" }`) to avoid a cargo-deny wildcard.

2. **Lay out layers as modules** inside the crate:
   ```
   src/lib.rs            # pub mod domain; (then application/infrastructure as they appear)
   src/domain/
     common/             # seedwork: entities/ values/ services/ factories/ errors/ events/
     <subdomain>/        # values/ entities/ services/ factories/ events.rs
   tests/                # integration / scenario tests
   ```
   `common/` mirrors `agate-audit`'s seedwork (base `Entity`, `AggregateRoot`, `ValueObject`,
   `DomainService`, `Factory`, `DomainError`, event machinery). Keep it per-crate — do **not**
   extract a cross-crate kernel.

3. **Preserve the dependency rule.** Dependencies flow inward only:
   `presentation → infrastructure → application → domain`. The crate graph is acyclic (Cargo
   enforces it); keep the *domain modules* free of async/I/O/framework imports.

4. **Wire modules:** every file is declared `pub mod ..` in its parent `mod.rs` and re-exported.

5. **Verify** with the `check-architecture` skill, then `just ci`.

See `add-domain-object` for populating the new context, and `AGENTS.md` for the conventions.
