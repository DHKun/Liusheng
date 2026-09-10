//! A single I/O worker emits bounded metadata batches to the SQLite owner.
//! It never owns a database connection or writes application state.
use super::scan::{self, Batch, Plan};
use crossbeam_channel::{Receiver, SendTimeoutError, Sender, bounded};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(super) struct Job {
    pub id: u64,
    pub plan: Plan,
    pub cancelled: Arc<AtomicBool>,
}

pub(super) enum Message {
    Batch(u64, Batch),
    Finished(u64, crate::Result<()>),
}

pub(super) struct Worker {
    jobs: Option<Sender<Job>>,
    pub events: Receiver<Message>,
    stopped: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(reader: scan::MetadataReader) -> std::io::Result<Self> {
        let (jobs, input) = bounded::<Job>(1);
        let (output, events) = bounded(4);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let handle = std::thread::Builder::new()
            .name("liusheng-scan-io".into())
            .spawn(move || {
                while let Ok(job) = input.recv() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let result = scan::execute_with_reader(
                        job.plan,
                        &job.cancelled,
                        |batch| {
                            let mut message = Message::Batch(job.id, batch);
                            loop {
                                if stop.load(Ordering::Acquire)
                                    || job.cancelled.load(Ordering::Acquire)
                                {
                                    return Err(crate::Error::Interrupted);
                                }
                                match output.send_timeout(message, Duration::from_millis(25)) {
                                    Ok(()) => return Ok(()),
                                    Err(SendTimeoutError::Timeout(value)) => message = value,
                                    Err(SendTimeoutError::Disconnected(_)) => {
                                        return Err(crate::Error::Interrupted);
                                    }
                                }
                            }
                        },
                        reader.as_ref(),
                    );
                    let mut done = Message::Finished(job.id, result);
                    loop {
                        if stop.load(Ordering::Acquire) {
                            return;
                        }
                        match output.send_timeout(done, Duration::from_millis(25)) {
                            Ok(()) => break,
                            Err(SendTimeoutError::Timeout(value)) => done = value,
                            Err(SendTimeoutError::Disconnected(_)) => return,
                        }
                    }
                }
            })?;
        Ok(Self {
            jobs: Some(jobs),
            events,
            stopped,
            handle: Some(handle),
        })
    }

    pub fn submit(&self, job: Job) -> crate::Result<()> {
        self.jobs
            .as_ref()
            .expect("scan worker is alive")
            .try_send(job)
            .map_err(|_| crate::Error::Other("扫描工作线程暂不可用".into()))
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.jobs.take();
        if let Some(handle) = self.handle.take() {
            // A filesystem syscall on an unavailable network mount may be stuck
            // in the kernel. Its pure read-only worker can finish independently;
            // late batches have no database consumer after the actor shuts down.
            let deadline = Instant::now() + Duration::from_millis(100);
            while !handle.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(2));
            }
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}
