# Agate

**Agate is a security gateway for LLM agents.** It is an inline reverse proxy
that inspects LLM-agent traffic (AG-UI protocol first), enforces policy on what
an agent may do, and records every decision to a tamper-evident, append-only
[RFC 6962](https://www.rfc-editor.org/rfc/rfc6962) Merkle transparency log —
**without changing agent code**.

It is built as a Cargo workspace of bounded-context crates following
Domain-Driven Design and Clean Architecture:

- `agate-crypto` — crypto agility: pluggable hash / signature / AEAD strategies.
- `agate-audit` — the RFC 6962 transparency log.
- `agate-proxy` — the data-plane inspection (the event → verdict seam).
- `agate-policy` — content & authorization decisions (tool allow/deny, redaction).
- `agate-server` — the composition root that wires proxy ↔ audit ↔ policy; the
  Docker entrypoint.

See `AGENTS.md` for the full architecture and contributor contract.

## Documentation

Full documentation — overview, getting started, configuration, architecture,
and the threat model — is published with Material for MkDocs and is **bilingual
(English + Russian)**.

- 📖 **Documentation site:** `https://C3EQUALZz.github.io/agate/`
- Build it locally:

  ```bash
  python -m pip install -r docs/requirements.txt
  mkdocs serve   # http://127.0.0.1:8000
  ```

The API reference is the rustdoc: `cargo doc --workspace --no-deps --open`.

## Examples

Runnable Python examples that put an AG-UI agent **behind Agate** and show its
protections (tool-call denial, secret redaction, the audit trail) in action live
under [`examples/`](examples/) — start with [`examples/README.md`](examples/README.md).

## License

[MIT](LICENSE).
