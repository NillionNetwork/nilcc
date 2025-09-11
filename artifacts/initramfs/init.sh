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
  docker_compose_disk=*)
    DOCKER_COMPOSE_DISK=${x#docker_compose_disk=}
    ;;
  docker_compose_hash=*)
    DOCKER_COMPOSE_HASH=${x#docker_compose_hash=}
    ;;
  debug_mode=1)
    DEBUG_MODE=1
    ;;
  esac
done

for var in ROOT VERITY_DISK VERITY_ROOT_HASH STATE_DISK DOCKER_COMPOSE_DISK DOCKER_COMPOSE_HASH; do
  if [ ! -n "$var" ]; then
    log "${var} not set!"
    exit 1
  fi
done

# unlock verity device
log "Opening $ROOT device and ensuring it matches root hash $VERITY_ROOT_HASH"
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

# Mount the ISO that contains the docker compose file into the path where cvm-agent will look it up.
mount -o loop "$DOCKER_COMPOSE_DISK" "$MNT_DIR/media/cvm-agent-entrypoint"

# Ensure docker compose hash exists.
DOCKER_COMPOSE_PATH="${MNT_DIR}/media/cvm-agent-entrypoint/docker-compose.yaml"
if [ ! -f "$DOCKER_COMPOSE_PATH" ]; then
  log "Docker compose file not found"
  exit 1
fi

# Validate the docker compose hash.
ACTUAL_HASH=$(sha256sum "$DOCKER_COMPOSE_PATH" | awk '{{ print $1 }}')
if [ "$ACTUAL_HASH" != "$DOCKER_COMPOSE_HASH" ]; then
  log "Docker compose hash mismatch: expected ${DOCKER_COMPOSE_HASH}, got ${ACTUAL_HASH}"
  exit 1
fi

log "Docker compose hash matches expected one: ${ACTUAL_HASH}"

if [ "${DEBUG_MODE}" = "1" ]; then
  # Create a user nillion:nillion
  useradd --root "$MNT_DIR" \
    -g sudo \
    -p '$6$.Uf8tj/pGoGkjrCh$eF6BIaqwEFdsA44YSc0hb1mKzrjJr0HfUb/2zEa/rIetxiDoW1Olya/MUA19Px2GrDK12ocTCme140Er1rUAb/' \
    -s /bin/bash \
    nillion
fi

mount --move /proc $MNT_DIR/proc
mount --move /sys $MNT_DIR/sys

log "Continuing normal boot"
exec switch_root $MNT_DIR/ /sbin/init
