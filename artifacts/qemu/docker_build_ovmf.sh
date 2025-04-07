#!/usr/bin/env bash
# This is the second phase of the process of building a OVMF with AMDSEV support.
# This script is called inside of docker by the build.sh script, go there to understand the whole context.
set -e

export DEBIAN_FRONTEND=noninteractive
apt update
apt install -y build-essential uuid-dev iasl nasm python3 gcc g++ make git \
 python3-pip python3-venv libssl-dev libelf-dev python-is-python3

cd /qemu/build/AMDSEV
./build.sh ovmf --package
