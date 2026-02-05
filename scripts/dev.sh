#!/bin/bash
# 开发模式：编译并运行 (debug 版本)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
IFACE="${1:-eth0}"

cd "$PROJECT_DIR"

echo "🔨 编译 eBPF 程序 (debug)..."
cargo xtask build-ebpf

echo "🔨 编译用户态程序 (debug)..."
cargo build -p rust-xdp-ddos-agent

echo "🚀 启动 Agent (debug 模式)..."
sudo RUST_LOG=debug ./target/debug/rust-xdp-ddos-agent --iface "$IFACE"
