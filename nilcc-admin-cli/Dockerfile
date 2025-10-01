FROM rust:1.88-alpine AS build

WORKDIR /opt/nillion
RUN apk add --no-cache musl-dev

COPY . .
RUN cargo build --release --locked -p nilcc-admin-cli

FROM alpine

WORKDIR /opt/nillion

COPY --from=build /opt/nillion/target/release/nilcc-admin-cli /opt/nillion
ENTRYPOINT ["/opt/nillion/nilcc-admin-cli"]
