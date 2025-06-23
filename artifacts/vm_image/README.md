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

### Running

Run `launch_vm.sh` to run a confidential VM. By default the VM will not have a tty attached. This behavior can be 
changed by setting the `NILCC_DEBUG` environment variable to 1. e.g.


```bash
NILCC_DEBUG=1 ./launch_vm.sh <args...>
```
