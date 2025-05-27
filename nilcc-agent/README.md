# nilCC Agent

A lightweight agent that provides CLI to create and manage VMs in QEMU
- VM provisioning via `QEMU`
- Executing Workloads (launching docker containers) via `cvm-agent`

## Development
1. Install QEMU
   1. `sudo apt update`
   2. `sudo apt install cpu-checker`
   3. `kvm-ok`
   4. `sudo apt install qemu-system-x86 qemu-utils qemu-kvm libvirt-daemon-system libvirt-clients ovmf`
2. Test
   1. To be able to view VM console in window when testing: `GDK_BACKEND=wayland cargo test -- --nocapture`

## Launch VM

### Non-confidential VM

- Create and run VM with name `vm-test-01`
```bash
nilcc-agent vm create --cpu 1 --ram-mib 512 --disk-gib 1 vm-test-01
```

- Gracefully stop VM - send ACPI shutdown signal and wait for it to stop
```bash
nilcc-agent vm stop vm-test-01
```

- Forcefully stop VM - power off VM straight away
```bash
nilcc-agent vm stop --force vm-test-01
```

### Confidential VM

Running CVM execution requires root permissions and OVMF firmware with AMD SEV support.

sudo example with PATH containing nilcc-agent: `sudo env "PATH=$PATH" nilcc-agent ...`

- Create and run CVM with name `cvm-test-01`
```bash
nilcc-agent vm create --cpu 1 --ram-mib 512 --disk-gib 1 --enable-cvm --bios-path /home/ubuntu/shared/AMDSEV/usr/local/share/qemu/OVMF.fd cvm-test-01
```
