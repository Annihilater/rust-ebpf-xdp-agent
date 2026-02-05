#!/bin/bash
# 重启 XDP DDoS Agent

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IFACE="${1:-eth0}"

echo "🔄 重启 XDP DDoS Agent..."

"$SCRIPT_DIR/stop.sh" "$IFACE"
sleep 1
"$SCRIPT_DIR/start.sh" "$IFACE"
