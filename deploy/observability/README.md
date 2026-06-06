# Agate observability stack (Prometheus + Grafana)

A ready-to-run **Prometheus + Grafana**, pre-provisioned with an Agate dashboard,
for visualizing the metrics Agate exposes.

## Prerequisites

Enable metrics in your `agate.toml` and publish the metrics port:

```toml
[observability.metrics]
enabled  = true
exporter = "prometheus"
bind     = "0.0.0.0:9090"
```

Run Agate with `-p 9090:9090` (or put it on the same Docker network — see below).

## Run

```bash
docker compose -f deploy/observability/docker-compose.yaml up
```

- **Grafana** → <http://localhost:3000> — opens anonymously (Viewer). The
  **“Agate — Overview”** dashboard is provisioned automatically. Log in as
  `admin` / `admin` to edit.
- **Prometheus** → <http://localhost:9091>.

By default Prometheus scrapes `host.docker.internal:9090`, i.e. an Agate whose
`:9090` is published to the host. If you instead run Agate as a container on a
shared Docker network, point `prometheus/prometheus.yml` at its container name
(e.g. `agate:9090`) and attach this stack to that network.

## Dashboard panels

| Panel | Query | What it tells you |
| --- | --- | --- |
| Runs / Denied / Dropped / Upstream errors (stats) | `*_total` | At-a-glance totals; **Dropped audit records** turns red if > 0. |
| Inspection outcomes | `sum by (outcome) (rate(agate_events_inspected_total[$__rate_interval]))` | The security picture — forward / buffer / transform (redact) / deny / terminate. |
| Runs | `rate(agate_runs_total[...])` | Proxied-run throughput. |
| Audit log writes | `rate(agate_audit_records_appended_total[...])` vs `_dropped_total` | Whether the transparency log keeps up; a rising **dropped** line is an alert. |
| Upstream errors | `rate(agate_upstream_errors_total[...])` | Failures talking to the upstream agent. |

The dashboard JSON lives at `grafana/dashboards/agate.json` — import it into an
existing Grafana, or extend it.
