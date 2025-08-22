#!/usr/bin/env bash
# docker phase of the kernel build

set -e

export DEBIAN_FRONTEND=noninteractive

apt update
apt install -y python3-venv ninja-build libglib2.0-dev python-is-python3 nasm iasl flex bison libelf-dev debhelper \
  ninja-build iasl nasm flex bison openssl dkms autoconf bc python3-pip git-lfs openssl libssl-dev cpio zstd rsync

cp /kernel/config-6.8.0-57-generic /boot/config

cd /kernel/build/$1/AMDSEV
./build.sh kernel $1 --package
