#!/usr/bin/env bash
# This script uploads the artifacts to S3 bucket

set -euo pipefail

SCRIPT_PATH=$(dirname $(realpath $0))

COMMIT=$(git rev-parse --short HEAD)
aws s3 cp --recursive "$SCRIPT_PATH/dist" "s3://nilcc/$(COMMIT)/"
