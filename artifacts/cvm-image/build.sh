#!/bin/bash

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))
IMAGE_URL=https://cloud-images.ubuntu.com/minimal/releases/noble/release-20250727/ubuntu-24.04-minimal-cloudimg-amd64.img
CVM_AGENT_PATH="${SCRIPT_PATH}/../../target/release/cvm-agent"
QEMU_STATIC_PATH=($SCRIPT_PATH/../dist/qemu-static.tar.gz)
QEMU_PATH=$SCRIPT_PATH/build/qemu/usr/local/bin
NBD_DEVICE=/dev/nbd1
NBD_MAIN_PARTITION="${NBD_DEVICE}p1"
NBD_BOOT_PARTITION="${NBD_DEVICE}p16"
OUTPUT_PATH=$SCRIPT_PATH/../dist/

[[ "$1" != "cpu" && "$1" != "gpu" ]] && echo "Invalid argument, use 'cpu' or 'gpu'" && exit 1
[[ ! -f "$CVM_AGENT_PATH" ]] && echo "cvm-agent binary not found, run 'cargo build --release -p cvm-agent'" && exit 1
[[ ! -f "$QEMU_STATIC_PATH" ]] && echo "QEMU static package not found, run 'download-core-artifacts.sh' first" && exit 1

# Unpack static qemu.
mkdir -p "$SCRIPT_PATH/build/qemu"
[[ ! -d "$SCRIPT_PATH/build/qemu/usr" ]] && tar -xzf "$QEMU_STATIC_PATH" -C "$SCRIPT_PATH/build/qemu/"

VM_TYPE=$1
shift

BUILD_PATH="$SCRIPT_PATH/build/$VM_TYPE"
sudo rm -rf "$BUILD_PATH"

SQUASHFS_PATH="$BUILD_PATH/image.squashfs"
VERITY_HASHES_PATH="$BUILD_PATH/verity-hash-dev"
VERITY_ROOT_HASH_PATH="$BUILD_PATH/root-hash"
OUTPUT_IMAGE="$BUILD_PATH/image.qcow2"
ISO_SOURCES_PATH="$BUILD_PATH/sources"
BASE_IMG_PATH="$SCRIPT_PATH/build/base-image.img"
IMG_PATH="$BUILD_PATH/image.qcow2"
SEED_PATH=$(mktemp /tmp/nilcc.XXXXX)
CDROM_PATH=$(mktemp /tmp/nilcc.XXXXX)
NILCC_VERSION=${NILCC_VERSION:-$(git rev-parse --short HEAD)}

sudo rm -rf "$ISO_SOURCES_PATH/packages"

mkdir -p "$ISO_SOURCES_PATH/packages"
mkdir -p "$ISO_SOURCES_PATH/nillion"

# Copy all kernel packages over
KERNEL_FILES=($OUTPUT_PATH/kernel/guest/linux-*.deb)
[[ ${#KERNEL_FILES[@]} == 0 ]] && echo "guest kernel not found, run 'download-core-artifacts.sh' first" && exit 1

cp $OUTPUT_PATH/kernel/guest/linux-headers.deb "$ISO_SOURCES_PATH/packages"
cp $OUTPUT_PATH/kernel/guest/linux-image.deb "$ISO_SOURCES_PATH/packages"
cp $OUTPUT_PATH/kernel/guest/linux-libc-dev.deb "$ISO_SOURCES_PATH/packages"

# Copy cvm-agent script and dependencies.
cp "$CVM_AGENT_PATH" "$ISO_SOURCES_PATH/nillion/"
cp "$SCRIPT_PATH/cvm-agent.service" "$ISO_SOURCES_PATH/nillion/"

# Store version and type so the VM can store this persistently.
echo "$NILCC_VERSION" >"$ISO_SOURCES_PATH/nillion/nilcc-version"
echo "$VM_TYPE" >"$ISO_SOURCES_PATH/nillion/nilcc-vm-type"

[[ ! -f "${BASE_IMG_PATH}" ]] && curl -L "$IMAGE_URL" -o "$BASE_IMG_PATH"

rm -f "$IMG_PATH"
cp "$BASE_IMG_PATH" "$IMG_PATH"
${QEMU_PATH}/qemu-img resize "$IMG_PATH" +5G

# Render the user-data file to include the base64 encoded setup file
user_data=$(mktemp)
contents=$(cat "$SCRIPT_PATH/setup.sh" | base64 -w 0)
cp "$SCRIPT_PATH/user-data.yaml" "$user_data"
sed -i "s/{SETUP_CONTENTS}/$contents/" "$user_data"
sed -i "s/{VM_TYPE}/${VM_TYPE}/" "$user_data"

# Build a seed disk with our cloud-init file.
cloud-localds "$SEED_PATH" "$user_data"

# Build an ISO with all of our custom data we want to plug into the VM.
mkisofs -U -o "$CDROM_PATH" "$BUILD_PATH/sources"

sudo ${QEMU_PATH}/qemu-system-x86_64 \
  -enable-kvm \
  -nographic \
  -no-reboot \
  -cpu EPYC-v4 \
  -machine q35 \
  -smp 12,maxcpus=31 \
  -m 16G,slots=5,maxmem=120G \
  -device virtio-net-pci,disable-legacy=on,iommu_platform=true,netdev=vmnic,romfile= \
  -netdev user,id=vmnic \
  -drive if=pflash,format=raw,unit=0,file=$SCRIPT_PATH/build/qemu/usr/local/share/qemu/OVMF.fd,readonly=on \
  -drive "if=virtio,format=qcow2,file=$IMG_PATH" \
  -drive "if=virtio,format=raw,file=$SEED_PATH" \
  -cdrom "$CDROM_PATH"

rm "$SEED_PATH" "$CDROM_PATH"

cleanup_nbd() {
  set +e
  echo "Unmounting ${MOUNT_POINT}"
  sudo umount "$MOUNT_POINT"
  sudo rmdir "$MOUNT_POINT"

  echo "Disconnecting nbd"
  sudo "${QEMU_PATH}/qemu-nbd" --disconnect "$NBD_DEVICE"
  sleep 2
  sudo rmmod nbd
}

MOUNT_POINT=$(mktemp -d /tmp/nilcc.XXXXX)
sudo modprobe nbd max_part=8
echo "Connecting ${OUTPUT_IMAGE} image via NBD to ${NBD_DEVICE}"
sudo "${QEMU_PATH}/qemu-nbd" --connect=$NBD_DEVICE "$OUTPUT_IMAGE"

echo "Sleeping to let qemu-nbd finish setting up device"
sleep 2

trap cleanup_nbd EXIT SIGINT

echo "Mounting ${NBD_MAIN_PARTITION} in $MOUNT_POINT"
sudo mount "$NBD_MAIN_PARTITION" "$MOUNT_POINT"

[[ ! -f "$MOUNT_POINT/var/lib/cvm-success" ]] && echo "cvm setup failed" && exit 1

echo "Setting up filesystem"
# Create a directory where we'll keep read only copies of mutable directories.
RO_PATH="$MOUNT_POINT/ro"
sudo mkdir -p "$RO_PATH"

# Move /var and /etc to the read only directory
sudo mv "${MOUNT_POINT}/var" "${MOUNT_POINT}/etc" "${RO_PATH}"

# Create directories that initrd will populate on start.
sudo mkdir "${MOUNT_POINT}/var" "${MOUNT_POINT}/etc" "${MOUNT_POINT}/media/cvm-agent-entrypoint"

# Delete /tmp
sudo rm -rf "${MOUNT_POINT}/tmp/*"

echo "Repackaging filesystem as squashfs"
rm -f "$SQUASHFS_PATH"
sudo mksquashfs "$MOUNT_POINT" "$SQUASHFS_PATH"

echo "Unmounting $MOUNT_POINT"
sudo umount "$MOUNT_POINT"

echo "Generating hashes for ${SQUASHFS_PATH}. This may take some time.."
sudo veritysetup format "${SQUASHFS_PATH}" "${VERITY_HASHES_PATH}" | grep "Root hash" | cut -f2 | tr -d '\n' >"${VERITY_ROOT_HASH_PATH}"

sudo mount "$NBD_BOOT_PARTITION" "$MOUNT_POINT"

sudo chown -R $(whoami) $SCRIPT_PATH/build/

# Copy VM image.
mkdir -p "$OUTPUT_PATH/$VM_TYPE"
cp "$SQUASHFS_PATH" "$OUTPUT_PATH/$VM_TYPE/disk.squashfs"

# Copy the verity output.
cp "${VERITY_ROOT_HASH_PATH}" "$OUTPUT_PATH/$VM_TYPE/root-hash"
cp "${VERITY_HASHES_PATH}" "$OUTPUT_PATH/$VM_TYPE/disk.verity"

# Copy kernel.
mkdir -p "$OUTPUT_PATH/kernel/"
sudo cp $MOUNT_POINT/vmlinuz-*snp* "$OUTPUT_PATH/$VM_TYPE/kernel"
sudo chown $(whoami) "$OUTPUT_PATH/$VM_TYPE/kernel"

# Copy OVMF.
cp "$SCRIPT_PATH/build/qemu/usr/local/share/qemu/OVMF.fd" "$OUTPUT_PATH/OVMF.fd"
