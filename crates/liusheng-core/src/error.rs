use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("解码错误: {0}")]
    Decode(#[from] symphonia::core::errors::Error),
    #[error("标签读取错误: {0}")]
    Tag(#[from] lofty::error::LoftyError),
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("没有可解码的音频轨: {0}")]
    NoAudioTrack(PathBuf),
    #[error("输出端不支持中途更换格式: {0:?} -> {1:?}")]
    SpecChanged(crate::audio::PcmSpec, crate::audio::PcmSpec),
    #[error("{0}")]
    Other(String),
}
