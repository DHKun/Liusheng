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
                    let words = liusheng_core::library::pinyin::query_terms(&job.query);
                    let mut result = Vec::new();
                    for (i, track) in job.tracks.iter().enumerate() {
                        if i % 256 == 0
                            && (version.load(Ordering::Acquire) != job.generation
                                || cancelled.load(Ordering::Relaxed))
                        {
                            break;
                        }
                        if liusheng_core::library::pinyin::matches_terms(&track.search_text, &words)
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
    pub fn cancel(&self, generation: u64) {
        self.generation.store(generation, Ordering::Release);
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).take();
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

#[cfg(test)]
mod tests {
    use super::*;
    use liusheng_core::library::{db, tags::TrackMeta};
    use std::time::Duration;

    fn tracks() -> Arc<Vec<Arc<TrackRow>>> {
        let directory = tempfile::tempdir().unwrap();
        let conn = db::open(&directory.path().join("library.db")).unwrap();
        for (path, title, artist) in [
            ("/one.FLAC", "晴天（Live）", "周杰伦"),
            ("/two.mp3", "江南", "林俊杰"),
        ] {
            db::upsert_track(
                &conn,
                path,
                0,
                &TrackMeta {
                    title: title.into(),
                    artist: artist.into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        Arc::new(
            db::all_tracks(&conn)
                .unwrap()
                .into_iter()
                .map(Arc::new)
                .collect(),
        )
    }

    #[test]
    fn worker_matches_composite_queries_and_case_insensitive_formats() {
        let service = SearchService::start().unwrap();
        let tracks = tracks();
        let expected = tracks.iter().position(|t| t.path == "/one.FLAC").unwrap();
        service.submit(SearchJob {
            generation: 1,
            query: "ZJL 晴天 live".into(),
            tracks,
            sort: 1,
            format: "flac".into(),
        });
        let (generation, rows) = service
            .events()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        assert_eq!(generation, 1);
        assert_eq!(rows, vec![expected]);
    }

    #[test]
    fn clearing_a_query_invalidates_pending_work_and_new_generation_succeeds() {
        let service = SearchService::start().unwrap();
        let tracks = tracks();
        service.submit(SearchJob {
            generation: 10,
            query: "absent".into(),
            tracks: tracks.clone(),
            sort: 0,
            format: String::new(),
        });
        service.cancel(11);
        assert_eq!(service.generation.load(Ordering::Acquire), 11);
        assert!(service.latest.lock().unwrap().is_none());
        service.submit(SearchJob {
            generation: 12,
            query: "ljj 江南".into(),
            tracks: tracks.clone(),
            sort: 0,
            format: String::new(),
        });
        loop {
            let (generation, rows) = service
                .events()
                .recv_timeout(Duration::from_secs(3))
                .unwrap();
            if generation == 12 {
                assert_eq!(rows.len(), 1);
                assert_eq!(tracks[rows[0]].path, "/two.mp3");
                break;
            }
            assert!(generation < 11); // An already queued response is filtered by the GUI generation.
        }
    }
}
