# Study Library account server
# Build:  docker build -t study-library-server .
# Run:    docker compose up -d   (see docker-compose.yml and DEPLOY.md)

FROM rust:1.90-bookworm AS build
WORKDIR /src
# Build dependencies first so they are cached between code changes
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
 && cargo build --release --locked \
 && rm -rf src target/release/deps/study_library_server* target/release/study-library-server
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /data studylib \
 && mkdir -p /data && chown studylib:studylib /data
COPY --from=build /src/target/release/study-library-server /usr/local/bin/study-library-server
USER studylib
WORKDIR /data
# Inside the container listen on all interfaces; docker-compose.yml publishes it only on the host's 127.0.0.1
ENV BIND=0.0.0.0:5764 \
    DB_PATH=/data/accounts.db
EXPOSE 5764
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD curl -fsS http://127.0.0.1:5764/v1/config || exit 1
CMD ["study-library-server"]
