#![no_std]
#![no_main]

use aya_ebpf::{
    bindings::xdp_action,
    macros::{map, xdp},
    maps::PerCpuArray,
    programs::XdpContext,
};
use aya_log_ebpf::info;
use network_types::{
    eth::{EthHdr, EtherType},
    ip::{IpProto, Ipv4Hdr},
    udp::UdpHdr,
};
use rust_xdp_ddos_agent_common::Counter;

/// 全局计数器 Map，每个 CPU 核心独立计数
#[map]
static GLOBAL_COUNTER: PerCpuArray<Counter> = PerCpuArray::with_max_entries(1, 0);

/// UDP flood 阈值 (每秒包数)
/// 生产环境建议根据网卡能力调整到 10k-100k+
const UDP_FLOOD_THRESHOLD: u64 = 5000;

/// 安全地从 XDP 上下文中获取指定偏移量的数据指针
#[inline(always)]
unsafe fn ptr_at<T>(ctx: &XdpContext, offset: usize) -> Result<*const T, ()> {
    let start = ctx.data();
    let end = ctx.data_end();
    let len = core::mem::size_of::<T>();

    if start + offset + len > end {
        return Err(());
    }
    Ok((start + offset) as *const T)
}

/// XDP 程序入口点
#[xdp]
pub fn xdp_ddos_guard(ctx: XdpContext) -> u32 {
    match try_xdp_ddos_guard(&ctx) {
        Ok(action) => action,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

/// XDP 程序主逻辑
fn try_xdp_ddos_guard(ctx: &XdpContext) -> Result<u32, ()> {
    // 解析以太网头部 (使用 read_unaligned 避免对齐问题)
    let eth_ptr = unsafe { ptr_at::<EthHdr>(ctx, 0)? };
    let ether_type = unsafe { core::ptr::addr_of!((*eth_ptr).ether_type).read_unaligned() };
    
    // 只处理 IPv4 包
    if ether_type != EtherType::Ipv4 {
        return Ok(xdp_action::XDP_PASS);
    }

    // 解析 IPv4 头部
    let ip_ptr = unsafe { ptr_at::<Ipv4Hdr>(ctx, EthHdr::LEN)? };
    let proto = unsafe { core::ptr::addr_of!((*ip_ptr).proto).read_unaligned() };
    
    // 只处理 UDP 包
    if proto != IpProto::Udp {
        return Ok(xdp_action::XDP_PASS);
    }

    // 计算 UDP 头部偏移量 (IP 头部长度可变)
    let ihl = unsafe { (*ip_ptr).ihl() };
    let ip_hdr_len = (ihl as usize) * 4;
    let udp_offset = EthHdr::LEN + ip_hdr_len;
    
    // 解析 UDP 头部
    let udp_ptr = unsafe { ptr_at::<UdpHdr>(ctx, udp_offset)? };
    
    // 白名单：放行 DNS 查询 (端口 53)
    let dest_port = unsafe { core::ptr::addr_of!((*udp_ptr).dest).read_unaligned() };
    if u16::from_be(dest_port) == 53 {
        return Ok(xdp_action::XDP_PASS);
    }

    // 更新计数器并检查阈值
    let counter = GLOBAL_COUNTER.get_ptr_mut(0).ok_or(())?;

    unsafe {
        (*counter).udp_packets = (*counter).udp_packets.wrapping_add(1);

        // 如果超过阈值，丢弃包
        if (*counter).udp_packets > UDP_FLOOD_THRESHOLD {
            (*counter).dropped = (*counter).dropped.wrapping_add(1);
            info!(ctx, "UDP flood detected! > {} pps, dropping packet", UDP_FLOOD_THRESHOLD);
            return Ok(xdp_action::XDP_DROP);
        }
    }

    Ok(xdp_action::XDP_PASS)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
