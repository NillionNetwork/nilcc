#!/usr/bin/env bash
# This script builds the kernel with support for AMDSEV for the host or guest, it uses two phases one in the host and
# the other in docker see docker_build.sh for docker phase
# Usage ./build.sh host|guest [--clean]

set -e
SCRIPT_PATH=$(dirname $(realpath $0))

# Latest commit on branch snp-latest at the time of writing.
COMMIT="e8b814d629a0c2073239828e63d50b125c013570"

[[ "$1" != "host" && "$1" != "guest" ]] && echo "Usage: $0 host|guest [--clean]" && exit 1
[[ "$2" == "--clean" && -d "$SCRIPT_PATH/build/$1" ]] && sudo rm -rf "$SCRIPT_PATH/build/$1"

[[ ! -d "$SCRIPT_PATH/build/$1" ]] && mkdir -p "$SCRIPT_PATH/build/$1"
cd "$SCRIPT_PATH/build/$1"

[[ ! -d "AMDSEV" ]] && git clone https://github.com/AMDESE/AMDSEV.git
cd AMDSEV/

echo "Checking out commit ${COMMIT}"
git checkout "$COMMIT"

if [[ -n $(git status --porcelain) ]]; then
  echo "Repo is dirty, not applying the patch AMDSEV.patch, run with --clean start from scratch and apply the patch"
else
  git apply ../../../AMDSEV-$1.patch
fi

docker run --rm -v "$SCRIPT_PATH:/kernel" ubuntu:24.04 bash /kernel/docker_build.sh $1
sudo chown -R $(whoami) $SCRIPT_PATH/build/

cp $SCRIPT_PATH/build/$1/AMDSEV/linux/*.deb $SCRIPT_PATH/build/$1
echo "Build finished, artifacts in build/$1"

[[ ! -d $SCRIPT_PATH/../dist/kernel/$1 ]] && mkdir -p $SCRIPT_PATH/../dist/kernel/$1
cp $SCRIPT_PATH/build/$1/AMDSEV/linux/*.deb "$SCRIPT_PATH/../dist/kernel/$1"
