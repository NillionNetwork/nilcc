# ubuntu autoinstall ISO

This directory contains the scripts to build an Ubuntu ISO with autoinstall support tailored for AMDSEV.

### How to build
First build the [kernels](../kernel/README.md) as they are dependencies of this build process.

#### Guest CPU only ISO
```bash
./build.sh guest cpu
```

#### Guest GPU ISO
```bash
./build.sh guest gpu
```

#### Host
```bash
./build.sh host
```