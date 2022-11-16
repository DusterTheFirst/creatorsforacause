FROM rust:alpine as builder

WORKDIR /project

COPY . .

RUN cargo build --release

FROM scratch

COPY --from=builder \
    /project/target/x86_64-unknown-linux-musl/release/creatorsforacause \
    /creatorsforacause

ENTRYPOINT [ "/creatorsforacause" ]