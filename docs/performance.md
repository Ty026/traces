# Performance validation

Measured 2026-09-12, on an AMD Ryzen 9 5900HS host, Linux 6.19, local NVMe SSD. The production Docker image ran as its non-root `traces` user with **2 CPU quota, 2 GiB memory, and 2 GiB total memory+swap** (no additional swap allowance). The load generator ran outside the container. This is a short mixed-load benchmark, not an endurance or capacity guarantee.

## Workload and result

The database was seeded with **1,000,000 spans**, approximately **4 KiB raw JSON per span**, 20 spans per trace, five workflows, model/tool/agent steps, and errors in 1% of spans. Pilot runs added records before the final run; exact before/after counts are in [the machine-readable report](benchmarks/sqlite-1m.json). SQLite stores raw payloads without compression. The final database was about 4.57 GiB. The synthetic padding is repetitive text, so this is not a benchmark of arbitrary production payload distributions.

Four readers targeted a combined 40 HTTP requests/s across 11 operations while one producer sent 100 spans/s for 30 seconds, then 1,000 spans/s for 10 seconds. Ingestion batches contained 10 or 100 spans, approximately 3.8 KiB each. Every response, accepted count, process commit count, and persisted span count was checked. Data and indexes were warm from seeding and pilot runs; daily backup did not overlap the final measured interval.

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

All measured read operations met the <=200 ms p95 target. Ingestion acknowledgement is queue admission; the separate persisted-count check proves these accepted records actually reached SQLite. Reported drain time is the time observed after the producer's final paced interval, not per-record durability latency. Docker-reported memory usage ranged from approximately **50–82 MiB** during sampling. Raw Docker memory/CPU samples are in [container-stats.json](benchmarks/container-stats.json); Docker memory usage excludes inactive file cache.

Before adding the normalized trigram index, the no-result search p95 was **4,019 ms** on the same million-span dataset in an unrestricted native pilot. Searching separate workflow/identifier/model/error fields removed the need to inspect all span payload pages. Input/output content remains unindexed globally. This before/after pilot identifies the bottleneck; the final acceptance numbers above come from the limited container.

## Frontend

The static frontend built successfully with zero Svelte/TypeScript diagnostics. Its JavaScript chunks total roughly 54 KiB gzip and its stylesheet about 6 KiB gzip; no external fonts or image requests are needed. The native release binary is about 8.5 MiB; the tested production image is about 94 MiB.

Real-browser tests against the Rust server verify authentication gating, static deep links, filters, pagination, returning with filters and scroll position, structured content, payload HTML escaping, export, one-time keys, ingestion-only scope, revocation, mobile layout, dialog focus/Escape, and deferred live updates. The 10,000-step fixture renders fewer than 45 tree rows at a time and requests only the selected body. A 120-frame scrolling probe measured p95 frame intervals of **18.9–27.5 ms** across runs on the host browser. This validates that browser/workload; it does not guarantee identical frame rates on every device. Screenshots in [screenshots/](screenshots/) were captured from these isolated fixtures.

## Reproduce

Use a fresh directory **on SSD** with at least 12 GiB free for the seed and a local backup. Avoid `/tmp` when it is tmpfs. The seed refuses a nonempty database and generates credentials only for that synthetic database. Never seed an operator database.

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

The script fails on HTTP/count errors or read p95 exceeding 200 ms. Increase phase durations for endurance testing. Retention volume, simultaneous long exports/backups, highly skewed traces, unusually large messages, cold cache, and much greater concurrency need separate load tests before raising the supported operating envelope.
