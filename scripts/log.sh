#!/bin/bash
# 查看 XDP DDoS Agent 日志 (通过 journalctl 或 dmesg)

echo "📜 XDP DDoS Agent 日志"
echo "======================"

# 如果使用 systemd 服务
if systemctl is-active --quiet rust-xdp-ddos-agent 2>/dev/null; then
    echo "从 journalctl 获取日志:"
    journalctl -u rust-xdp-ddos-agent -f
else
    echo "Agent 未作为 systemd 服务运行"
    echo ""
    echo "查看 eBPF 内核日志 (需要 root):"
    sudo cat /sys/kernel/debug/tracing/trace_pipe 2>/dev/null || \
        echo "无法访问 trace_pipe，可能需要启用 BPF 追踪"
fi
