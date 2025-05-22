#!/usr/bin/env bash
# This script launches the vm image to test the build.
#
# Note that because of the sev-snp options being used it must be launched inside an sev host.

set -e

SCRIPT_PATH=$(dirname $(realpath $0))

VM_IMAGE="${1:-$SCRIPT_PATH/build/vm_images/ubuntu24.04-cpu.qcow2}"
[[ ! -f "$VM_IMAGE" ]] && echo "VM image not found, run 'build.sh' first" && exit 1

INITRD="${INITRD:-$SCRIPT_PATH/../initramfs/build/initramfs.cpio.gz}"
[[ ! -f "$INITRD" ]] && echo "initrd not found, run '../initramfs/build.sh' first" && exit 1

QEMU_BASE_PATH="$SCRIPT_PATH/build/qemu/"
$QEMU_BASE_PATH/usr/local/bin/qemu-system-x86_64 \
  -m 1G \
  -enable-kvm -nographic -no-reboot \
  -machine confidential-guest-support=sev0,vmport=off \
  -object sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1,kernel-hashes=on \
  -cpu EPYC-v4 -machine q35 -smp 8,maxcpus=8 -m 16G,slots=2,maxmem=120G \
  -bios $QEMU_BASE_PATH/usr/local/share/qemu/OVMF.fd \
  -kernel ${SCRIPT_PATH}/build/kernel/cpu/vmlinuz-6.11.0-snp-guest-98f7e32f20d2 \
  -initrd "${INITRD}" \
  -append "console=ttyS0 earlyprintk=serial" \
  -drive file=$VM_IMAGE,if=none,id=disk0,format=qcow2 \
  -device virtio-scsi-pci,id=scsi0,disable-legacy=on,iommu_platform=true \
  -device scsi-hd,drive=disk0,bootindex=1 \
  -drive file=/tmp/nilcc.iso,id=disk1,media=cdrom,readonly=true \
  -fw_cfg name=opt/ovmf/X-PciMmio64Mb,string=151072
