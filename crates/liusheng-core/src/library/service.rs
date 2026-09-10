//! One library actor owns SQLite and recursive watchers. UI messages contain immutable snapshots.
use super::{
    Library, LibrarySnapshot, ScanStats,
    playlists::Playlist,
    watcher::{LibraryWatchEvent, LibraryWatcher},
};
use super::{
    scan::{self, ScanKind},
    scan_worker::{self, Message as ScanMessage},
};
use crate::settings::{AppPaths, AppSettings, SavedSession};
use crossbeam_channel::{Receiver, Sender, bounded, never, select_biased, tick, unbounded};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub enum LibraryCommand {
    Refresh,
    CancelScan,
    DiscoverDevices,
    Settings(AppSettings),
    SavePlaylist { name: String, paths: Vec<String> },
    RenamePlaylist { id: i64, name: String },
    DeletePlaylist(i64),
    LoadPlaylist(i64),
    ImportPlaylist(PathBuf),
    ExportPlaylist { path: PathBuf, paths: Vec<String> },
    OpenFiles(Vec<PathBuf>),
    Quit,
}
#[derive(Debug)]
pub enum LibraryEvent {
    Ready {
        settings: Box<AppSettings>,
        session: Box<SavedSession>,
        snapshot: Box<LibrarySnapshot>,
    },
    Snapshot {
        snapshot: Box<LibrarySnapshot>,
        stats: ScanStats,
        finished: bool,
    },
    Scanning,
    ScanFinished {
        stats: ScanStats,
        cancelled: bool,
    },
    Devices(String),
    InvalidateArtwork(Vec<PathBuf>),
    LyricsChanged(Vec<PathBuf>),
    Playlists(Vec<Playlist>),
    PlayPaths(Vec<String>),
    SettingsSaved(AppSettings),
    Notice(String),
    Error(String),
}
pub struct LibraryService {
    commands: Sender<LibraryCommand>,
    events: Receiver<LibraryEvent>,
    session: Arc<Mutex<Option<SavedSession>>>,
    cancelled: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl LibraryService {
    pub fn start(paths: AppPaths) -> std::io::Result<Self> {
        Self::start_inner(paths, Arc::new(super::tags::read_meta))
    }

    fn start_inner(paths: AppPaths, reader: scan::MetadataReader) -> std::io::Result<Self> {
        let (commands, rx) = bounded(32);
        let (events_tx, events) = unbounded();
        let session = Arc::new(Mutex::new(None));
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_session = session.clone();
        let stop = cancelled.clone();
        let worker = std::thread::Builder::new()
            .name("liusheng-library".into())
            .spawn(move || {
                if let Err(e) = run(
                    paths.clone(),
                    rx,
                    &events_tx,
                    &worker_session,
                    &stop,
                    reader,
                ) {
                    let _ = events_tx.send(LibraryEvent::Error(e.to_string()));
                }
                flush_session(&paths, &worker_session, &events_tx);
            })?;
        Ok(Self {
            commands,
            events,
            session,
            cancelled,
            worker: Some(worker),
        })
    }
    pub fn send(&self, command: LibraryCommand) -> bool {
        self.commands.try_send(command).is_ok()
    }
    pub fn events(&self) -> Receiver<LibraryEvent> {
        self.events.clone()
    }
    /// Coalesces frequent progress updates without filling the command queue.
    pub fn save_session(&self, session: SavedSession) {
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(session);
    }
}
impl Drop for LibraryService {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.try_send(LibraryCommand::Quit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn flush_session(
    paths: &AppPaths,
    pending: &Mutex<Option<SavedSession>>,
    events: &Sender<LibraryEvent>,
) {
    let session = pending.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(session) = session
        && let Err(e) = session.save(&paths.session)
    {
        // Retain the failed checkpoint unless a newer one arrived during I/O.
        let mut pending = pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.is_none() {
            *pending = Some(session);
        }
        let _ = events.send(LibraryEvent::Error(e.to_string()));
    }
}

fn watcher(settings: &AppSettings, events: &Sender<LibraryEvent>) -> Option<LibraryWatcher> {
    let roots = settings
        .music_roots
        .iter()
        .filter(|p| p.is_dir())
        .cloned()
        .collect::<Vec<_>>();
    match LibraryWatcher::start_many(&roots) {
        Ok(w) => Some(w),
        Err(e) => {
            let _ = events.send(LibraryEvent::Error(format!("目录监听：{e}")));
            None
        }
    }
}
fn publish(
    library: &Library,
    events: &Sender<LibraryEvent>,
    stats: ScanStats,
    finished: bool,
) -> crate::Result<()> {
    events
        .send(LibraryEvent::Snapshot {
            snapshot: Box::new(library.snapshot()?),
            stats,
            finished,
        })
        .ok();
    Ok(())
}
struct ActiveScan {
    id: u64,
    kind: ScanKind,
    cancelled: Arc<AtomicBool>,
    stats: ScanStats,
    opened: Vec<String>,
    dirty: bool,
    started: Instant,
    last_publish: Instant,
}

impl Drop for ActiveScan {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

fn run(
    paths: AppPaths,
    commands: Receiver<LibraryCommand>,
    events: &Sender<LibraryEvent>,
    session: &Mutex<Option<SavedSession>>,
    cancelled: &AtomicBool,
    reader: scan::MetadataReader,
) -> crate::Result<()> {
    let started = Instant::now();
    let mut settings = match AppSettings::load(&paths.settings) {
        Ok(s) => s,
        Err(e) => {
            events.send(LibraryEvent::Error(e.to_string())).ok();
            AppSettings::default()
        }
    };
    let saved = match SavedSession::load(&paths.session) {
        Ok(s) => s,
        Err(e) => {
            events.send(LibraryEvent::Error(e.to_string())).ok();
            SavedSession::default()
        }
    };
    let mut library = Library::open(&paths.database)?;
    events
        .send(LibraryEvent::Ready {
            settings: Box::new(settings.clone()),
            session: Box::new(saved),
            snapshot: Box::new(library.snapshot()?),
        })
        .ok();
    events
        .send(LibraryEvent::Playlists(library.playlists()?))
        .ok();
    if std::env::var_os("LIUSHENG_PROFILE").is_some() {
        eprintln!("[perf] cached_library_ms={}", started.elapsed().as_millis());
    }
    let scanner = scan_worker::Worker::start(reader)?;
    let scan_events = scanner.events.clone();
    let mut watching = watcher(&settings, events);
    let mut watch_events = watching.as_ref().map(|w| w.events()).unwrap_or_else(never);
    let timer = tick(Duration::from_millis(200));
    let initial_deadline = Instant::now() + Duration::from_millis(200);
    let mut full_pending = true;
    let mut changed_paths: Vec<PathBuf> = Vec::new();
    let mut imports: VecDeque<Vec<PathBuf>> = VecDeque::new();
    let mut active: Option<ActiveScan> = None;
    let mut generation = 0u64;
    let mut roots_online = settings
        .music_roots
        .iter()
        .map(|p| p.is_dir())
        .collect::<Vec<_>>();
    let mut last_mount_check = Instant::now();
    let mut last_session_write = Instant::now();
    while !cancelled.load(Ordering::Acquire) {
        // Deadlines are evaluated for every message as well as timer ticks.
        // A continuously ready watcher/command queue cannot postpone a checkpoint.
        if last_session_write.elapsed() >= Duration::from_secs(2) {
            flush_session(&paths, session, events);
            last_session_write = Instant::now();
        }
        if last_mount_check.elapsed() >= Duration::from_secs(15) {
            last_mount_check = Instant::now();
            let online = settings
                .music_roots
                .iter()
                .map(|p| p.is_dir())
                .collect::<Vec<_>>();
            if online != roots_online {
                roots_online = online;
                watching = watcher(&settings, events);
                watch_events = watching.as_ref().map(|w| w.events()).unwrap_or_else(never);
                full_pending = true;
            }
        }
        if active.is_none() && (Instant::now() >= initial_deadline || !imports.is_empty()) {
            let request = if let Some(paths) = imports.pop_front() {
                Some((ScanKind::Import, paths))
            } else if full_pending {
                full_pending = false;
                changed_paths.clear();
                Some((ScanKind::Full, settings.music_roots.clone()))
            } else if !changed_paths.is_empty() {
                Some((ScanKind::Changes, std::mem::take(&mut changed_paths)))
            } else {
                None
            };
            if let Some((kind, inputs)) = request {
                let exclusions = if kind == ScanKind::Import {
                    &[][..]
                } else {
                    &settings.excluded_directories
                };
                let plan = library.scan_plan(&settings.music_roots, exclusions, &inputs, kind)?;
                let job_cancelled = Arc::new(AtomicBool::new(false));
                generation = generation.wrapping_add(1);
                scanner.submit(scan_worker::Job {
                    id: generation,
                    plan,
                    cancelled: job_cancelled.clone(),
                })?;
                active = Some(ActiveScan {
                    id: generation,
                    kind,
                    cancelled: job_cancelled,
                    stats: ScanStats::default(),
                    opened: Vec::new(),
                    dirty: false,
                    started: Instant::now(),
                    last_publish: Instant::now(),
                });
                events.send(LibraryEvent::Scanning).ok();
            }
        }
        select_biased! {
            recv(timer) -> _ => {},
            recv(commands) -> command => {
                let Ok(command) = command else { break; };
                let result = (|| -> crate::Result<()> {
                    match command {
                        LibraryCommand::Quit => { cancelled.store(true, Ordering::Release); }
                        LibraryCommand::Refresh => { full_pending = true; }
                        LibraryCommand::CancelScan => {
                            full_pending = false;
                            changed_paths.clear();
                            imports.clear();
                            if let Some(task) = &active { task.cancelled.store(true, Ordering::Release); }
                            else { events.send(LibraryEvent::ScanFinished { stats: ScanStats::default(), cancelled: true }).ok(); }
                        }
                        LibraryCommand::DiscoverDevices => {
                            events.send(LibraryEvent::Devices(serde_json::to_string(&crate::devices::output_devices()).unwrap_or_else(|_| "[]".into()))).ok();
                        }
                        LibraryCommand::Settings(mut value) => {
                            value.validate()?;
                            value.music_roots.sort(); value.music_roots.dedup();
                            let roots_changed = value.music_roots != settings.music_roots || value.excluded_directories != settings.excluded_directories;
                            value.save(&paths.settings)?;
                            settings = value;
                            events.send(LibraryEvent::SettingsSaved(settings.clone())).ok();
                            if roots_changed {
                                if let Some(task) = active.as_ref().filter(|t| t.kind != ScanKind::Import) { task.cancelled.store(true, Ordering::Release); }
                                watching = watcher(&settings, events);
                                watch_events = watching.as_ref().map(|w| w.events()).unwrap_or_else(never);
                                roots_online = settings.music_roots.iter().map(|p| p.is_dir()).collect();
                                full_pending = true;
                            }
                        }
                        LibraryCommand::SavePlaylist { name, paths } => {
                            library.save_playlist(&name, &paths)?;
                            events.send(LibraryEvent::Playlists(library.playlists()?)).ok();
                        }
                        LibraryCommand::RenamePlaylist { id, name } => {
                            library.rename_playlist(id, &name)?;
                            events.send(LibraryEvent::Playlists(library.playlists()?)).ok();
                        }
                        LibraryCommand::DeletePlaylist(id) => {
                            library.delete_playlist(id)?;
                            events.send(LibraryEvent::Playlists(library.playlists()?)).ok();
                        }
                        LibraryCommand::LoadPlaylist(id) => { events.send(LibraryEvent::PlayPaths(library.playlist_paths(id)?)).ok(); }
                        LibraryCommand::ImportPlaylist(path) => {
                            let entries = super::playlists::read_m3u(&path)?;
                            library.save_playlist(path.file_stem().and_then(|s| s.to_str()).unwrap_or("导入歌单"), &entries)?;
                            events.send(LibraryEvent::Playlists(library.playlists()?)).ok();
                        }
                        LibraryCommand::ExportPlaylist { path, paths } => {
                            super::playlists::write_m3u(&path, &paths)?;
                            events.send(LibraryEvent::Notice("歌单已导出".into())).ok();
                        }
                        LibraryCommand::OpenFiles(inputs) => {
                            if imports.len() >= 32 { return Err(crate::Error::Other("待打开任务已满，请在导入完成后重试".into())); }
                            imports.push_back(inputs);
                            if let Some(task) = active.as_ref().filter(|t| t.kind != ScanKind::Import) {
                                task.cancelled.store(true, Ordering::Release);
                                full_pending = true;
                            }
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result { events.send(LibraryEvent::Error(error.to_string())).ok(); }
            }
            recv(scan_events) -> message => {
                match message {
                    Ok(ScanMessage::Batch(id, mut batch)) => {
                        if let Some(task) = active.as_mut().filter(|t| t.id == id && !t.cancelled.load(Ordering::Acquire)) {
                            task.opened.append(&mut batch.opened);
                            match library.apply_scan_batch(batch, &task.cancelled) {
                                Ok(stats) => { task.dirty |= stats.changed(); task.stats.merge(stats); }
                                Err(error) => {
                                    task.cancelled.store(true, Ordering::Release);
                                    events.send(LibraryEvent::Error(error.to_string())).ok();
                                }
                            }
                            if task.dirty && task.last_publish.elapsed() >= Duration::from_millis(400) {
                                publish(&library, events, task.stats.clone(), false)?;
                                task.last_publish = Instant::now();
                                task.dirty = false;
                            }
                        }
                    }
                    Ok(ScanMessage::Finished(id, result)) => {
                        if active.as_ref().is_some_and(|t| t.id == id) {
                            let mut task = active.take().unwrap();
                            let was_cancelled = task.cancelled.load(Ordering::Acquire) || matches!(result, Err(crate::Error::Interrupted));
                            if let Err(error) = result && !matches!(error, crate::Error::Interrupted) {
                                task.stats.failure(std::path::Path::new("扫描"), error);
                            }
                            if task.dirty { publish(&library, events, task.stats.clone(), true)?; }
                            if std::env::var_os("LIUSHENG_PROFILE").is_some() {
                                eprintln!("[perf] scan_ms={} visited={} state_rows={} changed={} cancelled={}",
                                    task.started.elapsed().as_millis(), task.stats.visited, task.stats.state_rows,
                                    task.stats.added + task.stats.updated + task.stats.removed, was_cancelled);
                            }
                            events.send(LibraryEvent::ScanFinished { stats: task.stats.clone(), cancelled: was_cancelled }).ok();
                            if task.kind == ScanKind::Import && !was_cancelled {
                                events.send(LibraryEvent::PlayPaths(std::mem::take(&mut task.opened))).ok();
                            }
                        }
                    }
                    Err(_) => return Err(crate::Error::Other("扫描工作线程已退出".into())),
                }
            }
            recv(watch_events) -> event => {
                match event {
                    Ok(LibraryWatchEvent::PathsChanged(paths)) => {
                        events.send(LibraryEvent::InvalidateArtwork(paths.clone())).ok();
                        events.send(LibraryEvent::LyricsChanged(paths.clone())).ok();
                        if !full_pending {
                            changed_paths.extend(paths);
                            changed_paths = scan::minimal_scopes(&changed_paths);
                            if changed_paths.len() > 2048 { changed_paths.clear(); full_pending = true; }
                        }
                    }
                    Ok(LibraryWatchEvent::Changed) => {
                        events.send(LibraryEvent::InvalidateArtwork(Vec::new())).ok();
                        full_pending = true;
                    }
                    Ok(LibraryWatchEvent::Error(error)) => { events.send(LibraryEvent::Error(error)).ok(); full_pending = true; }
                    Err(_) => watch_events = never(),
                }
            }
        }
    }
    drop(active); // Cancels in-flight I/O before its event receiver disappears.
    drop(watching);
    drop(scanner);
    Ok(())
}

#[cfg(test)]
mod tests;
