#!/bin/bash

set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

error_handler() {
  shutdown -P now
}

# Make sure to shutdown in case we hit an error during setup.
trap error_handler ERR SIGINT

# Mount the .iso that contains all the files we need.
CDROM="/tmp/cdrom"
mkdir -p "$CDROM"
mount -o loop /dev/sr0 "$CDROM"

apt update

# Install all packages we have built externally.
dpkg -i $CDROM/packages/*.deb

# Install docker compose
apt install -y --no-install-recommends docker-compose-v2 netplan.io gpg

# Copy over cvm-agent.
mkdir /opt/nillion
cp "$CDROM/nillion/cvm-agent" /opt/nillion
chmod +x /opt/nillion/cvm-agent

# Configure cvm-agent to auto start.
cp "$CDROM/nillion/cvm-agent.service" /etc/systemd/system/
systemctl daemon-reload
systemctl enable cvm-agent.service

# Copy metadata files
cp "$CDROM/nillion/nilcc-version" /opt/nillion
cp "$CDROM/nillion/nilcc-vm-type" /opt/nillion

# Remove any packages we no longer need.
rm -rf /etc/ssh/sshd_config.d
apt purge -y \
  apport \
  keyboard-configuration \
  linux-image-6.8* \
  linux-modules-6.8* \
  openssh-client \
  openssh-server \
  snapd \
  ubuntu-pro-client \
  ubuntu-release-upgrader-core \
  unattended-upgrades \
  valgrind
apt -y autoremove --purge

# Perform any GPU specific configs. Note that this is here because the autoremove above otherwise removes the nvidia
# drivers even if we `apt-mark hold` them.
if [ "$VM_TYPE" == "gpu" ]; then
  # Configure the nvidia repos
  curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
  curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | sed "s#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g" | tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
  apt update

  # Install initramfs-tools since that's needed for the nvidia driver to be configured successfully.
  apt -y install initramfs-tools

  # Install the nvidia driver, container toolkit, and zstd since it otherwise errors.
  apt -y install nvidia-driver-550-server-open nvidia-container-toolkit zstd gpg

  # Configure the nvidia docker runtime.
  nvidia-ctk runtime configure --runtime=docker

  # Enable nvidia lkca.
  echo "install nvidia /sbin/modprobe ecdsa_generic ecdh; /sbin/modprobe --ignore-install nvidia" >/etc/modprobe.d/nvidia-lkca.conf

  # Enable nvidia persistent mode.
  sed -i "/^ExecStart=/d" /usr/lib/systemd/system/nvidia-persistenced.service
  echo "ExecStart=/usr/bin/nvidia-persistenced --user nvidia-persistenced --uvm-persistence-mode --verbose" >>/usr/lib/systemd/system/nvidia-persistenced.service
fi

# Cleanup apt itself
apt -y clean
rm -rf /var/lib/apt/lists/* /var/log/*

# Remove /etc/fstab since we are in control of the boot process.
rm /etc/fstab

# Cleanup
umount "$CDROM"
rmdir "$CDROM"

# Disable cloud-init
touch /etc/cloud/cloud-init.disabled

# Mark the setup as successful. THIS MUST BE DONE AS OTHERWISE THE BUILD SCRIPT WILL ASSUME THE BUILD FAILED.
touch /var/lib/cvm-success
echo "Setup completed"
