use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase};

use crate::audio::PcmSpec;
use crate::error::{Error, Result};

/// 单个音频文件的解码器，逐块产出交错 i32 满量程样本。
pub struct AudioFileDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    spec: PcmSpec,
    time_base: Option<TimeBase>,
    duration_secs: Option<f64>,
}

impl AudioFileDecoder {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe().probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )?;

        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| Error::NoAudioTrack(path.to_path_buf()))?;
        let track_id = track.id;
        let time_base = track.time_base;
        let num_frames = track.num_frames;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or_else(|| Error::NoAudioTrack(path.to_path_buf()))?;

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())?;

        let cp = decoder.codec_params();
        let rate = cp.sample_rate.or(params.sample_rate).unwrap_or(0);
        if rate == 0 {
            return Err(Error::NoAudioTrack(path.to_path_buf()));
        }
        let channels = cp
            .channels
            .as_ref()
            .or(params.channels.as_ref())
            .map(|c| c.count())
            .unwrap_or(2) as u16;
        // 无位深信息的来源（浮点解码等）按 32 处理，输出端不移位
        let bits = cp.bits_per_sample.or(params.bits_per_sample).unwrap_or(32) as u16;
        let duration_secs = num_frames.map(|n| n as f64 / rate as f64);

        Ok(Self {
            format,
            decoder,
            track_id,
            spec: PcmSpec {
                rate,
                channels,
                bits,
            },
            time_base,
            duration_secs,
        })
    }

    pub fn spec(&self) -> PcmSpec {
        self.spec
    }

    pub fn duration_secs(&self) -> Option<f64> {
        self.duration_secs
    }

    /// 解码下一段样本，覆盖写入 out。返回 false 表示整曲播完。
    pub fn next_into(&mut self, out: &mut Vec<i32>) -> Result<bool> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => return Ok(false),
                Err(SymError::ResetRequired) => return Ok(false),
                // 截断的文件在尾部会以 UnexpectedEof 收场，按正常结束处理
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(false);
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buf) => {
                    if buf.is_empty() {
                        continue;
                    }
                    buf.copy_to_vec_interleaved(out);
                    return Ok(true);
                }
                // 单包损坏跳过，不中断整曲
                Err(SymError::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// 精确 seek，返回实际落点秒数。
    pub fn seek_secs(&mut self, secs: f64) -> Result<f64> {
        let time = Time::try_from_secs_f64(secs.max(0.0))
            .ok_or_else(|| Error::Other(format!("无效的 seek 目标: {secs}")))?;
        let seeked = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;
        self.decoder.reset();
        let actual = self
            .time_base
            .and_then(|tb| tb.calc_time(seeked.actual_ts))
            .map(|t| t.as_secs_f64())
            .unwrap_or(secs);
        Ok(actual)
    }
}
