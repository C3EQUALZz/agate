# Plan: SOLID / DI improvements

> Status: planned / not implemented. To be done on a dedicated branch **after**
> `feat/multi-backend-seam` merges into `main`. Findings come from a full
> architecture audit (crate graph, domain purity, DI wiring, `cargo deny`,
> lints — all pass); these are the four targeted gaps that remain.

## Scope

Four independent work items, ordered smallest-blast-radius first. Each is one
commit (Conventional Commits), each ends with the full local gate
(pre-commit hooks + tests) green. Items 1–2 touch `agate-proxy`'s upstream
path and are adjacent — do them back to back.

| # | Item | SOLID lens | Crates |
|---|------|-----------|--------|
| 1 | Structured `UpstreamError` | OCP, observability | agate-proxy |
| 2 | Ports (handles) in proxy DI | DIP | agate-proxy, agate-server |
| 3 | Per-section config validation | SRP, multi-backend prep | agate-server |
| 4 | Value-object encapsulation | encapsulation, conventions | agate-proxy |

Out of scope (deliberate, do **not** "fix"):

- The `match` arms in `Storage::connect` / `build_container` — that *is* the
  multi-backend seam: one explicit selection point per design
  (`docs/design/multi-backend-storage.md` §2). A registry/factory indirection
  would add machinery without removing the variant list.
- `FailModePolicy` as a decorator around `PolicyPort` — matches the project's
  behavior-vs-decorator rule (cross-cutting timeout, not use-case logic).
- Per-context `DomainError` duplication — bounded contexts stay independent; a
  shared error crate would couple them for the sake of three small enums.
- `presentation/http` using `infrastructure::ag_ui::parse_request` and the SSE
  decoder — wire-format parsing, not a store/port concern. Known exception;
  revisit only if a second wire protocol appears.

---

## 1. Structured `UpstreamError`

**Problem.** `UpstreamError(pub String)`
(`crates/agate-proxy/src/application/common/ports/upstream.rs:27`) erases the
failure kind: the reqwest adapter maps connect errors, timeouts, HTTP error
statuses, and mid-stream breaks all to `error.to_string()`
(`infrastructure/agent/reqwest_client.rs:44-50`). Metrics, logs, and the error
response cannot distinguish them, and any future retry policy or
per-kind fail mode is impossible without re-architecting.

**Change.**

- `application/common/ports/upstream.rs` — replace the newtype with an enum
  (still a plain application-layer type, no reqwest leakage):

  ```rust
  /// The upstream agent was unreachable, rejected the request, or the
  /// stream broke mid-response.
  #[derive(Debug, Clone)]
  pub enum UpstreamError {
      /// Could not establish a connection (DNS, refused, TLS).
      Connect(String),
      /// The connect or between-chunk read deadline elapsed.
      Timeout,
      /// The agent answered with a non-success HTTP status.
      Status(u16),
      /// The response stream broke or yielded an invalid frame.
      Stream(String),
  }
  ```

  Keep `Display` + `std::error::Error`; messages stay one-line and lowercase
  to match the existing log style.

- `infrastructure/agent/reqwest_client.rs` — map precisely:
  `error.is_timeout()` → `Timeout`; `error.is_connect()` → `Connect`;
  `error_for_status()` failure → `Status(code)`; chunk errors in
  `bytes_stream()` → `Stream` (or `Timeout` when `is_timeout()`).

- `presentation/http/error_handlers.rs` — map `Timeout` → **504**, everything
  else stays **502** (today everything is one status; this is the first
  user-visible payoff).

- `application/common/ports/metrics.rs` — extend
  `record_upstream_error(&self)` to `record_upstream_error(&self, kind: UpstreamErrorKind)`
  (a `&'static str` label per variant, mirroring `InspectionOutcome::label()`),
  and add the `kind` label to the counter in `ProxyMetricsRecorder`. Update the
  test fakes in `tests/application/common/fakes/`.

**Tests.** Unit tests on the adapter mapping (wiremock/httpmock fixture already
exists in `tests/integration/upstream.rs` — add cases: refused port → `Connect`,
slow body → `Timeout`, 500 → `Status(500)`). e2e: assert 504 on upstream
timeout.

**Done when** no `UpstreamError(` tuple construction remains, and
`rg "to_string\(\)" crates/agate-proxy/src/infrastructure/agent` shows no
blanket stringification.

---

## 2. Ports (handles) in proxy DI

**Problem.** The proxy handler injects concrete adapters —
`Inject<ReqwestAgentClient>`, `Inject<ProxyMetricsRecorder>`
(`presentation/http/mod.rs:53-60`) — so presentation names infrastructure,
violating the project rule "access goes through a port outside adapters".
agate-audit already solved this with the handle pattern
(`agate-audit/src/setup/ioc/handles.rs`); the proxy never adopted it. Swapping
the HTTP client or adding a second upstream provider currently means editing
presentation.

**Change.**

- New `crates/agate-proxy/src/setup/ioc/handles.rs` (same doc-comment rationale
  as audit's — froodi resolves concrete types, handles type-erase them):

  ```rust
  /// The upstream agent client for the configured provider.
  pub struct UpstreamAgentClientHandle(pub Arc<dyn UpstreamAgentClient>);

  /// The data-plane metrics recorder.
  pub struct ProxyMetricsHandle(pub Arc<dyn ProxyMetrics>);
  ```

- `setup/ioc/container.rs` — providers wrap the concrete adapters:
  the reqwest provider returns
  `UpstreamAgentClientHandle(Arc::new(ReqwestAgentClient::with_client(...)))`,
  the metrics provider `ProxyMetricsHandle(Arc::new(ProxyMetricsRecorder))`.

- `presentation/http/mod.rs` — handler takes
  `Inject(client): Inject<UpstreamAgentClientHandle>` /
  `Inject(metrics): Inject<ProxyMetricsHandle>`, reads `.0`; drop the
  `use crate::infrastructure::{ProxyMetricsRecorder, ReqwestAgentClient}`
  import and the `let metrics: Arc<dyn ProxyMetrics> = metrics;` coercion.

- Optional (decide during implementation): a
  `build_container_with_upstream(...)` test-facing constructor so e2e tests can
  inject a fake upstream without a live socket. Only add it if a test actually
  needs it — no speculative API.

**Tests.** Existing proxy e2e/integration suites must pass unchanged (they go
through the router, not the handler signature). Add no new tests unless the
optional constructor lands.

**Done when**
`rg "ReqwestAgentClient|ProxyMetricsRecorder" crates/agate-proxy/src/presentation`
returns nothing.

---

## 3. Per-section config validation

**Problem.** `AppConfig::validate()`
(`agate-server/src/setup/configs/app_config.rs:34-84`) is one flat method that
knows every section's rules, and it requires `audit.database_url`
unconditionally — a Postgres-specific rule applied regardless of
`audit.backend`. The moment a second backend lands (Phase 2 of the
multi-backend design), that check is wrong. SRP: each section owns its own
invariants; `AppConfig` only composes.

**Change.**

- Add `fn validate(&self) -> Result<(), String>` on `ProxySection`,
  `AuditSection`, `PolicySection` (and keep the existing TLS check on
  `TlsConfig` or inline it as `TlsConfig::validate`). Move each block of the
  current method verbatim — no behavior change for today's config.
- `AuditSection::validate` matches the backend:

  ```rust
  match self.backend {
      AuditBackend::Postgres => { /* database_url, pool knobs */ }
  }
  ```

  Pool-knob checks (`max_connections`, `acquire_timeout_secs`,
  `connect_backoff_secs`) move under the Postgres arm — they are pool settings,
  not generic audit settings.
- `AppConfig::validate` becomes a chain:
  `self.proxy.validate()?; self.audit.validate()?; self.policy.validate()?; self.tls.validate()`.
- Error messages keep the exact current wording (they name the TOML key and
  the env override; tests and operators rely on them).

**Tests.** Existing `validate_*` tests keep passing untouched — that is the
no-behavior-change check. Add one test pinning that the `database_url`
requirement is keyed to the Postgres backend (it documents the seam even while
only one variant exists).

**Done when** `AppConfig::validate` contains no field-level checks, only
delegation.

---

## 4. Value-object encapsulation in the proxy domain

**Problem.** Inspection value objects expose raw fields:
`ToolCallId(pub String)`, `MessageId(pub String)`, `RunId(pub Uuid)`,
`SessionId(pub Uuid)`
(`domain/inspection/values/identifiers.rs`) — any string, including blank, is
a "valid" id, and call sites poke `.0` (`context.run.0` in logging). Also
`Run::new` is `pub` (`domain/inspection/entities/run.rs:39`) while the audit
convention is `pub(crate)` construction for aggregates.

**Change.**

- `ToolCallId` / `MessageId`: private field; `new(impl Into<String>) ->
  Result<Self, DomainError>` rejecting blank (the protocol's ids are opaque but
  never empty — `DomainError::Field` already exists in the crate);
  `as_str(&self) -> &str`. Wire-side construction (`ag_ui` mapper, the
  `REQUEST_ORIGIN` synthetic ids in `application/inspection/inspector.rs`)
  switches to `new(...)`; the mapper surfaces a parse error for a blank id
  instead of smuggling it into the domain.
- `RunId` / `SessionId`: private field; `new(Uuid)` (infallible — any UUID is
  valid) + `value(&self) -> Uuid`; implement `Display` so logging becomes
  `%context.run` instead of `%context.run.0`.
- `Run::new` → `pub(crate)` (all construction is inside the crate:
  `inspect_stream` / tests). No factory — `Run` is transient per-request state,
  not a persisted aggregate; a factory would be ceremony.
- Sweep `.0` accesses across `agate-proxy` and `agate-server`
  (`rg "\.run\.0|\.session\.0|ToolCallId\(|MessageId\(" crates`) and migrate.

**Tests.** Existing domain/state-machine tests migrate mechanically. Add:
blank `ToolCallId`/`MessageId` rejected; `Display` round-trips the UUID.

**Done when** `rg "pub (String|Uuid)" crates/agate-proxy/src/domain` returns
nothing and no `.0` field access on these ids remains outside the crate's
domain module.

---

## Execution order & checkpoints

1. Branch `refactor/solid-di` off `main` (post-merge of
   `feat/multi-backend-seam`).
2. One commit per item, in table order; run the full gate after each.
3. Re-run the architecture audit (check-architecture) at the end — expected
   delta: presentation no longer references concrete adapters; everything else
   stays green.

Rollback is per-commit: items are independent except 1→2 sharing files
(merge conflicts only, no semantic coupling).
