//! macOS CoreAudio 共享输出。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    error: Option<String>,
}

struct Shared {
    state: Mutex<State>,
    cond: Condvar,
    discard_epoch: AtomicU64,
    discard_ack: AtomicU64,
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
    producer: Option<rtrb::Producer<i32>>,
    capacity: usize,
    cancelled: Arc<AtomicBool>,
}

impl CoreAudioSink {
    pub fn new() -> Result<Self> {
        Self::open(None)
    }

    #[cfg_attr(
        all(feature = "coreaudio-compile-check", target_os = "linux"),
        allow(dead_code)
    )]
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
                discard_epoch: AtomicU64::new(0),
                discard_ack: AtomicU64::new(0),
            }),
            stream: None,
            spec: None,
            paused: false,
            producer: None,
            capacity: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    fn configure(&mut self, spec: PcmSpec) -> Result<()> {
        let selected = select_config(&self.device, spec)?;
        let output_channels = selected.channels();
        let sample_format = selected.sample_format();
        let config = selected.config();
        let source_channels = spec.channels;

        self.shared
            .state
            .lock()
            .expect("CoreAudio state lock")
            .error = None;
        self.capacity =
            (spec.rate as usize * usize::from(source_channels) * BUFFER_DEPTH.as_millis() as usize
                / 1000)
                .max(source_channels as usize);
        let (producer, consumer) = rtrb::RingBuffer::new(self.capacity);
        self.producer = Some(producer);
        self.shared.discard_ack.store(
            self.shared.discard_epoch.load(Ordering::Acquire),
            Ordering::Release,
        );

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(
                &self.device,
                config,
                source_channels,
                output_channels,
                self.shared.clone(),
                consumer,
            ),
            SampleFormat::I16 => build_stream::<i16>(
                &self.device,
                config,
                source_channels,
                output_channels,
                self.shared.clone(),
                consumer,
            ),
            SampleFormat::I32 => build_stream::<i32>(
                &self.device,
                config,
                source_channels,
                output_channels,
                self.shared.clone(),
                consumer,
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

    fn check_wait(&self, deadline: Instant) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(Error::Interrupted);
        }
        Self::check_error(&self.shared.state.lock().expect("CoreAudio state lock"))?;
        if Instant::now() >= deadline {
            return Err(Error::Other("CoreAudio 输出等待超时，请检查设备".into()));
        }
        Ok(())
    }
    fn drain(&self) -> Result<()> {
        let deadline = Instant::now() + WAIT_LIMIT;
        while self
            .producer
            .as_ref()
            .is_some_and(|p| p.slots() < self.capacity)
        {
            self.check_wait(deadline)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        if let (Some(stream), Some(spec)) = (&self.stream, self.spec)
            && let Ok(frames) = stream.buffer_size()
        {
            let end = Instant::now()
                + Duration::from_secs_f64(f64::from(frames) / f64::from(spec.rate))
                    .min(BUFFER_DEPTH);
            while Instant::now() < end {
                self.check_wait(deadline)?;
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        Ok(())
    }
}

impl AudioSink for CoreAudioSink {
    fn set_cancel_flag(&mut self, flag: Arc<AtomicBool>) {
        self.cancelled = flag;
    }
    fn output_description(&self) -> Option<String> {
        self.spec.map(|s| {
            format!(
                "应用 → CoreAudio：{} Hz 源输入 / {} 声道\n设备混音与最终格式由 CoreAudio 管理",
                s.rate, s.channels
            )
        })
    }
    fn latency_secs(&self) -> f64 {
        match (&self.producer, self.spec) {
            (Some(p), Some(s)) => {
                self.capacity.saturating_sub(p.slots()) as f64
                    / (f64::from(s.rate) * f64::from(s.channels))
            }
            _ => 0.0,
        }
    }
    fn write(&mut self, spec: PcmSpec, samples: &[i32]) -> Result<()> {
        if spec.rate == 0
            || spec.channels == 0
            || !samples.len().is_multiple_of(spec.channels as usize)
        {
            return Err(Error::Other("PCM 格式或帧边界无效".into()));
        }
        if self.spec != Some(spec) {
            if self.spec.is_some() && !self.paused {
                self.drain()?;
            }
            self.stream = None;
            self.configure(spec)?;
        }
        let deadline = Instant::now() + WAIT_LIMIT;
        while self.shared.discard_ack.load(Ordering::Acquire)
            != self.shared.discard_epoch.load(Ordering::Acquire)
        {
            self.check_wait(deadline)?;
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut remaining = samples;
        while !remaining.is_empty() {
            self.check_wait(deadline)?;
            let p = self.producer.as_mut().expect("PCM producer initialized");
            let n =
                p.slots().min(remaining.len()) / spec.channels as usize * spec.channels as usize;
            if n == 0 {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            let (_, tail) = p.push_partial_slice(&remaining[..n]);
            remaining = &remaining[n - tail.len()..];
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
        let epoch = self.shared.discard_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        if self.stream.is_none() {
            self.shared.discard_ack.store(epoch, Ordering::Release);
        }
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
    mut consumer: rtrb::Consumer<i32>,
) -> Result<Stream> {
    let mut epoch = shared.discard_epoch.load(Ordering::Acquire);
    let callback_shared = shared.clone();
    let error_shared = shared;
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                let requested = callback_shared.discard_epoch.load(Ordering::Acquire);
                if epoch != requested {
                    let available = consumer.slots();
                    if let Ok(chunk) = consumer.read_chunk(available) {
                        chunk.commit_all();
                    }
                    epoch = requested;
                    callback_shared
                        .discard_ack
                        .store(requested, Ordering::Release);
                }
                fill_output(output, source_channels, output_channels, &mut consumer);
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
    consumer: &mut rtrb::Consumer<i32>,
) {
    output.fill(T::silence());
    let source_channels = usize::from(source_channels);
    let output_channels = usize::from(output_channels);
    if source_channels == 0 || output_channels == 0 {
        return;
    }
    let frames = (output.len() / output_channels).min(consumer.slots() / source_channels);
    if let Ok(chunk) = consumer.read_chunk(frames * source_channels) {
        let (a, b) = chunk.as_slices();
        let at = |index: usize| {
            if index < a.len() {
                a[index]
            } else {
                b[index - a.len()]
            }
        };
        for (frame_index, frame) in output
            .chunks_exact_mut(output_channels)
            .take(frames)
            .enumerate()
        {
            let start = frame_index * source_channels;
            for (channel, dst) in frame.iter_mut().enumerate() {
                let sample = match (source_channels, output_channels) {
                    (1, _) => at(start),
                    (2, 1) => ((i64::from(at(start)) + i64::from(at(start + 1))) / 2) as i32,
                    _ if channel < source_channels => at(start + channel),
                    _ => 0,
                };
                *dst = T::from_i32(sample);
            }
        }
        chunk.commit_all();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_preserves_stereo_and_zero_fills_short_input() {
        let (mut p, mut c) = rtrb::RingBuffer::new(8);
        let _ = p.push_partial_slice(&[1, 2, 3, 4]);
        let mut out = [-1i32; 8];
        fill_output(&mut out, 2, 2, &mut c);
        assert_eq!(out, [1, 2, 3, 4, 0, 0, 0, 0]);
    }
    #[test]
    fn mono_is_duplicated_with_complete_frame_consumption() {
        let (mut p, mut c) = rtrb::RingBuffer::new(4);
        let _ = p.push_partial_slice(&[7, 8]);
        let mut out = [0i32; 4];
        fill_output(&mut out, 1, 2, &mut c);
        assert_eq!(out, [7, 7, 8, 8]);
        assert_eq!(c.slots(), 0);
    }
}
