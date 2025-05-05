#!/usr/bin/env bash
# docker phase of creating a ubuntu autoinstall iso with AMDSEV support
# This script is called inside of docker by the build.sh script, go there to understand the whole context
set -e

SCRIPT_PATH=$(dirname $(realpath $0))

TYPE=$1
SUBTYPE=$2
NAME="${TYPE}$([[ "$SUBTYPE" == "" ]] && echo "" || echo "-${SUBTYPE}")"

BUILD_PATH="$SCRIPT_PATH/build/$TYPE"
[[ "$SUBTYPE" != "" ]] && BUILD_PATH="$BUILD_PATH/$SUBTYPE"

export DEBIAN_FRONTEND=noninteractive
apt update
apt install -y xorriso curl 7zip

# build autoinstall ISO
ISO_NAME="ubuntu-24.04.2-live-server-amd64.iso"
AUTOINSTALL_ISO_NAME="ubuntu-24.04.2-live-server-amd64-autoinstall-${NAME}.iso"

UBUNTU_ISO_PATH="$SCRIPT_PATH/build/$ISO_NAME"
[[ ! -f "${UBUNTU_ISO_PATH}" ]] && curl -L https://releases.ubuntu.com/noble/ubuntu-24.04.2-live-server-amd64.iso -o "$UBUNTU_ISO_PATH"

[[ ! -d "$BUILD_PATH/iso/" ]] && mkdir -p "$BUILD_PATH/iso/"
cd "$BUILD_PATH/iso"

[[ -d custom-iso ]] && rm -rf custom-iso
[[ -d BOOT ]] && rm -rf BOOT

7z -y x "$UBUNTU_ISO_PATH" -ocustom-iso
mv './custom-iso/[BOOT]' ./BOOT

mkdir -p custom-iso/nocloud/
cp $SCRIPT_PATH/user-data-$NAME.yaml ./custom-iso/nocloud/user-data
touch ./custom-iso/nocloud/meta-data

# Add kernel to iso
mkdir -p custom-iso/packages
cp ../kernel/*.deb custom-iso/packages

mkdir -p custom-iso/nillion
cp -r ${BUILD_PATH}/custom/* custom-iso/nillion

# add cuda-keyring to iso if guest gpu
if [[ "$TYPE" == "guest" && "$SUBTYPE" == "gpu" ]]; then
  curl -L https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2404/x86_64/cuda-keyring_1.1-1_all.deb -o custom-iso/packages/cuda-keyring_1.1-1_all.deb
fi

# add boot entry to grub to autoinstall
sed -i '7r /dev/stdin' custom-iso/boot/grub/grub.cfg <<EOF
menuentry "Autoinstall Ubuntu Server" {
    set gfxpayload=keep
    linux   /casper/vmlinuz autoinstall ds=nocloud\;s=/cdrom/nocloud/ debug verbose console=ttyS0 ---
    initrd  /casper/initrd
}
EOF
# set timeout to 2
sed -i 's/set timeout=30/set timeout=2/' custom-iso/boot/grub/grub.cfg

xorriso -as mkisofs -r \
  -V "Ubuntu 24.04 AUTO $NAME" \
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
