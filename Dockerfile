# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS dashboard
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY README.md LICENSE ./
COPY --from=dashboard /app/frontend/dist ./frontend/dist
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/rosettrism /usr/local/bin/rosettrism
ENV ROSETTRISM_DB=/data/rosettrism.sqlite
VOLUME ["/data"]
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/rosettrism"]
CMD ["server", "--host", "0.0.0.0", "--port", "8080"]
