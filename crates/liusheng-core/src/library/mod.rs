pub mod db;
pub mod pinyin;
pub mod playlists;
pub mod service;
pub mod tags;
pub mod watcher;

use crate::error::{Error, Result};
pub use db::{AlbumKey, AlbumSummary, ArtistSummary, TrackRow};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const AUDIO_EXTS: &[&str] = &["flac", "mp3", "m4a", "ogg", "wav", "aiff", "aif"];
pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| AUDIO_EXTS.contains(&s.to_ascii_lowercase().as_str()))
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}
impl ScanStats {
    fn failure(&mut self, path: &Path, error: impl std::fmt::Display) {
        self.failed += 1;
        if self.errors.len() < 100 {
            self.errors.push(format!("{}：{error}", path.display()));
        }
    }
    pub fn merge(&mut self, other: Self) {
        self.added += other.added;
        self.updated += other.updated;
        self.removed += other.removed;
        self.unchanged += other.unchanged;
        self.failed += other.failed;
        self.errors.extend(
            other
                .errors
                .into_iter()
                .take(100usize.saturating_sub(self.errors.len())),
        );
    }
    pub fn changed(&self) -> bool {
        self.added + self.updated + self.removed > 0
    }
}

/// Immutable, shareable UI snapshot. Every lookup index is built once on the library worker.
#[derive(Debug, Clone, Default)]
pub struct LibrarySnapshot {
    pub tracks: Vec<Arc<TrackRow>>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
    pub album_tracks: HashMap<AlbumKey, Vec<usize>>,
    pub artist_tracks: HashMap<String, Vec<usize>>,
    pub search_blobs: Arc<Vec<String>>,
}

pub struct Library {
    pub(crate) conn: Connection,
}
impl Library {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let conn = db::open(db_path)?;
        playlists::initialize(&conn)?;
        Ok(Self { conn })
    }
    pub fn snapshot(&self) -> Result<LibrarySnapshot> {
        let tracks = self
            .all_tracks()?
            .into_iter()
            .map(Arc::new)
            .collect::<Vec<_>>();
        let mut album_tracks: HashMap<AlbumKey, Vec<usize>> = HashMap::new();
        let mut artist_tracks: HashMap<String, Vec<usize>> = HashMap::new();
        let mut search_blobs = Vec::with_capacity(tracks.len());
        for (i, t) in tracks.iter().enumerate() {
            album_tracks
                .entry(AlbumKey {
                    album: t.album.clone(),
                    album_artist: t.album_artist.clone(),
                })
                .or_default()
                .push(i);
            artist_tracks.entry(t.artist.clone()).or_default().push(i);
            search_blobs.push(t.search_text.clone());
        }
        Ok(LibrarySnapshot {
            tracks,
            albums: self.albums()?,
            artists: self.artists()?,
            album_tracks,
            artist_tracks,
            search_blobs: Arc::new(search_blobs),
        })
    }
    pub fn scan(&mut self, root: &Path) -> Result<ScanStats> {
        self.scan_with_progress(root, &[], |_, _| {})
    }
    /// Commit bounded metadata batches; file I/O happens outside SQLite write transactions.
    /// Entries outside this root and under failed subtrees retain their cached metadata.
    pub fn scan_with_progress(
        &mut self,
        root: &Path,
        excluded: &[PathBuf],
        progress: impl FnMut(&Self, &ScanStats),
    ) -> Result<ScanStats> {
        self.scan_controlled(
            root,
            excluded,
            &std::sync::atomic::AtomicBool::new(false),
            progress,
        )
    }
    pub fn scan_controlled(
        &mut self,
        root: &Path,
        excluded: &[PathBuf],
        cancelled: &std::sync::atomic::AtomicBool,
        mut progress: impl FnMut(&Self, &ScanStats),
    ) -> Result<ScanStats> {
        if !root.is_dir() {
            return Err(Error::Other(format!(
                "音乐目录离线或不可读：{}",
                root.display()
            )));
        }
        let known = db::file_states(&self.conn)?;
        let mut stats = ScanStats::default();
        let mut seen = HashSet::new();
        let mut failed_scopes = Vec::new();
        let mut batch = Vec::new();
        let walker = walkdir::WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| !excluded.iter().any(|p| e.path().starts_with(p)));
        for entry in walker {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Err(Error::Other("扫描已取消".into()));
            }
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    let path = e.path().unwrap_or(root).to_owned();
                    stats.failure(&path, &e);
                    failed_scopes.push(path);
                    continue;
                }
            };
            if !entry.file_type().is_file() || !is_audio_path(entry.path()) {
                continue;
            }
            let path = entry.path();
            let key = path.to_string_lossy().into_owned();
            seen.insert(key.clone());
            self.prepare_file(path, &known, &mut batch, &mut stats);
            if batch.len() >= 128 {
                self.commit_batch(&mut batch)?;
                progress(self, &stats);
            }
        }
        self.commit_batch(&mut batch)?;
        let tx = self.conn.transaction()?;
        for path in known.keys() {
            let p = Path::new(path);
            if p.starts_with(root)
                && !seen.contains(path)
                && !excluded.iter().any(|e| p.starts_with(e))
                && !failed_scopes.iter().any(|e| p.starts_with(e))
            {
                db::delete_by_path(&tx, path)?;
                stats.removed += 1;
            }
        }
        tx.commit()?;
        progress(self, &stats);
        Ok(stats)
    }
    /// Update a de-duplicated set of filesystem paths. Directory events request a scoped scan.
    pub fn update_paths(
        &mut self,
        roots: &[PathBuf],
        excluded: &[PathBuf],
        paths: &[PathBuf],
    ) -> Result<ScanStats> {
        let known = db::file_states(&self.conn)?;
        let mut stats = ScanStats::default();
        let mut batch = Vec::new();
        let mut rescans = HashSet::new();
        for path in paths.iter().collect::<HashSet<_>>() {
            let Some(root) = roots.iter().find(|r| path.starts_with(r)) else {
                continue;
            };
            if !root.is_dir() {
                stats.failure(root, "目录离线，保留缓存");
                continue;
            }
            if excluded.iter().any(|e| path.starts_with(e)) {
                continue;
            }
            if path.is_dir()
                || (!is_audio_path(path) && known.keys().any(|k| Path::new(k).starts_with(path)))
            {
                rescans.insert(root.clone());
                continue;
            }
            if !is_audio_path(path) {
                continue;
            }
            match std::fs::metadata(path) {
                Ok(m) if m.is_file() => self.prepare_file(path, &known, &mut batch, &mut stats),
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let key = path.to_string_lossy();
                    if known.contains_key(key.as_ref()) {
                        db::delete_by_path(&self.conn, &key)?;
                        stats.removed += 1;
                    }
                }
                Err(e) => stats.failure(path, e),
            }
        }
        self.commit_batch(&mut batch)?;
        for root in rescans {
            stats.merge(self.scan_with_progress(&root, excluded, |_, _| {})?);
        }
        Ok(stats)
    }
    /// Import explicitly opened local files without traversing unrelated library roots.
    pub fn import_files(&mut self, paths: &[PathBuf]) -> Result<ScanStats> {
        let known = db::file_states(&self.conn)?;
        let mut batch = Vec::new();
        let mut stats = ScanStats::default();
        for path in paths {
            if is_audio_path(path) {
                self.prepare_file(path, &known, &mut batch, &mut stats);
            }
        }
        self.commit_batch(&mut batch)?;
        Ok(stats)
    }
    fn prepare_file(
        &self,
        path: &Path,
        known: &HashMap<String, (i64, i64)>,
        batch: &mut Vec<(String, i64, i64, tags::TrackMeta)>,
        stats: &mut ScanStats,
    ) {
        let before = match file_state(path) {
            Ok(s) => s,
            Err(e) => {
                stats.failure(path, e);
                return;
            }
        };
        let key = path.to_string_lossy().into_owned();
        let previous = known.get(&key);
        if previous == Some(&before) {
            stats.unchanged += 1;
            return;
        }
        match tags::read_meta(path) {
            Ok(meta) => {
                if file_state(path).ok() != Some(before) {
                    stats.failure(path, "文件仍在写入，将在下一次更新重试");
                    return;
                }
                batch.push((key, before.0, before.1, meta));
                if previous.is_some() {
                    stats.updated += 1
                } else {
                    stats.added += 1
                }
            }
            Err(e) => stats.failure(path, e),
        }
    }
    fn commit_batch(&mut self, batch: &mut Vec<(String, i64, i64, tags::TrackMeta)>) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        for (path, mtime, size, meta) in batch.iter() {
            db::upsert_track(&tx, path, *mtime, meta)?;
            db::set_file_size(&tx, path, *size)?;
        }
        tx.commit()?;
        batch.clear();
        Ok(())
    }
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackRow>> {
        db::search(&self.conn, &pinyin::normalize(query), limit)
    }
    pub fn all_tracks(&self) -> Result<Vec<TrackRow>> {
        db::all_tracks(&self.conn)
    }
    pub fn track_count(&self) -> Result<u64> {
        db::track_count(&self.conn)
    }
    pub fn albums(&self) -> Result<Vec<AlbumSummary>> {
        db::albums(&self.conn)
    }
    pub fn artists(&self) -> Result<Vec<ArtistSummary>> {
        db::artists(&self.conn)
    }
    pub fn playlists(&self) -> Result<Vec<playlists::Playlist>> {
        playlists::list(&self.conn)
    }
    pub fn playlist_paths(&self, id: i64) -> Result<Vec<String>> {
        playlists::entries(&self.conn, id)
    }
    pub fn save_playlist(&mut self, name: &str, paths: &[String]) -> Result<i64> {
        playlists::save(&mut self.conn, name, paths)
    }
    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        playlists::rename(&self.conn, id, name)
    }
    pub fn delete_playlist(&mut self, id: i64) -> Result<()> {
        playlists::delete(&mut self.conn, id)
    }
}
fn file_state(path: &Path) -> std::io::Result<(i64, i64)> {
    let m = std::fs::metadata(path)?;
    let ns = m
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_nanos();
    Ok((
        ns.min(i64::MAX as u128) as i64,
        m.len().min(i64::MAX as u64) as i64,
    ))
}
