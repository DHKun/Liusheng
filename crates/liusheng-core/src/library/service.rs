//! One library actor owns SQLite and recursive watchers. UI messages contain immutable snapshots.
use super::{
    Library, LibrarySnapshot, ScanStats,
    playlists::Playlist,
    watcher::{LibraryWatchEvent, LibraryWatcher},
};
use crate::settings::{AppPaths, AppSettings, SavedSession};
use crossbeam_channel::{Receiver, Sender, bounded, never, select, unbounded};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub enum LibraryCommand {
    Refresh,
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
        let (commands, rx) = bounded(32);
        let (events_tx, events) = unbounded();
        let session = Arc::new(Mutex::new(None));
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_session = session.clone();
        let stop = cancelled.clone();
        let worker = std::thread::Builder::new()
            .name("liusheng-library".into())
            .spawn(move || {
                if let Err(e) = run(paths.clone(), rx, &events_tx, &worker_session, &stop) {
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
fn refresh(
    library: &mut Library,
    settings: &AppSettings,
    events: &Sender<LibraryEvent>,
    cancelled: &AtomicBool,
) -> crate::Result<()> {
    let started = Instant::now();
    events.send(LibraryEvent::Scanning).ok();
    let mut total = ScanStats::default();
    let first_import = library.track_count()? == 0;
    let mut last_publish = Instant::now() - Duration::from_secs(1);
    for root in &settings.music_roots {
        if cancelled.load(Ordering::Acquire) {
            break;
        }
        match library.scan_controlled(
            root,
            &settings.excluded_directories,
            cancelled,
            |library, stats| {
                if first_import && last_publish.elapsed() >= Duration::from_millis(400) {
                    let _ = publish(library, events, stats.clone(), false);
                    last_publish = Instant::now();
                }
            },
        ) {
            Ok(stats) => total.merge(stats),
            Err(e) => {
                total.failed += 1;
                total.errors.push(e.to_string());
            }
        }
    }
    if std::env::var_os("LIUSHENG_PROFILE").is_some() {
        eprintln!(
            "[perf] scan_ms={} changed={} failed={}",
            started.elapsed().as_millis(),
            total.added + total.updated + total.removed,
            total.failed
        );
    }
    publish(library, events, total, true)
}
fn run(
    paths: AppPaths,
    commands: Receiver<LibraryCommand>,
    events: &Sender<LibraryEvent>,
    session: &Mutex<Option<SavedSession>>,
    cancelled: &AtomicBool,
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
    // Cached data has already been posted before watcher registration or filesystem traversal.
    let mut watching = watcher(&settings, events);
    let mut watch_events = watching.as_ref().map(|w| w.events()).unwrap_or_else(never);
    let mut initial_refresh_pending = true;
    let mut roots_online = settings
        .music_roots
        .iter()
        .map(|p| p.is_dir())
        .collect::<Vec<_>>();
    let mut last_mount_check = Instant::now();
    let mut last_session_write = Instant::now();
    while !cancelled.load(Ordering::Acquire) {
        select! {
            recv(commands)->command=>{
                let Ok(command)=command else{break};
                let result=(||->crate::Result<()>{
                    match command {
                        LibraryCommand::Quit=>{cancelled.store(true,Ordering::Release);}
                        LibraryCommand::DiscoverDevices=>{events.send(LibraryEvent::Devices(serde_json::to_string(&crate::devices::output_devices()).unwrap_or_else(|_|"[]".into()))).ok();}
                        LibraryCommand::Refresh=>refresh(&mut library,&settings,events,cancelled)?,
                        LibraryCommand::Settings(mut value)=>{
                            value.validate()?; value.music_roots.sort();value.music_roots.dedup();
                            let roots_changed=value.music_roots!=settings.music_roots||value.excluded_directories!=settings.excluded_directories;
                            value.save(&paths.settings)?;settings=value;
                            events.send(LibraryEvent::SettingsSaved(settings.clone())).ok();
                            if roots_changed {
                                watching=watcher(&settings,events);watch_events=watching.as_ref().map(|w|w.events()).unwrap_or_else(never);
                                refresh(&mut library,&settings,events,cancelled)?;
                            }
                        }
                        LibraryCommand::SavePlaylist{name,paths}=>{library.save_playlist(&name,&paths)?;events.send(LibraryEvent::Playlists(library.playlists()?)).ok();}
                        LibraryCommand::RenamePlaylist{id,name}=>{library.rename_playlist(id,&name)?;events.send(LibraryEvent::Playlists(library.playlists()?)).ok();}
                        LibraryCommand::DeletePlaylist(id)=>{library.delete_playlist(id)?;events.send(LibraryEvent::Playlists(library.playlists()?)).ok();}
                        LibraryCommand::LoadPlaylist(id)=>{events.send(LibraryEvent::PlayPaths(library.playlist_paths(id)?)).ok();}
                        LibraryCommand::ImportPlaylist(path)=>{
                            let entries=super::playlists::read_m3u(&path)?;
                            library.save_playlist(path.file_stem().and_then(|s|s.to_str()).unwrap_or("导入歌单"),&entries)?;
                            events.send(LibraryEvent::Playlists(library.playlists()?)).ok();
                        }
                        LibraryCommand::ExportPlaylist{path,paths}=>{
                            super::playlists::write_m3u(&path,&paths)?;
                            events.send(LibraryEvent::Notice("歌单已导出".into())).ok();
                        }
                        LibraryCommand::OpenFiles(inputs)=>{
                            let mut files=Vec::new();
                            for input in inputs {
                                if input.is_dir(){
                                    for entry in walkdir::WalkDir::new(&input).into_iter().filter_map(Result::ok){
                                        if cancelled.load(Ordering::Acquire){break;}
                                        if entry.file_type().is_file() && super::is_audio_path(entry.path()){files.push(entry.into_path());}
                                    }
                                }else if input.extension().is_some_and(|s|s.eq_ignore_ascii_case("m3u")||s.eq_ignore_ascii_case("m3u8")){
                                    files.extend(super::playlists::read_m3u(&input)?.into_iter().map(PathBuf::from));
                                }else if super::is_audio_path(&input){files.push(input);}
                            }
                            let files=files.into_iter().map(|p|std::fs::canonicalize(&p).unwrap_or(p)).collect::<Vec<_>>();
                            let stats=library.import_files(&files)?;publish(&library,events,stats,true)?;
                            events.send(LibraryEvent::PlayPaths(files.iter().map(|p|p.to_string_lossy().into_owned()).collect())).ok();
                        }
                    } Ok(())
                })();
                if let Err(e)=result{events.send(LibraryEvent::Error(e.to_string())).ok();}
            }
            recv(watch_events)->event=>{
                match event {
                    Ok(LibraryWatchEvent::PathsChanged(paths))=>{
                        events.send(LibraryEvent::InvalidateArtwork(paths.clone())).ok();
                        events.send(LibraryEvent::LyricsChanged(paths.clone())).ok();
                        match library.update_paths(&settings.music_roots,&settings.excluded_directories,&paths){
                            Ok(stats) if stats.changed()=>{publish(&library,events,stats,true)?;}
                            Ok(stats) if stats.failed>0=>{events.send(LibraryEvent::Error(stats.errors.join("\n"))).ok();}
                            Ok(_)=>{},Err(e)=>{events.send(LibraryEvent::Error(e.to_string())).ok();}
                        }
                    }
                    Ok(LibraryWatchEvent::Changed)=>{events.send(LibraryEvent::InvalidateArtwork(Vec::new())).ok();refresh(&mut library,&settings,events,cancelled)?;}
                    Ok(LibraryWatchEvent::Error(e))=>{events.send(LibraryEvent::Error(e)).ok();refresh(&mut library,&settings,events,cancelled)?;}
                    Err(_)=>{watch_events=never();}
                }
            }
            default(Duration::from_millis(200))=>{
                if initial_refresh_pending{initial_refresh_pending=false;refresh(&mut library,&settings,events,cancelled)?;}
                if last_session_write.elapsed()>=Duration::from_secs(2){flush_session(&paths,session,events);last_session_write=Instant::now();}
                if last_mount_check.elapsed()>=Duration::from_secs(15){
                    last_mount_check=Instant::now();let online=settings.music_roots.iter().map(|p|p.is_dir()).collect::<Vec<_>>();
                    if online!=roots_online {roots_online=online;watching=watcher(&settings,events);watch_events=watching.as_ref().map(|w|w.events()).unwrap_or_else(never);refresh(&mut library,&settings,events,cancelled)?;}
                }
            }
        }
    }
    drop(watching);
    Ok(())
}
