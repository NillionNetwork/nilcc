#!/bin/bash

set -euo pipefail

if [ $# -ne 2 ]; then
  echo "Usage: $0 <input-qcow2-image> <output-directory>"
  exit 1
fi

SCRIPT_PATH=$(dirname $(realpath $0))

QEMU_PATH=$SCRIPT_PATH/build/qemu/usr/local/bin
INPUT_IMAGE=$1
OUTPUT_DIR=$2
NBD_DEVICE=/dev/nbd1

cleanup() {
  set +e
  echo "Disconnecting nbd"
  sudo "${QEMU_PATH}/qemu-nbd" --disconnect $NBD_DEVICE
  sleep 2
  sudo rmmod nbd
}

trap cleanup EXIT SIGINT

mkdir -p "$OUTPUT_DIR"

echo "Adding NBD kernel module"
sudo modprobe nbd max_part=8

echo "Connecting ${INPUT_IMAGE} image via NBD to ${NBD_DEVICE}"
sudo "${QEMU_PATH}/qemu-nbd" --connect=$NBD_DEVICE $INPUT_IMAGE

echo "Sleeping to let qemu-nbd finish setting up device"
sleep 2

MOUNT_POINT=${OUTPUT_DIR}/root
# There's 2 partitions: the EFI one and the actual filesystem. Mount the second one.
PARTITION=${NBD_DEVICE}p2

echo "Mounting ${PARTITION} in ${MOUNT_POINT}"
sudo mkdir "$MOUNT_POINT"
sudo mount "${PARTITION}" "${MOUNT_POINT}"

echo "Setting up filesystem"
# Create a directory where we'll keep read only copies of mutable directories.
RO_PATH=${MOUNT_POINT}/ro
sudo mkdir "$RO_PATH"

# Move /var and /etc to the read only directory
sudo mv "${MOUNT_POINT}/var" "${MOUNT_POINT}/etc" "${RO_PATH}"

# Create directories that initrd will populate on start.
sudo mkdir "${MOUNT_POINT}/var" "${MOUNT_POINT}/etc" "${MOUNT_POINT}/media/cvm-agent-entrypoint"

# Delete /tmp
sudo rm -rf "${MOUNT_POINT}/tmp/*"

echo "Unmounting ${MOUNT_POINT}"
sudo umount "${MOUNT_POINT}"
sudo rmdir "$MOUNT_POINT"

echo "Generating hashes for ${PARTITION}. This may take some time.."
sudo veritysetup format "${PARTITION}" "${OUTPUT_DIR}/verity-hash-dev" | grep "Root hash" | cut -f2 | tr -d '\n' >"${OUTPUT_DIR}/root-hash"

echo "Output generated in ${OUTPUT_DIR}"
