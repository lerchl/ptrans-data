FROM docker.io/library/rust:1.93 AS builder

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

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

ENV DATABASE_URL=""
EXPOSE 3000
CMD ["/app/target/release/ptrans-data"]
