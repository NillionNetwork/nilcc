FROM rust:1.86-alpine AS build

WORKDIR /opt/nillion
RUN apk add --no-cache musl-dev

COPY . .
RUN cargo build --release --locked -p initrd-helper

FROM ubuntu:24.04

COPY artifacts/initramfs/build/kernel/kernel.deb /tmp/

RUN apt update \
  && apt upgrade -y \
  && apt install -y kmod cryptsetup-bin \
  && apt install -y /tmp/kernel.deb \
  && rm -rf /var/lib/apt/lists/* \
  && rm /tmp/kernel.deb

COPY artifacts/initramfs/init.sh /init
COPY --from=build /opt/nillion/target/release/initrd-helper /opt/nillion/initrd-helper

