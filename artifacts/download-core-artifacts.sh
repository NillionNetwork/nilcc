#!/bin/bash

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))
ARTIFACTS_VERSION=0.1.0
BASE_URL=https://nilcc.s3.eu-west-1.amazonaws.com/${ARTIFACTS_VERSION}/core
BASE_ARTIFACTS_PATH=$SCRIPT_PATH/dist/

mkdir -p "$BASE_ARTIFACTS_PATH"
curl -o "${BASE_ARTIFACTS_PATH}/qemu-static.tar.gz" "${BASE_URL}/qemu-static.tar.gz"
curl -o "${BASE_ARTIFACTS_PATH}/core-metadata.json" "${BASE_URL}/metadata.json"
