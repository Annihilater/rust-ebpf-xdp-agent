# Rust XDP DDoS 防护 Agent

基于 **Rust + eBPF + XDP** 实现的轻量级 DDoS 防护网络监控 Agent。

## ⚠️ 系统要求

**本项目只能在 Linux 上运行！** eBPF 和 XDP 是 Linux 内核专有技术。

### Linux 环境要求

- Linux 内核 ≥ 5.15（建议 6.6 / 6.12 LTS）
- clang-18+ 和 llvm
- libbpf-dev
- Rust nightly (1.80+)

### 安装依赖

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y clang llvm libbpf-dev linux-headers-$(uname -r)

# 安装 Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install nightly
rustup component add rust-src --toolchain nightly

# 安装 bpf-linker
cargo install bpf-linker
```

## 快速开始

### 1. 编译 eBPF 程序

```bash
cargo xtask build-ebpf
```

### 2. 编译用户态程序

```bash
cargo build --release
```

### 3. 运行 Agent (需要 root 权限)

```bash
sudo RUST_LOG=info ./target/release/rust-xdp-ddos-agent --iface eth0
```

或使用 xtask:

```bash
cargo xtask run --release --iface eth0
```

## 命令行参数

```
Usage: rust-xdp-ddos-agent [OPTIONS]

Options:
  -i, --iface <IFACE>                    网络接口名称 [default: eth0]
  -a, --alert-threshold <ALERT>          UDP flood 告警阈值 (pps) [default: 3000]
  -s, --stats-interval <INTERVAL>        统计输出间隔 (秒) [default: 1]
  -h, --help                             打印帮助信息
  -V, --version                          打印版本信息
```

## 项目结构

```
├── Cargo.toml                          # 工作区配置
├── .cargo/config.toml                  # Cargo 别名配置
├── rust-xdp-ddos-agent/                # 用户态 Agent
│   ├── Cargo.toml
│   └── src/main.rs
├── rust-xdp-ddos-agent-ebpf/           # eBPF 内核程序
│   ├── Cargo.toml
│   └── src/main.rs
├── rust-xdp-ddos-agent-common/         # 共享类型定义
│   ├── Cargo.toml
│   └── src/lib.rs
└── xtask/                              # 构建任务
    ├── Cargo.toml
    └── src/main.rs
```

## 工作原理

1. **XDP 程序** 运行在网卡驱动层，数据包刚从 DMA 进来就被处理
2. **解析以太网/IP/UDP 头部**，统计 UDP 包数量
3. **超过阈值时丢弃包** (XDP_DROP)，正常包放行 (XDP_PASS)
4. **用户态 Agent** 每秒读取统计数据并输出告警

## 进阶方向

- [ ] per-IP 速率限制 (HashMap + 令牌桶)
- [ ] RingBuf 遥测采样
- [ ] 动态白/黑名单
- [ ] XDP_REDIRECT 清洗
- [ ] Prometheus exporter

## macOS 开发

虽然无法在 macOS 上运行，但可以：

1. **使用 Linux 虚拟机**（OrbStack、UTM、Parallels）
2. **使用云服务器**（AWS、阿里云等）
3. **在 macOS 上编写代码**，然后在 Linux 上编译运行

```bash
# 在 Linux VM 中
git clone <repo>
cd rust-ebpf-xdp-agent
cargo xtask build --release
sudo RUST_LOG=info ./target/release/rust-xdp-ddos-agent --iface eth0
```

## License

MIT
