#!/usr/bin/env bash
# This script generates a JSON file that contains metadata about the artifacts.

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))

BUILD_TIMESTAMP=$(date +%s)
GIT_HASH=$(git rev-parse --short HEAD)

KERNEL_COMMIT=$(cat "$SCRIPT_PATH/core/kernel/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
QEMU_COMMIT=$(cat "$SCRIPT_PATH/core/qemu/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
INITRD=initramfs/initramfs.cpio.gz
INITRD_HASH=$(sha256sum "$SCRIPT_PATH/dist/$INITRD" | cut -d " " -f 1)
OVMF=vm_images/ovmf/OVMF.fd
OVMF_HASH=$(sha256sum "$SCRIPT_PATH/dist/$OVMF" | cut -d " " -f 1)
CPU_DISK=vm_images/cvm-cpu.squashfs
CPU_DISK_HASH=$(sha256sum "$SCRIPT_PATH/dist/$CPU_DISK" | cut -d " " -f 1)
CPU_VERITY_ROOT_HASH=$(cat "$SCRIPT_PATH/dist/vm_images/cvm-cpu-verity/root-hash")
CPU_VERITY_HASHES_DISK=vm_images/cvm-cpu-verity/verity-hash-dev
CPU_KERNEL=vm_images/kernel/cpu-vmlinuz
CPU_KERNEL_HASH=$(sha256sum "$SCRIPT_PATH/dist/$CPU_KERNEL" | cut -d " " -f 1)
GPU_DISK=vm_images/cvm-gpu.squashfs
GPU_DISK_HASH=$(sha256sum "$SCRIPT_PATH/dist/$GPU_DISK" | cut -d " " -f 1)
GPU_VERITY_ROOT_HASH=$(cat "$SCRIPT_PATH/dist/vm_images/cvm-gpu-verity/root-hash")
GPU_VERITY_HASHES_DISK=vm_images/cvm-gpu-verity/verity-hash-dev
GPU_KERNEL=vm_images/kernel/gpu-vmlinuz
GPU_KERNEL_HASH=$(sha256sum "$SCRIPT_PATH/dist/$GPU_KERNEL" | cut -d " " -f 1)

METADATA=$(
  cat <<EOF
{
  "built_at": ${BUILD_TIMESTAMP},
  "git_hash": "${GIT_HASH}",
  "kernel": {
    "commit": "${KERNEL_COMMIT}"
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
    "cmdline": "panic=-1 root=/dev/sda verity_disk=/dev/sdb verity_roothash={VERITY_ROOT_HASH} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={DOCKER_COMPOSE_HASH}",
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
