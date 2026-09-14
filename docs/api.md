# HTTP API

## Ingestion

Send `POST /v1/traces/ingest` with `Authorization: Bearer KEY` and a JSON body:

```json
{
  "data": [
    {
      "object": "trace",
      "id": "trace_example",
      "workflow_name": "Research assistant"
    }
  ]
}
```

The `data` array can contain up to 1,000 `trace` and `trace.span` objects. The request body limit is 16 MiB. The server preserves original JSON and inserts or updates each record by its globally unique ID. Spans can arrive before their trace. The `OpenAI-Beta: traces=v1` header is optional.

A successful response contains `{ "accepted": number, "rejected": number }`. Unknown or malformed records count as rejected. A malformed envelope fails the whole request. An empty array returns zero for both counts.

| Status | Meaning |
| --- | --- |
| `200` | Valid records entered the queue |
| `400` | Malformed JSON or envelope |
| `401` | Missing, invalid, expired or revoked key |
| `413` | Request body exceeds 16 MiB |
| `503` | Queue full or storage writer unavailable. Wait for `Retry-After: 2`, then retry with backoff |

### Storage and retries

The queue holds accepted records in memory until SQLite commits them. The writer flushes every 200 ms under normal load, with a target of less than one second. The queue allows up to 256 batches and an estimated 64 MiB by default. `QUEUE_BYTES` changes the memory limit. At most eight ingestion requests read and parse bodies at once.

If a write fails, the server keeps the batch and retries. It refuses new ingestion while the writer is unhealthy. Revoking a key does not remove records already in the queue.

A crash can lose any uncommitted backlog, including records that have waited longer than a second. SQLite uses WAL mode with NORMAL synchronization, so power loss can also lose recent commits. Normal shutdown drains the queue. If writes still fail after 45 seconds, the service reports the remaining record count and exits with an error.

## Workspace

All `/api/*` routes except `/api/health` require a GitHub browser session. Mutations also require `X-Traces-Request: 1`. If an `Origin` header is present, it must match `PUBLIC_BASE_URL`. API keys cannot access these routes, and browser cookies cannot authorize ingestion.

| Method | Path | Response |
| --- | --- | --- |
| GET | `/api/me` | `{id,login,email}` |
| GET | `/api/status` | `{pending,queuedBytes,committed,writerHealthy,maintenanceHealthy,retentionDays}`. Counts cover the current process |
| GET | `/api/traces` | `{items,nextCursor}` |
| GET | `/api/traces/:id` | Trace summary, metadata and original trace JSON |
| GET | `/api/traces/:id/spans` | `{items}` with span summaries, without inputs, outputs or raw bodies |
| GET | `/api/traces/:id/spans/:span` | Original span JSON |
| GET | `/api/traces/:id/search?q=...` | `{ids,truncated}`, with up to 1,000 span IDs matching text in the trace's raw records |
| GET | `/api/traces/:id/export` | Streamed `{data:[original_trace,...original_spans]}` JSON download |
| DELETE | `/api/traces/:id` | `204`. Deletes the trace and its spans. Later ingestion can recreate it |
| GET | `/api/keys` | `{items,legacyEnabled}`. Returns neither key hashes nor full keys. `legacyEnabled` indicates whether `TRACE_INGEST_TOKEN` is set |
| POST | `/api/keys` | Body `{name,expiresAt?:epoch_ms}`. Returns `{id,key}` with the full key shown only in this response |
| DELETE | `/api/keys/:id` | `204`. Revokes an existing key. Repeated revocation also returns `204` |
| POST | `/auth/logout` | `204`. Deletes the session and clears its cookie |

### Trace list filters

| Parameter | Meaning |
| --- | --- |
| `q` | Literal substring across trace ID, workflow, group, model and error fields. Maximum 512 bytes |
| `status` | `errors`, `open`, `no_errors` or `all` |
| `spanType` | Step type |
| `model` | Exact model name |
| `since`, `until` | Last received update, in epoch milliseconds |
| `limit` | Page size from 1 to 100. Default 50 |
| `cursor` | Opaque cursor from the previous response's `nextCursor` |

Results sort by last received update, newest first, then by ID descending. Pagination does not freeze the results. A trace updated between requests can move between pages. The API does not return a global total count.

Timestamps in trace and span summaries use epoch milliseconds. Timestamps in raw payloads retain their original form. An open trace has no spans or at least one span without an end timestamp. No recorded errors does not prove that a run completed.

## Sign-in and health

`GET /auth/config` returns `{ready}` without credentials or the email access list.

`GET /auth/github` starts GitHub sign-in with OAuth state and PKCE. Open it as a full page navigation. The callback at `/auth/github/callback` exchanges the code, checks verified emails and creates a seven-day session. Session cookies use HttpOnly and SameSite=Lax, plus Secure on HTTPS.

`GET /api/health` returns `{ok}`. It returns `503` when the writer is unavailable or stopping. It does not require a session or expose trace data.
