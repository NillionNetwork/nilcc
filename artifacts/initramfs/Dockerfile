FROM ubuntu:24.04

ARG KERNEL_VERSION

RUN apt update \
  && apt upgrade -y \
  && apt install -y --no-install-recommends \
    kmod \
    cryptsetup-bin \
    linux-modules-${KERNEL_VERSION}-generic \
  && rm -rf /var/lib/apt/lists/*

COPY artifacts/initramfs/init.sh /init

