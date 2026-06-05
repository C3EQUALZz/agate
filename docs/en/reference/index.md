# Reference

The **API reference for Agate is the rustdoc** generated from the crate source.
Every public type, trait, and function is documented at its definition, so the
rustdoc is always in sync with the code.

## Build the API docs locally

```bash
cargo doc --workspace --no-deps --open
```

This opens the generated documentation for every crate in the workspace:

- `agate_crypto` — hash / signature / AEAD strategies and factories.
- `agate_audit` — the transparency-log aggregate, ports, and adapters.
- `agate_proxy` — the inspection domain and the AG-UI adapter.
- `agate_policy` — the policy domain (`InspectedAction` → `PolicyDecision`).
- `agate_server` — the composition root.

!!! tip "Where to start reading"
    Begin at each crate's top-level module documentation (`lib.rs`), which gives
    a one-paragraph orientation and links into the layered modules. The
    [Architecture](../architecture/index.md) pages mirror that structure in
    prose.

The CI gate runs `just doc`, so documentation that fails to build (broken intra-doc
links, etc.) breaks the build — the rustdoc stays trustworthy.

!!! info "Hosted rustdoc"
    Hosting the generated rustdoc alongside this site (e.g. under a `/api/`
    path) is planned. Until then, generate it locally with the command above.
