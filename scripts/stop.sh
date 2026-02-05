#!/bin/bash
# 停止 XDP DDoS Agent

set -e

echo "🛑 停止 XDP DDoS Agent..."

# 查找并终止进程
if pgrep -f "rust-xdp-ddos-agent" > /dev/null; then
    sudo pkill -f "rust-xdp-ddos-agent" || true
    echo "✅ Agent 已停止"
else
    echo "ℹ️  Agent 未在运行"
fi

# 可选：手动卸载 XDP 程序
IFACE="${1:-eth0}"
if ip link show "$IFACE" 2>/dev/null | grep -q "xdp"; then
    echo "🔧 从 $IFACE 卸载 XDP 程序..."
    sudo ip link set dev "$IFACE" xdp off
    echo "✅ XDP 程序已卸载"
fi
