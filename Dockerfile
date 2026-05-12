# build v4 — defensive Covalent null handling
FROM rust:1.85-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build -p race-server --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/race-server /app/race-server
COPY --from=builder /app/packs /app/packs

EXPOSE 3001
ENV PORT=3001
CMD ["/app/race-server"]
