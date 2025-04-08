# qcow2 VM image

This directory contains the scripts to build a qcow2 VM image with AMDSEV support.

### How to build
First build [qemu](../qemu/README.md) and the [ubuntu iso](../autoinstall_ubuntu/README.md) as they are dependencies of this build process.

#### CPU only vm
```bash
./build.sh cpu
```

#### GPU vm
```bash
./build.sh gpu
```