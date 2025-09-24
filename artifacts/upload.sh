#!/usr/bin/env bash
# This script uploads the artifacts to S3 bucket

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>"
  exit 1
fi

SCRIPT_PATH=$(dirname $(realpath $0))
TARGET_URL="s3://nilcc/${1}/"

echo "Uploading to ${TARGET_URL}"
aws s3 cp --recursive "$SCRIPT_PATH/dist" "${TARGET_URL}"
