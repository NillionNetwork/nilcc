FROM rust:1.86-alpine AS build

WORKDIR /opt/nillion
RUN apk add --no-cache musl-dev git

COPY . .
RUN cargo build --release --locked -p nilcc-attester

FROM ghcr.io/astral-sh/uv:python3.12-alpine

WORKDIR /opt/nillion

COPY --from=build /opt/nillion/target/release/nilcc-attester /opt/nillion
COPY --from=build /opt/nillion/nilcc-attester/gpu-attester /opt/nillion/gpu-attester

ENV UV_PROJECT_ENVIRONMENT=/opt/nillion/.venv

RUN apk add --no-cache curl && uv sync --project gpu-attester

ENTRYPOINT ["uv", "run", "/opt/nillion/nilcc-attester"]

