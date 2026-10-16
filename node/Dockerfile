# Build stage
FROM rust:1-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
COPY --from=build /app/target/release/hic-node /usr/local/bin/hic-node
VOLUME /data
EXPOSE 4321
CMD ["hic-node"]
