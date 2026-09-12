# Agent Traces

When changing ingestion, authentication, storage, or deployment, read `docs/design.md` for the agreed contracts and failure behavior.

Keep SQLite work off Tokio executor threads. The bounded ingestion queue owns accepted, uncommitted data; never silently drop it while the process is healthy. Viewer APIs require a GitHub session; ingestion keys authorize only ingestion.

The frontend in `web/` exports static files. Keep runtime APIs in Rust. Preserve original payloads, and fetch large span bodies only when needed.

Validate backend changes with `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`. Validate frontend changes with `npm --prefix web run check`, `npm --prefix web run build`, and relevant browser tests. Report benchmark hardware and payload shape alongside results.
