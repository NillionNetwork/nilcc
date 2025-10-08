#!/usr/bin/env bash
# This script generates a JSON file that contains metadata about the artifacts.

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))

BUILD_TIMESTAMP=$(date +%s)
GIT_HASH=$(git rev-parse --short HEAD)

QEMU_COMMIT=$(cat "$SCRIPT_PATH/dist/core-metadata.json" | jq -r .qemu.commit)
INITRD=initramfs.cpio.gz
INITRD_HASH=$(sha256sum "$SCRIPT_PATH/dist/$INITRD" | cut -d " " -f 1)
OVMF=OVMF.fd
OVMF_HASH=$(sha256sum "$SCRIPT_PATH/dist/$OVMF" | cut -d " " -f 1)
CPU_DISK=cpu/disk.squashfs
CPU_DISK_HASH=$(sha256sum "$SCRIPT_PATH/dist/$CPU_DISK" | cut -d " " -f 1)
CPU_VERITY_ROOT_HASH=$(cat "$SCRIPT_PATH/dist/cpu/root-hash")
CPU_VERITY_HASHES_DISK=cpu/disk.verity
CPU_KERNEL=cpu/kernel
CPU_KERNEL_HASH=$(sha256sum "$SCRIPT_PATH/dist/$CPU_KERNEL" | cut -d " " -f 1)
GPU_DISK=gpu/disk.squashfs
GPU_DISK_HASH=$(sha256sum "$SCRIPT_PATH/dist/$GPU_DISK" | cut -d " " -f 1)
GPU_VERITY_ROOT_HASH=$(cat "$SCRIPT_PATH/dist/gpu/root-hash")
GPU_VERITY_HASHES_DISK=gpu/disk.verity
GPU_KERNEL=gpu/kernel
GPU_KERNEL_HASH=$(sha256sum "$SCRIPT_PATH/dist/$GPU_KERNEL" | cut -d " " -f 1)
KERNEL_CMDLINE="root=/dev/sda verity_disk=/dev/sdb verity_roothash={VERITY_ROOT_HASH} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={DOCKER_COMPOSE_HASH} panic=-1 random.trust_cpu=on random.trust_bootloader=off pci=realloc,nocrs"
GITHUB_RUN_ID=${GITHUB_RUN_ID:-null}

METADATA=$(
  cat <<EOF
{
  "build": {
    "timestamp": ${BUILD_TIMESTAMP},
    "git_hash": "${GIT_HASH}",
    "github_action_run_id": $GITHUB_RUN_ID
  },
  "git_hash": "${GIT_HASH}",
  "built_at": ${BUILD_TIMESTAMP},
  "kernel": {
    "commit": "<unused>"
  },
  "qemu": {
    "commit": "${QEMU_COMMIT}"
  },
  "ovmf": {
    "path": "${OVMF}",
    "sha256": "${OVMF_HASH}"
  },
  "initrd": {
    "path": "${INITRD}",
    "sha256": "${INITRD_HASH}"
  },
  "cvm": {
    "cmdline": "$KERNEL_CMDLINE",
    "images": {
      "cpu": {
        "disk": {
          "path": "${CPU_DISK}",
          "format": "raw",
          "sha256": "${CPU_DISK_HASH}"
        },
        "verity": {
          "disk": {
            "path": "${CPU_VERITY_HASHES_DISK}",
            "format": "raw"
          },
          "root_hash": "${CPU_VERITY_ROOT_HASH}"
        },
        "kernel": {
          "path": "${CPU_KERNEL}",
          "sha256": "${CPU_KERNEL_HASH}"
        }
      },
      "gpu": {
        "disk": {
          "path": "${GPU_DISK}",
          "format": "raw",
          "sha256": "${GPU_DISK_HASH}"
        },
        "verity": {
          "disk": {
            "path": "${GPU_VERITY_HASHES_DISK}",
            "format": "raw"
          },
          "root_hash": "${GPU_VERITY_ROOT_HASH}"
        },
        "kernel": {
          "path": "${GPU_KERNEL}",
          "sha256": "${GPU_KERNEL_HASH}"
        }
      }
    }
  }
}
EOF
)

echo $METADATA >"${SCRIPT_PATH}/dist/metadata.json"
