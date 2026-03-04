#!/bin/bash

# This script allows running a base cvm disk image in debug mode. This will create a copy of the base disk image,
# start a VM using it, and then toss it away after the VM shuts down.

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <cpu|gpu>"
  exit 1
fi

VM_TYPE=$1

if [[ $VM_TYPE != "cpu" && $VM_TYPE != "gpu" ]]; then
  echo "Parameter needs to be 'cpu' or 'gpu'"
  exit 1
fi

SCRIPT_PATH=$(dirname $(realpath $0))
DIST_PATH="$SCRIPT_PATH/../../dist"
INPUT_IMAGE="$DIST_PATH/$VM_TYPE/disk.squashfs"

# We will do everything in a tempdir that will get wiped out when we're done
TEMP=$(mktemp -d /tmp/nilcc.XXXXX)
RAW_IMAGE_PATH="$TEMP/image.raw"
OUTPUT_IMAGE="$TEMP/image.qcow2"
INITRAMFS=$TEMP/initramfs.cpio.gz

# Delete the whole tempdir on our way out
trap "rm -rf $TEMP" EXIT SIGINT

if [ ! -f "$INPUT_IMAGE" ]; then
  echo "Input VM image not created, run artifacts/cvm-image/build.sh $VM_TYPE first"
  exit 1
fi

# Create a dev initramfs which avoids the veritysetup dance
DOCKERFILE="$SCRIPT_PATH/initramfs.dockerfile" OUTPUT_FILENAME="$INITRAMFS" $SCRIPT_PATH/../../initramfs/build.sh

# Unpack the input squashfs image
unsquashfs -d "$TEMP/squashfs" "$INPUT_IMAGE"

# Create a new raw disk image and mount it
qemu-img create -f raw "$RAW_IMAGE_PATH" 5G
mkfs.ext4 "$RAW_IMAGE_PATH"
mkdir "$TEMP/mnt"
mount -o loop "$RAW_IMAGE_PATH" "$TEMP/mnt"

# Copy over all input files/directories from squashfs into our new image
cp -a $TEMP/squashfs/* "$TEMP/mnt"
umount "$TEMP/mnt"

# Now turn it into a qcow2
qemu-img convert -f raw -O qcow2 "$RAW_IMAGE_PATH" "$OUTPUT_IMAGE"

# Launch VM
sudo qemu-system-x86_64 \
  -enable-kvm \
  -nographic \
  -no-reboot \
  -cpu EPYC-v4 \
  -machine q35 \
  -smp 4 \
  -m 4G \
  -serial mon:stdio \
  -initrd "$INITRAMFS" \
  -kernel "$DIST_PATH/cpu/kernel" \
  -append "panic=-1 console=ttyS0" \
  -drive "if=virtio,format=qcow2,file=$OUTPUT_IMAGE"
