#!/usr/bin/env bash
# This script generates a JSON file that contains metadata about the artifacts.

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))

BUILD_TIMESTAMP=$(date +%s)
GIT_HASH=$(git rev-parse --short HEAD)
KERNEL_COMMIT=$(cat "$SCRIPT_PATH/kernel/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
QEMU_COMMIT=$(cat "$SCRIPT_PATH/qemu/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
GUEST_KERNEL_HEADERS=kernel/guest/linux-headers.deb
GUEST_KERNEL_HEADERS_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$GUEST_KERNEL_HEADERS" | cut -d " " -f 1)
GUEST_KERNEL_IMAGE=kernel/guest/linux-image.deb
GUEST_KERNEL_IMAGE_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$GUEST_KERNEL_IMAGE" | cut -d " " -f 1)
GUEST_KERNEL_IMAGE_DBG=kernel/guest/linux-image-dbg.deb
GUEST_KERNEL_IMAGE_DBG_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$GUEST_KERNEL_IMAGE_DBG" | cut -d " " -f 1)
GUEST_LIBC_DEV=kernel/guest/linux-libc-dev.deb
GUEST_LIBC_DEV_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$GUEST_LIBC_DEV" | cut -d " " -f 1)
HOST_KERNEL_HEADERS=kernel/host/linux-headers.deb
HOST_KERNEL_HEADERS_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$HOST_KERNEL_HEADERS" | cut -d " " -f 1)
HOST_KERNEL_IMAGE=kernel/host/linux-image.deb
HOST_KERNEL_IMAGE_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$HOST_KERNEL_IMAGE" | cut -d " " -f 1)
HOST_KERNEL_IMAGE_DBG=kernel/host/linux-image-dbg.deb
HOST_KERNEL_IMAGE_DBG_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$HOST_KERNEL_IMAGE_DBG" | cut -d " " -f 1)
HOST_LIBC_DEV=kernel/host/linux-libc-dev.deb
HOST_LIBC_DEV_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$HOST_LIBC_DEV" | cut -d " " -f 1)
QEMU=qemu-static.tar.gz
QEMU_HASH=$(sha256sum "$SCRIPT_PATH/../dist/$QEMU" | cut -d " " -f 1)
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

mkdir -p "${SCRIPT_PATH}/../dist"
echo $METADATA >"${SCRIPT_PATH}/../dist/core-metadata.json"
