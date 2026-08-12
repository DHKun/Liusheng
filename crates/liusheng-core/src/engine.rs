use std::path::PathBuf;
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};

use crate::audio::PcmSpec;
use crate::audio::decode::AudioFileDecoder;
use crate::audio::sink::AudioSink;

#[derive(Debug)]
pub enum PlayerCommand {
    SetQueue { paths: Vec<PathBuf>, start: usize },
    Play,
    Pause,
    Stop,
    Next,
    Prev,
    Seek(f64),
    AppendQueueItem(PathBuf),
    InsertNext(PathBuf),
    RemoveQueueItem(usize),
    ClearQueue,
}

enum EngineCommand {
    Player(PlayerCommand),
    Quit,
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    TrackStarted {
        index: usize,
        path: PathBuf,
        spec: PcmSpec,
        duration_secs: Option<f64>,
    },
    Progress {
        secs: f64,
    },
    Paused,
    Resumed,
    Stopped,
    QueueFinished,
    TrackError {
        path: PathBuf,
        message: String,
    },
    EngineError {
        message: String,
    },
}

/// 播放器句柄：命令进、事件出，解码与输出在独立线程。
pub struct Player {
    cmd_tx: Sender<EngineCommand>,
    events_rx: Receiver<PlayerEvent>,
    handle: Option<JoinHandle<()>>,
}

/// 已提前打开并解出首块样本的下一曲。
struct PreloadedTrack {
    index: usize,
    path: PathBuf,
    decoder: AudioFileDecoder,
    first_samples: Vec<i32>,
}

impl Player {
    pub fn new(sink: Box<dyn AudioSink>) -> Self {
        let (cmd_tx, cmd_rx) = unbounded();
        let (event_tx, events_rx) = unbounded();
        let handle = std::thread::Builder::new()
            .name("liusheng-engine".into())
            .spawn(move || Engine::new(sink, cmd_rx, event_tx).run())
            .expect("无法创建播放线程");
        Self {
            cmd_tx,
            events_rx,
            handle: Some(handle),
        }
    }

    pub fn send(&self, command: PlayerCommand) {
        let _ = self.cmd_tx.send(EngineCommand::Player(command));
    }

    pub fn events(&self) -> &Receiver<PlayerEvent> {
        &self.events_rx
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(EngineCommand::Quit);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

struct Engine {
    sink: Box<dyn AudioSink>,
    cmd_rx: Receiver<EngineCommand>,
    event_tx: Sender<PlayerEvent>,
    queue: Vec<PathBuf>,
    index: usize,
    current: Option<AudioFileDecoder>,
    preloaded: Option<PreloadedTrack>,
    playing: bool,
    buf: Vec<i32>,
    pos_frames: u64,
    next_progress_at: u64,
}

impl Engine {
    fn new(
        sink: Box<dyn AudioSink>,
        cmd_rx: Receiver<EngineCommand>,
        event_tx: Sender<PlayerEvent>,
    ) -> Self {
        Self {
            sink,
            cmd_rx,
            event_tx,
            queue: Vec::new(),
            index: 0,
            current: None,
            preloaded: None,
            playing: false,
            buf: Vec::new(),
            pos_frames: 0,
            next_progress_at: 0,
        }
    }

    fn run(mut self) {
        loop {
            // 播放中非阻塞收命令，空闲时阻塞等待，避免空转
            let cmd = if self.playing && self.current.is_some() {
                match self.cmd_rx.try_recv() {
                    Ok(c) => Some(c),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => break,
                }
            } else {
                match self.cmd_rx.recv() {
                    Ok(c) => Some(c),
                    Err(_) => break,
                }
            };
            if let Some(command) = cmd {
                match command {
                    EngineCommand::Player(command) => self.handle_command(command),
                    EngineCommand::Quit => break,
                }
                continue;
            }
            self.pump();
        }
        let _ = self.sink.discard();
    }

    fn emit(&self, ev: PlayerEvent) {
        let _ = self.event_tx.send(ev);
    }

    /// 输出端控制操作失败不致命，上报后继续。
    fn sink_op(&mut self, res: crate::error::Result<()>) {
        if let Err(e) = res {
            self.emit(PlayerEvent::EngineError {
                message: e.to_string(),
            });
        }
    }

    fn handle_command(&mut self, cmd: PlayerCommand) {
        match cmd {
            PlayerCommand::SetQueue { paths, start } => {
                self.index = start.min(paths.len().saturating_sub(1));
                self.queue = paths;
                self.current = None;
                self.preloaded = None;
                self.playing = false;
                let r = self.sink.discard();
                self.sink_op(r);
            }
            PlayerCommand::Play => {
                let r = self.sink.pause(false);
                self.sink_op(r);
                if self.current.is_some() {
                    if !self.playing {
                        self.playing = true;
                        self.emit(PlayerEvent::Resumed);
                    }
                } else if self.open_current_or_skip() {
                    self.playing = true;
                }
            }
            PlayerCommand::Pause => {
                if self.playing {
                    self.playing = false;
                    let r = self.sink.pause(true);
                    self.sink_op(r);
                    self.emit(PlayerEvent::Paused);
                }
            }
            PlayerCommand::Stop => {
                self.current = None;
                self.preloaded = None;
                self.playing = false;
                self.index = 0;
                let r = self.sink.discard();
                self.sink_op(r);
                self.emit(PlayerEvent::Stopped);
            }
            PlayerCommand::Next => {
                if self.index + 1 < self.queue.len() {
                    self.index += 1;
                    self.preloaded = None;
                    let r = self.sink.discard();
                    self.sink_op(r);
                    let was_playing = self.playing || self.current.is_some();
                    if self.open_current_or_skip() {
                        self.playing = was_playing;
                    }
                } else {
                    self.finish_queue();
                }
            }
            PlayerCommand::Prev => {
                self.index = self.index.saturating_sub(1);
                self.preloaded = None;
                let r = self.sink.discard();
                self.sink_op(r);
                let was_playing = self.playing || self.current.is_some();
                if self.open_current_or_skip() {
                    self.playing = was_playing;
                }
            }
            PlayerCommand::Seek(secs) => {
                if let Some(dec) = self.current.as_mut() {
                    let rate = dec.spec().rate;
                    match dec.seek_secs(secs) {
                        Ok(actual) => {
                            self.pos_frames = (actual * rate as f64) as u64;
                            self.next_progress_at = self.pos_frames;
                            let r = self.sink.discard();
                            self.sink_op(r);
                            self.emit(PlayerEvent::Progress { secs: actual });
                        }
                        Err(e) => self.emit(PlayerEvent::EngineError {
                            message: e.to_string(),
                        }),
                    }
                }
            }
            PlayerCommand::AppendQueueItem(path) => {
                self.queue.push(path);
                self.refresh_preloaded();
            }
            PlayerCommand::InsertNext(path) => {
                let insertion = (self.index + 1).min(self.queue.len());
                self.queue.insert(insertion, path);
                self.refresh_preloaded();
            }
            PlayerCommand::RemoveQueueItem(index) => self.remove_queue_item(index),
            PlayerCommand::ClearQueue => self.clear_queue(),
        }
    }

    fn remove_queue_item(&mut self, index: usize) {
        if index >= self.queue.len() {
            return;
        }
        let had_current = self.current.is_some();
        let was_playing = self.playing;
        self.queue.remove(index);

        if self.queue.is_empty() {
            self.clear_queue();
            return;
        }

        if index < self.index {
            self.index -= 1;
            self.refresh_preloaded();
            return;
        }
        if index > self.index {
            self.refresh_preloaded();
            return;
        }

        self.index = index.min(self.queue.len() - 1);
        self.preloaded = None;
        if !had_current {
            return;
        }

        self.current = None;
        self.playing = false;
        let result = self.sink.discard();
        self.sink_op(result);
        if self.open_current_or_skip() {
            self.playing = was_playing;
            if !was_playing {
                self.emit(PlayerEvent::Paused);
            }
        }
    }

    fn clear_queue(&mut self) {
        self.queue.clear();
        self.current = None;
        self.preloaded = None;
        self.playing = false;
        self.index = 0;
        self.pos_frames = 0;
        self.next_progress_at = 0;
        let result = self.sink.discard();
        self.sink_op(result);
        self.emit(PlayerEvent::Stopped);
    }

    fn refresh_preloaded(&mut self) {
        if self.current.is_none() {
            self.preloaded = None;
            return;
        }
        for (path, message) in self.preload_next() {
            self.emit(PlayerEvent::TrackError { path, message });
        }
    }

    /// 从 index 起打开第一首能解码的曲目；坏文件跳过并上报。
    /// 全部失败时收尾队列，返回 false。
    fn open_current_or_skip(&mut self) -> bool {
        while self.index < self.queue.len() {
            let path = self.queue[self.index].clone();
            match AudioFileDecoder::open(&path) {
                Ok(dec) => {
                    let spec = dec.spec();
                    let duration_secs = dec.duration_secs();
                    self.pos_frames = 0;
                    self.next_progress_at = 0;
                    self.current = Some(dec);
                    let preload_errors = self.preload_next();
                    self.emit(PlayerEvent::TrackStarted {
                        index: self.index,
                        path,
                        spec,
                        duration_secs,
                    });
                    for (path, message) in preload_errors {
                        self.emit(PlayerEvent::TrackError { path, message });
                    }
                    return true;
                }
                Err(e) => {
                    self.emit(PlayerEvent::TrackError {
                        path,
                        message: e.to_string(),
                    });
                    self.index += 1;
                }
            }
        }
        self.finish_queue();
        false
    }

    fn finish_queue(&mut self) {
        self.current = None;
        self.preloaded = None;
        self.playing = false;
        let _ = self.sink.flush();
        self.emit(PlayerEvent::QueueFinished);
    }

    fn pump(&mut self) {
        let Some(dec) = self.current.as_mut() else {
            return;
        };
        let spec = dec.spec();
        match dec.next_into(&mut self.buf) {
            Ok(true) => {
                if let Err(e) = self.sink.write(spec, &self.buf) {
                    let _ = self.event_tx.send(PlayerEvent::EngineError {
                        message: e.to_string(),
                    });
                    self.playing = false;
                    return;
                }
                self.pos_frames += spec.frames(self.buf.len());
                if self.pos_frames >= self.next_progress_at {
                    let secs = self.pos_frames as f64 / spec.rate as f64;
                    let _ = self.event_tx.send(PlayerEvent::Progress { secs });
                    // 进度事件按音频时间每半秒一次
                    self.next_progress_at = self.pos_frames + (spec.rate / 2) as u64;
                }
            }
            Ok(false) => {
                // 下一曲已经提前打开并解出首块样本，直接拼入输出。
                if !self.start_preloaded() {
                    self.finish_queue();
                }
            }
            Err(e) => {
                let path = self.queue.get(self.index).cloned().unwrap_or_default();
                let _ = self.event_tx.send(PlayerEvent::TrackError {
                    path,
                    message: e.to_string(),
                });
                self.index += 1;
                if self.index < self.queue.len() {
                    self.open_current_or_skip();
                } else {
                    self.finish_queue();
                }
            }
        }
    }

    /// 提前打开下一首可播放曲目并解出首块样本。
    /// 返回预加载期间遇到的坏文件，等当前曲目开始事件发出后再上报。
    fn preload_next(&mut self) -> Vec<(PathBuf, String)> {
        self.preloaded = None;
        let mut errors = Vec::new();
        let mut index = self.index + 1;
        while index < self.queue.len() {
            let path = self.queue[index].clone();
            match AudioFileDecoder::open(&path) {
                Ok(mut decoder) => {
                    let mut first_samples = Vec::new();
                    match decoder.next_into(&mut first_samples) {
                        Ok(_) => {
                            self.preloaded = Some(PreloadedTrack {
                                index,
                                path,
                                decoder,
                                first_samples,
                            });
                            break;
                        }
                        Err(e) => errors.push((path, e.to_string())),
                    }
                }
                Err(e) => errors.push((path, e.to_string())),
            }
            index += 1;
        }
        errors
    }

    /// 当前曲目结束时启用预加载结果，并先写入已解码的首块样本。
    fn start_preloaded(&mut self) -> bool {
        let Some(preloaded) = self.preloaded.take() else {
            return false;
        };
        self.index = preloaded.index;
        let spec = preloaded.decoder.spec();
        let duration_secs = preloaded.decoder.duration_secs();
        self.current = Some(preloaded.decoder);
        self.pos_frames = 0;
        self.next_progress_at = 0;
        self.emit(PlayerEvent::TrackStarted {
            index: self.index,
            path: preloaded.path,
            spec,
            duration_secs,
        });

        if !preloaded.first_samples.is_empty()
            && !self.write_samples(spec, &preloaded.first_samples)
        {
            return true;
        }

        for (path, message) in self.preload_next() {
            self.emit(PlayerEvent::TrackError { path, message });
        }
        true
    }

    fn write_samples(&mut self, spec: PcmSpec, samples: &[i32]) -> bool {
        if let Err(e) = self.sink.write(spec, samples) {
            self.emit(PlayerEvent::EngineError {
                message: e.to_string(),
            });
            self.playing = false;
            return false;
        }
        self.pos_frames += spec.frames(samples.len());
        if self.pos_frames >= self.next_progress_at {
            let secs = self.pos_frames as f64 / spec.rate as f64;
            self.emit(PlayerEvent::Progress { secs });
            self.next_progress_at = self.pos_frames + (spec.rate / 2) as u64;
        }
        true
    }
}
