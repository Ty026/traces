# Performance

Measured on September 12, 2026, on an AMD Ryzen 9 5900HS host running Linux 6.19 with a local NVMe SSD. The Docker container ran as the non-root `traces` user with a quota of 2 CPUs and 2 GiB of memory, with no additional swap. The load generator ran outside the container. The test covered 40 seconds of concurrent reads and writes. It did not measure long-term capacity.

## Workload and results

The seed contained 1,000,000 spans with about 4 KiB of raw JSON per span, 20 spans per trace and five workflows. Spans included model, tool and agent steps, with errors in 1% of records. Preliminary runs added records before the final measurement. The [JSON report](benchmarks/sqlite-1m.json) includes the exact counts before and after the run. SQLite stores raw payloads without compression, and the final database was about 4.57 GiB. Payloads used repetitive text padding, so their size distribution does not represent production traffic.

Four readers targeted a combined 40 HTTP requests/s across 11 operations while one producer sent 100 spans/s for 30 seconds, then 1,000 spans/s for 10 seconds. Ingestion batches contained 10 or 100 spans, with approximately 3.8 KiB per span. The script checked HTTP responses, accepted counts, the process commit count and the stored span count. Seeding and preliminary runs had warmed the data and indexes. No backup ran during the measurement.

| Measurement | Result |
| --- | ---: |
| Sustained write phase | 100.0 spans/s for 30 s |
| Peak write phase | 1,000.0 spans/s for 10 s |
| New records persisted | 13,000 / 13,000 |
| HTTP / count errors | 0 |
| Ingest acknowledgement p95 | 3.88 ms |
| Recent list p95 | 2.43 ms |
| Error filter p95 | 2.76 ms |
| Step-type filter p95 | 4.46 ms |
| Model filter p95 | 3.53 ms |
| Workflow substring search p95 | 43.40 ms |
| Error-text search p95 | 42.41 ms |
| No-result substring search p95 | 2.41 ms |
| Time-range filter p95 | 2.61 ms |
| Trace header / tree / selected body p95 | 1.93 / 1.96 / 1.75 ms |
| Largest observed backlog | 200 records, about 2.4 MiB estimated queue memory |

Every measured read operation had a p95 below 200 ms. The stored span count confirmed that all 13,000 accepted records reached SQLite. Ingestion acknowledgement measures queue admission. The reported drain time starts after the producer's final paced interval, so it does not measure how long each record waited for a commit. Docker reported about 50 to 82 MiB of memory use, excluding inactive file cache. See the raw [memory and CPU samples](benchmarks/container-stats.json).

A preliminary native run without container limits measured a no-result search p95 of 4,019 ms on the same million-span dataset. Adding a trigram index over workflow, identifier, model and error fields removed the need to scan span payload pages. Global search does not index input or output content. The results in the table come from the container with CPU and memory limits, so they are not a direct comparison with the native run.

## Frontend

At the time of the test, the frontend contained about 54 KiB of gzipped JavaScript and 6 KiB of gzipped CSS. Fonts and images were served locally. The native release binary was about 8.5 MiB and the Docker image about 94 MiB.

Browser tests use the Rust server to check sign-in requirements, deep links, filters, pagination and restored scroll position. They also cover message rendering, HTML escaping, export, key creation and revocation, mobile layout, dialog controls and refresh prompts.

The 10,000-step test renders fewer than 45 tree rows at a time and fetches only the selected body. A 120-frame scrolling test measured p95 frame intervals of 18.9 to 27.5 ms across runs in the host browser. These measurements apply to that host and fixture. The [screenshots](screenshots/) come from the same test fixtures.

## Reproduce

Use a new directory on an SSD with at least 12 GiB free for the test database and a backup. Avoid `/tmp` if it uses tmpfs. The seed command refuses a nonempty database and creates credentials for the test database. Keep it separate from your application data.

```bash
cargo build --release --example seed
mkdir -p data/benchmark
target/release/examples/seed data/benchmark/traces.sqlite3 1000000 4096 > data/benchmark/credentials.json
docker build -t agent-traces:local .
# Grant the image's non-root user access to this disposable fixture only.
docker run --rm --user 0 -v "$PWD/data/benchmark:/data" --entrypoint chown agent-traces:local -R 10001:10001 /data
docker run -d --name agent-traces-benchmark \
  --cpus=2 --memory=2g --memory-swap=2g --pids-limit=64 \
  -p 127.0.0.1:4180:3000 -v "$PWD/data/benchmark:/data" \
  -e PUBLIC_BASE_URL=http://127.0.0.1:4180 \
  -e ALLOWED_EMAILS=benchmark@example.com agent-traces:local
# Wait for /api/health and the initial backup to finish; check docker logs.
python3 scripts/benchmark.py \
  --credentials data/benchmark/credentials.json \
  --database data/benchmark/traces.sqlite3 \
  --output data/benchmark/result.json --seconds 30 --peak-seconds 10
docker stats --no-stream agent-traces-benchmark
docker stop agent-traces-benchmark
docker rm agent-traces-benchmark
```

The script fails if an HTTP request or record count check fails, a write phase misses its rate target, or read p95 exceeds 200 ms. Increase the phase durations to test sustained load. Run separate tests for your expected retention volume, concurrent exports and backups, uneven trace sizes, large messages, cold caches and higher concurrency.
