FROM ubuntu:24.04

COPY artifacts/initramfs/build/kernel/kernel.deb /tmp/

RUN apt update \
  && apt upgrade -y \
  && apt install -y kmod cryptsetup-bin \
  && apt install -y /tmp/kernel.deb \
  && rm -rf /var/lib/apt/lists/* \
  && rm /tmp/kernel.deb

COPY artifacts/initramfs/init.sh /init

