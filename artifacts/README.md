# Artifacts

This directory contains all the artifacts that are required to run a CVM in nilcc. The artifacts in here are split into 
2: the ones used in the host machine, and the ones used in the CVM.

Make sure to read the [parent README file](../README.md) to understand the different components involved in booting a 
CVM before reading this file.

## Host

The host setup requires:

* Compiling a kernel with AMD SEV/SNP support.
* Compiling qemu with AMD SEV/SNP support.
* Setting up the necessary BIOS settings.
* Installing the right nvidia drivers if GPU support is required.

In this repository, only the kernel and qemu compilation steps are addressed.

### Kernel

The kernel used in the host machine can be built by running:

```bash
./kernel/build.sh host
```

This will use a patched version of a linux kernel that supports AMD SEV/SNP and can therefore be used in the baremetal 
machine that runs CVMs.

### QEMU

QEMU with AMD SEV/SNP support can be built by running:

```bash
./qemu/build.sh
```

This will create a single `tar.gz` file that will contain all output artifacts for the QEMU build including its 
binaries, the OVMF to be used, etc.

## Guest

The guest setup is composed of various steps:

### Kernel

Similar to the host kernel, the guest kernel can be built by running the following command:

```bash
./kernel/build.sh guest
```

### initramfs

The initramfs can be built by running:

```bash
./initramfs/build.sh
```

### OS installation ISO

An installation ISO can be created by running the following command, specifying `gpu` or `cpu` depending on whether 
`gpu` support is desired or not:

```bash
./autoinstall_ubuntu/build.sh guest <cpu|gpu>
```

This will create an ISO file that will contain all necessary resources to get a guest operating system up.

### Base VM disk image

A base VM disk image can be created by running:

```bash
./vm_image/build.sh <cpu|gpu>
```

This will take some time but in the end will generate:

* A `qcow2` file that contains the disk image for the VM.
* A file that contains the verity hash device and another one that contains the root hash. See more about this 
[here](../README.md#dm-verity)

It is very important to note that all 3 files work in tandem: you cannot take one of the files from one build and the 
other 2 from another one. Doing this will otherwise cause the CVM boot to fail.

## Build artifacts

While every build step generates files under a `build` directory, all scripts will copy over the final artifacts into 
`./dist`.

### Public releases

All artifacts are also uploaded to an [s3 bucket](https://nilcc.s3.eu-west-1.amazonaws.com/) every time a change is 
merged which modifies any of them.
