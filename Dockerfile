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
COPY . .
RUN cargo build --release

FROM gcr.io/distroless/cc AS runtime

COPY --from=builder \
    /app/target/release/creatorsforacause \
    /

CMD [ "/creatorsforacause" ]