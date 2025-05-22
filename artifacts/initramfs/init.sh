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
ROOT=/dev/sda2
BOOT_PROOF_DIR=$MNT_DIR/etc/nilcc-boot

mount $ROOT $MNT_DIR

# Note, eventually we will want to not do this rm so `mkdir` fails. For now it's good to have it so we can
# boot a vm multiple times from the same .cow2
rm -rf $BOOT_PROOF_DIR
mkdir $BOOT_PROOF_DIR

head -c 64 /dev/urandom >$BOOT_PROOF_DIR/input

modprobe sev-guest
/opt/nillion/initrd-helper report $BOOT_PROOF_DIR/report.json --data $(cat $BOOT_PROOF_DIR/input | base64 -w 0)

mount --move /proc $MNT_DIR/proc
mount --move /sys $MNT_DIR/sys
exec switch_root $MNT_DIR/ /sbin/init
