#!/usr/bin/env bash
# This scripts installs ubuntu in a qemu VM creating a base vm image.

set -ex

SCRIPT_PATH=$(dirname $(realpath $0))
[[ "$1" != "cpu" && "$1" != "gpu" ]] && echo "Invalid argument, use 'cpu' or 'gpu'" && exit 1
TYPE=$1
shift

AUTOINSTALL_UBUNTU_ISO_PATH="${SCRIPT_PATH}/../autoinstall_ubuntu/build/guest/$TYPE/iso/ubuntu-24.04.2-live-server-amd64-autoinstall-guest-${TYPE}.iso"
[[ ! -f "$AUTOINSTALL_UBUNTU_ISO_PATH" ]] && echo "Ubuntu autoinstall ISO not found, run 'autoinstall_ubuntu/build.sh guest $TYPE' first" && exit 1

QEMU_STATIC_CHECK=($SCRIPT_PATH/../qemu/build/qemu-static-*.tar.gz)
[[ ${#QEMU_STATIC_CHECK[@]} == 0 ]] && echo "QEMU static package not found, run 'qemu/build.sh' first" && exit 1
[[ ${#QEMU_STATIC_CHECK[@]} > 1 ]] && echo "More than one QEMU static package found, only one can be used" && exit 1
QEMU_STATIC_PATH=${QEMU_STATIC_CHECK[0]}

[[ $1 == "--clean" && -d "$SCRIPT_PATH/build" ]] && sudo rm -rf "$SCRIPT_PATH/build"

KERNEL_PATH="$SCRIPT_PATH/build/kernel/$TYPE"

[[ ! -d "$SCRIPT_PATH/build/vm_images" ]] && mkdir -p "$SCRIPT_PATH/build/vm_images"
[[ ! -d "$KERNEL_PATH" ]] && mkdir -p "$KERNEL_PATH"
[[ ! -d "$SCRIPT_PATH/build/qemu" ]] && mkdir -p "$SCRIPT_PATH/build/qemu"

# Install static qemu
[[ ! -d "$SCRIPT_PATH/build/qemu/usr" ]] && tar -xzf "$QEMU_STATIC_PATH" -C "$SCRIPT_PATH/build/qemu/"

export QEMU_PATH=$SCRIPT_PATH/build/qemu/usr/local/bin

# Create VM image
VM_IMAGE_PATH="$SCRIPT_PATH/build/vm_images/ubuntu24.04-$TYPE.qcow2"
rm -f "$VM_IMAGE_PATH"
[[ ! -f "$VM_IMAGE_PATH" ]] && $QEMU_PATH/qemu-img create -f qcow2 "$VM_IMAGE_PATH" 10G

SSH_FORWARD_PORT=2221

# Install ubuntu on VM
sudo $QEMU_PATH/qemu-system-x86_64 \
  -enable-kvm -nographic -no-reboot -cpu EPYC-v4 -machine q35 \
  -smp 12,maxcpus=31 -m 16G,slots=5,maxmem=120G \
  -drive if=pflash,format=raw,unit=0,file=$SCRIPT_PATH/build/qemu/usr/local/share/qemu/OVMF.fd,readonly=on \
  -drive file=$VM_IMAGE_PATH,if=none,id=disk0,format=qcow2 \
  -device virtio-scsi-pci,id=scsi0,disable-legacy=on,iommu_platform=true \
  -device scsi-hd,drive=disk0 \
  -device virtio-net-pci,disable-legacy=on,iommu_platform=true,netdev=vmnic,romfile= \
  -netdev user,id=vmnic,hostfwd=tcp::$SSH_FORWARD_PORT-:22 \
  -cdrom $AUTOINSTALL_UBUNTU_ISO_PATH \
  -virtfs local,path="$KERNEL_PATH",mount_tag=hostshare,security_model=passthrough,id=hostshare

sudo chown -R $(whoami) $SCRIPT_PATH/build/

# At this point the VM image is built. Now we need to use veritysetup to create a merkle tree for the image,
# saving both the tree and root hash to use them later during boot to verify the integrity of the disk.
VERITY_OUTPUT=$SCRIPT_PATH/build/vm_images/ubuntu24.04-${TYPE}-verity
sudo rm -rf "$VERITY_OUTPUT"
$SCRIPT_PATH/setup-verity.sh "$VM_IMAGE_PATH" "$VERITY_OUTPUT"

sudo chown -R $(whoami) "$VERITY_OUTPUT"

[[ ! -d $SCRIPT_PATH/../dist/vm_images ]] && mkdir -p $SCRIPT_PATH/../dist/vm_images
cp $VM_IMAGE_PATH "$SCRIPT_PATH/../dist/vm_images/"
[[ ! -d "$SCRIPT_PATH/../dist/vm_images/kernel/" ]] && mkdir -p "$SCRIPT_PATH/../dist/vm_images/kernel/"
cp -r $KERNEL_PATH "$SCRIPT_PATH/../dist/vm_images/kernel/"

# Copy the verity output directory entirely
cp -r "$VERITY_OUTPUT" "$SCRIPT_PATH/../dist/vm_images/"
