//! Shared state and commands for platform media-control adapters.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    #[default]
    Stopped,
}

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
    pub repeat_mode: u8,
    pub shuffle: bool,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, PartialEq)]
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
    SetRepeatMode(u8),
    SetShuffle(bool),
    OpenUri(String),
    Raise,
    Quit,
    ServiceError(String),
}
