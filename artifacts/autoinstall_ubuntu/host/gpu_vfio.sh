#!/usr/bin/env bash

for gpu_id in `lspci -nn -D | grep -i nvidia | cut -d " " -f1`
do
    dev="/sys/bus/pci/devices/$gpu_id"
    group=$(readlink -e $dev/iommu_group | cut -d "/" -f 5)
    echo "vfio-pci" > $dev/driver_override
    echo $gpu_id > $dev/driver/unbind
    echo $gpu_id > /sys/bus/pci/drivers_probe

    # check that group is now in vfio
    [[ -e /dev/vfio/$group ]] && echo "Bind of GPU to VFIO-PCI sucessful " || echo "Failed to bind GPU to VFIO-PCI"
done