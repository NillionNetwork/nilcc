#!/usr/bin/env bash
# This script launches the vm image to test the build.

set -e

SCRIPT_PATH=$(dirname $(realpath $0))

VM_IMAGE="${1:-$SCRIPT_PATH/build/vm_images/ubuntu24.04.qcow2}"
[[ ! -f "$VM_IMAGE" ]] && echo "VM image not found, run 'build.sh' first" && exit 1

SSH_FORWARD_PORT=2222

QEMU_BASE_PATH="$SCRIPT_PATH/build/qemu/"
$QEMU_BASE_PATH/usr/local/bin/qemu-system-x86_64 \
-enable-kvm -nographic -no-reboot \
-cpu EPYC-v4 -machine q35 -smp 8,maxcpus=8 -m 16G,slots=2,maxmem=120G \
-drive if=pflash,format=raw,unit=0,file=$QEMU_BASE_PATH/usr/local/share/qemu/OVMF_CODE.fd,readonly=on \
-drive file=$VM_IMAGE,if=none,id=disk0,format=qcow2 \
-device virtio-scsi-pci,id=scsi0,disable-legacy=on,iommu_platform=true \
-device scsi-hd,drive=disk0 \
-device virtio-net-pci,disable-legacy=on,iommu_platform=true,netdev=vmnic,romfile= \
-netdev user,id=vmnic,hostfwd=tcp::$SSH_FORWARD_PORT-:22 \
-fw_cfg name=opt/ovmf/X-PciMmio64Mb,string=151072
