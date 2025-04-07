#!/usr/bin/env bash
# docker phase of creating a base vm image with AMDSEV support
# This script is called inside of docker by the build.sh script, go there to understand the whole context
set -e

export PATH=/vm_image/build/qemu/usr/local/bin:$PATH

export DEBIAN_FRONTEND=noninteractive
apt update
apt install -y xorriso curl 7zip

# build autoinstall ISO
ISO_NAME="ubuntu-24.04.2-live-server-amd64.iso"
AUTOINSTALL_ISO_NAME="ubuntu-24.04.2-live-server-amd64-autoinstall.iso"
UBUNTU_ISO_PATH="/vm_image/build/isos/$ISO_NAME"
[[ ! -f "${UBUNTU_ISO_PATH}" ]] && curl -L https://releases.ubuntu.com/noble/ubuntu-24.04.2-live-server-amd64.iso -o "$UBUNTU_ISO_PATH"

cd  "/vm_image/build/isos"

if [[ ! -f "./$AUTOINSTALL_ISO_NAME" ]]; then
  [[ -d custom-iso ]] && rm -rf custom-iso
  [[ -d BOOT ]] && rm -rf BOOT

  7z -y x "./$ISO_NAME" -ocustom-iso
  mv  './custom-iso/[BOOT]' ./BOOT

  mkdir -p custom-iso/nocloud/
  cp ../../ubuntu-autoinstall-user-data.yaml ./custom-iso/nocloud/user-data
  touch ./custom-iso/nocloud/meta-data

  # Add guest kernel to iso
  mkdir -p custom-iso/packages
  cp ../guest_kernel/*.deb custom-iso/packages


  # add boot entry to grub to autoinstall
  sed -i '7r /dev/stdin' custom-iso/boot/grub/grub.cfg <<EOF
menuentry "Autoinstall Ubuntu Server" {
      set gfxpayload=keep
      linux   /casper/vmlinuz autoinstall ds=nocloud\;s=/cdrom/nocloud/ debug verbose console=ttyS0 ---
      initrd  /casper/initrd
}
EOF

  xorriso -as mkisofs -r \
    -V 'Ubuntu 24.04 AUTO (EFIBIOS)' \
    -o ./$AUTOINSTALL_ISO_NAME \
    --grub2-mbr ./BOOT/1-Boot-NoEmul.img \
    -partition_offset 16 \
    --mbr-force-bootable \
    -append_partition 2 28732ac11ff8d211ba4b00a0c93ec93b ./BOOT/2-Boot-NoEmul.img \
    -appended_part_as_gpt \
    -iso_mbr_part_type a2a0d0ebe5b9334487c068b6b72699c7 \
    -c '/boot.catalog' \
    -b '/boot/grub/i386-pc/eltorito.img' \
    -no-emul-boot -boot-load-size 4 -boot-info-table --grub2-boot-info \
    -eltorito-alt-boot \
    -e '--interval:appended_partition_2:::' \
    -no-emul-boot \
    ./custom-iso/
fi

AUTOINSTALL_UBUNTU_ISO_PATH="/vm_image/build/isos/$AUTOINSTALL_ISO_NAME"

# Create VM image
VM_IMAGE_PATH="/vm_image/build/vm_images/ubuntu24.04.qcow2"
[[ ! -f "$VM_IMAGE_PATH" ]] && qemu-img create -f qcow2 "$VM_IMAGE_PATH" 500G

SSH_FORWARD_PORT=2222

# Install ubuntu on VM
qemu-system-x86_64 \
-enable-kvm -nographic -no-reboot -cpu EPYC-v4 -machine q35 \
-smp 12,maxcpus=31 -m 16G,slots=5,maxmem=120G \
-drive if=pflash,format=raw,unit=0,file=/vm_image/build/qemu/usr/local/share/qemu/OVMF_CODE.fd,readonly=on \
-drive file=$VM_IMAGE_PATH,if=none,id=disk0,format=qcow2 \
-device virtio-scsi-pci,id=scsi0,disable-legacy=on,iommu_platform=true \
-device scsi-hd,drive=disk0 \
-device virtio-net-pci,disable-legacy=on,iommu_platform=true,netdev=vmnic,romfile= \
-netdev user,id=vmnic,hostfwd=tcp::$SSH_FORWARD_PORT-:22 \
-cdrom $AUTOINSTALL_UBUNTU_ISO_PATH

