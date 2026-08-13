//! macOS CoreAudio 共享输出。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample, Stream, StreamConfig, SupportedStreamConfig};

use crate::audio::PcmSpec;
use crate::audio::sink::AudioSink;
use crate::error::{Error, Result};

const BUFFER_DEPTH: Duration = Duration::from_millis(200);
const WAIT_LIMIT: Duration = Duration::from_secs(10);

#[derive(Default)]
struct State {
    ring: VecDeque<i32>,
    capacity: usize,
    error: Option<String>,
}

struct Shared {
    state: Mutex<State>,
    cond: Condvar,
}

impl Shared {
    fn fail(&self, message: String) {
        let mut state = self.state.lock().expect("CoreAudio 状态锁未损坏");
        state.error.get_or_insert(message);
        self.cond.notify_all();
    }
}

/// 使用默认 CoreAudio 输出设备。流按曲目来源采样率创建，声道布局由回调适配。
pub struct CoreAudioSink {
    device: cpal::Device,
    shared: Arc<Shared>,
    stream: Option<Stream>,
    spec: Option<PcmSpec>,
    paused: bool,
}

impl CoreAudioSink {
    pub fn new() -> Result<Self> {
        Self::open(None)
    }

    pub(crate) fn new_cancelable(cancelled: &AtomicBool) -> Result<Self> {
        Self::open(Some(cancelled))
    }

    fn open(cancelled: Option<&AtomicBool>) -> Result<Self> {
        if cancelled.is_some_and(|cancelled| cancelled.load(Ordering::Acquire)) {
            return Err(Error::Other("CoreAudio 连接已取消".into()));
        }
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| Error::Other("找不到 macOS 默认音频输出设备".into()))?;
        device
            .default_output_config()
            .map_err(|error| Error::Other(format!("读取 CoreAudio 默认格式失败：{error}")))?;
        Ok(Self {
            device,
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
                cond: Condvar::new(),
            }),
            stream: None,
            spec: None,
            paused: false,
        })
    }

    fn configure(&mut self, spec: PcmSpec) -> Result<()> {
        let selected = select_config(&self.device, spec)?;
        let output_channels = selected.channels();
        let sample_format = selected.sample_format();
        let config = selected.config();
        let source_channels = spec.channels;

        {
            let mut state = self.shared.state.lock().expect("CoreAudio 状态锁未损坏");
            state.ring.clear();
            state.error = None;
            state.capacity = (spec.rate as usize * usize::from(source_channels))
                .saturating_mul(BUFFER_DEPTH.as_millis() as usize)
                / 1000;
            state.capacity = state.capacity.max(usize::from(source_channels));
        }

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(
                &self.device,
                config,
                source_channels,
                output_channels,
                self.shared.clone(),
            ),
            SampleFormat::I16 => build_stream::<i16>(
                &self.device,
                config,
                source_channels,
                output_channels,
                self.shared.clone(),
            ),
            SampleFormat::I32 => build_stream::<i32>(
                &self.device,
                config,
                source_channels,
                output_channels,
                self.shared.clone(),
            ),
            _ => Err(Error::Other(format!(
                "CoreAudio 返回了暂不支持的样本格式：{sample_format}"
            ))),
        }?;
        if !self.paused {
            stream
                .play()
                .map_err(|error| Error::Other(format!("启动 CoreAudio 输出失败：{error}")))?;
        }
        self.stream = Some(stream);
        self.spec = Some(spec);
        Ok(())
    }

    fn check_error(state: &MutexGuard<'_, State>) -> Result<()> {
        match &state.error {
            Some(error) => Err(Error::Other(error.clone())),
            None => Ok(()),
        }
    }

    fn drain(&self) -> Result<()> {
        let deadline = Instant::now() + WAIT_LIMIT;
        let mut state = self.shared.state.lock().expect("CoreAudio 状态锁未损坏");
        while !state.ring.is_empty() {
            Self::check_error(&state)?;
            if Instant::now() >= deadline {
                return Err(Error::Other("等待 CoreAudio 缓冲播空超时".into()));
            }
            let (next, _) = self
                .shared
                .cond
                .wait_timeout(state, Duration::from_millis(100))
                .expect("CoreAudio 状态锁未损坏");
            state = next;
        }
        drop(state);
        if let (Some(stream), Some(spec)) = (&self.stream, self.spec)
            && let Ok(frames) = stream.buffer_size()
        {
            let tail = Duration::from_secs_f64(f64::from(frames) / f64::from(spec.rate));
            std::thread::sleep(tail.min(BUFFER_DEPTH));
        }
        Ok(())
    }
}

impl AudioSink for CoreAudioSink {
    fn write(&mut self, spec: PcmSpec, samples: &[i32]) -> Result<()> {
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
        if self.spec != Some(spec) {
            if self.spec.is_some() && !self.paused {
                self.drain()?;
            }
            self.stream = None;
            self.configure(spec)?;
        }

        let mut offset = 0;
        let mut state = self.shared.state.lock().expect("CoreAudio 状态锁未损坏");
        while offset < samples.len() {
            Self::check_error(&state)?;
            let space = state.capacity.saturating_sub(state.ring.len());
            if space == 0 {
                let (next, _) = self
                    .shared
                    .cond
                    .wait_timeout(state, Duration::from_millis(100))
                    .expect("CoreAudio 状态锁未损坏");
                state = next;
                continue;
            }
            let count = space.min(samples.len() - offset);
            state.ring.extend(&samples[offset..offset + count]);
            offset += count;
        }
        Ok(())
    }

    fn pause(&mut self, paused: bool) -> Result<()> {
        if self.paused == paused {
            return Ok(());
        }
        self.paused = paused;
        if let Some(stream) = &self.stream {
            if paused {
                stream.pause()
            } else {
                stream.play()
            }
            .map_err(|error| Error::Other(format!("切换 CoreAudio 播放状态失败：{error}")))?;
        }
        Ok(())
    }

    fn discard(&mut self) -> Result<()> {
        let mut state = self.shared.state.lock().expect("CoreAudio 状态锁未损坏");
        state.ring.clear();
        drop(state);
        self.shared.cond.notify_all();
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.stream.is_none() {
            return Ok(());
        }
        if self.paused {
            self.discard()?;
        } else {
            self.drain()?;
        }
        self.stream = None;
        self.spec = None;
        Ok(())
    }
}

fn select_config(device: &cpal::Device, spec: PcmSpec) -> Result<SupportedStreamConfig> {
    device
        .supported_output_configs()
        .map_err(|error| Error::Other(format!("读取 CoreAudio 输出格式失败：{error}")))?
        .filter(|range| {
            range.contains_rate(spec.rate)
                && matches!(
                    range.sample_format(),
                    SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32
                )
        })
        .min_by_key(|range| {
            let channel_distance = range.channels().abs_diff(spec.channels);
            let sample_preference = match range.sample_format() {
                SampleFormat::F32 => 0,
                SampleFormat::I32 => 1,
                SampleFormat::I16 => 2,
                _ => 3,
            };
            (channel_distance, sample_preference)
        })
        .map(|range| range.with_sample_rate(spec.rate))
        .ok_or_else(|| Error::Other(format!("默认 CoreAudio 设备不支持 {} Hz 输入", spec.rate)))
}

trait OutputSample: SizedSample + Copy {
    fn silence() -> Self;
    fn from_i32(sample: i32) -> Self;
}

impl OutputSample for f32 {
    fn silence() -> Self {
        0.0
    }

    fn from_i32(sample: i32) -> Self {
        sample as f32 / 2_147_483_648.0
    }
}

impl OutputSample for i16 {
    fn silence() -> Self {
        0
    }

    fn from_i32(sample: i32) -> Self {
        (sample >> 16) as i16
    }
}

impl OutputSample for i32 {
    fn silence() -> Self {
        0
    }

    fn from_i32(sample: i32) -> Self {
        sample
    }
}

fn build_stream<T: OutputSample>(
    device: &cpal::Device,
    config: StreamConfig,
    source_channels: u16,
    output_channels: u16,
    shared: Arc<Shared>,
) -> Result<Stream> {
    let callback_shared = shared.clone();
    let error_shared = shared;
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                fill_output(output, source_channels, output_channels, &callback_shared);
            },
            move |error| error_shared.fail(format!("CoreAudio 输出错误：{error}")),
            None,
        )
        .map_err(|error| Error::Other(format!("创建 CoreAudio 输出流失败：{error}")))
}

fn fill_output<T: OutputSample>(
    output: &mut [T],
    source_channels: u16,
    output_channels: u16,
    shared: &Shared,
) {
    let source_channels = usize::from(source_channels);
    let output_channels = usize::from(output_channels);
    let mut state = shared.state.lock().expect("CoreAudio 状态锁未损坏");
    for output_frame in output.chunks_mut(output_channels) {
        if state.ring.len() < source_channels {
            output_frame.fill(T::silence());
            continue;
        }
        for (channel, output_sample) in output_frame.iter_mut().enumerate() {
            let sample = match (source_channels, output_channels) {
                (1, _) => state.ring[0],
                (2, 1) => ((i64::from(state.ring[0]) + i64::from(state.ring[1])) / 2) as i32,
                _ if channel < source_channels => state.ring[channel],
                _ => 0,
            };
            *output_sample = T::from_i32(sample);
        }
        for _ in 0..source_channels {
            state.ring.pop_front();
        }
    }
    drop(state);
    shared.cond.notify_all();
}
