# Agent Traces

Internal OpenAI-compatible tracing ingest service and viewer for `agent-js`.

## Setup

Create `.env` from `.env.example`:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/agent-traces
TRACE_INGEST_TOKEN=change-me
```

Install and migrate:

```bash
npm install
# Create the database first if your Postgres server does not have it yet.
npm run db:generate
npm run db:migrate
npm run dev
```

Open http://localhost:3000/traces.

## Ingest

The ingest endpoint is compatible with the OpenAI tracing exporter payload shape:

```http
POST /v1/traces/ingest
Authorization: Bearer $TRACE_INGEST_TOKEN
OpenAI-Beta: traces=v1
Content-Type: application/json

{ "data": [trace_or_span_json, ...] }
```

The route accepts `trace` and `trace.span` items, stores raw JSON, and builds indexes for the viewer.

Point the agent tracing exporter at this service by setting its endpoint to:

```txt
http://<trace-host>:3000/v1/traces/ingest
```

## Health

```bash
curl http://localhost:3000/api/health
```

## Development

```bash
npm run lint
npm run build
npm run db:studio
```
