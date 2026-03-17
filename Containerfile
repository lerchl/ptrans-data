FROM docker.io/library/rust:1.93 AS builder

ENV SQLX_OFFLINE=true
WORKDIR /app
COPY Cargo.toml Cargo.lock ./

RUN mkdir src
RUN echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

COPY .sqlx ./.sqlx
COPY src ./src
RUN cargo build --release

FROM docker.io/library/debian:bookworm

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
ENV DATABASE_URL=""
COPY --from=builder /app/target/release/ptrans-data /usr/local/bin/ptrans-data

EXPOSE 3000
CMD ["ptrans-data"]
