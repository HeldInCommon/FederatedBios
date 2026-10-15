# syntax=docker/dockerfile:1

# Build and runtime share a Debian release so the binary's glibc matches.
FROM rust:1-slim-trixie AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --locked && cp target/release/hic-node /usr/local/bin/hic-node

FROM debian:trixie-slim
# Run as an unprivileged user. /data is created here, owned by that user, so a
# fresh named volume mounted over it inherits the ownership.
RUN useradd --system --uid 10001 --no-create-home hic \
    && mkdir /data && chown hic /data
COPY --from=build /usr/local/bin/hic-node /usr/local/bin/hic-node
USER hic
ENV HIC_ADDR=0.0.0.0:4321 \
    HIC_DATA_DIR=/data
EXPOSE 4321
VOLUME /data
ENTRYPOINT ["hic-node"]
