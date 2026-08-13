#[cfg(target_os = "linux")]
pub mod alsa_sink;
#[cfg(target_os = "macos")]
pub mod coreaudio_sink;
pub mod decode;
#[cfg(target_os = "linux")]
pub mod hardware_volume;
#[cfg(not(target_os = "linux"))]
#[path = "hardware_volume_stub.rs"]
pub mod hardware_volume;
#[cfg(target_os = "linux")]
pub mod pipewire_sink;
pub mod resampling_sink;
pub mod sink;

/// PCM 流格式。样本在内存中统一为交错 i32 满量程；`bits` 记录来源有效位深，
/// 输出端据此还原原始比特（16 位右移 16、24 位右移 8），保证整数路径无损。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmSpec {
    pub rate: u32,
    pub channels: u16,
    pub bits: u16,
}

impl PcmSpec {
    pub fn frames(&self, samples: usize) -> u64 {
        (samples / self.channels.max(1) as usize) as u64
    }
}
