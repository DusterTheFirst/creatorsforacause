FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef

# Apt dependencies
RUN apt update && apt install -y protobuf-compiler lld

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY --chown=root:root . .
RUN set -eux; \
    # Make Git happy (fly.toml does not get copied when running `fly deploy`)
    git restore fly.toml; \
    cargo build --release; \
    objcopy --compress-debug-sections target/release/creatorsforacause /app/creatorsforacause

FROM gcr.io/distroless/cc AS runtime

COPY --from=builder \
    /app/creatorsforacause \
    /creatorsforacause

CMD [ "/creatorsforacause" ]