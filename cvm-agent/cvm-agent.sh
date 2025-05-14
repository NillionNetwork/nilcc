#!/bin/bash

set -euo pipefail

ROOT_PATH="/media/cvm-agent-entrypoint"
cd "${ROOT_PATH}"
export PROXY_HOSTNAME=$(cat metadata.json | jq -r .hostname)
export API_CONTAINER=$(cat metadata.json | jq -r .api.container)
export API_PORT=$(cat metadata.json | jq -r .api.port)

echo "Using hostname: ${PROXY_HOSTNAME}"

# Start user's containers and the built-in services in a single compose network
docker compose -f docker-compose.yaml -f /opt/nillion/services/docker-compose.yaml up -d
