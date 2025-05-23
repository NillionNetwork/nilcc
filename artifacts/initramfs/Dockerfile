FROM ubuntu:24.04

RUN apt update \
  && apt upgrade -y \
  && rm -rf /var/lib/apt/lists/*

COPY init.sh /init
