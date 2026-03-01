# Grafana Service Account Bridge for VR Client

**Date:** 2026-02-28
**Status:** Approved

## Problem

The VR headset (northcloud-oculus) needs trusted access to Prometheus metrics and Loki logs hosted in the north-cloud Docker stack. The observability services are internal to the Docker network; only Grafana is publicly exposed via Nginx at `https://northcloud.one/grafana/`.

## Approach

Route all observability queries through Grafana's datasource proxy API (`POST /api/ds/query`) using a Grafana service account token with Viewer role. No new services, no infrastructure changes.

## Data Flow

```
Oculus Headset (northcloud-oculus)
  │
  │  HTTPS (Bearer token)
  ▼
northcloud.one/grafana/api/ds/query
  │
  │  Nginx reverse proxy
  ▼
Grafana (port 3000, Docker internal)
  ├──► Prometheus (uid: "prometheus", http://prometheus:9090)
  └──► Loki (uid: "loki", http://loki:3100)
```

One HTTP endpoint, one auth token, one request format. Grafana routes to the correct backend based on `datasource.uid` in the request body.

## north-cloud Changes

None to code or infrastructure. One-time manual step:

1. Create a Grafana service account named `oculus-vr` with **Viewer** role
2. Generate a service account token
3. Provide the token to the VR client config

Grafana already has Prometheus (uid: `prometheus`) and Loki (uid: `loki`) provisioned as datasources. Nginx already proxies `/grafana/` to Grafana.

## northcloud-oculus Changes

### Consolidate to GrafanaClient

Replace the three-client architecture (PrometheusClient, GrafanaClient, LokiClient each hitting different endpoints) with GrafanaClient as the single data path.

### Config

- `DataIngestionConfig` holds a single `GrafanaConfig` with base URL and service account token
- Base URL and token loaded from environment variables (`GRAFANA_URL`, `GRAFANA_TOKEN`)

### Prometheus queries (existing)

The current `GrafanaClient.fetch_nodes()` already sends PromQL via `POST /api/ds/query` with a datasource UID and parses metric frame responses. This works as-is with `datasource_uid: "prometheus"`.

### Loki queries (new work)

Add a `fetch_logs()` method to `GrafanaClient` that:
- Sends a Loki query via `POST /api/ds/query` with `datasource_uid: "loki"`
- Parses the Loki-style frame response (log streams with timestamp/line pairs)
- Runs the existing log analysis logic (critical/warning pattern matching) to produce `NodeStatus` entries

### Polling loop

`poll_data_sources` instantiates `GrafanaClient` calls only — one per datasource UID (prometheus, loki). No direct Prometheus or Loki HTTP calls.

## Security

- Service account token: **Viewer** role (read-only, least privilege)
- HTTPS via Nginx/Caddy TLS termination
- Token stored as environment variable, not hardcoded
- Token revocable in Grafana UI without redeployment

## Error Handling

Existing `GrafanaClient` error handling covers all cases:
- 401/403 → `AuthError` (token expired or revoked)
- Non-success HTTP → `NetworkError`
- JSON parse failure → `ParseError`
- Grafana down → warning logged, VR renders with stale data

## Decisions

| Decision | Rationale |
|----------|-----------|
| Grafana proxy over direct access | Single auth, single endpoint, no new Nginx routes |
| Service account over JWT/API key | Built-in Grafana feature, role-based, revocable |
| Viewer role | Least privilege — query only |
| Unified `/api/ds/query` over datasource proxy URLs | Consistent request/response format, one code path |
| Env vars for config | No hardcoded secrets, easy to change per environment |
