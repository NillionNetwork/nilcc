# qcow2 VM image

This directory contains the scripts to build a qcow2 VM image with AMDSEV support.

### How to build
First build [qemu](../qemu/README.md) and the [guest kernel](../kernel/README.md) as they are dependencies of this build process.

Then:
```bash
./build.sh
```
