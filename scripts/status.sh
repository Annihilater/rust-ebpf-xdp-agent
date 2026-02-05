#!/bin/bash
# 检查 XDP DDoS Agent 状态

IFACE="${1:-eth0}"

echo "📊 XDP DDoS Agent 状态"
echo "========================"

# 检查进程
echo ""
echo "进程状态:"
if pgrep -f "rust-xdp-ddos-agent" > /dev/null; then
    echo "  ✅ Agent 正在运行"
    ps aux | grep "[r]ust-xdp-ddos-agent" | awk '{print "     PID: "$2", CPU: "$3"%, MEM: "$4"%"}'
else
    echo "  ❌ Agent 未运行"
fi

# 检查 XDP 程序
echo ""
echo "XDP 程序状态 ($IFACE):"
if ip link show "$IFACE" 2>/dev/null | grep -q "xdp"; then
    echo "  ✅ XDP 程序已附加"
    ip link show "$IFACE" | grep -E "xdp|prog_id"
else
    echo "  ❌ 没有 XDP 程序附加到 $IFACE"
fi

# 检查 BPF maps
echo ""
echo "BPF Maps:"
if command -v bpftool &>/dev/null; then
    sudo bpftool map list 2>/dev/null | grep -E "GLOBAL_COUNTER|percpu_array" || echo "  没有找到相关 maps"
else
    echo "  ⚠️  bpftool 未安装，无法显示 maps"
fi
