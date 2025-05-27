#!/bin/sh

set -e

# Constrain where binaries are looked up
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

[ -d /dev ] || mkdir -m 0755 /dev
[ -d /root ] || mkdir -m 0700 /root
[ -d /sys ] || mkdir /sys
[ -d /proc ] || mkdir /proc
[ -d /tmp ] || mkdir /tmp

mkdir -p /var/lock
mount -t sysfs -o nodev,noexec,nosuid sysfs /sys
mount -t proc -o nodev,noexec,nosuid proc /proc
mount -t devtmpfs -o nosuid,mode=0755 udev /dev
mkdir /dev/pts
mount -t devpts -o noexec,nosuid,gid=5,mode=0620 devpts /dev/pts || true

MNT_DIR=/root
BOOT_PROOF_DIR=$MNT_DIR/var/lib/nilcc-boot

# Parse command line options
for x in $(cat /proc/cmdline); do
  case $x in
  root=*)
    ROOT=${x#root=}
    ;;
  verity_disk=*)
    VERITY_DISK=${x#verity_disk=}
    ;;
  verity_roothash=*)
    VERITY_ROOT_HASH=${x#verity_roothash=}
    ;;
  state_disk=*)
    STATE_DISK=${x#state_disk=}
    ;;
  esac
done

# unlock verity device
veritysetup open $ROOT root $VERITY_DISK $VERITY_ROOT_HASH

# mount root disk as read-only
mount -o ro,noload /dev/mapper/root $MNT_DIR

# Generate a random password and use LUKS to encrypt the state disk with it.
STATE_PASSWORD=$(head -c 64 /dev/random | base64 -w 0)
echo "$STATE_PASSWORD" | cryptsetup luksFormat "$STATE_DISK"

# Now open the disk and format it using ext4
echo "$STATE_PASSWORD" | cryptsetup luksOpen "$STATE_DISK" state
mkfs.ext4 /dev/mapper/state
unset STATE_PASSWORD

# Mount the now encrypted and formatted state disk on /var and clear it up.
mount /dev/mapper/state "${MNT_DIR}/var"

# Now copy over the original /var into the new one.
cp -r "${MNT_DIR}/ro/var" "${MNT_DIR}/"

# Create a tmpfs for /etc and copy over the /ro contents to it
mount -t tmpfs -o size=1024M tmpfs "$MNT_DIR/etc"
cp -r "${MNT_DIR}/ro/etc" "${MNT_DIR}/"

# Create a tmpfs for /tmp.
mount -t tmpfs -o size=1024M tmpfs "$MNT_DIR/tmp"

# Create a mount where the cvm-agent will mount the docker-compose ISO.
mount -t tmpfs -o size=4M tmpfs "$MNT_DIR/media/cvm-agent-entrypoint"

# Generate an attestation using random data
modprobe sev-guest
mkdir -p $BOOT_PROOF_DIR
head -c 64 /dev/urandom >$BOOT_PROOF_DIR/input
/opt/nillion/initrd-helper report $BOOT_PROOF_DIR/report.json --data $(cat $BOOT_PROOF_DIR/input | base64 -w 0)

mount --move /proc $MNT_DIR/proc
mount --move /sys $MNT_DIR/sys
exec switch_root $MNT_DIR/ /sbin/init
