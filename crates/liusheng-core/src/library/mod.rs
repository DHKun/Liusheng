pub mod db;
pub mod pinyin;
pub mod playlists;
mod scan;
mod scan_worker;
pub mod service;
pub mod tags;
pub mod watcher;

use crate::error::{Error, Result};
pub use db::{AlbumKey, AlbumSummary, ArtistSummary, TrackRow};
use rusqlite::Connection;
use std::cell::{Cell, RefCell};
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
    pub visited: usize,
    pub state_rows: usize,
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
        self.visited += other.visited;
        self.state_rows += other.state_rows;
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
    pub tracks: Arc<Vec<Arc<TrackRow>>>,
    pub albums: Arc<Vec<AlbumSummary>>,
    pub artists: Arc<Vec<ArtistSummary>>,
    pub album_tracks: Arc<HashMap<AlbumKey, Vec<usize>>>,
    pub artist_tracks: Arc<HashMap<String, Vec<usize>>>,
    pub album_indices: Arc<HashMap<AlbumKey, usize>>,
    pub artist_indices: Arc<HashMap<String, usize>>,
    pub album_search: Arc<HashMap<AlbumKey, Arc<str>>>,
    pub artist_search: Arc<HashMap<String, Arc<str>>>,
}

pub struct Library {
    pub(crate) conn: Connection,
    snapshot_cache: RefCell<Option<LibrarySnapshot>>,
    snapshot_version: Cell<u64>,
    changed_paths: RefCell<HashSet<String>>,
}
impl Library {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let conn = db::open(db_path)?;
        playlists::initialize(&conn)?;
        Ok(Self {
            conn,
            snapshot_cache: RefCell::new(None),
            snapshot_version: Cell::new(0),
            changed_paths: RefCell::new(HashSet::new()),
        })
    }
    pub fn snapshot(&self) -> Result<LibrarySnapshot> {
        let old = self.snapshot_cache.borrow();
        if self.snapshot_version.get() == self.conn.total_changes()
            && let Some(snapshot) = old.as_ref()
        {
            return Ok(snapshot.clone());
        }
        let changed = self.changed_paths.borrow();
        let mut tracks = if let Some(previous) = old
            .as_ref()
            .filter(|old| !changed.is_empty() && changed.len() < old.tracks.len() / 4)
        {
            let mut tracks = previous
                .tracks
                .iter()
                .filter(|t| !changed.contains(&t.path))
                .cloned()
                .collect::<Vec<_>>();
            for path in changed.iter() {
                if let Some(track) = db::track_by_path(&self.conn, path)? {
                    tracks.push(Arc::new(track));
                }
            }
            tracks.sort_by(|a, b| {
                (
                    &a.album_artist,
                    &a.album,
                    a.disc_no,
                    a.track_no,
                    &a.title,
                    &a.path,
                )
                    .cmp(&(
                        &b.album_artist,
                        &b.album,
                        b.disc_no,
                        b.track_no,
                        &b.title,
                        &b.path,
                    ))
            });
            tracks
        } else {
            self.all_tracks()?
                .into_iter()
                .map(Arc::new)
                .collect::<Vec<_>>()
        };
        // Reuse immutable track rows even after an initial import or a large update.
        if let Some(previous) = old.as_ref() {
            for (row, prior) in tracks.iter_mut().zip(previous.tracks.iter()) {
                if row == prior {
                    *row = prior.clone();
                }
            }
        }
        let mut album_tracks: HashMap<AlbumKey, Vec<usize>> = HashMap::new();
        let mut artist_tracks: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, t) in tracks.iter().enumerate() {
            album_tracks
                .entry(AlbumKey {
                    album: t.album.clone(),
                    album_artist: t.album_artist.clone(),
                })
                .or_default()
                .push(i);
            artist_tracks.entry(t.artist.clone()).or_default().push(i);
        }
        let albums = self.albums()?;
        let artists = self.artists()?;
        let album_indices = albums
            .iter()
            .enumerate()
            .map(|(i, a)| (a.key.clone(), i))
            .collect();
        let artist_indices = artists
            .iter()
            .enumerate()
            .map(|(i, a)| (a.key.clone(), i))
            .collect();
        // Pure text work happens on the library owner, once per changed summary.
        let album_search = albums
            .iter()
            .map(|a| {
                let reused = old.as_ref().and_then(|o| {
                    o.album_indices
                        .get(&a.key)
                        .and_then(|i| o.albums.get(*i))
                        .filter(|prior| prior.title == a.title && prior.artist == a.artist)
                        .and_then(|_| o.album_search.get(&a.key))
                        .cloned()
                });
                (
                    a.key.clone(),
                    reused.unwrap_or_else(|| {
                        Arc::from(pinyin::search_blob(&format!("{} {}", a.title, a.artist)))
                    }),
                )
            })
            .collect();
        let artist_search = artists
            .iter()
            .map(|a| {
                (
                    a.key.clone(),
                    old.as_ref()
                        .and_then(|o| o.artist_search.get(&a.key))
                        .cloned()
                        .unwrap_or_else(|| Arc::from(pinyin::search_blob(&a.name))),
                )
            })
            .collect();
        let snapshot = LibrarySnapshot {
            tracks: Arc::new(tracks),
            albums: Arc::new(albums),
            artists: Arc::new(artists),
            album_tracks: Arc::new(album_tracks),
            artist_tracks: Arc::new(artist_tracks),
            album_indices: Arc::new(album_indices),
            artist_indices: Arc::new(artist_indices),
            album_search: Arc::new(album_search),
            artist_search: Arc::new(artist_search),
        };
        drop(changed);
        drop(old);
        self.changed_paths.borrow_mut().clear();
        self.snapshot_version.set(self.conn.total_changes());
        self.snapshot_cache.replace(Some(snapshot.clone()));
        Ok(snapshot)
    }
    pub fn scan(&mut self, root: &Path) -> Result<ScanStats> {
        self.scan_with_progress(root, &[], |_, _| {})
    }

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
        progress: impl FnMut(&Self, &ScanStats),
    ) -> Result<ScanStats> {
        if !root.is_dir() {
            return Err(Error::Other(format!(
                "音乐目录离线或不可读：{}",
                root.display()
            )));
        }
        let plan = self.scan_plan(
            &[root.to_owned()],
            excluded,
            &[root.to_owned()],
            scan::ScanKind::Full,
        )?;
        self.execute_plan(plan, cancelled, progress)
    }

    /// Reconcile the smallest notified subtrees; exact file notifications query
    /// only that indexed path. The caller's cancellation flag reaches every batch.
    pub fn update_paths(
        &mut self,
        roots: &[PathBuf],
        excluded: &[PathBuf],
        paths: &[PathBuf],
    ) -> Result<ScanStats> {
        self.update_paths_controlled(
            roots,
            excluded,
            paths,
            &std::sync::atomic::AtomicBool::new(false),
        )
    }

    pub fn update_paths_controlled(
        &mut self,
        roots: &[PathBuf],
        excluded: &[PathBuf],
        paths: &[PathBuf],
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<ScanStats> {
        let plan = self.scan_plan(roots, excluded, paths, scan::ScanKind::Changes)?;
        self.execute_plan(plan, cancelled, |_, _| {})
    }

    pub fn import_files(&mut self, paths: &[PathBuf]) -> Result<ScanStats> {
        let plan = self.scan_plan(&[], &[], paths, scan::ScanKind::Import)?;
        self.execute_plan(plan, &std::sync::atomic::AtomicBool::new(false), |_, _| {})
    }

    fn scan_plan(
        &self,
        roots: &[PathBuf],
        excluded: &[PathBuf],
        paths: &[PathBuf],
        kind: scan::ScanKind,
    ) -> Result<scan::Plan> {
        let inputs = if kind == scan::ScanKind::Import {
            paths.to_vec()
        } else {
            scan::minimal_scopes(paths)
        };
        let mut scopes = Vec::new();
        for path in inputs {
            if excluded.iter().any(|e| path.starts_with(e)) {
                continue;
            }
            let root = if kind == scan::ScanKind::Import {
                path.clone()
            } else if let Some(root) = roots
                .iter()
                .filter(|r| path.starts_with(r))
                .max_by_key(|r| r.components().count())
            {
                root.clone()
            } else {
                continue;
            };
            let known = db::file_states_in_scope(&self.conn, &path)?;
            scopes.push(scan::Scope { path, root, known });
        }
        Ok(scan::Plan {
            scopes,
            excluded: excluded.to_vec(),
            kind,
        })
    }

    fn execute_plan(
        &mut self,
        plan: scan::Plan,
        cancelled: &std::sync::atomic::AtomicBool,
        mut progress: impl FnMut(&Self, &ScanStats),
    ) -> Result<ScanStats> {
        let mut total = ScanStats::default();
        scan::execute(plan, cancelled, |batch| {
            total.merge(self.apply_scan_batch(batch, cancelled)?);
            progress(self, &total);
            Ok(())
        })?;
        Ok(total)
    }

    fn apply_scan_batch(
        &mut self,
        batch: scan::Batch,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<ScanStats> {
        use std::sync::atomic::Ordering;
        if cancelled.load(Ordering::Acquire) {
            return Err(Error::Interrupted);
        }
        let mut stats = batch.stats;
        // Verify disappeared mounts outside the write transaction. Candidate
        // states are also compared to SQLite so a stale scan cannot undo an import.
        let removals = batch
            .removed
            .into_iter()
            .filter(|item| {
                item.root.is_dir()
                    && std::fs::metadata(&item.path)
                        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            })
            .collect::<Vec<_>>();
        let tx = self.conn.transaction()?;
        let mut changed_paths = Vec::new();
        for item in batch.prepared {
            if cancelled.load(Ordering::Acquire) {
                return Err(Error::Interrupted);
            }
            let existing = db::file_state_for_path(&tx, &item.path)?;
            if existing == Some(item.state) {
                stats.unchanged += 1;
                continue;
            }
            if !item.import && existing != item.expected {
                continue;
            }
            db::upsert_track(&tx, &item.path, item.state.0, &item.meta)?;
            db::set_file_size(&tx, &item.path, item.state.1)?;
            changed_paths.push(item.path);
            if existing.is_some() {
                stats.updated += 1;
            } else {
                stats.added += 1;
            }
        }
        for item in removals {
            if cancelled.load(Ordering::Acquire) {
                return Err(Error::Interrupted);
            }
            if db::file_state_for_path(&tx, &item.path)? == Some(item.expected) {
                db::delete_by_path(&tx, &item.path)?;
                changed_paths.push(item.path);
                stats.removed += 1;
            }
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(Error::Interrupted);
        }
        tx.commit()?;
        self.changed_paths.borrow_mut().extend(changed_paths);
        Ok(stats)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackRow>> {
        db::search(&self.conn, query, limit)
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
