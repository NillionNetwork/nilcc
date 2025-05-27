#!/usr/bin/env bash
# This script launches the vm image to test the build.
#
# Note that because of the sev-snp options being used it must be launched inside an sev host.

set -e

SCRIPT_PATH=$(dirname $(realpath $0))

VM_IMAGE="${1:-$SCRIPT_PATH/build/vm_images/ubuntu24.04-cpu.qcow2}"
[[ ! -f "$VM_IMAGE" ]] && echo "VM image not found, run 'build.sh' first" && exit 1

VM_IMAGE_HASH_DEV="${1:-$SCRIPT_PATH/build/vm_images/ubuntu24.04-cpu-verity/verity-hash-dev}"
[[ ! -f "$VM_IMAGE_HASH_DEV" ]] && echo "VM disk hashes dev not found, run 'build.sh' first" && exit 1

VM_IMAGE_ROOT_HASH=$(cat "${1:-$SCRIPT_PATH/build/vm_images/ubuntu24.04-cpu-verity/root-hash}")

INITRD="${INITRD:-$SCRIPT_PATH/../initramfs/build/initramfs.cpio.gz}"
[[ ! -f "$INITRD" ]] && echo "initrd not found, run '../initramfs/build.sh' first" && exit 1

STATE_DISK=$(mktemp)

cleanup() {
  rm ${STATE_DISK}
}

trap cleanup EXIT SIGINT

QEMU_BASE_PATH="$SCRIPT_PATH/build/qemu/"
$QEMU_BASE_PATH/usr/local/bin/qemu-img create -f raw "$STATE_DISK" 10G

$QEMU_BASE_PATH/usr/local/bin/qemu-system-x86_64 \
  -enable-kvm -nographic -no-reboot \
  -machine confidential-guest-support=sev0,vmport=off \
  -object sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1,kernel-hashes=on \
  -cpu EPYC-v4 -machine q35 -smp 8,maxcpus=8 -m 16G,slots=2,maxmem=120G \
  -bios $QEMU_BASE_PATH/usr/local/share/qemu/OVMF.fd \
  -kernel ${SCRIPT_PATH}/build/kernel/cpu/vmlinuz-6.11.0-snp-guest-98f7e32f20d2 \
  -initrd "${INITRD}" \
  -append "console=ttyS0 earlyprintk=serial root=/dev/sda2 verity_disk=/dev/sdb verity_roothash=${VM_IMAGE_ROOT_HASH} state_disk=/dev/sdc" \
  -drive file=$VM_IMAGE,if=none,id=disk0,format=qcow2 \
  -device virtio-scsi-pci,id=scsi0,disable-legacy=on,iommu_platform=true \
  -device scsi-hd,drive=disk0,bootindex=1 \
  -drive file=$VM_IMAGE_HASH_DEV,if=none,id=disk1,format=raw \
  -device virtio-scsi-pci,id=scsi1,disable-legacy=on,iommu_platform=true \
  -device scsi-hd,drive=disk1,bootindex=2 \
  -drive file=$STATE_DISK,if=none,id=disk2,format=raw \
  -device virtio-scsi-pci,id=scsi2,disable-legacy=on,iommu_platform=true \
  -device scsi-hd,drive=disk2,bootindex=3 \
  -drive file=/tmp/nilcc.iso,id=disk3,media=cdrom,readonly=true \
  -fw_cfg name=opt/ovmf/X-PciMmio64Mb,string=151072
