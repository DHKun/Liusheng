//! 非 Linux 平台的硬件音量占位实现。

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeState {
    pub percent: u8,
    pub muted: bool,
    pub can_mute: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeChange {
    Percent(u8),
    Muted(bool),
}

pub struct HardwareVolume;

impl HardwareVolume {
    pub fn open(_device: &str, _element_name: &str) -> Result<Self> {
        Err(Error::Other("硬件音量控制仅支持 Linux ALSA".into()))
    }

    pub fn state(&self) -> Result<VolumeState> {
        Err(Error::Other("硬件音量控制仅支持 Linux ALSA".into()))
    }

    pub fn apply(&self, _change: VolumeChange) -> Result<VolumeState> {
        Err(Error::Other("硬件音量控制仅支持 Linux ALSA".into()))
    }
}
