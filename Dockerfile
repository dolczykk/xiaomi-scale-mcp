FROM rust:1.97-bullseye AS builder

WORKDIR /src

COPY xiaomi-client xiaomi-client
COPY xiaomi-scale-mcp xiaomi-scale-mcp
COPY docker/Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock

RUN ls -la
RUN cargo build -r --package xiaomi_scale_mcp --target-dir=/app/target

FROM debian:bullseye-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/xiaomi_scale_mcp xiaomi_scale_mcp
COPY config.example.toml config.toml

CMD [ "./xiaomi_scale_mcp" ]
