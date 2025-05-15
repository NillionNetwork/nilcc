#!/bin/bash

set -euo pipefail

ROOT_PATH="/media/cvm-agent-entrypoint"
cd "${ROOT_PATH}"

# Pull variables from the input metadata file that describes what we're running.
proxy_hostname=$(cat metadata.json | jq -r .hostname)
proxy_target=$(cat metadata.json | jq -r '"\(.api.container):\(.api.port)"')

# Take the input caddyfile and replace the hostname being proxied and the target container.
caddyfile=$(cat /opt/nillion/services/Caddyfile)
caddyfile="${caddyfile//\{NILCC_PROXY_HOSTNAME\}/${proxy_hostname}}"
caddyfile="${caddyfile//\{NILCC_PROXY_TARGET\}/${proxy_target}}"

# Create a tempfile and write the caddyfile to it
caddyfile_path=$(mktemp)
echo "$caddyfile" >"$caddyfile_path"

# Export the path so docker compose sees it
export "CADDY_INPUT_FILE=${caddyfile_path}"

# Start user's containers and the built-in services in a single compose network
echo "Redirecting traffic for ${proxy_hostname} to ${proxy_target}"
docker compose -f docker-compose.yaml -f /opt/nillion/services/docker-compose.yaml up -d
