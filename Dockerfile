# build v6 — keep runtime on rust image to avoid missing shared libs at startup
FROM rust:1.95-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    gcc \
    g++ \
    cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build -p race-server --release

FROM rust:1.95-slim

WORKDIR /app
COPY --from=builder /app/target/release/race-server /app/race-server
COPY --from=builder /app/packs /app/packs

EXPOSE 8080
ENV PORT=8080
CMD ["/app/race-server"]
