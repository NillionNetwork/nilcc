#!/usr/bin/env python3

import sys
import argparse
import base64
import logging


NRAS_URL = "https://nras.attestation.nvidia.com/v3/attest/gpu"


def disable_logging():
    logger = logging.getLogger("sdk-logger")
    logger.setLevel(logging.CRITICAL + 1)
    handler = logging.StreamHandler(sys.stdout)
    logger.addHandler(handler)


def main():
    # Disable logging before we import the nvidia package since it otherwise logs
    # disable_logging()
    from nv_attestation_sdk import attestation

    parser = argparse.ArgumentParser(prog="gpu-attester")
    parser.add_argument("nonce", type=str)
    args = parser.parse_args()

    client = attestation.Attestation()
    client.set_name("nilcc-gpu-attestation")
    client.set_nonce(args.nonce)
    client.add_verifier(
        attestation.Devices.GPU, attestation.Environment.REMOTE, NRAS_URL, ""
    )

    evidence = client.get_evidence()
    if not client.attest(evidence):
        sys.stderr.write("could not generate attestation\n")
        sys.exit(1)

    token = client.get_token()
    b64_token = base64.b64encode(token.encode("utf-8")).decode("utf-8")
    print(b64_token)


if __name__ == "__main__":
    main()
