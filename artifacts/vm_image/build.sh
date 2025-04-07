#!/usr/bin/env bash
# This scripts creates am ubuntu server autoinstall iso with AMDSEV support and uses it to install ubuntu in a qemu VM creating a base vm image.
# It has two phases one in the host and the other in docker see docker_build.sh for docker phase.
set -e

SCRIPT_PATH=$(dirname $(realpath $0))

QEMU_STATIC_CHECK=($SCRIPT_PATH/../qemu/build/qemu-static-*.tar.gz)
[[ ${#QEMU_STATIC_CHECK[@]} == 0 ]] && echo "QEMU static package not found, run 'qemu/build.sh' first" && exit 1
[[ ${#QEMU_STATIC_CHECK[@]} > 1 ]] && echo "More than one QEMU static package found, only one can be used" && exit 1
QEMU_STATIC_PATH=${QEMU_STATIC_CHECK[0]}

GUEST_KERNEL_CHECK=($SCRIPT_PATH/../kernel/build/guest/linux-*.deb)
[[ ${#GUEST_KERNEL_CHECK[@]} == 0 ]] && echo "Guest kernel not found, run 'kernel/build.sh guest' first" && exit 1

[[ $1 == "--clean" && -d "$SCRIPT_PATH/build" ]] && sudo rm -rf "$SCRIPT_PATH/build"

[[ ! -d "$SCRIPT_PATH/build/isos" ]] && mkdir -p "$SCRIPT_PATH/build/isos"
[[ ! -d "$SCRIPT_PATH/build/vm_images" ]] && mkdir -p "$SCRIPT_PATH/build/vm_images"
[[ ! -d "$SCRIPT_PATH/build/qemu" ]] && mkdir -p "$SCRIPT_PATH/build/qemu"
[[ ! -d "$SCRIPT_PATH/build/guest_kernel" ]] && mkdir -p "$SCRIPT_PATH/build/guest_kernel"

# Install static qemu
[[ ! -d "$SCRIPT_PATH/build/qemu/usr" ]] && tar -xzf "$QEMU_STATIC_PATH" -C "$SCRIPT_PATH/build/qemu/"

# copy guest kernel
GUEST_KERNEL_CP_CHECK=($SCRIPT_PATH/build/guest_kernel/linux-*.deb)
echo $GUEST_KERNEL_CP_CHECK # TODO
[[ ${#GUEST_KERNEL_CP_CHECK[@]} == 1 ]] && cp $SCRIPT_PATH/../kernel/build/guest/linux-*.deb "$SCRIPT_PATH/build/guest_kernel/"
docker run --rm --privileged -v "$SCRIPT_PATH:/vm_image" -it ubuntu:24.04 bash /vm_image/docker_build.sh