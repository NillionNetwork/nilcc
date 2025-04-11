#!/usr/bin/env bash
# This script uploads the artifacts to S3 bucket

SCRIPT_PATH=$(dirname $(realpath $0))

aws s3 cp --recursive "$SCRIPT_PATH/dist" "s3://nilcc/$(date +%d-%m-%Y)/"