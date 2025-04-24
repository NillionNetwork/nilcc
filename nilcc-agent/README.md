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
