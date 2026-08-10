//! ALSA 硬件播放音量控制。

use alsa::Mixer;
use alsa::mixer::{Selem, SelemChannelId, SelemId};

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

/// 将 ALSA simple mixer 的范围、声道和静音开关收进三个入口。
pub struct HardwareVolume {
    mixer: Mixer,
    element_id: SelemId,
    device: String,
    element_name: String,
}

impl HardwareVolume {
    pub fn open(device: &str, element_name: &str) -> Result<Self> {
        let mixer = Mixer::new(device, false)
            .map_err(|error| Error::Other(format!("打开 ALSA mixer {device} 失败：{error}")))?;
        let volume = Self {
            mixer,
            element_id: SelemId::new(element_name, 0),
            device: device.to_owned(),
            element_name: element_name.to_owned(),
        };
        let element = volume.element()?;
        if !element.has_playback_volume() {
            return Err(volume.error("没有播放音量控件"));
        }
        if playback_channels(&element).next().is_none() {
            return Err(volume.error("没有可用播放声道"));
        }
        Ok(volume)
    }

    pub fn state(&self) -> Result<VolumeState> {
        self.mixer
            .handle_events()
            .map_err(|error| self.alsa_error("同步硬件音量事件失败", error))?;
        let element = self.element()?;
        let (minimum, maximum) = element.get_playback_volume_range();
        let mut total = 0i128;
        let mut count = 0i128;
        for channel in playback_channels(&element) {
            let raw = element
                .get_playback_volume(channel)
                .map_err(|error| self.alsa_error("读取播放音量失败", error))?;
            total += i128::from(raw);
            count += 1;
        }
        if count == 0 {
            return Err(self.error("没有可用播放声道"));
        }
        let average = (total / count) as i64;
        let can_mute = element.has_playback_switch();
        let muted = if can_mute {
            playback_channels(&element).try_fold(false, |muted, channel| {
                element
                    .get_playback_switch(channel)
                    .map(|enabled| muted || enabled == 0)
                    .map_err(|error| self.alsa_error("读取硬件静音状态失败", error))
            })?
        } else {
            false
        };
        Ok(VolumeState {
            percent: raw_to_percent(average, minimum, maximum),
            muted,
            can_mute,
        })
    }

    pub fn apply(&self, change: VolumeChange) -> Result<VolumeState> {
        {
            let element = self.element()?;
            match change {
                VolumeChange::Percent(percent) => {
                    let (minimum, maximum) = element.get_playback_volume_range();
                    let raw = percent_to_raw(percent, minimum, maximum);
                    element
                        .set_playback_volume_all(raw)
                        .map_err(|error| self.alsa_error("设置硬件音量失败", error))?;
                }
                VolumeChange::Muted(muted) => {
                    if !element.has_playback_switch() {
                        return Err(self.error("没有硬件静音开关"));
                    }
                    element
                        .set_playback_switch_all(i32::from(!muted))
                        .map_err(|error| self.alsa_error("设置硬件静音失败", error))?;
                }
            }
        }
        self.state()
    }

    fn element(&self) -> Result<Selem<'_>> {
        self.mixer
            .find_selem(&self.element_id)
            .ok_or_else(|| self.error(&format!("找不到 {} 播放控件", self.element_name)))
    }

    fn error(&self, message: &str) -> Error {
        Error::Other(format!("ALSA mixer {}：{message}", self.device))
    }

    fn alsa_error(&self, action: &str, error: alsa::Error) -> Error {
        self.error(&format!("{action}：{error}"))
    }
}

fn playback_channels<'element, 'mixer>(
    element: &'element Selem<'mixer>,
) -> impl Iterator<Item = SelemChannelId> + 'element {
    SelemChannelId::all()
        .iter()
        .copied()
        .filter(|channel| element.has_playback_channel(*channel))
}

fn raw_to_percent(raw: i64, minimum: i64, maximum: i64) -> u8 {
    if maximum <= minimum {
        return 100;
    }
    let raw = raw.clamp(minimum, maximum);
    let numerator = i128::from(raw - minimum) * 100;
    let denominator = i128::from(maximum - minimum);
    ((numerator + denominator / 2) / denominator) as u8
}

fn percent_to_raw(percent: u8, minimum: i64, maximum: i64) -> i64 {
    if maximum <= minimum {
        return maximum;
    }
    let percent = i128::from(percent.min(100));
    let span = i128::from(maximum - minimum);
    let offset = (span * percent + 50) / 100;
    minimum + offset as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_mixer_values_round_to_stable_percentages() {
        assert_eq!(raw_to_percent(0, 0, 74), 0);
        assert_eq!(raw_to_percent(37, 0, 74), 50);
        assert_eq!(raw_to_percent(51, 0, 74), 69);
        assert_eq!(raw_to_percent(74, 0, 74), 100);
        assert_eq!(raw_to_percent(90, 0, 74), 100);
        assert_eq!(raw_to_percent(7, 7, 7), 100);
    }

    #[test]
    fn percentages_cover_the_entire_raw_mixer_range() {
        assert_eq!(percent_to_raw(0, 0, 74), 0);
        assert_eq!(percent_to_raw(50, 0, 74), 37);
        assert_eq!(percent_to_raw(69, 0, 74), 51);
        assert_eq!(percent_to_raw(100, 0, 74), 74);
        assert_eq!(percent_to_raw(120, 0, 74), 74);
        assert_eq!(percent_to_raw(50, -50, 50), 0);
    }
}
