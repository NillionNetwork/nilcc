FROM rust:1.86-alpine AS build

WORKDIR /opt/nillion
RUN apk add --no-cache musl-dev git perl make bash
COPY . .
RUN cargo build --release --locked -p nilcc-attester

FROM scratch
WORKDIR /opt/nillion
COPY --from=build /opt/nillion/target/release/nilcc-attester /opt/nillion
ENTRYPOINT ["/opt/nillion/nilcc-attester"]

