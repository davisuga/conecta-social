FROM rust:1.87-slim-bookworm AS builder
WORKDIR /app
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*
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
