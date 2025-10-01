FROM rust:1.88 AS build

WORKDIR /opt/nillion
RUN apt update && apt install -y git

COPY . .
RUN cargo build --release --locked -p nilcc-attester

FROM ghcr.io/astral-sh/uv:python3.12-trixie-slim

WORKDIR /opt/nillion

COPY --from=build /opt/nillion/target/release/nilcc-attester /opt/nillion
COPY --from=build /opt/nillion/nilcc-attester/gpu-attester /opt/nillion/gpu-attester

ENV UV_PROJECT_ENVIRONMENT=/opt/nillion/.venv

RUN apt update && \
  apt install -y curl && \
  apt clean && \
  apt autoremove && \
  rm -rf /var/lib/apt/lists/* && \
  uv sync --project gpu-attester

ENTRYPOINT ["uv", "run", "/opt/nillion/nilcc-attester"]

