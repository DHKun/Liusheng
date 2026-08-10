use std::path::Path;
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
        let (raw_tx, raw_rx) = unbounded();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = raw_tx.send(event);
        })
        .map_err(watch_error)?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(watch_error)?;

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
                        if !debounce_changes(&raw_events, &stop, &events) {
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
) -> bool {
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
                        deadline = Instant::now() + CHANGE_DEBOUNCE;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        let _ = events.send(LibraryWatchEvent::Error(error.to_string()));
                    }
                }
            }
            recv(timer) -> _ => {
                return events.send(LibraryWatchEvent::Changed).is_ok();
            }
        }
    }
}

fn affects_library(event: &Event) -> bool {
    match event.kind {
        EventKind::Access(_) => false,
        EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder) => true,
        EventKind::Modify(ModifyKind::Name(_)) => true,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => event
            .paths
            .iter()
            .any(|path| is_audio_path(path) || is_cover_sidecar(path)),
        EventKind::Any | EventKind::Other => event
            .paths
            .iter()
            .any(|path| is_audio_path(path) || is_cover_sidecar(path) || path.is_dir()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write;

    use notify::event::{AccessKind, AccessMode, DataChange};

    use super::*;

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
            LibraryWatchEvent::Changed
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
