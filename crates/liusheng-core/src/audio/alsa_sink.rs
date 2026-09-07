use alsa::pcm::{Access, Format, HwParams, PCM, State};
use alsa::{Direction, ValueOr};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::audio::PcmSpec;
use crate::audio::sink::AudioSink;
use crate::error::{Error, Result};

const BUFFER_TIME_US: u32 = 200_000;
const PERIOD_TIME_US: u32 = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WireFormat {
    S16,
    S24Packed,
    S32,
}

impl WireFormat {
    fn alsa(self) -> Format {
        match self {
            Self::S16 => Format::S16LE,
            Self::S24Packed => Format::S243LE,
            Self::S32 => Format::S32LE,
        }
    }

    fn bytes_per_sample(self) -> usize {
        match self {
            Self::S16 => 2,
            Self::S24Packed => 3,
            Self::S32 => 4,
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
/// 按设备能力协商原生整数 PCM，逐次验证生效格式。
/// 构造时立即打开设备，让占用冲突在切换输出模式时返回。
pub struct AlsaSink {
    device: String,
    pcm: PCM,
    active: Option<ActiveSpec>,
    scratch: Vec<u8>,
    paused: bool,
    cancelled: Arc<AtomicBool>,
    native_cd_specs: Vec<(u16, WireFormat)>,
    can_pause: bool,
}

impl AlsaSink {
    pub fn new(device: impl Into<String>) -> Result<Self> {
        let device = device.into();
        let pcm =
            PCM::new(&device, Direction::Playback, true).map_err(|error| Error::AudioDevice {
                code: error.errno(),
                message: format!("ALSA {device}：{error}"),
            })?;
        let mut native_cd_specs = Vec::new();
        if let Ok(params) = HwParams::any(&pcm)
            && params.test_rate(44_100).is_ok()
        {
            for channels in [1u16, 2, 4, 6, 8] {
                for wire in [WireFormat::S16, WireFormat::S24Packed, WireFormat::S32] {
                    if params.test_channels(u32::from(channels)).is_ok()
                        && params.test_format(wire.alsa()).is_ok()
                    {
                        native_cd_specs.push((channels, wire));
                    }
                }
            }
        }
        Ok(Self {
            native_cd_specs,
            can_pause: true,
            device,
            pcm,
            active: None,
            scratch: Vec::new(),
            paused: false,
            cancelled: Arc::new(AtomicBool::new(false)),
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
                self.drain_interruptible()?;
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
        self.can_pause = can_pause;

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

    fn drain_interruptible(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(Error::Interrupted);
            }
            match self.pcm.drain() {
                Ok(()) => return Ok(()),
                Err(e) if e.errno() == 11 && Instant::now() < deadline => {
                    let _ = self.pcm.wait(Some(10));
                }
                Err(e) => return Err(self.error("排空输出失败", e)),
            }
        }
    }
    fn error(&self, action: &str, error: alsa::Error) -> Error {
        alsa_error(&self.device, action, error)
    }

    fn write_all_frames(&self, bytes: &[u8], frame_bytes: usize) -> Result<()> {
        let io = self.pcm.io_bytes();
        let mut offset = 0;
        let deadline = Instant::now() + Duration::from_secs(3);
        while offset < bytes.len() {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(Error::Interrupted);
            }
            if Instant::now() >= deadline {
                return Err(Error::Other("ALSA 输出等待超时，请检查设备".into()));
            }
            match io.writei(&bytes[offset..]) {
                Ok(0) => {
                    return Err(Error::Other(format!(
                        "ALSA 设备 {} 写入了 0 帧",
                        self.device
                    )));
                }
                Ok(frames) => offset += frames * frame_bytes,
                Err(error) if error.errno() == 11 => {
                    let _ = self.pcm.wait(Some(10));
                }
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
    fn set_cancel_flag(&mut self, flag: Arc<AtomicBool>) {
        self.cancelled = flag;
    }
    fn latency_secs(&self) -> f64 {
        self.active
            .map(|a| self.pcm.delay().unwrap_or(0).max(0) as f64 / f64::from(a.source.rate))
            .unwrap_or(0.0)
    }
    fn pause_discards_buffer(&self) -> bool {
        !self.can_pause
    }
    fn supports_native(&self, spec: PcmSpec) -> bool {
        spec.rate == 44_100
            && wire_format(spec)
                .is_ok_and(|wire| self.native_cd_specs.contains(&(spec.channels, wire)))
    }
    fn output_description(&self) -> Option<String> {
        self.active.map(|a| {
            format!(
                "ALSA 硬件 {}：{} Hz / {} / {} 声道",
                self.device,
                a.source.rate,
                a.wire.alsa(),
                a.source.channels
            )
        })
    }
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
        if !self.can_pause {
            if paused {
                self.pcm.drop().map_err(|e| self.error("暂停输出失败", e))?;
            } else if self.active.is_some() {
                self.pcm
                    .prepare()
                    .map_err(|e| self.error("恢复输出失败", e))?;
            }
            self.paused = paused;
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
            self.drain_interruptible()?;
        }
        self.active = None;
        Ok(())
    }
}

fn wire_format(spec: PcmSpec) -> Result<WireFormat> {
    if spec.channels == 0 || spec.channels > 32 || spec.rate == 0 {
        return Err(Error::Other("PCM 采样率或声道数无效".into()));
    }
    match spec.bits {
        1..=16 => Ok(WireFormat::S16),
        17..=24 => Ok(WireFormat::S24Packed),
        25..=32 => Ok(WireFormat::S32),
        bits => Err(Error::Other(format!("ALSA PCM 位深无效：{bits}"))),
    }
}

fn pack_samples(format: WireFormat, samples: &[i32], output: &mut Vec<u8>) {
    output.clear();
    output.reserve(samples.len() * format.bytes_per_sample());
    match format {
        WireFormat::S32 => {
            for sample in samples {
                output.extend_from_slice(&sample.to_le_bytes());
            }
        }
        WireFormat::S16 => {
            for &sample in samples {
                output.extend_from_slice(&((sample >> 16) as i16).to_le_bytes());
            }
        }
        WireFormat::S24Packed => {
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
            WireFormat::S16
        );
        assert_eq!(
            wire_format(PcmSpec {
                rate: 96_000,
                channels: 2,
                bits: 24,
            })
            .unwrap(),
            WireFormat::S24Packed
        );
    }

    #[test]
    fn invalid_pcm_formats_have_actionable_errors() {
        for spec in [
            PcmSpec {
                rate: 0,
                channels: 2,
                bits: 16,
            },
            PcmSpec {
                rate: 48000,
                channels: 0,
                bits: 16,
            },
            PcmSpec {
                rate: 48000,
                channels: 2,
                bits: 0,
            },
        ] {
            assert!(wire_format(spec).is_err());
        }
        assert_eq!(
            wire_format(PcmSpec {
                rate: 44100,
                channels: 1,
                bits: 32
            })
            .unwrap(),
            WireFormat::S32
        );
    }

    #[test]
    fn samples_are_packed_bit_exactly_for_the_hardware_formats() {
        let samples = [i32::MIN, -65_536, 0, 65_536, i32::MAX];
        let mut output = Vec::new();

        pack_samples(WireFormat::S16, &samples, &mut output);
        assert_eq!(
            output,
            [0x00, 0x80, 0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0xff, 0x7f]
        );

        let samples = [i32::MIN, -256, 0, 256, i32::MAX];
        pack_samples(WireFormat::S24Packed, &samples, &mut output);
        assert_eq!(
            output,
            [
                0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0xff, 0xff,
                0x7f,
            ]
        );
    }
}
