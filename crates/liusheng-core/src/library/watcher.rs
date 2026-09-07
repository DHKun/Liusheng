use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, after, select, unbounded};
use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::artwork::is_cover_sidecar;
use crate::error::{Error, Result};

use super::is_audio_path;

const CHANGE_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryWatchEvent {
    Changed,
    PathsChanged(Vec<PathBuf>),
    Error(String),
}

/// 递归监听曲库目录，并把文件系统事件压缩为稳定的曲库变更信号。
pub struct LibraryWatcher {
    _watcher: RecommendedWatcher,
    events: Receiver<LibraryWatchEvent>,
    stop: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl LibraryWatcher {
    pub fn start(root: &Path) -> Result<Self> {
        Self::start_many(&[root.to_owned()])
    }

    pub fn start_many(roots: &[PathBuf]) -> Result<Self> {
        // FSEvents can return canonical paths (e.g. /private/var instead of
        // /var). Preserve the configured spelling used as the database key.
        // Capture the roots while they exist so deletion events need no I/O.
        let roots_for_events = roots
            .iter()
            .map(|root| root.canonicalize().map(|physical| (physical, root.clone())))
            .collect::<std::io::Result<Vec<_>>>()?;
        let (raw_tx, raw_rx) = unbounded();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let event = event.map(|event| map_event_roots(event, &roots_for_events));
            let _ = raw_tx.send(event);
        })
        .map_err(watch_error)?;
        for root in roots {
            watcher
                .watch(root, RecursiveMode::Recursive)
                .map_err(watch_error)?;
        }

        let (events_tx, events) = unbounded();
        let (stop, stop_rx) = unbounded();
        let worker = std::thread::Builder::new()
            .name("liusheng-library-watch".to_owned())
            .spawn(move || run_event_loop(raw_rx, stop_rx, events_tx))?;

        Ok(Self {
            _watcher: watcher,
            events,
            stop,
            worker: Some(worker),
        })
    }

    pub fn events(&self) -> Receiver<LibraryWatchEvent> {
        self.events.clone()
    }
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Convert backend paths to every matching configured root. This is a lexical
/// operation: removed paths and symlink roots retain their pre-event identity.
fn map_event_roots(mut event: Event, roots: &[(PathBuf, PathBuf)]) -> Event {
    let paths = std::mem::take(&mut event.paths);
    for path in paths {
        let before = event.paths.len();
        for (physical, configured) in roots {
            if let Ok(suffix) = path.strip_prefix(physical) {
                event.paths.push(configured.join(suffix));
            }
        }
        if event.paths.len() == before {
            event.paths.push(path);
        }
    }
    event
}

fn watch_error(error: notify::Error) -> Error {
    Error::Other(error.to_string())
}

fn run_event_loop(
    raw_events: Receiver<notify::Result<Event>>,
    stop: Receiver<()>,
    events: Sender<LibraryWatchEvent>,
) {
    loop {
        select! {
            recv(stop) -> _ => return,
            recv(raw_events) -> event => {
                let Ok(event) = event else {
                    return;
                };
                match event {
                    Ok(event) if affects_library(&event) => {
                        if !debounce_changes(&raw_events, &stop, &events, event) {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        let _ = events.send(LibraryWatchEvent::Error(error.to_string()));
                    }
                }
            }
        }
    }
}

fn debounce_changes(
    raw_events: &Receiver<notify::Result<Event>>,
    stop: &Receiver<()>,
    events: &Sender<LibraryWatchEvent>,
    first: Event,
) -> bool {
    let mut overflow = first.need_rescan();
    let mut paths: HashSet<PathBuf> = first.paths.into_iter().collect();
    let max_deadline = Instant::now() + Duration::from_secs(2);
    let mut deadline = Instant::now() + CHANGE_DEBOUNCE;
    loop {
        let timer = after(deadline.saturating_duration_since(Instant::now()));
        select! {
            recv(stop) -> _ => return false,
            recv(raw_events) -> event => {
                let Ok(event) = event else {
                    return false;
                };
                match event {
                    Ok(event) if affects_library(&event) => {
                        deadline = (Instant::now() + CHANGE_DEBOUNCE).min(max_deadline);
                        overflow |= event.need_rescan();
                        if paths.len() < 2048 { paths.extend(event.paths); } else { overflow = true; }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        let _ = events.send(LibraryWatchEvent::Error(error.to_string()));
                    }
                }
            }
            recv(timer) -> _ => {
                let event = if overflow || paths.is_empty() { LibraryWatchEvent::Changed }
                    else { let mut paths: Vec<_> = paths.into_iter().collect(); paths.sort(); LibraryWatchEvent::PathsChanged(paths) };
                return events.send(event).is_ok();
            }
        }
    }
}

fn affects_library(event: &Event) -> bool {
    match event.kind {
        EventKind::Access(_) => false,
        EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder) => true,
        EventKind::Modify(ModifyKind::Name(_)) => true,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
            event.paths.iter().any(|path| {
                is_audio_path(path)
                    || is_cover_sidecar(path)
                    || path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("lrc"))
            })
        }
        EventKind::Any | EventKind::Other => event.paths.iter().any(|path| {
            is_audio_path(path)
                || is_cover_sidecar(path)
                || path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("lrc"))
                || path.is_dir()
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write;

    use notify::event::{AccessKind, AccessMode, DataChange};

    use super::*;

    #[test]
    fn canonical_backend_paths_use_configured_database_keys() {
        let roots = [(
            PathBuf::from("/private/var/music"),
            PathBuf::from("/var/music"),
        )];
        let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path("/private/var/music/album/track.wav".into());
        let mapped = map_event_roots(event, &roots);
        assert_eq!(
            mapped.paths,
            vec![PathBuf::from("/var/music/album/track.wav")]
        );
        assert!(affects_library(&mapped));
    }

    #[test]
    fn deleted_paths_map_without_canonicalizing_the_missing_file() {
        let roots = [(
            PathBuf::from("/physical/music"),
            PathBuf::from("/music-link"),
        )];
        let event = Event::new(EventKind::Remove(RemoveKind::Folder))
            .add_path("/physical/music/deleted-album".into());
        let mapped = map_event_roots(event, &roots);
        assert_eq!(
            mapped.paths,
            vec![PathBuf::from("/music-link/deleted-album")]
        );
        assert_eq!(mapped.kind, EventKind::Remove(RemoveKind::Folder));
    }

    #[test]
    fn path_mapping_respects_components_and_multiple_root_aliases() {
        let roots = [
            (PathBuf::from("/physical/music"), PathBuf::from("/music-a")),
            (PathBuf::from("/physical/music"), PathBuf::from("/music-b")),
        ];
        let event = Event::new(EventKind::Any)
            .add_path("/physical/music/track.wav".into())
            .add_path("/physical/music-backup/other.wav".into());
        assert_eq!(
            map_event_roots(event, &roots).paths,
            vec![
                PathBuf::from("/music-a/track.wav"),
                PathBuf::from("/music-b/track.wav"),
                PathBuf::from("/physical/music-backup/other.wav"),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn watcher_keeps_symlink_root_spelling_for_create_and_delete() {
        let directory = tempfile::tempdir().unwrap();
        let physical = directory.path().join("physical");
        let alias = directory.path().join("music-link");
        std::fs::create_dir(&physical).unwrap();
        std::os::unix::fs::symlink(&physical, &alias).unwrap();
        let watcher = LibraryWatcher::start(&alias).unwrap();
        let events = watcher.events();
        let track = alias.join("track.wav");
        std::fs::write(&track, b"test audio event").unwrap();
        assert_eq!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            LibraryWatchEvent::PathsChanged(vec![track.clone()])
        );
        std::fs::remove_file(&track).unwrap();
        assert_eq!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            LibraryWatchEvent::PathsChanged(vec![track])
        );
    }

    #[test]
    fn filters_access_and_library_events() {
        let access = Event::new(EventKind::Access(AccessKind::Close(AccessMode::Write)))
            .add_path("track.wav".into());
        let sidecar = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path("cover.jpg".into());
        let unrelated = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path("booklet.pdf".into());
        let audio = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path("TRACK.FLAC".into());
        let folder =
            Event::new(EventKind::Remove(RemoveKind::Folder)).add_path("deleted-album".into());

        assert!(!affects_library(&access));
        assert!(affects_library(&sidecar));
        assert!(!affects_library(&unrelated));
        assert!(affects_library(&audio));
        assert!(affects_library(&folder));
    }

    #[test]
    fn coalesces_a_burst_of_audio_changes() {
        let dir = tempfile::tempdir().unwrap();
        let watcher = LibraryWatcher::start(dir.path()).unwrap();
        let events = watcher.events();
        let track = dir.path().join("track.wav");

        std::fs::write(&track, b"header").unwrap();
        let mut file = OpenOptions::new().append(true).open(&track).unwrap();
        file.write_all(b"samples").unwrap();
        file.sync_all().unwrap();

        assert_eq!(
            events.recv_timeout(Duration::from_secs(3)).unwrap(),
            LibraryWatchEvent::PathsChanged(vec![track.clone()])
        );
        assert!(events.recv_timeout(Duration::from_millis(750)).is_err());
    }

    #[test]
    fn ignores_unsupported_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let watcher = LibraryWatcher::start(dir.path()).unwrap();
        let events = watcher.events();

        std::fs::write(dir.path().join("booklet.pdf"), b"document").unwrap();

        assert!(events.recv_timeout(Duration::from_millis(750)).is_err());
    }
}
