//! Rust XDP DDoS 防护 Agent
//!
//! 用户态程序，负责：
//! - 加载和附加 eBPF 程序到网络接口
//! - 读取 eBPF Map 中的统计数据
//! - 实时监控和告警
//! - 每秒重置计数器

use anyhow::{ Context, Result };
use aya::{ maps::{PerCpuArray, PerCpuValues}, programs::{ Xdp, XdpFlags }, Ebpf };
use aya_log::EbpfLogger;
use clap::Parser;
use log::{ info, warn };
use rust_xdp_ddos_agent_common::Counter;
use std::time::Duration;
use tokio::signal;
use tokio::time::interval;

/// XDP DDoS 防护 Agent 命令行参数
#[derive(Parser, Debug)]
#[command(author, version, about = "Rust XDP DDoS 防护 Agent", long_about = None)]
struct Args {
    /// 要附加 XDP 程序的网络接口名称
    #[arg(short, long, default_value = "eth0")]
    iface: String,

    /// UDP flood 告警阈值 (每秒包数)
    #[arg(short, long, default_value = "3000")]
    alert_threshold: u64,

    /// 统计输出间隔 (秒)
    #[arg(short, long, default_value = "1")]
    stats_interval: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    info!("🚀 正在启动 XDP DDoS Agent...");

    // 加载 eBPF 程序
    // 注意：需要用 cargo xtask build-ebpf 先编译 eBPF 程序
    #[cfg(debug_assertions)]
    let mut bpf = Ebpf::load(
        include_bytes_aligned!("../../rust-xdp-ddos-agent-ebpf/target/bpfel-unknown-none/debug/rust-xdp-ddos-agent")
    )?;

    #[cfg(not(debug_assertions))]
    let mut bpf = Ebpf::load(
        include_bytes_aligned!("../../rust-xdp-ddos-agent-ebpf/target/bpfel-unknown-none/release/rust-xdp-ddos-agent")
    )?;

    // 初始化 eBPF 日志
    if let Err(e) = EbpfLogger::init(&mut bpf) {
        warn!("无法初始化 eBPF 日志: {}", e);
    }

    // 获取并加载 XDP 程序
    let program: &mut Xdp = bpf
        .program_mut("xdp_ddos_guard")
        .context("找不到 xdp_ddos_guard 程序")?
        .try_into()?;

    program.load()?;

    // 附加到网络接口
    program
        .attach(&args.iface, XdpFlags::default())
        .context(format!("无法附加到接口 {}", args.iface))?;

    info!(
        "✅ XDP DDoS Agent 已启动 | 接口: {} | 告警阈值: >{} UDP pps",
        args.iface,
        args.alert_threshold
    );

    // 获取计数器 Map
    let mut counter: PerCpuArray<_, Counter> = bpf
        .take_map("GLOBAL_COUNTER")
        .context("找不到 GLOBAL_COUNTER map")?
        .try_into()?;

    // 创建定时器
    let mut stats_timer = interval(Duration::from_secs(args.stats_interval));

    // 主循环
    loop {
        tokio::select! {
            _ = stats_timer.tick() => {
                // 读取所有 CPU 核心的计数
                match counter.get(&0, 0) {
                    Ok(values) => {
                        let udp_total: u64 = values.iter().map(|c| c.udp_packets).sum();
                        let dropped_total: u64 = values.iter().map(|c| c.dropped).sum();

                        if udp_total > args.alert_threshold {
                            warn!(
                                "⚠️  [ALERT] 疑似 UDP flood！当前 ≈ {} pkt/s，已丢包 {} 个",
                                udp_total, dropped_total
                            );
                        } else {
                            info!("📊 正常 → UDP: {} pkt/s, 丢弃: {}", udp_total, dropped_total);
                        }

                        // 重置计数器 (每秒统计)
                        // 获取CPU核心数并创建对应的PerCpuValues
                        let num_cpus = aya::util::nr_cpus().unwrap_or(1);
                        let reset_values: Vec<Counter> = (0..num_cpus).map(|_| Counter {
                            udp_packets: 0,
                            dropped: 0,
                        }).collect();
                        if let Ok(per_cpu_values) = PerCpuValues::try_from(reset_values) {
                            if let Err(e) = counter.set(0, per_cpu_values, 0) {
                                warn!("重置计数器失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("读取计数器失败: {}", e);
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("🛑 收到退出信号，正在停止 Agent...");
                break;
            }
        }
    }

    info!("👋 XDP DDoS Agent 已停止");
    Ok(())
}

/// 用于对齐 eBPF 字节码的宏
#[macro_export]
macro_rules! include_bytes_aligned {
    ($path:expr) => {
        {
        #[repr(C, align(8))]
        struct Aligned<T: ?Sized>(T);
        static ALIGNED: &Aligned<[u8]> = &Aligned(*include_bytes!($path));
        &ALIGNED.0
        }
    };
}
