//! 独占输出用高质量采样率适配。

use std::collections::VecDeque;

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use crate::audio::PcmSpec;
use crate::audio::sink::AudioSink;
use crate::error::{Error, Result};

const CD_RATE: u32 = 44_100;
const TARGET_RATE: u32 = 96_000;
const TARGET_BITS: u16 = 24;
const CHUNK_FRAMES: usize = 1024;
const SINC_LENGTH: usize = 256;
const RATIO_NUMERATOR: usize = 320;
const MAX_DRAIN_BLOCKS: usize = 16;
const I32_SCALE: f64 = 2_147_483_648.0;
const I24_SCALE: f64 = 8_388_608.0;

/// 仅转换目标硬件无法接收的 44.1 kHz 流，其余格式逐样本直通。
pub struct ResamplingSink {
    inner: Box<dyn AudioSink>,
    stream: Option<ResampleStream>,
}

impl ResamplingSink {
    pub fn new(inner: Box<dyn AudioSink>) -> Self {
        Self {
            inner,
            stream: None,
        }
    }

    fn finish_resampling(&mut self) -> Result<()> {
        let Some(mut stream) = self.stream.take() else {
            return Ok(());
        };
        stream.drain(self.inner.as_mut())
    }
}

impl AudioSink for ResamplingSink {
    fn write(&mut self, spec: PcmSpec, samples: &[i32]) -> Result<()> {
        validate_samples(spec, samples)?;
        if samples.is_empty() {
            return Ok(());
        }

        if spec.rate != CD_RATE {
            self.finish_resampling()?;
            return self.inner.write(spec, samples);
        }

        let channels = usize::from(spec.channels);
        if self
            .stream
            .as_ref()
            .is_some_and(|stream| stream.channels != channels)
        {
            self.finish_resampling()?;
        }
        if self.stream.is_none() {
            self.stream = Some(ResampleStream::new(channels)?);
        }

        self.stream
            .as_mut()
            .expect("重采样流已创建")
            .push(samples, self.inner.as_mut())
    }

    fn pause(&mut self, paused: bool) -> Result<()> {
        self.inner.pause(paused)
    }

    fn discard(&mut self) -> Result<()> {
        self.stream = None;
        self.inner.discard()
    }

    fn flush(&mut self) -> Result<()> {
        self.finish_resampling()?;
        self.inner.flush()
    }
}

struct ResampleStream {
    channels: usize,
    output_spec: PcmSpec,
    resampler: Async<f64>,
    pending: VecDeque<f64>,
    input_block: Vec<f64>,
    output_block: Vec<f64>,
    output_samples: Vec<i32>,
    startup_frames_to_skip: usize,
    input_frames: u64,
    output_frames: u64,
}

impl ResampleStream {
    fn new(channels: usize) -> Result<Self> {
        let parameters =
            SincInterpolationParameters::new(SINC_LENGTH, WindowFunction::BlackmanHarris2)
                // 96_000 / 44_100 = 320 / 147；320 个相位使每个输出点落在预计算 sinc 上。
                .oversampling_factor(RATIO_NUMERATOR)
                .interpolation(SincInterpolationType::Nearest);
        let resampler = Async::<f64>::new_sinc(
            f64::from(TARGET_RATE) / f64::from(CD_RATE),
            1.0,
            &parameters,
            CHUNK_FRAMES,
            channels,
            FixedAsync::Input,
        )
        .map_err(|error| Error::Other(format!("创建 44.1→96 kHz 重采样器失败：{error}")))?;
        let startup_frames_to_skip = resampler.output_delay();
        let output_block_samples = resampler.output_frames_max() * channels;
        let chunk_samples = CHUNK_FRAMES * channels;
        Ok(Self {
            channels,
            output_spec: PcmSpec {
                rate: TARGET_RATE,
                channels: channels as u16,
                bits: TARGET_BITS,
            },
            resampler,
            pending: VecDeque::with_capacity(chunk_samples * 2),
            input_block: vec![0.0; chunk_samples],
            output_block: vec![0.0; output_block_samples],
            output_samples: Vec::new(),
            startup_frames_to_skip,
            input_frames: 0,
            output_frames: 0,
        })
    }

    fn push(&mut self, samples: &[i32], sink: &mut dyn AudioSink) -> Result<()> {
        self.input_frames += (samples.len() / self.channels) as u64;
        self.pending
            .extend(samples.iter().map(|&sample| f64::from(sample) / I32_SCALE));

        let chunk_samples = CHUNK_FRAMES * self.channels;
        while self.pending.len() >= chunk_samples {
            for slot in &mut self.input_block {
                *slot = self.pending.pop_front().expect("已确认有完整输入块");
            }
            self.process_block(None, sink)?;
        }
        Ok(())
    }

    fn drain(&mut self, sink: &mut dyn AudioSink) -> Result<()> {
        if self.input_frames == 0 {
            return Ok(());
        }

        if !self.pending.is_empty() {
            let partial_frames = self.pending.len() / self.channels;
            self.input_block.fill(0.0);
            for slot in self.input_block.iter_mut().take(self.pending.len()) {
                *slot = self.pending.pop_front().expect("待处理样本长度稳定");
            }
            self.process_block(Some(partial_frames), sink)?;
        }

        let expected = expected_output_frames(self.input_frames);
        for _ in 0..MAX_DRAIN_BLOCKS {
            if self.output_frames >= expected {
                return Ok(());
            }
            self.input_block.fill(0.0);
            self.process_block(Some(0), sink)?;
        }

        Err(Error::Other(format!(
            "重采样器收尾不足：期望 {expected} 帧，得到 {} 帧",
            self.output_frames
        )))
    }

    fn process_block(
        &mut self,
        partial_frames: Option<usize>,
        sink: &mut dyn AudioSink,
    ) -> Result<()> {
        let input = InterleavedSlice::new(&self.input_block, self.channels, CHUNK_FRAMES)
            .map_err(|error| Error::Other(format!("创建重采样输入缓冲失败：{error}")))?;
        let output_capacity = self.output_block.len() / self.channels;
        let mut output =
            InterleavedSlice::new_mut(&mut self.output_block, self.channels, output_capacity)
                .map_err(|error| Error::Other(format!("创建重采样输出缓冲失败：{error}")))?;
        let indexing = partial_frames.map(|frames| Indexing::new().partial_len(frames));
        let (_, output_frames) = self
            .resampler
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .map_err(|error| Error::Other(format!("44.1→96 kHz 重采样失败：{error}")))?;
        self.emit_output(output_frames, sink)
    }

    fn emit_output(&mut self, available_frames: usize, sink: &mut dyn AudioSink) -> Result<()> {
        let skipped = self.startup_frames_to_skip.min(available_frames);
        self.startup_frames_to_skip -= skipped;

        let expected = expected_output_frames(self.input_frames);
        let remaining = expected.saturating_sub(self.output_frames) as usize;
        let frames = (available_frames - skipped).min(remaining);
        if frames == 0 {
            return Ok(());
        }

        let start = skipped * self.channels;
        let end = start + frames * self.channels;
        self.output_samples.clear();
        self.output_samples.extend(
            self.output_block[start..end]
                .iter()
                .copied()
                .map(float_to_24bit_i32),
        );
        sink.write(self.output_spec, &self.output_samples)?;
        self.output_frames += frames as u64;
        Ok(())
    }
}

fn validate_samples(spec: PcmSpec, samples: &[i32]) -> Result<()> {
    if spec.channels == 0 {
        return Err(Error::Other("音频声道数不能为 0".into()));
    }
    if !samples.len().is_multiple_of(usize::from(spec.channels)) {
        return Err(Error::Other(format!(
            "音频样本数 {} 不能整除 {} 个声道",
            samples.len(),
            spec.channels
        )));
    }
    Ok(())
}

fn expected_output_frames(input_frames: u64) -> u64 {
    let scaled = u128::from(input_frames) * u128::from(TARGET_RATE);
    scaled.div_ceil(u128::from(CD_RATE)) as u64
}

fn float_to_24bit_i32(sample: f64) -> i32 {
    let sample = (sample * I24_SCALE)
        .round()
        .clamp(-I24_SCALE, I24_SCALE - 1.0) as i32;
    sample << 8
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Default)]
    struct Capture {
        writes: Vec<(PcmSpec, Vec<i32>)>,
        discards: usize,
        flushes: usize,
    }

    struct CaptureSink(Arc<Mutex<Capture>>);

    impl AudioSink for CaptureSink {
        fn write(&mut self, spec: PcmSpec, samples: &[i32]) -> Result<()> {
            self.0.lock().unwrap().writes.push((spec, samples.to_vec()));
            Ok(())
        }

        fn discard(&mut self) -> Result<()> {
            self.0.lock().unwrap().discards += 1;
            Ok(())
        }

        fn flush(&mut self) -> Result<()> {
            self.0.lock().unwrap().flushes += 1;
            Ok(())
        }
    }

    fn capture_sink() -> (ResamplingSink, Arc<Mutex<Capture>>) {
        let capture = Arc::new(Mutex::new(Capture::default()));
        let sink = ResamplingSink::new(Box::new(CaptureSink(capture.clone())));
        (sink, capture)
    }

    #[test]
    fn native_hardware_rates_are_bit_exact_passthrough() {
        let (mut sink, capture) = capture_sink();
        let samples = vec![i32::MIN, i32::MAX, -65_536, 65_536];
        for spec in [
            PcmSpec {
                rate: 48_000,
                channels: 2,
                bits: 16,
            },
            PcmSpec {
                rate: 96_000,
                channels: 2,
                bits: 24,
            },
        ] {
            sink.write(spec, &samples).unwrap();
        }
        sink.flush().unwrap();

        let capture = capture.lock().unwrap();
        assert_eq!(capture.writes.len(), 2);
        assert_eq!(capture.writes[0].1, samples);
        assert_eq!(capture.writes[1].1, samples);
        assert_eq!(capture.flushes, 1);
    }

    #[test]
    fn cd_audio_is_streamed_to_exact_96khz_length_without_channel_bleed() {
        let (mut sink, capture) = capture_sink();
        let spec = PcmSpec {
            rate: CD_RATE,
            channels: 2,
            bits: 16,
        };
        let input_frames = 2_500usize;
        let mut samples = vec![0; input_frames * 2];
        samples[1_000 * 2] = i32::MAX / 4;

        let first = 333 * 2;
        let second = first + 1_127 * 2;
        sink.write(spec, &samples[..first]).unwrap();
        sink.write(spec, &samples[first..second]).unwrap();
        sink.write(spec, &samples[second..]).unwrap();
        sink.flush().unwrap();

        let capture = capture.lock().unwrap();
        assert!(!capture.writes.is_empty());
        assert!(capture.writes.iter().all(|(actual, _)| {
            *actual
                == PcmSpec {
                    rate: TARGET_RATE,
                    channels: 2,
                    bits: TARGET_BITS,
                }
        }));
        let output = capture
            .writes
            .iter()
            .flat_map(|(_, samples)| samples.iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(
            output.len() / 2,
            expected_output_frames(input_frames as u64) as usize
        );
        assert!(output.iter().step_by(2).any(|&sample| sample != 0));
        assert!(output.iter().skip(1).step_by(2).all(|&sample| sample == 0));
        assert!(output.iter().all(|&sample| sample & 0xff == 0));
    }

    #[test]
    fn discard_clears_pending_filter_state() {
        let (mut sink, capture) = capture_sink();
        let spec = PcmSpec {
            rate: CD_RATE,
            channels: 2,
            bits: 24,
        };
        let mut discarded = vec![0; 800 * 2];
        discarded[300 * 2] = i32::MAX / 4;
        sink.write(spec, &discarded).unwrap();
        sink.discard().unwrap();
        sink.write(spec, &vec![0; 2_048 * 2]).unwrap();
        sink.flush().unwrap();

        let capture = capture.lock().unwrap();
        assert_eq!(capture.discards, 1);
        assert_eq!(
            capture
                .writes
                .iter()
                .map(|(_, samples)| samples.len() / 2)
                .sum::<usize>(),
            expected_output_frames(2_048) as usize
        );
        assert!(
            capture
                .writes
                .iter()
                .flat_map(|(_, samples)| samples)
                .all(|&sample| sample == 0)
        );
    }
}
