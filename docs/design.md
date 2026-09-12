# Agreed design

Replace Next.js/PostgreSQL with Rust (Axum/Tokio), SvelteKit static export, and local SQLite. One service instance, local SSD, no shared network filesystem. Old Postgres data stays untouched; no migration.

## Ingestion contract

`POST /v1/traces/ingest`, Bearer authentication, `{ "data": [ ... ] }`, at most 1,000 object records. `trace` and `trace.span` retain original JSON. Invalid items are counted as rejected; malformed envelopes return 400. Upserts and spans arriving before traces must work. IDs remain globally unique.

200 `{accepted,rejected}` means admission to a bounded in-memory queue. Flush within one second under normal load. Retain failed batches and retry; stop admitting while the writer is unhealthy. Saturation returns retryable 503. Abrupt termination can lose all uncommitted records, including backlogs older than one second. Normal shutdown drains the queue. Limit queued bytes as well as batch count.

## Authentication

GitHub OAuth checks verified emails from `/user/emails` against exact addresses in `ALLOWED_EMAILS` (case-insensitive). Empty allowlist denies access. All admitted users administer the shared dataset and keys. Cookie sessions protect viewer/read/export/delete/key APIs. OAuth state, PKCE, HttpOnly cookies and same-origin mutation checks protect browser authentication. API keys authorize ingestion only: names, optional expiry, last usage, revocation, one-time plaintext display, hashes stored. Optional `TRACE_INGEST_TOKEN` remains environment-managed and identified in the UI.

`PUBLIC_BASE_URL` determines `/auth/github/callback`. Operator supplies OAuth credentials. Real GitHub verification follows when supplied; tests use a controlled OAuth provider, never a production authentication bypass.

## Data and UI

Retain entire traces for 30 days since last receipt; configurable, 0 disables cleanup. Daily consistent backups, retain last 7; CLI restore while stopped. Local backups support recovery; off-machine replication is operator-managed.

English developer-console UI: neutral colors, thin separators, light/dark. Find a trace, inspect virtualized tree/waterfall, read structured messages and tool calls. Cursor pagination; date, status, type, model and text filters; restore list filters/scroll. Resizable panes, lazy span payloads, copy, JSON export, span deep links, keyboard navigation, unobtrusive refresh, clear loading/empty/error states. Absence of errors is not proof of completion. Global normalized-field search plus content search inside one trace. Global payload search and aggregate analytics are outside v1.

## Delivery and performance acceptance

Binary, Docker image, Compose, documentation, local tests and reproducible benchmarks. Formal deployment is separate. Target: 2 CPU cores, 2 GiB RAM, SSD; 1 million spans; sustained 100 spans/s, short peaks of 1,000 spans/s; common list/filter API p95 <=200ms; smooth 10,000-span tree with lazy bodies. Publish hardware, query mix, concurrency, payload sizes, duration, throughput, queue drain and limitations. Test million-span reads during ingestion. Fast enqueue responses alone do not demonstrate SQLite throughput.
