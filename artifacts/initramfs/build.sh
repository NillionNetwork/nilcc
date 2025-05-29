#!/bin/bash

set -e

SCRIPT_PATH=$(dirname $(realpath $0))

BUILD_DIR="${SCRIPT_PATH}/build"
OUT="$BUILD_DIR/initramfs.cpio.gz"

echo "Preparing directories.."
rm -rf "$BUILD_DIR"
INITRD_DIR=$BUILD_DIR/initramfs
KERNEL_DIR=$BUILD_DIR/kernel

mkdir -p $INITRD_DIR $KERNEL_DIR

KERNEL_DEB=$(ls $SCRIPT_PATH/../kernel/build/guest/linux-image-6.11.0-snp-guest-*.deb | grep -v 'dbg_6.11')
[[ ${#KERNEL_DEB[@]} == 0 ]] && echo "Kernel not found, run 'kernel/build.sh guest' first" && exit 1
[[ ${#KERNEL_DEB[@]} > 1 ]] && echo "More than one kernel package found, only one can be used" && exit 1

cp "$KERNEL_DEB" $KERNEL_DIR/kernel.deb

echo "Building Docker image"
DOCKER_IMG="nilcc-initramfs"

# Create a random container name
DOCKER_CONTAINER="nilcc-initramfs-$(tr -dc 'A-Za-z0-9' </dev/urandom | head -c 16)"

cleanup() {
  echo "Removing container ${DOCKER_CONTAINER}"
  docker rm $DOCKER_CONTAINER >/dev/null
}

# Build our docker image.
docker build -t $DOCKER_IMG -f $SCRIPT_PATH/Dockerfile $SCRIPT_PATH/../../

# Run the container. This will run and stop it immediately since it does nothing by default.
echo "Running container ${DOCKER_CONTAINER}"
docker run --name $DOCKER_CONTAINER $DOCKER_IMG
trap cleanup EXIT SIGINT

# Now export the stopped container's filesystem so we use that as a base for our inintrd.
echo "Exporting filesystem"
docker export $DOCKER_CONTAINER | tar xpf - -C $INITRD_DIR

# Clean up.
echo "Removing unnecessary files and directories"
rm -rf \
  $INITRD_DIR/.dockerenv \
  $INITRD_DIR/boot \
  $INITRD_DIR/dev \
  $INITRD_DIR/home \
  $INITRD_DIR/media \
  $INITRD_DIR/mnt \
  $INITRD_DIR/proc \
  $INITRD_DIR/root \
  $INITRD_DIR/srv \
  $INITRD_DIR/sys \
  $INITRD_DIR/tmp

# We need to clear the "s" permission bit from some executables like `mount`
echo "Changing permissions"
chmod -st $INITRD_DIR/usr/bin/* >/dev/null 2>&1 || true

# Repackage the patched filesystem.
echo "Repackaging initrd"
pushd $INITRD_DIR >/dev/null
find . -print0 | cpio --null -ov --format=newc 2>/dev/null | gzip -1 >$OUT
popd >/dev/null

INITRD_SIZE=$(du -h $OUT | cut -f1)
echo "initrd image generated at ${OUT}, size: ${INITRD_SIZE}"

[[ ! -d $SCRIPT_PATH/../dist/initramfs ]] && mkdir -p $SCRIPT_PATH/../dist/initramfs
cp "$OUT" "$SCRIPT_PATH/../dist/initramfs"
