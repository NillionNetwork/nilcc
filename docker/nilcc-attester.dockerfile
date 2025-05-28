FROM rust:1.86-alpine AS build

WORKDIR /opt/nillion
RUN apk add --no-cache musl-dev git

COPY . .
RUN cargo build --release --locked -p nilcc-attester

FROM alpine
RUN apk add --no-cache curl
WORKDIR /opt/nillion
COPY --from=build /opt/nillion/target/release/nilcc-attester /opt/nillion
ENTRYPOINT ["/opt/nillion/nilcc-attester"]

