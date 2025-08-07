FROM rust:1.88-alpine AS build

WORKDIR /opt/nillion
RUN apk add --no-cache musl-dev perl make

COPY . .
RUN cargo build --release --locked -p nilcc-verifier

FROM alpine

WORKDIR /opt/nillion

COPY --from=build /opt/nillion/target/release/nilcc-verifier /opt/nillion
ENTRYPOINT ["/opt/nillion/nilcc-verifier"]
