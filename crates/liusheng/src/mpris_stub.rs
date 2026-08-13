//! 非 Linux 平台的媒体控制占位实现。

use std::sync::mpsc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    #[default]
    Stopped,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaybackSnapshot {
    pub status: PlaybackStatus,
    pub has_track: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: String,
    pub path: String,
    pub duration_us: i64,
    pub position_us: i64,
    pub track_number: Option<u32>,
    pub queue_index: usize,
    pub queue_len: usize,
    pub seekable: bool,
    pub hardware_volume_available: bool,
    pub hardware_volume_percent: u8,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Command {
    Next,
    Previous,
    Pause,
    PlayPause,
    Stop,
    Play,
    SeekRelative(i64),
    SeekAbsolute(i64),
    SetVolume(f64),
}

pub struct Service;

impl Service {
    #[allow(clippy::unnecessary_wraps)]
    pub fn start() -> Result<(Self, mpsc::Receiver<Command>), String> {
        let (_commands, receiver) = mpsc::channel();
        Ok((Self, receiver))
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn publish(&self, _snapshot: PlaybackSnapshot) -> Result<(), String> {
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn seeked(&self, _position_us: i64) -> Result<(), String> {
        Ok(())
    }
}
