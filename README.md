# Agent Traces

Collect and inspect agent runs on your own server. Browse traces, follow model and tool calls through an execution tree and timeline, and read their inputs, outputs and errors.

Agent Traces runs as one Rust process with a Svelte UI and a local SQLite database. The server needs neither Node.js nor a separate database service.

![Trace list with workflow names, errors and durations](docs/screenshots/traces-light.png)

## Get started

Copy the example configuration:

```bash
cp .env.example .env
```

Configure [GitHub sign-in](#github-sign-in) in `.env`, then start the server with Docker Compose:

```bash
docker compose up --build -d
```

Open http://localhost:3000 and sign in. All allowed users share access to the traces and API keys.

To build from source, install Rust 1.88 or later, Node.js 24 or later, and a C compiler for SQLite. After configuring `.env`, run:

```bash
npm --prefix web ci
npm --prefix web run build
cargo run --release
```

## GitHub sign-in

Create a GitHub OAuth App under Settings > Developer settings > OAuth Apps. For local use, set:

| Setting | Value |
| --- | --- |
| Homepage URL | `http://localhost:3000` |
| Authorization callback URL | `http://localhost:3000/auth/github/callback` |

Add the app credentials and the email addresses allowed to sign in to `.env`:

```dotenv
PUBLIC_BASE_URL=http://localhost:3000
GITHUB_CLIENT_ID=your-client-id
GITHUB_CLIENT_SECRET=your-client-secret
ALLOWED_EMAILS=alice@example.com,bob@example.com
```

For a hosted instance, replace `http://localhost:3000` in all three places with your HTTPS origin, such as `https://traces.example.com`. Set up a reverse proxy to terminate TLS and forward requests to the service.

The app requests the `read:user` and `user:email` scopes. At least one verified GitHub email must match `ALLOWED_EMAILS`. Private email addresses count, and matching ignores case. An empty list blocks all sign-ins.

Sessions last seven days. To remove someone's access, remove their email from `ALLOWED_EMAILS` and restart the service. Their existing session will stop working. The app checks GitHub email verification at each sign-in and does not store GitHub access tokens.

## Send traces

Open **API keys**, create a key, and copy it before closing the dialog. Set `TRACE_API_KEY` in the shell running your exporter or request:

```bash
export TRACE_API_KEY='paste-your-key-here'
curl http://localhost:3000/v1/traces/ingest \
  -H "Authorization: Bearer $TRACE_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"data":[{"object":"trace","id":"trace_example","workflow_name":"Research assistant"}]}'
```

The endpoint accepts `trace` and `trace.span` records in a `data` array. It stores the original JSON and updates records by ID. You can send spans before their trace. Each request can contain up to 1,000 records and 16 MiB.

A `200` response reports how many records the server accepted or rejected. Accepted records enter an in-memory queue and usually reach SQLite within a second. A crash can lose records still in the queue. On `503`, wait for the `Retry-After` interval and retry with backoff. See the [API reference](docs/api.md) for record handling, limits and durability details.

Use a separate key for each agent or environment so you can revoke access independently. Keys can send traces but cannot read or delete data. Revoking a key blocks new requests. Records already accepted remain queued for storage.

You can also set `TRACE_INGEST_TOKEN` on the server. It works alongside keys created in the UI. Remove it from the environment and restart the server to revoke it.

## Inspect runs

Filter traces by time, status, step type or model. Search across trace IDs, workflow names, groups, models and errors. Within a trace, search the full step records, including inputs and outputs.

Select a step to read its messages, tool calls or raw JSON. You can resize the execution panel, copy a link to a step, or export the whole trace as JSON. Returning to the list restores your filters and scroll position. When new data arrives, a refresh prompt lets you choose when to load it.

Use `/` to focus trace search and `G`, then `T` to return to the trace list. In the execution tree, use the arrow keys to select steps and expand or collapse branches.

An **Open** trace has no steps or at least one step without an end time. **No errors** means no errors were recorded. It does not prove that the run completed.

## Deployment

Run one instance per database on a local disk. The service locks the database to prevent a second instance from opening it. Shared filesystems such as NFS and SMB are not supported.

Compose stores SQLite and backups in the `traces-data` volume and binds port 3000 to the host's loopback address. To change the host port, set both `TRACES_PORT=4180` and `PUBLIC_BASE_URL=http://localhost:4180` in `.env`, then update the OAuth App URLs.

The container runs as UID 10001, with a limit of 2 CPUs and 2 GiB of memory. If you replace the named volume with a bind mount, make the directory writable by UID 10001. Compose allows 60 seconds for shutdown and restarts the service unless you stop it. Enable Docker at boot if the service should survive a host restart.

`GET /api/health` returns `503` when the storage writer is unavailable or stopping. The workspace reports queued records and backup or cleanup failures. Logs record service errors without trace payloads or credentials.

## Configuration

| Variable | Default | Meaning |
| --- | --- | --- |
| `BIND_ADDR` | `127.0.0.1:3000` | HTTP listen address |
| `PUBLIC_BASE_URL` | `http://localhost:3000` | Browser-facing origin for OAuth and request validation |
| `TRACES_PORT` | `3000` | Host port for Docker Compose |
| `DATABASE_PATH` | `data/traces.sqlite3` | SQLite database path |
| `STATIC_DIR` | `web/build` | Built frontend files |
| `BACKUP_DIR` | `data/backups` | Backup directory |
| `RETENTION_DAYS` | `30` | Days to keep traces after their last received update. `0` disables cleanup |
| `QUEUE_BYTES` | `67108864` | Estimated queue memory limit, between 1 KiB and 1 GiB |
| `GITHUB_CLIENT_ID` / `GITHUB_CLIENT_SECRET` | empty | GitHub OAuth App credentials |
| `ALLOWED_EMAILS` | empty | Comma-separated verified email addresses allowed to sign in |
| `TRACE_INGEST_TOKEN` | empty | Optional ingestion token set in the environment |
| `RUST_LOG` | `agent_traces=info` | Log filter |

Compose sets `BIND_ADDR`, `DATABASE_PATH`, `STATIC_DIR` and `BACKUP_DIR` to paths and addresses inside the container. See [compose.yaml](compose.yaml).

## Backups and retention

Hourly cleanup deletes whole traces after `RETENTION_DAYS` without an update. Late data resets that timer. SQLite reuses the freed space, so deleting traces does not shrink the database file. Size your disk for your ingest rate, average span size and retention period.

The server creates a consistent backup daily and at startup if no recent backup exists. It keeps the latest seven `traces-*.sqlite3` files. Backups include trace content, API key hashes and sessions. Copy them to another machine if you need to recover from disk or host failure.

To create a backup while the service is running:

```bash
docker compose exec traces agent-traces backup
```

To restore a backup, stop the service first:

```bash
docker compose stop traces
docker compose run --rm traces restore /data/backups/CHOSEN_BACKUP.sqlite3
docker compose up -d
```

For a source build, use `cargo run --release -- backup` or `cargo run --release -- restore PATH_TO_BACKUP`.

Restore checks the backup's integrity and schema, saves a backup of the current database, and clears browser sessions and pending OAuth logins. API keys return to their state at the time of the snapshot. Review them after restarting, since a key revoked after the backup may be active again.

## Development

For frontend development, run these commands in separate terminals:

```bash
BIND_ADDR=127.0.0.1:3001 cargo run
npm --prefix web run dev
```

Keep `PUBLIC_BASE_URL=http://localhost:3000`. Vite forwards API, authentication and ingestion requests to Rust.

Run the checks before submitting changes:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
npm --prefix web run check
npm --prefix web run build
cargo build
npm --prefix web exec -- playwright install chromium
npm --prefix web test
```

Browser tests start a Rust server with a temporary SQLite database and test sessions. The sign-in navigation test simulates the OAuth redirect. Backend integration tests use a controlled OAuth provider to check PKCE, state replay and verified email access.

See the [API reference](docs/api.md), [design contracts](docs/design.md) and [performance results](docs/performance.md) for more detail.
