FROM rust:1.86 AS build

WORKDIR /opt/nillion
RUN apt update && apt install -y git

COPY . .
RUN cargo build --release --locked -p nilcc-attester

FROM ghcr.io/astral-sh/uv:python3.12-alpine

WORKDIR /opt/nillion

COPY --from=build /opt/nillion/target/release/nilcc-attester /opt/nillion
COPY --from=build /opt/nillion/nilcc-attester/gpu-attester /opt/nillion/gpu-attester

ENV UV_PROJECT_ENVIRONMENT=/opt/nillion/.venv

RUN apk add curl && uv sync --project gpu-attester

ENTRYPOINT ["uv", "run", "/opt/nillion/nilcc-attester"]

