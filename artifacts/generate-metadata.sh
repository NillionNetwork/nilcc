#!/usr/bin/env bash
# This script generates a JSON file that contains metadata about the artifacts.

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))

KERNEL_COMMIT=$(cat artifacts/kernel/build.sh | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
QEMU_COMMIT=$(cat artifacts/qemu/build.sh | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')

METADATA=$(
  cat <<EOF
{
  "kernel": {
    "commit": "${KERNEL_COMMIT}"
  },
  "qemu": {
    "commit": "${QEMU_COMMIT}"
  }
}
EOF
)

echo $METADATA >"${SCRIPT_PATH}/dist/metadata.json"
