FROM rust:1.88-alpine AS build
WORKDIR /opt/nillion

RUN apk add --no-cache musl-dev git pkgconf openssl-dev

COPY . .
RUN RUSTFLAGS="-Ctarget-feature=-crt-static" cargo build --release --locked -p heartbeat-funder

FROM alpine
WORKDIR /opt/nillion
RUN apk add libgcc openssl

COPY --from=build /opt/nillion/target/release/heartbeat-funder /opt/nillion
ENTRYPOINT ["/opt/nillion/heartbeat-funder"]
