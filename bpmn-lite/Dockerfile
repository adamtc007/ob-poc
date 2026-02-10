FROM rust:1.84-bookworm AS builder

RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release -p bpmn-lite-server --features postgres

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bpmn-lite-server /usr/local/bin/

EXPOSE 50051

CMD ["bpmn-lite-server"]
