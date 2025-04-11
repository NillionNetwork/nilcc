#!/usr/bin/env bash
# This script builds a static version of QEMU and OVMF with AMDSEV support.
# The process have three phases: first in the host machine, second in a docker container that uses alpine that provides
# static libraries precompiled and third in a docker container that uses ubuntu that provides the tools to build OVMF.
# The second phase is in docker_build.sh.
# The third phase is in docker_build_ovmf.sh.
set -e

SCRIPT_PATH=$(dirname $(realpath $0))

[[ "$1" == "--clean" && -d "$SCRIPT_PATH/build" ]] && sudo rm -rf "$SCRIPT_PATH/build"

[[ ! -d "$SCRIPT_PATH/build" ]] && mkdir -p "$SCRIPT_PATH/build"
cd "$SCRIPT_PATH/build"

[[ ! -d "$SCRIPT_PATH/build/AMDSEV" ]] && git clone https://github.com/AMDESE/AMDSEV.git
cd AMDSEV/
[[ "$(git branch --show-current)" != "snp-latest" ]] && git checkout snp-latest
COMMIT=$(git rev-parse --short HEAD)
if [[ -n $(git status --porcelain) ]]; then
  echo "Repo is dirty, not applying the patch AMDSEV.patch, run with --clean start from scratch and apply the patch"
else
  git apply ../../AMDSEV.patch
fi

cd ../..

docker run --rm -v "$SCRIPT_PATH:/qemu" alpine sh -c "apk add bash; bash /qemu/docker_build.sh"
docker run --rm -v "$SCRIPT_PATH:/qemu" ubuntu:24.04 sh -c "bash /qemu/docker_build_ovmf.sh"
sudo chown -R $(whoami) $SCRIPT_PATH/build/

PACKAGE_PATH="$SCRIPT_PATH/build/qemu-static-${COMMIT}-$(date +%d-%m-%Y).tar.gz"
tar -czf "$PACKAGE_PATH" -C build/AMDSEV usr

[[ ! -d $SCRIPT_PATH/../dist/ ]] && mkdir -p $SCRIPT_PATH/../dist/
cp $PACKAGE_PATH "$SCRIPT_PATH/../dist/"

echo "Build done, output: $PACKAGE_PATH"