#!/bin/bash

set -euo pipefail

ROOT_PATH="/media/cvm-agent-entrypoint"
cd "${ROOT_PATH}"
export PROXY_HOSTNAME=$(cat metadata.json | jq -r .hostname)
export API_CONTAINER=$(cat metadata.json | jq -r .api_container)

echo "Using hostname: ${PROXY_HOSTNAME}"

# Start user's containers and the caddy proxy
docker compose -f docker-compose.yaml -f /opt/nillion/caddy/docker-compose.yaml up -d
