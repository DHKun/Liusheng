use alsa::pcm::{Access, Format, HwParams, PCM, State};
use alsa::{Direction, ValueOr};

use crate::audio::PcmSpec;
use crate::audio::sink::AudioSink;
use crate::error::{Error, Result};

const BUFFER_TIME_US: u32 = 200_000;
const PERIOD_TIME_US: u32 = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WireFormat {
    S16Le,
    S24_3Le,
}

impl WireFormat {
    fn alsa(self) -> Format {
        match self {
            Self::S16Le => Format::S16LE,
            Self::S24_3Le => Format::S243LE,
        }
    }

    fn bytes_per_sample(self) -> usize {
        match self {
            Self::S16Le => 2,
            Self::S24_3Le => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveSpec {
    source: PcmSpec,
    wire: WireFormat,
}

/// ALSA `hw` 独占输出 adapter。
///
/// 当前目标设备只接受双声道 48/96 kHz，以及 S16_LE 或 S24_3LE。
/// 构造时立即打开设备，让占用冲突在切换输出模式时返回。
pub struct AlsaSink {
    device: String,
    pcm: PCM,
    active: Option<ActiveSpec>,
    scratch: Vec<u8>,
    paused: bool,
}

impl AlsaSink {
    pub fn new(device: impl Into<String>) -> Result<Self> {
        let device = device.into();
        let pcm = PCM::new(&device, Direction::Playback, false)
            .map_err(|error| alsa_error(&device, "打开独占设备失败", error))?;
        Ok(Self {
            device,
            pcm,
            active: None,
            scratch: Vec::new(),
            paused: false,
        })
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    fn configure(&mut self, spec: PcmSpec, wire: WireFormat) -> Result<()> {
        if self.active.is_some() {
            if self.paused {
                self.pcm
                    .drop()
                    .map_err(|error| self.error("丢弃旧格式缓冲失败", error))?;
            } else {
                self.pcm
                    .drain()
                    .map_err(|error| self.error("排空旧格式缓冲失败", error))?;
            }
        }

        // `HwParams::any` 会将 PCM 重新带回可配置状态；alsa 0.11 与
        // macOS 端 CPAL 使用同一 alsa-sys 版本，因此这里统一走该入口。
        let hwp =
            HwParams::any(&self.pcm).map_err(|error| self.error("读取硬件参数失败", error))?;
        hwp.set_access(Access::RWInterleaved)
            .map_err(|error| self.error("设置交错访问失败", error))?;
        hwp.set_format(wire.alsa())
            .map_err(|error| self.error("设置样本格式失败", error))?;
        hwp.set_channels(u32::from(spec.channels))
            .map_err(|error| self.error("设置声道数失败", error))?;
        hwp.set_rate_resample(false)
            .map_err(|error| self.error("关闭 ALSA 重采样失败", error))?;
        hwp.set_rate(spec.rate, ValueOr::Nearest)
            .map_err(|error| self.error("设置采样率失败", error))?;
        hwp.set_buffer_time_near(BUFFER_TIME_US, ValueOr::Nearest)
            .map_err(|error| self.error("设置硬件缓冲失败", error))?;
        hwp.set_period_time_near(PERIOD_TIME_US, ValueOr::Nearest)
            .map_err(|error| self.error("设置硬件周期失败", error))?;
        self.pcm
            .hw_params(&hwp)
            .map_err(|error| self.error("应用硬件参数失败", error))?;
        drop(hwp);

        let current = self
            .pcm
            .hw_params_current()
            .map_err(|error| self.error("读取生效参数失败", error))?;
        let actual_rate = current
            .get_rate()
            .map_err(|error| self.error("读取生效采样率失败", error))?;
        let actual_channels = current
            .get_channels()
            .map_err(|error| self.error("读取生效声道数失败", error))?;
        let actual_format = current
            .get_format()
            .map_err(|error| self.error("读取生效样本格式失败", error))?;
        let buffer_size = current
            .get_buffer_size()
            .map_err(|error| self.error("读取硬件缓冲大小失败", error))?;
        let period_size = current
            .get_period_size()
            .map_err(|error| self.error("读取硬件周期大小失败", error))?;
        let can_pause = current.can_pause();
        drop(current);

        if actual_rate != spec.rate
            || actual_channels != u32::from(spec.channels)
            || actual_format != wire.alsa()
        {
            return Err(Error::Other(format!(
                "ALSA 设备 {} 未按原始格式打开：请求 {} Hz / {} 声道 / {}，实际 {} Hz / {} 声道 / {}",
                self.device,
                spec.rate,
                spec.channels,
                wire.alsa(),
                actual_rate,
                actual_channels,
                actual_format
            )));
        }
        if !can_pause {
            return Err(Error::Other(format!(
                "ALSA 设备 {} 不支持硬件暂停",
                self.device
            )));
        }

        let swp = self
            .pcm
            .sw_params_current()
            .map_err(|error| self.error("读取软件参数失败", error))?;
        swp.set_start_threshold(period_size)
            .map_err(|error| self.error("设置启动阈值失败", error))?;
        swp.set_avail_min(period_size.min(buffer_size))
            .map_err(|error| self.error("设置唤醒阈值失败", error))?;
        self.pcm
            .sw_params(&swp)
            .map_err(|error| self.error("应用软件参数失败", error))?;
        drop(swp);

        self.active = Some(ActiveSpec { source: spec, wire });
        Ok(())
    }

    fn error(&self, action: &str, error: alsa::Error) -> Error {
        alsa_error(&self.device, action, error)
    }

    fn write_all_frames(&self, bytes: &[u8], frame_bytes: usize) -> Result<()> {
        let io = self.pcm.io_bytes();
        let mut offset = 0;
        while offset < bytes.len() {
            match io.writei(&bytes[offset..]) {
                Ok(0) => {
                    return Err(Error::Other(format!(
                        "ALSA 设备 {} 写入了 0 帧",
                        self.device
                    )));
                }
                Ok(frames) => offset += frames * frame_bytes,
                Err(error) => self
                    .pcm
                    .try_recover(error, true)
                    .map_err(|error| self.error("恢复输出失败", error))?,
            }
        }
        Ok(())
    }
}

impl AudioSink for AlsaSink {
    fn write(&mut self, spec: PcmSpec, samples: &[i32]) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let wire = wire_format(spec)?;
        if !samples.len().is_multiple_of(usize::from(spec.channels)) {
            return Err(Error::Other(format!(
                "音频样本数 {} 不能整除 {} 个声道",
                samples.len(),
                spec.channels
            )));
        }
        let active = ActiveSpec { source: spec, wire };
        if self.active != Some(active) {
            self.configure(spec, wire)?;
        }
        if self.paused {
            return Err(Error::Other("暂停时收到 ALSA 音频数据".into()));
        }

        pack_samples(wire, samples, &mut self.scratch);
        let frame_bytes = wire.bytes_per_sample() * usize::from(spec.channels);
        self.write_all_frames(&self.scratch, frame_bytes)
    }

    fn pause(&mut self, paused: bool) -> Result<()> {
        if self.paused == paused {
            return Ok(());
        }
        match self.pcm.state() {
            State::Running | State::Paused => self
                .pcm
                .pause(paused)
                .map_err(|error| self.error("切换暂停状态失败", error))?,
            _ => {}
        }
        self.paused = paused;
        Ok(())
    }

    fn discard(&mut self) -> Result<()> {
        if self.active.is_none() {
            return Ok(());
        }
        self.pcm
            .drop()
            .map_err(|error| self.error("丢弃硬件缓冲失败", error))?;
        self.pcm
            .prepare()
            .map_err(|error| self.error("重新准备设备失败", error))?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.active.is_none() {
            return Ok(());
        }
        if self.paused {
            self.pcm
                .drop()
                .map_err(|error| self.error("丢弃暂停缓冲失败", error))?;
        } else {
            self.pcm
                .drain()
                .map_err(|error| self.error("排空硬件缓冲失败", error))?;
        }
        self.active = None;
        Ok(())
    }
}

fn wire_format(spec: PcmSpec) -> Result<WireFormat> {
    if spec.channels != 2 {
        return Err(Error::Other(format!(
            "ALSA 独占模式只支持双声道，当前为 {} 声道",
            spec.channels
        )));
    }
    if !matches!(spec.rate, 48_000 | 96_000) {
        return Err(Error::Other(format!(
            "ALSA 独占模式当前支持 48/96 kHz，当前为 {} Hz",
            spec.rate
        )));
    }
    match spec.bits {
        1..=16 => Ok(WireFormat::S16Le),
        17..=24 => Ok(WireFormat::S24_3Le),
        bits => Err(Error::Other(format!(
            "ALSA 独占模式当前支持最高 24 位，当前为 {bits} 位"
        ))),
    }
}

fn pack_samples(format: WireFormat, samples: &[i32], output: &mut Vec<u8>) {
    output.clear();
    output.reserve(samples.len() * format.bytes_per_sample());
    match format {
        WireFormat::S16Le => {
            for &sample in samples {
                output.extend_from_slice(&((sample >> 16) as i16).to_le_bytes());
            }
        }
        WireFormat::S24_3Le => {
            for &sample in samples {
                let bytes = (sample >> 8).to_le_bytes();
                output.extend_from_slice(&bytes[..3]);
            }
        }
    }
}

fn alsa_error(device: &str, action: &str, error: alsa::Error) -> Error {
    Error::Other(format!("ALSA 设备 {device} {action}：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_formats_are_selected_without_alsa_conversion() {
        assert_eq!(
            wire_format(PcmSpec {
                rate: 48_000,
                channels: 2,
                bits: 16,
            })
            .unwrap(),
            WireFormat::S16Le
        );
        assert_eq!(
            wire_format(PcmSpec {
                rate: 96_000,
                channels: 2,
                bits: 24,
            })
            .unwrap(),
            WireFormat::S24_3Le
        );
    }

    #[test]
    fn unsupported_target_formats_have_actionable_errors() {
        let rate_error = wire_format(PcmSpec {
            rate: 44_100,
            channels: 2,
            bits: 16,
        })
        .unwrap_err()
        .to_string();
        assert!(rate_error.contains("48/96 kHz"));

        let channel_error = wire_format(PcmSpec {
            rate: 48_000,
            channels: 1,
            bits: 16,
        })
        .unwrap_err()
        .to_string();
        assert!(channel_error.contains("双声道"));

        let depth_error = wire_format(PcmSpec {
            rate: 48_000,
            channels: 2,
            bits: 32,
        })
        .unwrap_err()
        .to_string();
        assert!(depth_error.contains("最高 24 位"));
    }

    #[test]
    fn samples_are_packed_bit_exactly_for_the_hardware_formats() {
        let samples = [i32::MIN, -65_536, 0, 65_536, i32::MAX];
        let mut output = Vec::new();

        pack_samples(WireFormat::S16Le, &samples, &mut output);
        assert_eq!(
            output,
            [0x00, 0x80, 0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0xff, 0x7f]
        );

        let samples = [i32::MIN, -256, 0, 256, i32::MAX];
        pack_samples(WireFormat::S24_3Le, &samples, &mut output);
        assert_eq!(
            output,
            [
                0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0xff, 0xff,
                0x7f,
            ]
        );
    }
}
