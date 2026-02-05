# 用 Rust + eBPF + XDP 实现一个简易但实用的 DDoS 防护网络监控 Agent

## 目录

- [一、核心原理：为什么 XDP 能打赢 DDoS？](#一核心原理为什么-XDP-能打赢-DDoS)
- [二、整体架构一图秒懂](#二整体架构一图秒懂)
- [三、快速上手（10–15 分钟跑通）](#三快速上手1015-分钟跑通)
  - [环境要求（2026 年推荐配置）](#环境要求2026-年推荐配置)
  - [生成项目](#生成项目)
- [四、eBPF 核心代码（ebpf/src/main.rs）](#四eBPF-核心代码ebpfsrcmainrs)
- [五、用户态监控 Agent（src/main.rs 精简版）](#五用户态监控-Agentsrcmainrs-精简版)
- [六、进阶方向（生产可用建议）](#六进阶方向生产可用建议)
- [七、最后](#七最后)

[https://mp.weixin.qq.com/s/a7jva4YyoVQmXzP3Nj1tPw](https://mp.weixin.qq.com/s/a7jva4YyoVQmXzP3Nj1tPw "https://mp.weixin.qq.com/s/a7jva4YyoVQmXzP3Nj1tPw")

2026 年了，DDoS 攻击依然是互联网最常见的“核弹级”威胁。单次攻击峰值轻松破 Tbps，PPS（每秒包数）动辄几千万甚至上亿。传统用户态防火墙（iptables、nftables、甚至一些商业 WAF/ADS）在这种体量下基本无解——内核协议栈本身就会被打崩。

而 **eBPF + XDP** 已经成为 Cloudflare、阿里云、腾讯云、Fastly、Gcore 等一线厂商对抗海量 DDoS 的标配技术。它能在数据包刚从网卡 DMA 进来、**还没分配 sk_buff、还没进入协议栈**的时候就完成丢弃或重定向，延迟低至纳秒级，单核处理能力轻松达到 10Mpps+。

今天我们用 **Rust + aya** 框架，手写一个**轻量、可运行、易扩展的 UDP flood 防护 + 实时监控 Agent**，全套代码控制在 250 行左右，适合：

- • 学习 eBPF/XDP 原理
- • 快速做 POC
- • 在边缘节点、中小游戏服、VPN 节点、直播推流节点等场景落地初级防护

### 一、核心原理：为什么 XDP 能打赢 DDoS？

传统网络包处理路径（简化）：

```bash
网卡硬件 → DMA → 驱动 ring buffer → 分配 sk_buff → 内核 L2/L3/L4 处理 → netfilter → socket → 用户态

```

DDoS 场景下，每一步都可能成为瓶颈：内存分配、锁竞争、上下文切换、中断风暴……

**XDP（eXpress Data Path）** 把 eBPF 程序挂载到了**最早的阶段**——驱动的 RX 处理函数中，包刚进内核，还没分配 sk_buff。

XDP 支持的几种关键返回动作：

- • **XDP_PASS** → 正常走内核协议栈
- • **XDP_DROP** → 直接在驱动层丢弃（开销极低）
- • **XDP_TX** → 从当前网卡发回（可用于响应式清洗）
- • **XDP_REDIRECT** → 重定向到另一张网卡 / AF_XDP socket（常用于送往用户态蜜罐或硬件清洗设备）

**为什么这么高效？**

- • 零拷贝：直接操作网卡 DMA buffer 中的原始字节
- • 零上下文切换：在 NAPI 软中断或硬中断上下文中运行
- • 多核天然并行：每个 CPU 核心独立处理自己的 RX queue
- • verifier 静态检查：保证程序安全、无死循环、无非法内存访问
- • CO-RE（Compile Once - Run Everywhere）：通过 BTF，一套代码适配不同内核版本

典型 DDoS 防护逻辑演进路径：

1. 1\. 全局 UDP 包速率统计（本文采用，最简单）
2. 2\. per-IP 速率限制（HashMap + bpf_ktime_get_ns() 滑动窗口 / 令牌桶）
3. 3\. 多维度指纹匹配（源 IP + 目的端口 + 包长 + TTL + IP ID …）
4. 4\. 动态白/黑名单 + RingBuf 遥测采样
5. 5\. 与硬件卸载（SmartNIC 如 BlueField-3）结合

### 二、整体架构一图秒懂

```markdown
物理网卡 → [XDP eBPF 程序]
↓
解析 Eth → IP → UDP → 计数/判断
↓
BPF Map（PerCpuArray / HashMap / RingBuf）
↓
Rust 用户态 Agent（aya）
→ 实时统计 / 告警 / 热更新阈值 / 下发规则
```

### 三、快速上手（10–15 分钟跑通）

#### 环境要求（2026 年推荐配置）

- • Linux 内核 ≥ 5.15（建议 6.6 / 6.12 LTS）
- • clang-18+、llvm、libbpf-dev、rustc 1.80+ 或 nightly
- • 已安装：cargo-generate、cargo-xtask、bpf-linker

```bash
cargo install cargo-generate cargo-xtask bpf-linker

```

#### 生成项目

```bash
cargo generate --name rust-xdp-ddos-agent \
  https://github.com/aya-rs/aya-template

cd rust-xdp-ddos-agent
# 选择 program_type = xdp
```

### 四、eBPF 核心代码（ebpf/src/main.rs）

```rust
#![no_std]
#![no_main]

use aya_ebpf::{bindings::xdp_action, macros::xdp, programs::XdpContext};
use aya_ebpf::maps::PerCpuArray;
use aya_log_ebpf::{info};
use network_types::{
    eth::{EtherType, EthHdr},
    ip::{IpProto, Ipv4Hdr},
    udp::UdpHdr,
};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Counter {
    pub udp_packets: u64,
    pub dropped: u64,
}

#[map]
static mut GLOBAL_COUNTER: PerCpuArray<Counter> = PerCpuArray::with_max_entries(1, 0);

const UDP_FLOOD_THRESHOLD: u64 = 5000;  // 生产建议 10k–100k+ 根据网卡能力调整

#[inline(always)]
unsafe fn ptr_at<T>(ctx: &XdpContext, offset: usize) -> Result<*const T, ()> {
    let ptr = ctx.data() + offset;
    if ptr + core::mem::size_of::<T>() > ctx.data_end() {
        return Err(());
    }
    Ok(ptr as *const T)
}

#[xdp]
pub fn xdp_ddos_guard(ctx: XdpContext) -> u32 {
    if let Ok(action) = try_xdp_ddos_guard(&ctx) {
        action
    } else {
        xdp_action::XDP_ABORTED
    }
}

fn try_xdp_ddos_guard(ctx: &XdpContext) -> Result<u32, ()> {
    let eth = unsafe { *ptr_at::<EthHdr>(ctx, 0)? };
    if eth.ether_type != EtherType::Ipv4.to_be() {
        return Ok(xdp_action::XDP_PASS);
    }

    let ip = unsafe { *ptr_at::<Ipv4Hdr>(ctx, EthHdr::LEN)? };
    if ip.proto != IpProto::Udp.to_be() {
        return Ok(xdp_action::XDP_PASS);
    }

    // 示例：放行 DNS 53 端口（可扩展白名单）
    let udp_offset = EthHdr::LEN + (ip.ihl() as usize * 4);
    let udp = unsafe { *ptr_at::<UdpHdr>(ctx, udp_offset)? };
    if u16::from_be(udp.dest) == 53 {
        return Ok(xdp_action::XDP_PASS);
    }

    let counter = unsafe { GLOBAL_COUNTER.get_ptr_mut(0).ok_or(())? };
    unsafe {
        (*counter).udp_packets = (*counter).udp_packets.wrapping_add(1);

        if (*counter).udp_packets > UDP_FLOOD_THRESHOLD {
            (*counter).dropped = (*counter).dropped.wrapping_add(1);
            info!(ctx, "UDP flood! > {} pps, dropping", UDP_FLOOD_THRESHOLD);
            return Ok(xdp_action::XDP_DROP);
        }
    }

    Ok(xdp_action::XDP_PASS)
}
```

### 五、用户态监控 Agent（src/main.rs 精简版）

```rust
use anyhow::Context;
use aya::{Ebpf, maps::PerCpuArray, programs::{Xdp, XdpFlags}};
use aya_log::EbpfLogger;
use clap::Parser;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug)]
struct Opt {
    #[clap(short, long, default_value = "eth0")]
    iface: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    // 加载并附加程序
    let mut bpf = Ebpf::load_file("target/bpfel-unknown-none/debug/rust-xdp-ddos-agent")?;
    EbpfLogger::init(&mut bpf)?;

    let prog: &mut Xdp = bpf.program_mut("xdp_ddos_guard")?.try_into()?;
    prog.load()?;
    prog.attach(&opt.iface, XdpFlags::default())?;

    let mut counter: PerCpuArray<_, Counter> = bpf.take_map("GLOBAL_COUNTER")?.try_into()?;

    println!("🚀 XDP DDoS Agent 已启动 | 接口: {} | 阈值: >{} UDP pps → DROP", opt.iface, 5000);

    loop {
        let stats: Vec<Counter> = counter.iter().collect();
        let udp_total: u64 = stats.iter().map(|c| c.udp_packets).sum();
        let dropped: u64 = stats.iter().map(|c| c.dropped).sum();

        if udp_total > 3000 {
            println!("[ALERT] 疑似 UDP flood！当前 ≈ {} pkt/s，已丢包 {} 个", udp_total, dropped);
        } else {
            println!("正常 → UDP: {} pkt/s", udp_total);
        }

        // 每秒重置计数（生产建议用 ringbuf + 用户态滑动窗口）
        counter.set(0, Counter { udp_packets: 0, dropped }, 0)?;

        sleep(Duration::from_secs(1)).await;
    }
}
```

运行：

```bash
cargo xtask build-ebpf
cargo build --release
sudo RUST_LOG=info target/release/rust-xdp-ddos-agent --iface eth0
```

### 六、进阶方向（生产可用建议）

1. 1\. per-IP 限速：HashMap<源IP, {count:u64, last_ts:u64}> + 令牌桶/滑动窗口
2. 2\. RingBuf 遥测：把被 DROP 的 5 元组采样推到用户态
3. 3\. 动态规则：增加白/黑名单 HashMap，支持用户态热更新
4. 4\. 重定向清洗：XDP_REDIRECT → AF_XDP 或另一张网卡
5. 5\. 指标暴露：集成 Prometheus exporter

### 七、最后

不到 250 行 Rust 代码，你就拥有了一个能在 10 Gbps+ 环境下稳定丢弃 UDP 洪水的初级防护 Agent。

eBPF + XDP 的真正魅力在于：**在网络最早期、以极低开销、可编程、可热更新地丢弃恶意流量**，让宝贵的 CPU、内存留给真正合法的用户。

2026 年，这仍然是边缘计算、游戏云、CDN、5G MEC 等场景最值得掌握的技术之一。
