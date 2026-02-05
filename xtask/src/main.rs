//! xtask - 构建任务管理器
//!
//! 提供便捷的构建命令：
//! - `cargo xtask build-ebpf` - 编译 eBPF 程序
//! - `cargo xtask build` - 编译 eBPF 和用户态程序
//! - `cargo xtask run` - 编译并运行 (需要 root 权限)

use std::process::Command;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about = "XDP DDoS Agent 构建工具")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 编译 eBPF 程序
    BuildEbpf {
        /// 使用 release 模式编译
        #[arg(long)]
        release: bool,
    },
    /// 编译所有程序 (eBPF + 用户态)
    Build {
        /// 使用 release 模式编译
        #[arg(long)]
        release: bool,
    },
    /// 编译并运行 (需要 sudo)
    Run {
        /// 使用 release 模式编译
        #[arg(long)]
        release: bool,
        /// 网络接口名称
        #[arg(short, long, default_value = "eth0")]
        iface: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::BuildEbpf { release } => build_ebpf(release),
        Commands::Build { release } => {
            build_ebpf(release)?;
            build_userspace(release)
        }
        Commands::Run { release, iface } => {
            build_ebpf(release)?;
            build_userspace(release)?;
            run_agent(release, &iface)
        }
    }
}

fn build_ebpf(release: bool) -> Result<()> {
    println!("🔨 编译 eBPF 程序...");

    let mut cmd = Command::new("cargo");
    cmd.current_dir("rust-xdp-ddos-agent-ebpf")
        .env("CARGO_CFG_BPF_TARGET_ARCH", std::env::consts::ARCH)
        .args([
            "+nightly",
            "build",
            "--target=bpfel-unknown-none",
            "-Z",
            "build-std=core",
        ]);

    if release {
        cmd.arg("--release");
    }

    let status = cmd.status().context("无法执行 cargo build")?;
    if !status.success() {
        bail!("eBPF 编译失败");
    }

    println!("✅ eBPF 程序编译完成");
    Ok(())
}

fn build_userspace(release: bool) -> Result<()> {
    println!("🔨 编译用户态程序...");

    let mut cmd = Command::new("cargo");
    cmd.args(["build", "-p", "rust-xdp-ddos-agent"]);

    if release {
        cmd.arg("--release");
    }

    let status = cmd.status().context("无法执行 cargo build")?;
    if !status.success() {
        bail!("用户态程序编译失败");
    }

    println!("✅ 用户态程序编译完成");
    Ok(())
}

fn run_agent(release: bool, iface: &str) -> Result<()> {
    println!("🚀 启动 Agent...");

    let binary = if release {
        "target/release/rust-xdp-ddos-agent"
    } else {
        "target/debug/rust-xdp-ddos-agent"
    };

    let status = Command::new("sudo")
        .env("RUST_LOG", "info")
        .args([binary, "--iface", iface])
        .status()
        .context("无法启动 Agent")?;

    if !status.success() {
        bail!("Agent 运行失败");
    }

    Ok(())
}
