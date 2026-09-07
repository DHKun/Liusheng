//! Versioned application preferences and atomic, private session storage.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub version: u32,
    pub music_roots: Vec<PathBuf>,
    pub excluded_directories: Vec<PathBuf>,
    pub exclusive_device: String,
    pub mixer_device: String,
    pub mixer_element: String,
    pub prefer_exclusive: bool,
    pub close_to_tray: bool,
    pub restore_session: bool,
    pub lyric_offsets: HashMap<String, i32>,
}
impl Default for AppSettings {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let root = if cfg!(target_os = "linux") && Path::new("/data/Music").is_dir() {
            PathBuf::from("/data/Music")
        } else {
            home.join("Music")
        };
        Self {
            version: 1,
            music_roots: vec![root],
            excluded_directories: Vec::new(),
            exclusive_device: "hw:Hybrid,0".into(),
            mixer_device: "hw:Hybrid".into(),
            mixer_element: "PCM".into(),
            prefer_exclusive: false,
            close_to_tray: true,
            restore_session: true,
            lyric_offsets: HashMap::new(),
        }
    }
}
impl AppSettings {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let value: Self = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::Other(format!("设置读取失败：{e}")))?;
                if value.version > 1 {
                    return Err(Error::Other(
                        "设置文件来自较新版本，请先备份并使用对应版本打开".into(),
                    ));
                }
                Ok(value)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        save_json(path, self)
    }
    pub fn validate(&self) -> Result<()> {
        if self
            .music_roots
            .iter()
            .chain(&self.excluded_directories)
            .any(|p| !p.is_absolute())
        {
            return Err(Error::Other("音乐目录和排除目录须使用绝对路径".into()));
        }
        if self.exclusive_device.trim().is_empty()
            || self.mixer_device.trim().is_empty()
            || self.mixer_element.trim().is_empty()
        {
            return Err(Error::Other("输出设备和音量控件名称须填写完整".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedSession {
    pub version: u32,
    pub queue: std::sync::Arc<Vec<String>>,
    pub current_index: usize,
    pub position_ms: i32,
    pub repeat_mode: u8,
    pub shuffle: bool,
    pub page: String,
    pub width: i32,
    pub height: i32,
    pub album_scroll: f64,
    pub track_scroll: f64,
}
impl SavedSession {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let session: Self = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::Other(format!("播放会话读取失败：{e}")))?;
                if session.version > 1 {
                    return Err(Error::Other(
                        "播放会话来自较新的应用版本，请保留原文件并使用对应版本".into(),
                    ));
                }
                Ok(session)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        save_json(path, self)
    }
}

pub fn save_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let data = serde_json::to_vec_pretty(value).map_err(|e| Error::Other(e.to_string()))?;
    atomic_write(path, &data)
}

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| Error::Other("文件路径缺少父目录".into()))?;
    std::fs::create_dir_all(parent)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = parent.join(format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(data)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        #[cfg(unix)]
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub database: PathBuf,
    pub settings: PathBuf,
    pub session: PathBuf,
    pub covers: PathBuf,
}
impl AppPaths {
    pub fn discover() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| Error::Other("无法确定用户目录".into()))?;
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("liusheng");
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("liusheng");
        let cache = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"))
            .join("liusheng/covers");
        // Retain existing data locations on every platform for upgrade compatibility.
        Ok(Self {
            database: data.join("library.db"),
            settings: config.join("settings.json"),
            session: data.join("session.json"),
            covers: cache,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_and_session_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = AppSettings {
            close_to_tray: false,
            ..Default::default()
        };
        settings.save(&path).unwrap();
        assert!(!AppSettings::load(&path).unwrap().close_to_tray);
        let session = SavedSession {
            queue: std::sync::Arc::new(vec!["/音乐/a.flac".into()]),
            position_ms: 1234,
            ..Default::default()
        };
        session.save(&path).unwrap();
        assert_eq!(SavedSession::load(&path).unwrap().position_ms, 1234);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn malformed_settings_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"broken").unwrap();
        assert!(AppSettings::load(&path).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"broken");
    }
}
