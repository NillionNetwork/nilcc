#!/usr/bin/env bash
# This script generates a JSON file that contains metadata about the artifacts.

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))

BUILD_TIMESTAMP=$(date +%s)
GIT_HASH=$(git rev-parse --short HEAD)
BASE_ARTIFACTS_PATH=$SCRIPT_PATH/../dist
KERNEL_COMMIT=$(cat "$SCRIPT_PATH/kernel/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
QEMU_COMMIT=$(cat "$SCRIPT_PATH/qemu/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
GUEST_KERNEL_HEADERS=core/kernel/guest/linux-headers.deb
GUEST_KERNEL_HEADERS_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$GUEST_KERNEL_HEADERS" | cut -d " " -f 1)
GUEST_KERNEL_IMAGE=core/kernel/guest/linux-image.deb
GUEST_KERNEL_IMAGE_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$GUEST_KERNEL_IMAGE" | cut -d " " -f 1)
GUEST_KERNEL_IMAGE_DBG=core/kernel/guest/linux-image-dbg.deb
GUEST_KERNEL_IMAGE_DBG_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$GUEST_KERNEL_IMAGE_DBG" | cut -d " " -f 1)
GUEST_LIBC_DEV=core/kernel/guest/linux-libc-dev.deb
GUEST_LIBC_DEV_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$GUEST_LIBC_DEV" | cut -d " " -f 1)
HOST_KERNEL_HEADERS=core/kernel/host/linux-headers.deb
HOST_KERNEL_HEADERS_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$HOST_KERNEL_HEADERS" | cut -d " " -f 1)
HOST_KERNEL_IMAGE=core/kernel/host/linux-image.deb
HOST_KERNEL_IMAGE_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$HOST_KERNEL_IMAGE" | cut -d " " -f 1)
HOST_KERNEL_IMAGE_DBG=core/kernel/host/linux-image-dbg.deb
HOST_KERNEL_IMAGE_DBG_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$HOST_KERNEL_IMAGE_DBG" | cut -d " " -f 1)
HOST_LIBC_DEV=core/kernel/host/linux-libc-dev.deb
HOST_LIBC_DEV_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$HOST_LIBC_DEV" | cut -d " " -f 1)
QEMU=core/qemu-static.tar.gz
QEMU_HASH=$(sha256sum "$BASE_ARTIFACTS_PATH/$QEMU" | cut -d " " -f 1)
METADATA=$(
  cat <<EOF
{
  "built_at": ${BUILD_TIMESTAMP},
  "git_hash": "${GIT_HASH}",
  "kernel": {
    "commit": "${KERNEL_COMMIT}",
    "guest": {
      "files": {
        "headers": {
          "path": "${GUEST_KERNEL_HEADERS}",
          "sha256": "${GUEST_KERNEL_HEADERS_HASH}"
        },
        "image": {
          "path": "${GUEST_KERNEL_IMAGE}",
          "sha256": "${GUEST_KERNEL_IMAGE_HASH}"
        },
        "image_dbg": {
          "path": "${GUEST_KERNEL_IMAGE_DBG}",
          "sha256": "${GUEST_KERNEL_IMAGE_DBG_HASH}"
        },
        "libc_dev": {
          "path": "${GUEST_LIBC_DEV}",
          "sha256": "${GUEST_LIBC_DEV_HASH}"
        }
      }
    },
    "host": {
      "files": {
        "headers": {
          "path": "${HOST_KERNEL_HEADERS}",
          "sha256": "${HOST_KERNEL_HEADERS_HASH}"
        },
        "image": {
          "path": "${HOST_KERNEL_IMAGE}",
          "sha256": "${HOST_KERNEL_IMAGE_HASH}"
        },
        "image_dbg": {
          "path": "${HOST_KERNEL_IMAGE_DBG}",
          "sha256": "${HOST_KERNEL_IMAGE_DBG_HASH}"
        },
        "libc_dev": {
          "path": "${HOST_LIBC_DEV}",
          "sha256": "${HOST_LIBC_DEV_HASH}"
        }
      }
    }
  },
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
