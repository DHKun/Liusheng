//! 非 Linux 平台的媒体控制占位实现。

use std::sync::mpsc;

pub use crate::media_types::{Command, PlaybackSnapshot, PlaybackStatus};

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
