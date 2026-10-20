# The Held in Common node: run this on your own machine to be your own
# personal commons, then share to a shared commons (or keep it on your metal).
#
#   docker run -d --name hic-node -p 8080:8080 -v hic-node:/data \
#     -e HIC_INSTANCE="My node" ghcr.io/heldincommon/node
#
# Then open http://localhost:8080.

# ---- build ----
FROM rust:1-slim AS build
WORKDIR /app
COPY Cargo.toml main.rs style.css avatar.js ./
RUN cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim
# ca-certificates: sharing out to an https commons needs a trust store.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/hic-commons /usr/local/bin/hic-commons
VOLUME /data
EXPOSE 8080
ENV HIC_DATA=/data PORT=8080 HIC_INSTANCE="My node"
CMD ["hic-commons"]
