# HTTP API

## Ingestion

`POST /v1/traces/ingest` accepts `{ "data": [trace_or_span, ...] }` with `Authorization: Bearer KEY`. Response: `{ "accepted": number, "rejected": number }`. A 200 acknowledges queue admission. 400: malformed JSON/envelope; 401: invalid/expired/revoked credential; 413: request body too large; 503: retryable saturation/storage failure (Retry-After: 2). An empty valid batch is accepted with zero counts.

## Workspace

All `/api/*` routes except `/api/health` require a GitHub browser session, including read/export routes. Mutations also require `X-Traces-Request: 1` and a same-origin `Origin` header when supplied. Cookies are not accepted as ingestion credentials.

| Method | Path | Response |
| --- | --- | --- |
| GET | `/api/me` | `{id,login,email}` |
| GET | `/api/status` | `{pending,queuedBytes,committed,writerHealthy,maintenanceHealthy,retentionDays}`; counts are process-local |
| GET | `/api/traces` | `{items,nextCursor}` |
| GET | `/api/traces/:id` | Trace summary, metadata and original trace JSON |
| GET | `/api/traces/:id/spans` | `{items}` with lightweight span headers; no input/output/raw bodies |
| GET | `/api/traces/:id/spans/:span` | Original span JSON |
| GET | `/api/traces/:id/search?q=...` | `{ids,truncated}`; up to 1,000 matching span IDs from raw contents within this trace |
| GET | `/api/traces/:id/export` | Streamed `{data:[original_trace,...original_spans]}` JSON attachment |
| DELETE | `/api/traces/:id` | 204; deletes trace and spans. Future arrivals can recreate it |
| GET | `/api/keys` | `{items,legacyEnabled}`; no key digests or full credentials |
| POST | `/api/keys` | Body `{name,expiresAt?:epoch_ms}`; response `{id,key}` once |
| DELETE | `/api/keys/:id` | 204; idempotent revocation of an existing key |
| POST | `/auth/logout` | 204; destroys session and clears cookie |

List parameters: `q` (literal substring across ID/workflow/group/model/error fields, <=512 bytes), `status=errors|open|no_errors|all`, `spanType`, `model` (exact match), `since` and `until` (epoch milliseconds, last receipt), `limit` (1–100, default 50), and opaque `cursor`. Sort: last receipt descending, then ID descending. Cursors are not stable snapshots: traces updated during paging can move; no frozen history or global total count is promised. The UI preserves filters while navigating into a trace and lets users apply background updates explicitly.

All JSON timestamps for normalized headers are epoch milliseconds; raw payload timestamps remain unchanged. No recorded errors does not prove a run is complete. A trace has open/unknown state if no spans exist or any span has no end timestamp.

## OAuth and health

`GET /auth/config` returns `{ready}` without revealing credentials/allowlists. `/auth/github` starts state-bound, PKCE-protected login; `/auth/github/callback` exchanges the code, validates verified emails and creates a seven-day session. `/api/health` exposes only `{ok}` and HTTP status.
