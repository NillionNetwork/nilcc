#!/usr/bin/env bash
# This script builds a static version of QEMU with AMDSEV support.
# The process have two phases one in the host machine and the other in a docker container that uses alpine that provides static libraries precompiled.
# The second phase is in docker_build.sh.
set -e

SCRIPT_PATH=$(dirname $(realpath $0))

[[ -d "$SCRIPT_PATH/build" ]] && rm -rf "$SCRIPT_PATH/build"

mkdir -p "$SCRIPT_PATH/build"
cd "$SCRIPT_PATH/build"

git clone https://github.com/AMDESE/AMDSEV.git
cd AMDSEV/
git checkout snp-latest
COMMIT=$(git rev-parse --short HEAD)
git apply ../../AMDSEV.patch

cd ../..

docker run -v .:/qemu -it alpine sh -c "apk add bash; bash /qemu/docker_build.sh"
PACKAGE_PATH="$SCRIPT_PATH/build/qemu-static-${COMMIT}-$(date +%d-%m-%Y).tar.gz"
tar -czf "$PACKAGE_PATH" build/AMDSEV/usr

echo "Build done, output: $PACKAGE_PATH"