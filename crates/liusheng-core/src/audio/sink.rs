use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::audio::PcmSpec;
use crate::error::{Error, Result};

/// 音频输出端。样本为交错 i32 满量程，spec.bits 指示来源有效位深。
pub trait AudioSink: Send {
    fn write(&mut self, spec: PcmSpec, samples: &[i32]) -> Result<()>;
    /// 暂停/恢复输出。带缓冲的实现应立即停声而非播完缓冲。
    fn pause(&mut self, _paused: bool) -> Result<()> {
        Ok(())
    }
    /// 丢弃已写入未播出的数据。切歌、seek、停止时调用，保证响应即时。
    fn discard(&mut self) -> Result<()> {
        Ok(())
    }
    /// 播完全部已写入数据并收尾。队列播完与退出时调用。
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

/// 丢弃全部输出，只计数，供测试与基准。
pub struct NullSink {
    counter: Arc<AtomicU64>,
}

impl NullSink {
    pub fn new() -> (Self, Arc<AtomicU64>) {
        let counter = Arc::new(AtomicU64::new(0));
        (
            Self {
                counter: counter.clone(),
            },
            counter,
        )
    }
}

impl AudioSink for NullSink {
    fn write(&mut self, _spec: PcmSpec, samples: &[i32]) -> Result<()> {
        self.counter
            .fetch_add(samples.len() as u64, Ordering::Relaxed);
        Ok(())
    }
}

/// 按来源位深写 wav 文件，用于验证解码与 gapless 的正确性。
/// 格式在首次写入时锁定，中途变更报错。
pub struct WavSink {
    path: PathBuf,
    writer: Option<(PcmSpec, hound::WavWriter<std::io::BufWriter<std::fs::File>>)>,
}

impl WavSink {
    pub fn create(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            writer: None,
        }
    }
}

impl AudioSink for WavSink {
    fn write(&mut self, spec: PcmSpec, samples: &[i32]) -> Result<()> {
        if self.writer.is_none() {
            let bits = match spec.bits {
                16 => 16,
                24 => 24,
                _ => 32,
            };
            let wspec = hound::WavSpec {
                channels: spec.channels,
                sample_rate: spec.rate,
                bits_per_sample: bits,
                sample_format: hound::SampleFormat::Int,
            };
            let w = hound::WavWriter::create(&self.path, wspec)
                .map_err(|e| Error::Other(format!("创建 wav 失败: {e}")))?;
            self.writer = Some((spec, w));
        }
        let (locked, w) = self.writer.as_mut().unwrap();
        if *locked != spec {
            return Err(Error::SpecChanged(*locked, spec));
        }
        let shift = match spec.bits {
            16 => 16,
            24 => 8,
            _ => 0,
        };
        for &s in samples {
            let v = s >> shift;
            let res = if spec.bits == 16 {
                w.write_sample(v as i16)
            } else {
                w.write_sample(v)
            };
            res.map_err(|e| Error::Other(format!("写 wav 失败: {e}")))?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if let Some((_, w)) = self.writer.take() {
            w.finalize()
                .map_err(|e| Error::Other(format!("wav 收尾失败: {e}")))?;
        }
        Ok(())
    }
}
