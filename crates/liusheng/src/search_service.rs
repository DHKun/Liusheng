use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use liusheng_core::library::TrackRow;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

pub struct SearchJob {
    pub generation: u64,
    pub query: String,
    pub tracks: Arc<Vec<Arc<TrackRow>>>,
    pub sort: i32,
    pub format: String,
}
pub struct SearchService {
    latest: Arc<Mutex<Option<SearchJob>>>,
    wake: Sender<()>,
    generation: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    events: Receiver<(u64, Vec<usize>)>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl SearchService {
    pub fn start() -> std::io::Result<Self> {
        let latest = Arc::new(Mutex::new(None::<SearchJob>));
        let pending = latest.clone();
        let (wake, rx) = bounded(1);
        let (tx, events) = unbounded();
        let generation = Arc::new(AtomicU64::new(0));
        let version = generation.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let worker = std::thread::Builder::new()
            .name("liusheng-search".into())
            .spawn(move || {
                while rx.recv().is_ok() && !cancelled.load(Ordering::Acquire) {
                    let Some(job) = pending.lock().unwrap_or_else(|e| e.into_inner()).take() else {
                        continue;
                    };
                    let words = job.query.split_whitespace().collect::<Vec<_>>();
                    let mut result = Vec::new();
                    for (i, track) in job.tracks.iter().enumerate() {
                        if i % 256 == 0
                            && (version.load(Ordering::Acquire) != job.generation
                                || cancelled.load(Ordering::Relaxed))
                        {
                            break;
                        }
                        if words.iter().all(|w| track.search_text.contains(w))
                            && (job.format.is_empty()
                                || std::path::Path::new(&track.path)
                                    .extension()
                                    .is_some_and(|e| e.eq_ignore_ascii_case(&job.format)))
                        {
                            result.push(i);
                        }
                    }
                    if version.load(Ordering::Acquire) != job.generation {
                        continue;
                    }
                    match job.sort {
                        1 => result.sort_by(|a, b| job.tracks[*a].title.cmp(&job.tracks[*b].title)),
                        2 => {
                            result.sort_by(|a, b| job.tracks[*a].artist.cmp(&job.tracks[*b].artist))
                        }
                        3 => result.sort_by_key(|i| std::cmp::Reverse(job.tracks[*i].year)),
                        4 => result.sort_by_key(|i| job.tracks[*i].duration_ms),
                        _ => {}
                    }
                    if version.load(Ordering::Acquire) == job.generation {
                        tx.send((job.generation, result)).ok();
                    }
                }
            })?;
        Ok(Self {
            latest,
            wake,
            generation,
            stop,
            events,
            worker: Some(worker),
        })
    }
    pub fn submit(&self, job: SearchJob) {
        self.generation.store(job.generation, Ordering::Release);
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(job);
        let _ = self.wake.try_send(());
    }
    pub fn events(&self) -> Receiver<(u64, Vec<usize>)> {
        self.events.clone()
    }
}
impl Drop for SearchService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
