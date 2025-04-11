#!/usr/bin/env bash
# This scripts creates am ubuntu server autoinstall iso with AMDSEV support.
# It has two phases one in the host and the other in docker see docker_build.sh for docker phase.
# usage: ./build.sh guest|host [cpu|gpu] [--clean]
set -e

SCRIPT_PATH=$(dirname $(realpath $0))

[[ "$1" != "guest" && "$1" != "host" ]] && echo "Invalid argument, use 'guest' or 'host'" && exit 1
[[ "$1" == "guest" && "$2" != "cpu" && "$2" != "gpu" ]] && echo "Invalid argument, use 'cpu' or 'gpu'" && exit 1

TYPE=$1
shift
[[ "$TYPE" == "guest" ]] && SUBTYPE=$1 && shift

CLEAN=$1

BUILD_PATH="$SCRIPT_PATH/build/$TYPE"
[[ "$SUBTYPE" != "" ]] && BUILD_PATH="$BUILD_PATH/$SUBTYPE"

[[ "$CLEAN" == "--clean" ]] && rm -rf "$BUILD_PATH"
[[ ! -d "$BUILD_PATH" ]] && mkdir -p "$BUILD_PATH"
[[ ! -d "$BUILD_PATH/kernel" ]] && mkdir -p "$BUILD_PATH/kernel"
[[ ! -d "$BUILD_PATH/custom" ]] && mkdir -p "$BUILD_PATH/custom"

# Copy kernel
KERNEL_CHECK=($SCRIPT_PATH/../kernel/build/$TYPE/linux-*.deb)
[[ ${#KERNEL_CHECK[@]} == 0 ]] && echo "$TYPE kernel not found, run 'kernel/build.sh $TYPE' first" && exit 1
cp $SCRIPT_PATH/../kernel/build/$TYPE/linux-*.deb "$BUILD_PATH/kernel/"

# Copy cvm-agent script and dependencies.
cp $SCRIPT_PATH/../../cvm-agent/cvm-agent.sh "$BUILD_PATH/custom/"
cp -r $SCRIPT_PATH/../../cvm-agent/services/ "$BUILD_PATH/custom/"
cp $SCRIPT_PATH/cvm-agent.service "$BUILD_PATH/custom/"

docker run --rm --privileged -v "$SCRIPT_PATH:/iso" ubuntu:24.04 bash /iso/docker_build.sh $TYPE $SUBTYPE
sudo chown -R $(whoami) $SCRIPT_PATH/build/

[[ ! -d $SCRIPT_PATH/../dist/isos ]] && mkdir -p $SCRIPT_PATH/../dist/isos
cp $BUILD_PATH/iso/ubuntu-*.iso "$SCRIPT_PATH/../dist/isos"