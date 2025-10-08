#!/usr/bin/env bash
# This script generates a JSON file that contains metadata about the artifacts.

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))

BUILD_TIMESTAMP=$(date +%s)
GIT_HASH=$(git rev-parse --short HEAD)
BASE_ARTIFACTS_PATH=$SCRIPT_PATH/../dist
QEMU_COMMIT=$(cat "$SCRIPT_PATH/qemu/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
QEMU=core/qemu-static.tar.gz
QEMU_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$QEMU" | cut -d " " -f 1)
GITHUB_RUN_ID=${GITHUB_RUN_ID:-null}

METADATA=$(
  cat <<EOF
{
  "build": {
    "timestamp": ${BUILD_TIMESTAMP},
    "git_hash": "${GIT_HASH}",
    "github_action_run_id": $GITHUB_RUN_ID
  },
  "built_at": ${BUILD_TIMESTAMP},
  "git_hash": "${GIT_HASH}",
  "qemu": {
    "commit": "${QEMU_COMMIT}",
    "file": {
      "path": "${QEMU}" ,
      "sha256": "${QEMU_HASH}"
    }
  }
}
EOF
)

echo $METADATA >"${SCRIPT_PATH}/../dist/core/metadata.json"
