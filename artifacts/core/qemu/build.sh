#!/usr/bin/env bash
# This script builds a static version of QEMU and OVMF with AMDSEV support.
# The process have three phases: first in the host machine, second in a docker container that uses alpine that provides
# static libraries precompiled and third in a docker container that uses ubuntu that provides the tools to build OVMF.
# The second phase is in docker_build.sh.
# The third phase is in docker_build_ovmf.sh.
set -e

SCRIPT_PATH=$(dirname $(realpath $0))

# Latest commit on branch snp-latest at the time of writing.
COMMIT="e8b814d629a0c2073239828e63d50b125c013570"

[[ "$1" == "--clean" && -d "$SCRIPT_PATH/build" ]] && sudo rm -rf "$SCRIPT_PATH/build"

[[ ! -d "$SCRIPT_PATH/build" ]] && mkdir -p "$SCRIPT_PATH/build"
cd "$SCRIPT_PATH/build"

[[ ! -d "$SCRIPT_PATH/build/AMDSEV" ]] && git clone https://github.com/AMDESE/AMDSEV.git
cd AMDSEV/
echo "Checking out commit ${COMMIT}"
git checkout "$COMMIT"
if [[ -n $(git status --porcelain) ]]; then
  echo "Repo is dirty, not applying the patch AMDSEV.patch, run with --clean start from scratch and apply the patch"
else
  git apply ../../AMDSEV.patch
fi

cd ../..

docker run --rm -v "$SCRIPT_PATH:/qemu" alpine:3.22.0 sh -c "apk add bash; bash /qemu/docker_build.sh"
docker run --rm -v "$SCRIPT_PATH:/qemu" ubuntu:24.04 sh -c "bash /qemu/docker_build_ovmf.sh"
sudo chown -R $(whoami) $SCRIPT_PATH/build/

PACKAGE_PATH="$SCRIPT_PATH/build/qemu-static.tar.gz"
tar -czf "$PACKAGE_PATH" -C build/AMDSEV usr

mkdir -p $SCRIPT_PATH/../../dist/core/
cp $PACKAGE_PATH "$SCRIPT_PATH/../../dist/core"

echo "Build done, output: $PACKAGE_PATH"
