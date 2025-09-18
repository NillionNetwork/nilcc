#!/usr/bin/env bash
# This script generates a JSON file that contains metadata about the artifacts.

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))

KERNEL_COMMIT=$(cat "$SCRIPT_PATH/kernel/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
QEMU_COMMIT=$(cat "$SCRIPT_PATH/qemu/build.sh" | sed -n -e 's/^COMMIT="\(.*\)"/\1/p')
INITRD=initramfs/initramfs.cpio.gz
INITRD_HASH=$(sha256sum "$SCRIPT_PATH/dist/$INITRD" | cut -d " " -f 1)
OVMF=vm_images/ovmf/OVMF.fd
OVMF_HASH=$(sha256sum "$SCRIPT_PATH/dist/$OVMF" | cut -d " " -f 1)
CPU_DISK=vm_images/cvm-cpu.qcow2
CPU_DISK_HASH=$(sha256sum "$SCRIPT_PATH/dist/$CPU_DISK" | cut -d " " -f 1)
CPU_VERITY_ROOT_HASH=$(cat "$SCRIPT_PATH/dist/vm_images/cvm-cpu-verity/root-hash")
CPU_VERITY_HASHES_DISK=vm_images/cvm-cpu-verity/verity-hash-dev
GPU_DISK=vm_images/cvm-gpu.qcow2
GPU_DISK_HASH=$(sha256sum "$SCRIPT_PATH/dist/$GPU_DISK" | cut -d " " -f 1)
GPU_VERITY_ROOT_HASH=$(cat "$SCRIPT_PATH/dist/vm_images/cvm-gpu-verity/root-hash")
GPU_VERITY_HASHES_DISK=vm_images/cvm-gpu-verity/verity-hash-dev

METADATA=$(
  cat <<EOF
{
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
    "cmdline": "panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={VERITY_ROOT_HASH} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={DOCKER_COMPOSE_HASH}",
    "images": {
      "cpu": {
        "disk": {
          "path": "${CPU_DISK}",
          "format": "qcow2",
          "sha256": "${CPU_DISK_HASH}"
        },
        "verity": {
          "disk": {
            "path": "${CPU_VERITY_HASHES_DISK}",
            "format": "raw"
          },
          "root_hash": "${CPU_VERITY_ROOT_HASH}"
        }
      },
      "gpu": {
        "disk": {
          "path": "${GPU_DISK}",
          "format": "qcow2",
          "sha256": "${GPU_DISK_HASH}"
        },
        "verity": {
          "disk": {
            "path": "${GPU_VERITY_HASHES_DISK}",
            "format": "raw"
          },
          "root_hash": "${GPU_VERITY_ROOT_HASH}"
        }
      }
    }
  }
}
EOF
)

echo $METADATA >"${SCRIPT_PATH}/dist/metadata.json"
