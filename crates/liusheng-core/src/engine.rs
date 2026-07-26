use std::path::PathBuf;
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};

use crate::audio::PcmSpec;
use crate::audio::decode::AudioFileDecoder;
use crate::audio::sink::AudioSink;

#[derive(Debug)]
pub enum Command {
    SetQueue { paths: Vec<PathBuf>, start: usize },
    Play,
    Pause,
    Stop,
    Next,
    Prev,
    Seek(f64),
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
    cmd_tx: Sender<Command>,
    events_rx: Receiver<PlayerEvent>,
    handle: Option<JoinHandle<()>>,
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

    pub fn send(&self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn events(&self) -> &Receiver<PlayerEvent> {
        &self.events_rx
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Command::Quit);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

struct Engine {
    sink: Box<dyn AudioSink>,
    cmd_rx: Receiver<Command>,
    event_tx: Sender<PlayerEvent>,
    queue: Vec<PathBuf>,
    index: usize,
    current: Option<AudioFileDecoder>,
    playing: bool,
    buf: Vec<i32>,
    pos_frames: u64,
    next_progress_at: u64,
}

impl Engine {
    fn new(
        sink: Box<dyn AudioSink>,
        cmd_rx: Receiver<Command>,
        event_tx: Sender<PlayerEvent>,
    ) -> Self {
        Self {
            sink,
            cmd_rx,
            event_tx,
            queue: Vec::new(),
            index: 0,
            current: None,
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
            if let Some(cmd) = cmd {
                if matches!(cmd, Command::Quit) {
                    break;
                }
                self.handle_command(cmd);
                continue;
            }
            self.pump();
        }
        let _ = self.sink.flush();
    }

    fn emit(&self, ev: PlayerEvent) {
        let _ = self.event_tx.send(ev);
    }

    fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::SetQueue { paths, start } => {
                self.index = start.min(paths.len().saturating_sub(1));
                self.queue = paths;
                self.current = None;
                self.playing = false;
            }
            Command::Play => {
                if self.current.is_some() {
                    if !self.playing {
                        self.playing = true;
                        self.emit(PlayerEvent::Resumed);
                    }
                } else if self.open_current_or_skip() {
                    self.playing = true;
                }
            }
            Command::Pause => {
                if self.playing {
                    self.playing = false;
                    self.emit(PlayerEvent::Paused);
                }
            }
            Command::Stop => {
                self.current = None;
                self.playing = false;
                self.index = 0;
                self.emit(PlayerEvent::Stopped);
            }
            Command::Next => {
                if self.index + 1 < self.queue.len() {
                    self.index += 1;
                    let was_playing = self.playing || self.current.is_some();
                    if self.open_current_or_skip() {
                        self.playing = was_playing;
                    }
                } else {
                    self.finish_queue();
                }
            }
            Command::Prev => {
                self.index = self.index.saturating_sub(1);
                let was_playing = self.playing || self.current.is_some();
                if self.open_current_or_skip() {
                    self.playing = was_playing;
                }
            }
            Command::Seek(secs) => {
                if let Some(dec) = self.current.as_mut() {
                    let rate = dec.spec().rate;
                    match dec.seek_secs(secs) {
                        Ok(actual) => {
                            self.pos_frames = (actual * rate as f64) as u64;
                            self.next_progress_at = self.pos_frames;
                            self.emit(PlayerEvent::Progress { secs: actual });
                        }
                        Err(e) => self.emit(PlayerEvent::EngineError {
                            message: e.to_string(),
                        }),
                    }
                }
            }
            Command::Quit => unreachable!("Quit 在 run 循环中处理"),
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
                    self.emit(PlayerEvent::TrackStarted {
                        index: self.index,
                        path,
                        spec,
                        duration_secs,
                    });
                    self.current = Some(dec);
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
                // 当前曲目播完，立即接下一曲，不冲刷输出，保证无缝
                self.index += 1;
                if self.index < self.queue.len() {
                    self.open_current_or_skip();
                } else {
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
}
