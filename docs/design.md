# Design contracts

Agent Traces runs as one Rust service using Axum and Tokio. It serves a SvelteKit static build and stores data in SQLite on a local SSD. Run one instance per database. Shared network filesystems are not supported.

## Ingestion

`POST /v1/traces/ingest` uses Bearer authentication and accepts `{ "data": [ ... ] }` with at most 1,000 object records. Preserve the original JSON for `trace` and `trace.span`. Count invalid records as rejected and return `400` for malformed envelopes. Support updates by globally unique ID and spans arriving before their trace.

A `200` response with `{accepted,rejected}` acknowledges admission to a bounded in-memory queue. The queue owns accepted records until they commit. Limit both queued bytes and batch count. Flush within one second under normal load.

Keep failed batches and retry them. Stop accepting new ingestion while the writer is unhealthy. Return a retryable `503` when the queue is full. Abrupt termination can lose all uncommitted records, including records older than one second. Drain the queue during normal shutdown.

Keep SQLite work off Tokio executor threads.

## Authentication

GitHub OAuth checks verified addresses from `/user/emails` against `ALLOWED_EMAILS`. Require an exact match, ignoring case. An empty access list denies everyone. All allowed users administer the same traces and keys.

Cookie sessions protect viewer, read, export, delete and key-management APIs. Use OAuth state, PKCE, HttpOnly cookies and same-origin mutation checks. API keys authorize ingestion only. Support key names, optional expiration, last usage and revocation. Show each full key once and store its hash. Identify the optional `TRACE_INGEST_TOKEN` separately in the UI because the server administrator manages it through the environment.

Build the `/auth/github/callback` URL from `PUBLIC_BASE_URL`. GitHub sign-in requires OAuth App credentials. Tests use a controlled OAuth provider without adding a production authentication bypass.

## Retention and recovery

Keep each trace for 30 days after its last received update by default. Make retention configurable and let `0` disable cleanup. Create consistent backups daily and keep the latest seven. Restore through the CLI while the service is stopped. Administrators are responsible for copying backups off the machine.

## Viewer

Use neutral colors, thin separators and light and dark themes. Let users find a trace, follow its execution tree and timeline, and read model messages and tool calls.

Support cursor pagination and date, status, step type, model and text filters. Restore list filters and scroll position when returning from a trace. Let users resize panels, copy content, export JSON, link to a step and navigate with the keyboard. Fetch large span bodies only when selected. Show loading, empty and error states, and offer new data without interrupting the current view.

Search normalized fields across traces and raw content within one trace. Global payload search and aggregate analytics are outside the current scope. Do not treat the absence of errors as proof of completion.

## Delivery and performance targets

Provide a binary, Docker image, Compose configuration, documentation, tests and reproducible benchmarks.

Target a host with 2 CPU cores, 2 GiB RAM and an SSD:

- Store 1 million spans.
- Sustain 100 spans/s and handle short peaks of 1,000 spans/s.
- Keep common list and filter API latency at p95 of 200 ms or less during ingestion.
- Keep a 10,000-span tree responsive while fetching bodies on demand.

Report hardware, query mix, concurrency, payload sizes, duration, throughput, queue drain and limitations with benchmark results. Verify that accepted records reach SQLite. Queue admission latency alone does not measure write throughput.
