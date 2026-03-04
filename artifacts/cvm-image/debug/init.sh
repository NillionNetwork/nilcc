#!/bin/sh

set -e

log() {
  echo >/dev/kmsg $@
}

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

log "Mounting disk"
MNT_DIR=/root
mount /dev/vda "$MNT_DIR"
mkdir -p "${MNT_DIR}/media/state/tmp"
chmod 777 "${MNT_DIR}/media/state/tmp"
cp -r "${MNT_DIR}/ro/var" "${MNT_DIR}/media/state/"

# Create a user nillion:nillion
useradd --root "$MNT_DIR" \
  -g sudo \
  -p '$6$.Uf8tj/pGoGkjrCh$eF6BIaqwEFdsA44YSc0hb1mKzrjJr0HfUb/2zEa/rIetxiDoW1Olya/MUA19Px2GrDK12ocTCme140Er1rUAb/' \
  -s /bin/bash \
  nillion

log "Continuing normal boot"
exec switch_root "$MNT_DIR" /sbin/init
