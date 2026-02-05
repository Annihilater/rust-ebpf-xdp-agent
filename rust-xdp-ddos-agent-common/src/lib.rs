#![no_std]

/// 全局计数器结构，用于 eBPF 和用户态共享
#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct Counter {
    /// UDP 包计数
    pub udp_packets: u64,
    /// 被丢弃的包计数
    pub dropped: u64,
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for Counter {}
