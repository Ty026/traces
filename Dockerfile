FROM node:24-bookworm-slim AS web
WORKDIR /build/web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run check && npm run build

FROM rust:1.98-bookworm AS rust
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && printf 'fn main() {}\n' > src/main.rs && cargo build --locked --release && rm -rf src
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs src/lib.rs && cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home traces && mkdir -p /data/backups && chown -R traces:traces /data
COPY --from=rust /build/target/release/agent-traces /usr/local/bin/agent-traces
COPY --from=web /build/web/build /app/web
ENV BIND_ADDR=0.0.0.0:3000 DATABASE_PATH=/data/traces.sqlite3 BACKUP_DIR=/data/backups STATIC_DIR=/app/web
USER traces
EXPOSE 3000
VOLUME /data
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s CMD curl -fsS http://127.0.0.1:3000/api/health || exit 1
ENTRYPOINT ["agent-traces"]
