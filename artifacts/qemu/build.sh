#!/usr/bin/env bash
set -e

SCRIP_PATH=$(dirname $(realpath $0))

[[ -d "$SCRIP_PATH/build" ]] && rm -rf "$SCRIP_PATH/build"

mkdir -p "$SCRIP_PATH/build"
cd "$SCRIP_PATH/build"

git clone https://github.com/AMDESE/AMDSEV.git
cd AMDSEV/
git checkout snp-latest
COMMIT=$(git rev-parse --short HEAD)
git apply ../../AMDSEV.patch

cd ../..

docker run -v .:/qemu -it alpine sh -c "apk add bash; bash /qemu/docker_build.sh"
PACKAGE_PATH="$SCRIP_PATH/build/qemu-static-${COMMIT}-$(date +%d-%m-%Y).tar.gz"
tar -czf "$PACKAGE_PATH" build/AMDSEV/usr

echo "Build done, output: $PACKAGE_PATH"