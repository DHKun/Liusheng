pub mod db;
pub mod pinyin;
pub mod tags;

use std::collections::HashSet;
use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;
pub use db::TrackRow;

/// 扫描收录的扩展名，与 Symphonia 开启的解码 feature 对应。
const AUDIO_EXTS: &[&str] = &["flac", "mp3", "m4a", "ogg", "wav", "aiff", "aif"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub failed: usize,
}

pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(db_path: &Path) -> Result<Self> {
        Ok(Self {
            conn: db::open(db_path)?,
        })
    }

    /// 增量扫描：新文件入库，mtime 变化的重读，磁盘上消失的删除。
    pub fn scan(&mut self, root: &Path) -> Result<ScanStats> {
        let mut stats = ScanStats::default();
        let known = db::path_mtimes(&self.conn)?;
        let mut seen: HashSet<String> = HashSet::new();
        let tx = self.conn.transaction()?;

        for entry in walkdir::WalkDir::new(root).follow_links(true) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    stats.failed += 1;
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let is_audio = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
                .unwrap_or(false);
            if !is_audio {
                continue;
            }
            let path_str = path.to_string_lossy().into_owned();
            let mtime = tags::file_mtime_secs(path);
            seen.insert(path_str.clone());
            let existing = known.get(&path_str);
            if existing == Some(&mtime) {
                stats.unchanged += 1;
                continue;
            }
            match tags::read_meta(path) {
                Ok(meta) => {
                    db::upsert_track(&tx, &path_str, mtime, &meta)?;
                    if existing.is_some() {
                        stats.updated += 1;
                    } else {
                        stats.added += 1;
                    }
                }
                Err(_) => stats.failed += 1,
            }
        }

        for path in known.keys() {
            if !seen.contains(path) {
                db::delete_by_path(&tx, path)?;
                stats.removed += 1;
            }
        }
        tx.commit()?;
        Ok(stats)
    }

    /// 支持原文、全拼、首字母三种输入形式的搜索。
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackRow>> {
        db::search(&self.conn, &pinyin::normalize(query), limit)
    }

    pub fn all_tracks(&self) -> Result<Vec<TrackRow>> {
        db::all_tracks(&self.conn)
    }

    pub fn track_count(&self) -> Result<u64> {
        db::track_count(&self.conn)
    }
}
