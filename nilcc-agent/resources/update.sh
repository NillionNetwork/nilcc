#!/bin/bash

set -euo pipefail

if [ $# -ne 3 ]; then
  echo "Usage: $0 <downloaded-agent-path> <system-agent-path> <config-file>"
  exit 1
fi

downloaded_agent_path=$1
system_agent_path=$2
config_file=$3

"$downloaded_agent_path" validate-config "$config_file"

mv "$downloaded_agent_path" "$system_agent_path"
systemctl restart nilcc-agent
