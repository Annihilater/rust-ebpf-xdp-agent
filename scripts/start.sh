#!/bin/bash
# 启动 XDP DDoS Agent
# 需要 root 权限

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/rust-xdp-ddos-agent"
IFACE="${1:-eth0}"

# 检查是否为 root
if [ "$EUID" -ne 0 ]; then
    echo "❌ 需要 root 权限运行 XDP 程序"
    echo "   请使用: sudo $0 $IFACE"
    exit 1
fi

# 检查二进制文件是否存在
if [ ! -f "$BINARY" ]; then
    echo "❌ 未找到可执行文件: $BINARY"
    echo "   请先编译: cargo xtask build --release"
    exit 1
fi

# 检查网络接口是否存在
if ! ip link show "$IFACE" &>/dev/null; then
    echo "❌ 网络接口不存在: $IFACE"
    echo "   可用接口:"
    ip link show | grep -E "^[0-9]+:" | awk -F': ' '{print "   - "$2}'
    exit 1
fi

echo "🚀 启动 XDP DDoS Agent..."
echo "   接口: $IFACE"
echo "   按 Ctrl+C 停止"

export RUST_LOG=info
exec "$BINARY" --iface "$IFACE"
