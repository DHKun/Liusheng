use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, never, select, unbounded};

#[cfg(target_os = "linux")]
use crate::audio::alsa_sink::AlsaSink;
#[cfg(target_os = "macos")]
use crate::audio::coreaudio_sink::CoreAudioSink;
#[cfg(target_os = "linux")]
use crate::audio::pipewire_sink::PipeWireSink;
#[cfg(target_os = "linux")]
use crate::audio::resampling_sink::ResamplingSink;
use crate::audio::sink::AudioSink;
use crate::engine::{Player, PlayerCommand, PlayerEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputConfig {
    pub initial_mode: OutputMode,
    pub exclusive_device: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputError {
    pub mode: OutputMode,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    Playback(PlayerEvent),
    Switching {
        from: OutputMode,
        to: OutputMode,
    },
    Active {
        mode: OutputMode,
    },
    Restored {
        mode: OutputMode,
        error: OutputError,
    },
    Unavailable {
        target_error: OutputError,
        restore_error: Option<OutputError>,
    },
}

pub enum SessionCommand {
    Playback(PlayerCommand),
    Switch(OutputMode),
    RetryOutput,
}

enum WorkerCommand {
    Public(SessionCommand),
    Quit,
}

trait OutputAdapterFactory: Send + Sync + 'static {
    fn open(
        &self,
        mode: OutputMode,
        config: &OutputConfig,
        cancelled: &AtomicBool,
    ) -> std::result::Result<Box<dyn AudioSink>, OutputError>;
}

#[cfg(target_os = "linux")]
const EXCLUSIVE_OPEN_TIMEOUT: Duration = Duration::from_secs(6);
#[cfg(target_os = "linux")]
const EXCLUSIVE_OPEN_RETRY: Duration = Duration::from_millis(100);

struct SystemOutputAdapterFactory;

impl OutputAdapterFactory for SystemOutputAdapterFactory {
    fn open(
        &self,
        mode: OutputMode,
        config: &OutputConfig,
        cancelled: &AtomicBool,
    ) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
        if cancelled.load(Ordering::Acquire) {
            return Err(OutputError {
                mode,
                message: "输出打开已取消".into(),
            });
        }
        open_system_output(mode, config, cancelled)
    }
}

#[cfg(target_os = "linux")]
fn open_system_output(
    mode: OutputMode,
    config: &OutputConfig,
    cancelled: &AtomicBool,
) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
    match mode {
        OutputMode::Shared => PipeWireSink::new_cancelable(cancelled)
            .map(|sink| Box::new(sink) as Box<dyn AudioSink>)
            .map_err(|error| OutputError {
                mode,
                message: error.to_string(),
            }),
        OutputMode::Exclusive => {
            let deadline = Instant::now() + EXCLUSIVE_OPEN_TIMEOUT;
            loop {
                if cancelled.load(Ordering::Acquire) {
                    return Err(OutputError {
                        mode,
                        message: "独占输出打开已取消".into(),
                    });
                }
                match AlsaSink::new(&config.exclusive_device) {
                    Ok(sink) => return Ok(Box::new(ResamplingSink::new(Box::new(sink)))),
                    Err(_) if Instant::now() < deadline => {
                        std::thread::sleep(EXCLUSIVE_OPEN_RETRY);
                    }
                    Err(error) => {
                        return Err(OutputError {
                            mode,
                            message: error.to_string(),
                        });
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn open_system_output(
    mode: OutputMode,
    _config: &OutputConfig,
    cancelled: &AtomicBool,
) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
    match mode {
        OutputMode::Shared => CoreAudioSink::new_cancelable(cancelled)
            .map(|sink| Box::new(sink) as Box<dyn AudioSink>)
            .map_err(|error| OutputError {
                mode,
                message: error.to_string(),
            }),
        OutputMode::Exclusive => Err(OutputError {
            mode,
            message: "独占输出仅支持 Linux ALSA".into(),
        }),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn open_system_output(
    mode: OutputMode,
    _config: &OutputConfig,
    _cancelled: &AtomicBool,
) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
    Err(OutputError {
        mode,
        message: "当前平台没有可用的音频输出后端".into(),
    })
}

#[derive(Clone, Default)]
struct PlaybackResume {
    paths: Vec<PathBuf>,
    start: usize,
    position_secs: f64,
    playing: bool,
    has_queue: bool,
    has_current: bool,
}

impl PlaybackResume {
    fn observe_command(&mut self, command: &PlayerCommand) {
        match command {
            PlayerCommand::SetQueue { paths, start } => {
                self.paths.clone_from(paths);
                self.start = (*start).min(paths.len().saturating_sub(1));
                self.position_secs = 0.0;
                self.playing = false;
                self.has_queue = !paths.is_empty();
                self.has_current = false;
            }
            PlayerCommand::Play => {
                self.playing = true;
                self.has_current = self.has_queue;
            }
            PlayerCommand::Pause => self.playing = false,
            PlayerCommand::Stop => {
                self.start = 0;
                self.position_secs = 0.0;
                self.playing = false;
                self.has_current = false;
            }
            PlayerCommand::Seek(secs) => self.position_secs = secs.max(0.0),
            PlayerCommand::Next => {
                if self.start + 1 < self.paths.len() {
                    self.start += 1;
                    self.position_secs = 0.0;
                    self.has_current = true;
                } else {
                    self.position_secs = 0.0;
                    self.playing = false;
                    self.has_current = false;
                }
            }
            PlayerCommand::Prev => {
                if self.has_queue {
                    self.start = self.start.saturating_sub(1);
                    self.position_secs = 0.0;
                    self.has_current = true;
                }
            }
            PlayerCommand::AppendQueueItem(path) => {
                self.paths.push(path.clone());
                self.has_queue = true;
            }
            PlayerCommand::InsertNext(path) => {
                let insertion = (self.start + 1).min(self.paths.len());
                self.paths.insert(insertion, path.clone());
                self.has_queue = true;
            }
            PlayerCommand::RemoveQueueItem(index) => {
                if *index >= self.paths.len() {
                    return;
                }
                self.paths.remove(*index);
                self.has_queue = !self.paths.is_empty();
                if !self.has_queue {
                    self.start = 0;
                    self.position_secs = 0.0;
                    self.playing = false;
                    self.has_current = false;
                } else if *index < self.start {
                    self.start -= 1;
                } else if *index == self.start && self.has_current {
                    self.start = self.start.min(self.paths.len() - 1);
                    self.position_secs = 0.0;
                }
            }
            PlayerCommand::ClearQueue => {
                self.paths.clear();
                self.start = 0;
                self.position_secs = 0.0;
                self.playing = false;
                self.has_queue = false;
                self.has_current = false;
            }
        }
    }

    fn observe_event(&mut self, event: &PlayerEvent) {
        match event {
            PlayerEvent::TrackStarted { index, .. } => {
                self.start = *index;
                self.position_secs = 0.0;
                self.playing = true;
                self.has_current = true;
            }
            PlayerEvent::Progress { secs } => self.position_secs = *secs,
            PlayerEvent::Paused => self.playing = false,
            PlayerEvent::Resumed => self.playing = true,
            PlayerEvent::Stopped | PlayerEvent::QueueFinished => {
                self.playing = false;
                self.has_current = false;
            }
            PlayerEvent::EngineError { .. } => self.playing = false,
            PlayerEvent::TrackError { .. } => {}
        }
    }
}

fn create_player(sink: Box<dyn AudioSink>, resume: &PlaybackResume) -> Player {
    let player = Player::new(sink);
    if resume.has_queue {
        player.send(PlayerCommand::SetQueue {
            paths: resume.paths.clone(),
            start: resume.start,
        });
    }
    if resume.has_current {
        player.send(PlayerCommand::Play);
        if resume.position_secs > 0.0 {
            player.send(PlayerCommand::Seek(resume.position_secs));
        }
        if !resume.playing {
            player.send(PlayerCommand::Pause);
        }
    }
    player
}

struct ActivePlayer {
    player: Player,
    events: Receiver<PlayerEvent>,
}

impl ActivePlayer {
    fn new(sink: Box<dyn AudioSink>, resume: &PlaybackResume) -> Self {
        let player = create_player(sink, resume);
        let events = player.events().clone();
        Self { player, events }
    }
}

fn close_player(
    active: ActivePlayer,
    resume: &mut PlaybackResume,
    events_tx: &Sender<SessionEvent>,
) {
    let ActivePlayer { player, events } = active;
    drop(player);
    for event in events.try_iter() {
        resume.observe_event(&event);
        let _ = events_tx.send(SessionEvent::Playback(event));
    }
}

struct UnavailableState {
    previous: Option<OutputMode>,
    target: OutputMode,
}

enum TransactionResult {
    Active(ActivePlayer),
    Restored {
        player: ActivePlayer,
        error: OutputError,
    },
    Unavailable {
        target_error: OutputError,
        restore_error: OutputError,
    },
}

fn switch_player(
    factory: &dyn OutputAdapterFactory,
    config: &OutputConfig,
    previous: OutputMode,
    target: OutputMode,
    resume: &PlaybackResume,
    cancelled: &AtomicBool,
) -> TransactionResult {
    match factory.open(target, config, cancelled) {
        Ok(sink) => TransactionResult::Active(ActivePlayer::new(sink, resume)),
        Err(error) => match factory.open(previous, config, cancelled) {
            Ok(sink) => TransactionResult::Restored {
                player: ActivePlayer::new(sink, resume),
                error,
            },
            Err(restore_error) => TransactionResult::Unavailable {
                target_error: error,
                restore_error,
            },
        },
    }
}

fn send_playback(
    active: &Option<ActivePlayer>,
    resume: &mut PlaybackResume,
    command: PlayerCommand,
) {
    resume.observe_command(&command);
    if let Some(active) = active {
        active.player.send(command);
    }
}

fn run_session(
    config: OutputConfig,
    factory: Arc<dyn OutputAdapterFactory>,
    command_rx: Receiver<WorkerCommand>,
    events_tx: Sender<SessionEvent>,
    cancelled: Arc<AtomicBool>,
) {
    let mut mode = config.initial_mode;
    let mut resume = PlaybackResume::default();
    let mut active = match factory.open(mode, &config, &cancelled) {
        Ok(sink) => {
            let active = ActivePlayer::new(sink, &resume);
            let _ = events_tx.send(SessionEvent::Active { mode });
            Some(active)
        }
        Err(target_error) => {
            let _ = events_tx.send(SessionEvent::Unavailable {
                target_error,
                restore_error: None,
            });
            None
        }
    };
    let mut unavailable = active.is_none().then_some(UnavailableState {
        previous: None,
        target: mode,
    });
    let mut pending_playback = VecDeque::new();
    let mut pending_switch = None;
    let mut pending_retry = false;

    'worker: loop {
        let command = if let Some(target) = pending_switch.take() {
            WorkerCommand::Public(SessionCommand::Switch(target))
        } else if pending_retry {
            pending_retry = false;
            WorkerCommand::Public(SessionCommand::RetryOutput)
        } else {
            let player_events = active
                .as_ref()
                .map(|active| active.events.clone())
                .unwrap_or_else(never);
            select! {
                recv(player_events) -> event => {
                    if let Ok(event) = event {
                        resume.observe_event(&event);
                        let _ = events_tx.send(SessionEvent::Playback(event));
                    }
                    continue;
                }
                recv(command_rx) -> command => {
                    let Ok(command) = command else {
                        break 'worker;
                    };
                    command
                }
            }
        };

        let mut completed_transaction = false;
        match command {
            WorkerCommand::Public(SessionCommand::Playback(command)) => {
                if active.is_some() {
                    send_playback(&active, &mut resume, command);
                } else {
                    pending_playback.push_back(command);
                }
            }
            WorkerCommand::Public(SessionCommand::Switch(target)) => {
                if active.is_some() && target == mode {
                    continue;
                }
                let previous = mode;
                let _ = events_tx.send(SessionEvent::Switching {
                    from: previous,
                    to: target,
                });
                if let Some(player) = active.take() {
                    close_player(player, &mut resume, &events_tx);
                }
                match switch_player(
                    factory.as_ref(),
                    &config,
                    previous,
                    target,
                    &resume,
                    &cancelled,
                ) {
                    TransactionResult::Active(player) => {
                        active = Some(player);
                        mode = target;
                        unavailable = None;
                        let _ = events_tx.send(SessionEvent::Active { mode });
                    }
                    TransactionResult::Restored { player, error } => {
                        active = Some(player);
                        unavailable = None;
                        let _ = events_tx.send(SessionEvent::Restored {
                            mode: previous,
                            error,
                        });
                    }
                    TransactionResult::Unavailable {
                        target_error,
                        restore_error,
                    } => {
                        unavailable = Some(UnavailableState {
                            previous: Some(previous),
                            target,
                        });
                        let _ = events_tx.send(SessionEvent::Unavailable {
                            target_error,
                            restore_error: Some(restore_error),
                        });
                    }
                }
                completed_transaction = true;
            }
            WorkerCommand::Public(SessionCommand::RetryOutput) => {
                let Some(failed) = unavailable.take() else {
                    continue;
                };
                let from = failed.previous.unwrap_or(failed.target);
                let _ = events_tx.send(SessionEvent::Switching {
                    from,
                    to: failed.target,
                });
                match failed.previous {
                    Some(previous) => match switch_player(
                        factory.as_ref(),
                        &config,
                        previous,
                        failed.target,
                        &resume,
                        &cancelled,
                    ) {
                        TransactionResult::Active(player) => {
                            active = Some(player);
                            mode = failed.target;
                            let _ = events_tx.send(SessionEvent::Active { mode });
                        }
                        TransactionResult::Restored { player, error } => {
                            active = Some(player);
                            mode = previous;
                            let _ = events_tx.send(SessionEvent::Restored { mode, error });
                        }
                        TransactionResult::Unavailable {
                            target_error,
                            restore_error,
                        } => {
                            unavailable = Some(failed);
                            let _ = events_tx.send(SessionEvent::Unavailable {
                                target_error,
                                restore_error: Some(restore_error),
                            });
                        }
                    },
                    None => match factory.open(failed.target, &config, &cancelled) {
                        Ok(sink) => {
                            active = Some(ActivePlayer::new(sink, &resume));
                            mode = failed.target;
                            let _ = events_tx.send(SessionEvent::Active { mode });
                        }
                        Err(target_error) => {
                            unavailable = Some(failed);
                            let _ = events_tx.send(SessionEvent::Unavailable {
                                target_error,
                                restore_error: None,
                            });
                        }
                    },
                }
                completed_transaction = true;
            }
            WorkerCommand::Quit => break,
        }

        if !completed_transaction {
            continue;
        }

        let mut latest_switch = None;
        let mut retry_requested = false;
        while let Ok(command) = command_rx.try_recv() {
            match command {
                WorkerCommand::Public(SessionCommand::Playback(command)) => {
                    pending_playback.push_back(command);
                }
                WorkerCommand::Public(SessionCommand::Switch(target)) => {
                    latest_switch = Some(target);
                }
                WorkerCommand::Public(SessionCommand::RetryOutput) => retry_requested = true,
                WorkerCommand::Quit => break 'worker,
            }
        }
        if active.is_some() {
            while let Some(command) = pending_playback.pop_front() {
                send_playback(&active, &mut resume, command);
            }
        }
        pending_switch = latest_switch.filter(|target| active.is_none() || *target != mode);
        pending_retry = retry_requested && unavailable.is_some() && pending_switch.is_none();
    }
}

pub struct OutputSession {
    commands: Sender<WorkerCommand>,
    events: Receiver<SessionEvent>,
    cancelled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl OutputSession {
    pub fn start(config: OutputConfig) -> Self {
        Self::start_inner(config, Arc::new(SystemOutputAdapterFactory))
    }

    #[cfg(test)]
    fn start_with_factory(config: OutputConfig, factory: Arc<dyn OutputAdapterFactory>) -> Self {
        Self::start_inner(config, factory)
    }

    fn start_inner(config: OutputConfig, factory: Arc<dyn OutputAdapterFactory>) -> Self {
        let (commands, command_rx) = unbounded();
        let (events_tx, events) = unbounded();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let worker = std::thread::Builder::new()
            .name("liusheng-output-session".into())
            .spawn(move || run_session(config, factory, command_rx, events_tx, worker_cancelled))
            .expect("无法创建输出会话线程");
        Self {
            commands,
            events,
            cancelled,
            worker: Some(worker),
        }
    }

    pub fn send(&self, command: SessionCommand) {
        let _ = self.commands.send(WorkerCommand::Public(command));
    }

    pub fn events(&self) -> &Receiver<SessionEvent> {
        &self.events
    }
}

impl Drop for OutputSession {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.send(WorkerCommand::Quit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    use crate::audio::PcmSpec;
    use crate::audio::sink::AudioSink;
    use crate::error::Result;

    use super::*;

    struct SilentSink;

    impl AudioSink for SilentSink {
        fn write(&mut self, _spec: PcmSpec, _samples: &[i32]) -> Result<()> {
            Ok(())
        }
    }

    struct PacedSink;

    impl AudioSink for PacedSink {
        fn write(&mut self, _spec: PcmSpec, _samples: &[i32]) -> Result<()> {
            std::thread::sleep(Duration::from_millis(5));
            Ok(())
        }
    }

    struct AlwaysOpensPaced;

    impl OutputAdapterFactory for AlwaysOpensPaced {
        fn open(
            &self,
            _mode: OutputMode,
            _config: &OutputConfig,
            _cancelled: &AtomicBool,
        ) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
            Ok(Box::new(PacedSink))
        }
    }

    struct GateDiscardSink {
        discard_count: usize,
        gate_at: usize,
        discard_started: crossbeam_channel::Sender<()>,
        release_discard: crossbeam_channel::Receiver<()>,
    }

    impl AudioSink for GateDiscardSink {
        fn write(&mut self, _spec: PcmSpec, _samples: &[i32]) -> Result<()> {
            std::thread::sleep(Duration::from_millis(5));
            Ok(())
        }

        fn discard(&mut self) -> Result<()> {
            self.discard_count += 1;
            if self.discard_count == self.gate_at {
                let _ = self.discard_started.send(());
                self.release_discard.recv().unwrap();
            }
            Ok(())
        }
    }

    struct GateFirstPlayerFactory {
        opens: Mutex<usize>,
        gate_at: usize,
        discard_started: crossbeam_channel::Sender<()>,
        release_discard: crossbeam_channel::Receiver<()>,
    }

    impl OutputAdapterFactory for GateFirstPlayerFactory {
        fn open(
            &self,
            _mode: OutputMode,
            _config: &OutputConfig,
            _cancelled: &AtomicBool,
        ) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
            let open_count = {
                let mut opens = self.opens.lock().unwrap();
                *opens += 1;
                *opens
            };
            if open_count == 1 {
                Ok(Box::new(GateDiscardSink {
                    discard_count: 0,
                    gate_at: self.gate_at,
                    discard_started: self.discard_started.clone(),
                    release_discard: self.release_discard.clone(),
                }))
            } else {
                Ok(Box::new(PacedSink))
            }
        }
    }

    struct GatedRecordingFactory {
        opens: Arc<Mutex<Vec<OutputMode>>>,
        second_started: crossbeam_channel::Sender<()>,
        release_second: crossbeam_channel::Receiver<()>,
    }

    struct SlowSecondOpen {
        opens: Mutex<usize>,
        second_started: crossbeam_channel::Sender<()>,
    }

    impl OutputAdapterFactory for SlowSecondOpen {
        fn open(
            &self,
            mode: OutputMode,
            _config: &OutputConfig,
            cancelled: &AtomicBool,
        ) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
            let count = {
                let mut opens = self.opens.lock().unwrap();
                *opens += 1;
                *opens
            };
            if count == 2 {
                let _ = self.second_started.send(());
                let deadline = std::time::Instant::now() + Duration::from_millis(500);
                while std::time::Instant::now() < deadline {
                    if cancelled.load(Ordering::Acquire) {
                        return Err(OutputError {
                            mode,
                            message: "cancelled".into(),
                        });
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            Ok(Box::new(SilentSink))
        }
    }

    impl OutputAdapterFactory for GatedRecordingFactory {
        fn open(
            &self,
            mode: OutputMode,
            _config: &OutputConfig,
            _cancelled: &AtomicBool,
        ) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
            let open_count = {
                let mut opens = self.opens.lock().unwrap();
                opens.push(mode);
                opens.len()
            };
            if open_count == 2 {
                let _ = self.second_started.send(());
                self.release_second.recv().unwrap();
            }
            Ok(Box::new(PacedSink))
        }
    }

    struct AlwaysOpens;

    impl OutputAdapterFactory for AlwaysOpens {
        fn open(
            &self,
            _mode: OutputMode,
            _config: &OutputConfig,
            _cancelled: &AtomicBool,
        ) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
            Ok(Box::new(SilentSink))
        }
    }

    struct ScriptedFactory {
        results: Mutex<VecDeque<std::result::Result<(), &'static str>>>,
    }

    impl ScriptedFactory {
        fn new(results: impl IntoIterator<Item = std::result::Result<(), &'static str>>) -> Self {
            Self {
                results: Mutex::new(results.into_iter().collect()),
            }
        }
    }

    impl OutputAdapterFactory for ScriptedFactory {
        fn open(
            &self,
            mode: OutputMode,
            _config: &OutputConfig,
            _cancelled: &AtomicBool,
        ) -> std::result::Result<Box<dyn AudioSink>, OutputError> {
            match self.results.lock().unwrap().pop_front().unwrap() {
                Ok(()) => Ok(Box::new(SilentSink)),
                Err(message) => Err(OutputError {
                    mode,
                    message: message.into(),
                }),
            }
        }
    }

    fn write_test_track(path: &std::path::Path, frames: usize) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..frames {
            writer.write_sample(0_i16).unwrap();
            writer.write_sample(0_i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn initial_mode_becomes_active() {
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(AlwaysOpens),
        );

        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Active {
                mode: OutputMode::Shared
            })
        ));
    }

    #[test]
    fn switching_output_publishes_one_transaction() {
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(AlwaysOpens),
        );
        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Active {
                mode: OutputMode::Shared
            })
        ));

        session.send(SessionCommand::Switch(OutputMode::Exclusive));

        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Switching {
                from: OutputMode::Shared,
                to: OutputMode::Exclusive
            })
        ));
        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Active {
                mode: OutputMode::Exclusive
            })
        ));
    }

    #[test]
    fn failed_target_restores_the_previous_mode() {
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(ScriptedFactory::new([Ok(()), Err("target failed"), Ok(())])),
        );
        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Active {
                mode: OutputMode::Shared
            })
        ));

        session.send(SessionCommand::Switch(OutputMode::Exclusive));

        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Switching { .. })
        ));
        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Restored {
                mode: OutputMode::Shared,
                error: OutputError {
                    mode: OutputMode::Exclusive,
                    ref message,
                },
            }) if message == "target failed"
        ));
    }

    #[test]
    fn unavailable_session_can_retry_the_target_mode() {
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(ScriptedFactory::new([
                Ok(()),
                Err("target failed"),
                Err("restore failed"),
                Ok(()),
            ])),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        session.send(SessionCommand::Switch(OutputMode::Exclusive));
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Unavailable {
                target_error: OutputError {
                    mode: OutputMode::Exclusive,
                    ref message,
                },
                restore_error: Some(OutputError {
                    mode: OutputMode::Shared,
                    message: ref restore_message,
                }),
            }) if message == "target failed" && restore_message == "restore failed"
        ));

        session.send(SessionCommand::RetryOutput);

        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Switching {
                from: OutputMode::Shared,
                to: OutputMode::Exclusive,
            })
        ));
        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Active {
                mode: OutputMode::Exclusive,
            })
        ));
    }

    #[test]
    fn playback_commands_and_events_cross_the_session_interface() {
        let dir = tempfile::tempdir().unwrap();
        let track = dir.path().join("track.wav");
        write_test_track(&track, 8_000);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(AlwaysOpens),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        session.send(SessionCommand::Playback(PlayerCommand::SetQueue {
            paths: vec![track.clone()],
            start: 0,
        }));
        session.send(SessionCommand::Playback(PlayerCommand::Play));

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut started = false;
        while std::time::Instant::now() < deadline {
            if matches!(
                session.events().recv_timeout(Duration::from_millis(50)),
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, .. })) if path == track
            ) {
                started = true;
                break;
            }
        }
        assert!(started, "输出会话没有转发 TrackStarted");
    }

    #[test]
    fn switching_output_restores_the_current_track_and_recent_progress() {
        let dir = tempfile::tempdir().unwrap();
        let track = dir.path().join("track.wav");
        write_test_track(&track, 8_000 * 8);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(AlwaysOpensPaced),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Playback(PlayerCommand::SetQueue {
            paths: vec![track.clone()],
            start: 0,
        }));
        session.send(SessionCommand::Playback(PlayerCommand::Play));

        let mut checkpoint = None;
        while checkpoint.is_none() {
            match session
                .events()
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
            {
                SessionEvent::Playback(PlayerEvent::Progress { secs }) if secs >= 0.5 => {
                    checkpoint = Some(secs);
                }
                _ => {}
            }
        }
        session.send(SessionCommand::Switch(OutputMode::Exclusive));

        let mut active = false;
        let mut restarted = false;
        let mut restored_progress = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline && restored_progress.is_none() {
            match session.events().recv_timeout(Duration::from_millis(100)) {
                Ok(SessionEvent::Active {
                    mode: OutputMode::Exclusive,
                }) => active = true,
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, .. }))
                    if active && path == track =>
                {
                    restarted = true;
                }
                Ok(SessionEvent::Playback(PlayerEvent::Progress { secs }))
                    if restarted && secs >= checkpoint.unwrap() =>
                {
                    restored_progress = Some(secs);
                }
                _ => {}
            }
        }

        assert!(active, "切换后未进入独占输出");
        assert!(restarted, "切换后未恢复当前曲目");
        assert!(restored_progress.is_some(), "切换后未恢复最近进度");
    }

    #[test]
    fn switching_immediately_after_play_restores_the_queued_track() {
        let dir = tempfile::tempdir().unwrap();
        let track = dir.path().join("track.wav");
        write_test_track(&track, 8_000 * 8);
        let (discard_started_tx, discard_started_rx) = crossbeam_channel::bounded(1);
        let (release_discard_tx, release_discard_rx) = crossbeam_channel::bounded(1);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(GateFirstPlayerFactory {
                opens: Mutex::new(0),
                gate_at: 1,
                discard_started: discard_started_tx,
                release_discard: release_discard_rx,
            }),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        session.send(SessionCommand::Playback(PlayerCommand::SetQueue {
            paths: vec![track.clone()],
            start: 0,
        }));
        discard_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Playback(PlayerCommand::Play));
        session.send(SessionCommand::Switch(OutputMode::Exclusive));
        let switching = loop {
            match session.events().recv_timeout(Duration::from_secs(1)) {
                Ok(SessionEvent::Switching { .. }) => break true,
                Ok(_) => {}
                Err(_) => break false,
            }
        };
        release_discard_tx.send(()).unwrap();
        assert!(switching, "快速播放后未开始输出切换");

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut active = false;
        let mut restarted = false;
        while std::time::Instant::now() < deadline && !restarted {
            match session.events().recv_timeout(Duration::from_millis(100)) {
                Ok(SessionEvent::Active {
                    mode: OutputMode::Exclusive,
                }) => active = true,
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, .. }))
                    if active && path == track =>
                {
                    restarted = true;
                }
                _ => {}
            }
        }

        assert!(active && restarted, "快速切换后未恢复刚开始的曲目");
    }

    #[test]
    fn switching_during_next_restores_the_new_track() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.wav");
        let second = dir.path().join("second.wav");
        write_test_track(&first, 8_000 * 8);
        write_test_track(&second, 8_000 * 8);
        let (discard_started_tx, discard_started_rx) = crossbeam_channel::bounded(1);
        let (release_discard_tx, release_discard_rx) = crossbeam_channel::bounded(1);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(GateFirstPlayerFactory {
                opens: Mutex::new(0),
                gate_at: 2,
                discard_started: discard_started_tx,
                release_discard: release_discard_rx,
            }),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Playback(PlayerCommand::SetQueue {
            paths: vec![first.clone(), second.clone()],
            start: 0,
        }));
        session.send(SessionCommand::Playback(PlayerCommand::Play));
        loop {
            if matches!(
                session.events().recv_timeout(Duration::from_secs(2)),
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, .. })) if path == first
            ) {
                break;
            }
        }

        session.send(SessionCommand::Playback(PlayerCommand::Next));
        discard_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Switch(OutputMode::Exclusive));
        let switching = loop {
            match session.events().recv_timeout(Duration::from_secs(1)) {
                Ok(SessionEvent::Switching { .. }) => break true,
                Ok(_) => {}
                Err(_) => break false,
            }
        };
        release_discard_tx.send(()).unwrap();
        assert!(switching, "快速切歌后未开始输出切换");

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut active = false;
        let mut restarted = None;
        while std::time::Instant::now() < deadline && restarted.is_none() {
            match session.events().recv_timeout(Duration::from_millis(100)) {
                Ok(SessionEvent::Active {
                    mode: OutputMode::Exclusive,
                }) => active = true,
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { index, path, .. }))
                    if active =>
                {
                    restarted = Some((index, path));
                }
                _ => {}
            }
        }

        assert_eq!(restarted, Some((1, second)), "快速切换后恢复了旧曲目");
    }

    #[test]
    fn switching_after_removing_the_current_track_restores_the_updated_queue() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.wav");
        let next = dir.path().join("next.wav");
        write_test_track(&first, 8_000 * 8);
        write_test_track(&next, 8_000 * 8);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(AlwaysOpensPaced),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Playback(PlayerCommand::SetQueue {
            paths: vec![first.clone(), next.clone()],
            start: 0,
        }));
        session.send(SessionCommand::Playback(PlayerCommand::Play));
        loop {
            if matches!(
                session.events().recv_timeout(Duration::from_secs(2)),
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, .. })) if path == first
            ) {
                break;
            }
        }

        session.send(SessionCommand::Playback(PlayerCommand::RemoveQueueItem(0)));
        loop {
            if matches!(
                session.events().recv_timeout(Duration::from_secs(2)),
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { ref path, index, .. }))
                    if *path == next && index == 0
            ) {
                break;
            }
        }
        session.send(SessionCommand::Switch(OutputMode::Exclusive));

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut active = false;
        let mut restored = false;
        while std::time::Instant::now() < deadline && !restored {
            match session.events().recv_timeout(Duration::from_millis(100)) {
                Ok(SessionEvent::Active {
                    mode: OutputMode::Exclusive,
                }) => active = true,
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, index, .. }))
                    if active && path == next && index == 0 =>
                {
                    restored = true;
                }
                _ => {}
            }
        }

        assert!(active && restored, "切换后恢复了移除前的旧队列");
    }

    #[test]
    fn switching_after_inserting_next_restores_the_updated_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.wav");
        let original_next = dir.path().join("original-next.wav");
        let inserted = dir.path().join("inserted.wav");
        write_test_track(&first, 8_000 * 8);
        write_test_track(&original_next, 8_000 * 8);
        write_test_track(&inserted, 8_000 * 8);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(AlwaysOpensPaced),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Playback(PlayerCommand::SetQueue {
            paths: vec![first.clone(), original_next],
            start: 0,
        }));
        session.send(SessionCommand::Playback(PlayerCommand::Play));
        loop {
            if matches!(
                session.events().recv_timeout(Duration::from_secs(2)),
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, .. })) if path == first
            ) {
                break;
            }
        }

        session.send(SessionCommand::Playback(PlayerCommand::InsertNext(
            inserted.clone(),
        )));
        session.send(SessionCommand::Switch(OutputMode::Exclusive));

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut active = false;
        let mut restored = false;
        while std::time::Instant::now() < deadline && !restored {
            match session.events().recv_timeout(Duration::from_millis(100)) {
                Ok(SessionEvent::Active {
                    mode: OutputMode::Exclusive,
                }) => active = true,
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, index, .. }))
                    if active && path == first && index == 0 =>
                {
                    session.send(SessionCommand::Playback(PlayerCommand::Next));
                }
                Ok(SessionEvent::Playback(PlayerEvent::TrackStarted { path, index, .. }))
                    if active && path == inserted && index == 1 =>
                {
                    restored = true;
                }
                _ => {}
            }
        }

        assert!(active && restored, "切换后未保留插入的下一首");
    }

    #[test]
    fn switches_received_during_a_transaction_collapse_to_the_last_target() {
        let opens = Arc::new(Mutex::new(Vec::new()));
        let (second_started_tx, second_started_rx) = crossbeam_channel::bounded(1);
        let (release_tx, release_rx) = crossbeam_channel::bounded(1);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(GatedRecordingFactory {
                opens: opens.clone(),
                second_started: second_started_tx,
                release_second: release_rx,
            }),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        session.send(SessionCommand::Switch(OutputMode::Exclusive));
        second_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Switch(OutputMode::Shared));
        session.send(SessionCommand::Switch(OutputMode::Exclusive));
        release_tx.send(()).unwrap();

        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Switching { .. })
        ));
        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Active {
                mode: OutputMode::Exclusive
            })
        ));
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(
            opens.lock().unwrap().as_slice(),
            &[OutputMode::Shared, OutputMode::Exclusive]
        );
    }

    #[test]
    fn dropping_the_session_cancels_an_output_open() {
        let (second_started_tx, second_started_rx) = crossbeam_channel::bounded(1);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(SlowSecondOpen {
                opens: Mutex::new(0),
                second_started: second_started_tx,
            }),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Switch(OutputMode::Exclusive));
        second_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        let started = std::time::Instant::now();
        drop(session);

        assert!(
            started.elapsed() < Duration::from_millis(100),
            "退出等待了正在打开的 adapter"
        );
    }

    #[test]
    fn output_can_switch_back_to_shared_mode() {
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(AlwaysOpens),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Switch(OutputMode::Exclusive));
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        session.send(SessionCommand::Switch(OutputMode::Shared));

        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Switching {
                from: OutputMode::Exclusive,
                to: OutputMode::Shared,
            })
        ));
        assert!(matches!(
            session.events().recv_timeout(Duration::from_secs(1)),
            Ok(SessionEvent::Active {
                mode: OutputMode::Shared,
            })
        ));
    }

    #[test]
    fn paused_playback_remains_paused_after_switching_output() {
        let dir = tempfile::tempdir().unwrap();
        let track = dir.path().join("track.wav");
        write_test_track(&track, 8_000 * 8);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(AlwaysOpensPaced),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Playback(PlayerCommand::SetQueue {
            paths: vec![track],
            start: 0,
        }));
        session.send(SessionCommand::Playback(PlayerCommand::Play));
        loop {
            if matches!(
                session.events().recv_timeout(Duration::from_secs(2)),
                Ok(SessionEvent::Playback(PlayerEvent::Progress { secs })) if secs >= 0.5
            ) {
                break;
            }
        }
        session.send(SessionCommand::Playback(PlayerCommand::Pause));
        loop {
            if matches!(
                session.events().recv_timeout(Duration::from_secs(1)),
                Ok(SessionEvent::Playback(PlayerEvent::Paused))
            ) {
                break;
            }
        }

        session.send(SessionCommand::Switch(OutputMode::Exclusive));

        let mut active = false;
        let mut paused = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline && !paused {
            match session.events().recv_timeout(Duration::from_millis(100)) {
                Ok(SessionEvent::Active {
                    mode: OutputMode::Exclusive,
                }) => active = true,
                Ok(SessionEvent::Playback(PlayerEvent::Paused)) if active => paused = true,
                _ => {}
            }
        }
        assert!(active && paused, "切换后未恢复暂停状态");
    }

    #[test]
    fn playback_commands_wait_for_an_in_flight_switch() {
        let dir = tempfile::tempdir().unwrap();
        let track = dir.path().join("track.wav");
        write_test_track(&track, 8_000 * 8);
        let opens = Arc::new(Mutex::new(Vec::new()));
        let (second_started_tx, second_started_rx) = crossbeam_channel::bounded(1);
        let (release_tx, release_rx) = crossbeam_channel::bounded(1);
        let session = OutputSession::start_with_factory(
            OutputConfig {
                initial_mode: OutputMode::Shared,
                exclusive_device: "test-device".into(),
            },
            Arc::new(GatedRecordingFactory {
                opens,
                second_started: second_started_tx,
                release_second: release_rx,
            }),
        );
        let _ = session
            .events()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Playback(PlayerCommand::SetQueue {
            paths: vec![track],
            start: 0,
        }));
        session.send(SessionCommand::Playback(PlayerCommand::Play));
        loop {
            if matches!(
                session.events().recv_timeout(Duration::from_secs(2)),
                Ok(SessionEvent::Playback(PlayerEvent::Progress { secs })) if secs >= 0.5
            ) {
                break;
            }
        }

        session.send(SessionCommand::Switch(OutputMode::Exclusive));
        second_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        session.send(SessionCommand::Playback(PlayerCommand::Stop));
        release_tx.send(()).unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut active = false;
        let mut stopped = false;
        while std::time::Instant::now() < deadline && !stopped {
            match session.events().recv_timeout(Duration::from_millis(100)) {
                Ok(SessionEvent::Active {
                    mode: OutputMode::Exclusive,
                }) => active = true,
                Ok(SessionEvent::Playback(PlayerEvent::Stopped)) if active => stopped = true,
                _ => {}
            }
        }
        assert!(active && stopped, "切换期间的 Stop 未交给新 Player");
    }
}
