#!/usr/bin/env bash
# This is the second phase of the process of building a static qemu with AMDSEV support.
# This script is called inside of docker by the build.sh script, go there to understand the whole context.
set -e

apk update
apk add gcc autoconf automake libtool sqlite ninja vim bash git python3 musl-dev build-base glib-dev gettext-dev shadow sudo meson iasl \
 glib-static gettext-static zlib-static util-linux-static bzip2-static ncurses-static libslirp-dev zlib-dev zlib-static libxkbcommon-static \
 libxkbcommon-dev libx11-static zstd-static zstd-dev zstd-libs libpng libpng-dev libpng-static libselinux libselinux-dev libselinux-static \
 libudev-zero libudev-zero-dev pixman pixman-dev pixman-static nasm perl pcre2-static

# install static libslirp
git clone https://gitlab.freedesktop.org/slirp/libslirp.git /tmp/libslirp
cd /tmp/libslirp
git checkout v4.7.0
meson setup --default-library static build
ninja -C build install
rm -rf /tmp/libslirp

cd /qemu/build/AMDSEV
./build.sh qemu --package
