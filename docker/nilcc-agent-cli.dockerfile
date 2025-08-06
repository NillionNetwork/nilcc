FROM rust:1.86-alpine AS build

WORKDIR /opt/nillion
RUN apk add --no-cache musl-dev

COPY . .
RUN cargo build --release --locked -p nilcc-agent-cli

FROM alpine

WORKDIR /opt/nillion

COPY --from=build /opt/nillion/target/release/nilcc-agent-cli /opt/nillion
ENTRYPOINT ["/opt/nillion/nilcc-agent-cli"]
