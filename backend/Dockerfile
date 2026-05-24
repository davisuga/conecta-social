FROM rust:1.83-slim-bookworm AS chef
WORKDIR /app
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/* \
 && cargo install cargo-chef --locked --version 0.1.71

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --bin conecta-social-backend

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd -r app && useradd -r -g app app
WORKDIR /app
COPY --from=builder /app/target/release/conecta-social-backend /usr/local/bin/conecta-social-backend
COPY migrations ./migrations
USER app
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["conecta-social-backend"]
