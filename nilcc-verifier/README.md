# nilcc-verifier

This tool allows requesting attestation reports from a confidential VMs running in nilcc and validate that:

* The measurement in the reports is correct, meaning the VMs are running the code that we think they're running.
* The signature in the attestation report is correct, meaning it was signed by a confidential VM running on an AMD SEV 
enabled CPU.

## Usage

This tool currently requires the kernel, initrd, OVMF file and hashes used during boot to be available locally. Run 
`nilcc-verifier -h` to learn more on how to use it.
