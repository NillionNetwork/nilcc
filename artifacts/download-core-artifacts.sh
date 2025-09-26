#!/bin/bash

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))
ARTIFACTS_VERSION=0.1.2
BASE_URL=https://nilcc.s3.eu-west-1.amazonaws.com/${ARTIFACTS_VERSION}
BASE_ARTIFACTS_PATH=$SCRIPT_PATH/dist/

for type in guest host; do
  mkdir -p "$BASE_ARTIFACTS_PATH/kernel/${type}"

  for file in linux-headers.deb linux-image.deb linux-image-dbg.deb linux-libc-dev.deb; do
    curl -o "${BASE_ARTIFACTS_PATH}/kernel/${type}/${file}" "${BASE_URL}/kernel/${type}/${file}"
  done
done

curl -o "${BASE_ARTIFACTS_PATH}/qemu-static.tar.gz" "${BASE_URL}/qemu-static.tar.gz"
