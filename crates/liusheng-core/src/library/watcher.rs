use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, at, never, select_biased, unbounded};
use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::artwork::is_cover_sidecar;
use crate::error::{Error, Result};

use super::is_audio_path;

const CHANGE_DEBOUNCE: Duration = Duration::from_millis(500);
const MAX_BATCH_DELAY: Duration = Duration::from_secs(2);
const MAX_CHANGED_PATHS: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryWatchEvent {
    /// The backend lost detail; reconcile all configured library roots.
    Changed,
    /// Sorted, unique invalidation scopes. Entries may be files OR directories;
    /// native backends may split, repeat or add ancestor notifications.
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
                event.paths.push(if suffix.as_os_str().is_empty() {
                    configured.clone()
                } else {
                    configured.join(suffix)
                });
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

/// Aggregation and scheduling use an explicit monotonic timestamp so unit tests
/// can prove debounce semantics independently of native delivery and scheduling.
#[derive(Default)]
struct PendingChanges {
    paths: HashSet<PathBuf>,
    rescan: bool,
    deadline: Option<Instant>,
    max_deadline: Option<Instant>,
}

impl PendingChanges {
    fn push(&mut self, event: Event, now: Instant) {
        if !affects_library(&event) {
            return;
        }
        let max_deadline = *self.max_deadline.get_or_insert(now + MAX_BATCH_DELAY);
        self.deadline = Some((now + CHANGE_DEBOUNCE).min(max_deadline));

        if event.need_rescan() || event.paths.is_empty() {
            self.rescan = true;
            self.paths.clear();
        }
        if self.rescan {
            return;
        }
        // Enforce the bound for every path, including the very first native
        // notification. Repeated paths at the limit remain a precise batch.
        for path in event.paths {
            if self.paths.contains(&path) {
                continue;
            }
            if self.paths.len() == MAX_CHANGED_PATHS {
                self.rescan = true;
                self.paths.clear();
                break;
            }
            self.paths.insert(path);
        }
    }

    fn take_due(&mut self, now: Instant) -> Option<LibraryWatchEvent> {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.finish()
        } else {
            None
        }
    }

    fn finish(&mut self) -> Option<LibraryWatchEvent> {
        self.deadline?;
        let batch = std::mem::take(self);
        if batch.rescan {
            Some(LibraryWatchEvent::Changed)
        } else {
            let mut paths: Vec<_> = batch.paths.into_iter().collect();
            paths.sort();
            Some(LibraryWatchEvent::PathsChanged(paths))
        }
    }
}

fn run_event_loop(
    raw_events: Receiver<notify::Result<Event>>,
    stop: Receiver<()>,
    events: Sender<LibraryWatchEvent>,
) {
    let mut pending = PendingChanges::default();
    loop {
        let timer = pending.deadline.map(at).unwrap_or_else(never);
        // A permanently ready native queue must not starve shutdown or an
        // expired deadline. The 2s upper bound is independent of event volume.
        select_biased! {
            recv(stop) -> _ => return,
            recv(timer) -> _ => {
                if let Some(event) = pending.take_due(Instant::now())
                    && events.send(event).is_err()
                {
                    return;
                }
            }
            recv(raw_events) -> event => {
                match event {
                    Ok(Ok(event)) => pending.push(event, Instant::now()),
                    Ok(Err(error)) => {
                        // LibraryService treats an error as a request to reconcile.
                        if events.send(LibraryWatchEvent::Error(error.to_string())).is_err() {
                            return;
                        }
                    }
                    Err(_) => {
                        // Unexpected backend closure preserves the last batch.
                        // Explicit shutdown takes precedence and discards it.
                        if let Some(event) = pending.finish() {
                            let _ = events.send(event);
                        }
                        return;
                    }
                }
            }
        }
    }
}

fn is_library_file(path: &Path) -> bool {
    is_audio_path(path)
        || is_cover_sidecar(path)
        || path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lrc"))
}

fn affects_library(event: &Event) -> bool {
    // Rescan can have zero paths or an unrelated-looking path. Inspect it before
    // the extension/kind filter; notify explicitly says any object may differ.
    if event.need_rescan() {
        return true;
    }
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    if event.paths.is_empty() {
        return true;
    }
    match event.kind {
        EventKind::Access(_) => false,
        EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder) => true,
        // Keep BOTH rename endpoints, including folders and temporary filenames.
        EventKind::Modify(ModifyKind::Name(_)) => true,
        EventKind::Create(CreateKind::File) | EventKind::Remove(RemoveKind::File) => {
            event.paths.iter().any(|path| is_library_file(path))
        }
        EventKind::Modify(_) => event
            .paths
            .iter()
            .any(|path| is_library_file(path) || path.is_dir()),
        // Imprecise notifications can refer to directories already deleted.
        // Preserve that scope; Library::update_paths reconciles cached descendants.
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Any | EventKind::Other => event
            .paths
            .iter()
            .any(|path| is_library_file(path) || !path.is_file()),
    }
}

#[cfg(test)]
#[path = "watcher/tests.rs"]
mod tests;
