use std::path::Path;
use std::time::UNIX_EPOCH;

use lofty::prelude::*;
use lofty::tag::ItemKey;

#[derive(Debug, Clone, Default)]
pub struct TrackMeta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub genre: String,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub bit_depth: Option<u8>,
    pub channels: u8,
}

/// 读标签与技术属性。无标签时标题回退为文件名。
pub fn read_meta(path: &Path) -> crate::Result<TrackMeta> {
    let tagged = lofty::read_from_path(path)?;
    let props = tagged.properties();
    let mut meta = TrackMeta {
        duration_ms: props.duration().as_millis() as u64,
        sample_rate: props.sample_rate().unwrap_or(0),
        bit_depth: props.bit_depth(),
        channels: props.channels().unwrap_or(2),
        ..Default::default()
    };
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        meta.title = tag.title().map(|s| s.to_string()).unwrap_or_default();
        meta.artist = tag.artist().map(|s| s.to_string()).unwrap_or_default();
        meta.album = tag.album().map(|s| s.to_string()).unwrap_or_default();
        meta.album_artist = tag
            .get_string(ItemKey::AlbumArtist)
            .map(str::to_string)
            .unwrap_or_default();
        meta.track_no = tag.track();
        meta.disc_no = tag.disk();
        // 年份可能存成 "2003" 或完整日期 "2003-11-13"，取前四位数字
        meta.year = tag
            .get_string(ItemKey::Year)
            .or_else(|| tag.get_string(ItemKey::RecordingDate))
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse().ok());
        meta.genre = tag.genre().map(|s| s.to_string()).unwrap_or_default();
    }
    if meta.title.is_empty() {
        meta.title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("未知标题")
            .to_string();
    }
    Ok(meta)
}

pub fn file_mtime_nanos(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
