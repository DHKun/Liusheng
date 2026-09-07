use super::*;
use crossbeam_channel::{bounded, unbounded};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

pub(super) struct PreloadResult {
    pub generation: u64,
    pub index: usize,
    pub path: PathBuf,
    pub track: Result<PreloadedTrack, String>,
}
struct Request {
    generation: u64,
    index: usize,
    path: PathBuf,
}
pub(super) struct Preloader {
    latest: Arc<Mutex<Option<Request>>>,
    wake: Sender<()>,
    pub results: Receiver<PreloadResult>,
    generation: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Preloader {
    pub fn new(events: Sender<PlayerEvent>) -> Self {
        let latest = Arc::new(Mutex::new(None::<Request>));
        let pending = latest.clone();
        let (wake, rx) = bounded(1);
        let (tx, results) = unbounded();
        let generation = Arc::new(AtomicU64::new(0));
        let version = generation.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let worker = std::thread::Builder::new()
            .name("liusheng-preload".into())
            .spawn(move || {
                while rx.recv().is_ok() && !cancelled.load(Ordering::Acquire) {
                    let Some(request) = pending.lock().unwrap_or_else(|e| e.into_inner()).take()
                    else {
                        continue;
                    };
                    let result = (|| -> crate::Result<PreloadedTrack> {
                        let mut decoder = AudioFileDecoder::open(&request.path)?;
                        let spec = decoder.spec();
                        let target = (spec.rate as usize / 4) * spec.channels as usize;
                        let mut first_samples = Vec::with_capacity(target);
                        let mut block = Vec::new();
                        while first_samples.len() < target.max(1) {
                            if cancelled.load(Ordering::Acquire)
                                || version.load(Ordering::Acquire) != request.generation
                            {
                                return Err(crate::Error::Interrupted);
                            }
                            if !decoder.next_into(&mut block)? {
                                break;
                            }
                            first_samples.extend_from_slice(&block);
                        }
                        Ok(PreloadedTrack {
                            index: request.index,
                            path: request.path.clone(),
                            decoder,
                            first_samples,
                        })
                    })()
                    .map_err(|e| e.to_string());
                    if version.load(Ordering::Acquire) != request.generation {
                        continue;
                    }
                    if result.is_ok() {
                        events
                            .send(PlayerEvent::PreloadReady {
                                path: request.path.clone(),
                            })
                            .ok();
                    }
                    if tx
                        .send(PreloadResult {
                            generation: request.generation,
                            index: request.index,
                            path: request.path,
                            track: result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("无法创建预加载线程");
        Self {
            latest,
            wake,
            results,
            generation,
            stop,
            worker: Some(worker),
        }
    }
    pub fn request(&self, index: usize, path: PathBuf) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(Request {
            generation,
            index,
            path,
        });
        let _ = self.wake.try_send(());
        generation
    }
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}
impl Drop for Preloader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
