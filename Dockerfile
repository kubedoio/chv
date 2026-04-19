# Multi-stage Rust build + nginx frontend
# Usage: docker compose up -d
# Access: http://localhost:80

FROM rust:1.83-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY cmd/ cmd/
COPY crates/ crates/
COPY gen/ gen/
COPY proto/ proto/

RUN cargo build --release \
    --bin chv-controlplane \
    --bin chv-agent \
    --bin chv-stord \
    --bin chv-nwd

# Frontend build
FROM node:22-bookworm-slim AS ui-builder

WORKDIR /build/ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci --ignore-scripts
COPY ui/ .
RUN npx svelte-kit sync && npx vite build

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    nginx ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Cloud Hypervisor binary (not in Debian repos — must be provided)
# Download from: https://github.com/cloud-hypervisor/cloud-hypervisor/releases
# Or mount from host via docker-compose volume
COPY --from=builder /build/target/release/chv-controlplane /usr/local/bin/
COPY --from=builder /build/target/release/chv-agent /usr/local/bin/
COPY --from=builder /build/target/release/chv-stord /usr/local/bin/
COPY --from=builder /build/target/release/chv-nwd /usr/local/bin/

# Frontend
COPY --from=ui-builder /build/ui/build /opt/chv/ui

# Nginx config
COPY docs/examples/nginx/chv-ui.conf /etc/nginx/sites-enabled/default

# Migrations
COPY cmd/chv-controlplane/migrations /usr/local/share/chv/migrations

# Runtime dirs
RUN mkdir -p /run/chv/controlplane /run/chv/agent /run/chv/stord /run/chv/nwd \
    /var/lib/chv/storage /var/lib/chv/images /etc/chv

# Default config (auto-generates jwt_secret on first run)
RUN printf '[database]\nurl = "sqlite:///var/lib/chv/controlplane.db"\nmigrations_dir = "/usr/local/share/chv/migrations"\n' > /etc/chv/controlplane.toml

# Entrypoint starts all services
COPY deploy/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 80 8443

ENTRYPOINT ["/entrypoint.sh"]
