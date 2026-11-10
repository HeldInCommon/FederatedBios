# The Held in Common server, as a container: a node by default, or a commons
# with HIC_ROLE=commons.
#
#   docker run -d --name hic-node -p 8080:8080 -v hic-node:/data \
#     -e HIC_INSTANCE="My node" ghcr.io/heldincommon/hic-node
#
# Then open http://localhost:8080.

# ---- build ----
FROM rust:1-slim AS build
WORKDIR /app
# Cargo.lock pins every dependency, and --locked refuses to change it, so the
# image builds the same code every time.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
RUN cargo build --release --locked

# ---- runtime ----
FROM debian:bookworm-slim
# ca-certificates: sharing out to an https commons needs a trust store.
# curl: used by the health check below.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data hic \
    && mkdir -p /data && chown hic /data
COPY --from=build /app/target/release/hic-commons /usr/local/bin/hic-commons
USER hic
VOLUME /data
EXPOSE 8080
# A container is reached through Docker's port mapping, so it listens on all
# interfaces. (Outside Docker, a node listens only on this computer.)
ENV HIC_DATA=/data PORT=8080 HIC_BIND=0.0.0.0 HIC_INSTANCE="My node"
HEALTHCHECK --interval=30s --timeout=3s CMD curl -fsS http://127.0.0.1:8080/health || exit 1
CMD ["hic-commons"]
