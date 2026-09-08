FROM node:22-bookworm AS admin
WORKDIR /admin
COPY admin/package.json admin/package-lock.json ./
RUN npm ci
COPY admin ./
RUN npm run build

FROM rust:1-bookworm AS build
RUN apt-get update \
    && apt-get install -y --no-install-recommends git pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --bin openhub

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/openhub /usr/local/bin/openhub
COPY --from=admin /admin/dist /srv/openhub
ENV OPENHUB_HOST=0.0.0.0
ENV OPENHUB_PORT=8080
ENV OPENHUB_PUBLIC_ORIGIN=https://openhub.run
ENV OPENHUB_DATA_DIR=/data/repos
ENV OPENHUB_CELLS_DIR=/data/cells
ENV OPENHUB_CELLS_STORAGE_DIR=/data/cells-storage
ENV OPENHUB_DATABASE_PATH=/data/openhub.db
ENV OPENHUB_STATIC_DIR=/srv/openhub
EXPOSE 8080
VOLUME ["/data"]
CMD ["openhub"]
