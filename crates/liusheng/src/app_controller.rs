use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use liusheng_core::audio::alsa_sink::AlsaSink;
use liusheng_core::audio::hardware_volume::{HardwareVolume, VolumeChange, VolumeState};
use liusheng_core::audio::pipewire_sink::PipeWireSink;
use liusheng_core::audio::resampling_sink::ResamplingSink;
use liusheng_core::audio::sink::AudioSink;
use liusheng_core::engine::{Command, Player, PlayerEvent};
use liusheng_core::library::watcher::{LibraryWatchEvent, LibraryWatcher};
use liusheng_core::library::{AlbumSummary, Library, ScanStats, TrackRow};

use crate::mpris::{
    Command as MprisCommand, PlaybackSnapshot, PlaybackStatus as MprisPlaybackStatus,
    Service as MprisService,
};

const EXCLUSIVE_DEVICE: &str = "hw:Hybrid,0";
const EXCLUSIVE_OPEN_TIMEOUT: Duration = Duration::from_secs(6);
const EXCLUSIVE_OPEN_RETRY: Duration = Duration::from_millis(100);
const HARDWARE_MIXER_DEVICE: &str = "hw:Hybrid";
const HARDWARE_MIXER_ELEMENT: &str = "PCM";
const MUSIC_ROOT: &str = "/data/Music";

#[derive(Clone)]
struct PlaybackResume {
    paths: Vec<PathBuf>,
    start: usize,
    position_secs: f64,
    playing: bool,
}

enum OutputSwitchOutcome {
    Switched(Player),
    Restored {
        player: Player,
        target_error: String,
    },
    Failed {
        target_error: String,
        restore_error: String,
    },
}

pub struct AppControllerRust {
    status: QString,
    track_count: i32,
    album_count: i32,
    selected_album_index: i32,
    selected_track_count: i32,
    album_open: bool,
    scanning: bool,
    playback_initializing: bool,
    has_current_track: bool,
    seekable: bool,
    playing: bool,
    current_title: QString,
    current_artist: QString,
    current_track_path: QString,
    current_duration_ms: i32,
    position_ms: i32,
    playback_error: QString,
    exclusive_output: bool,
    output_switching: bool,
    output_status: QString,
    output_error: QString,
    hardware_volume_available: bool,
    hardware_volume_percent: i32,
    hardware_muted: bool,
    hardware_mute_available: bool,
    hardware_volume_error: QString,
    albums: Vec<AlbumSummary>,
    tracks: Vec<TrackRow>,
    selected_tracks: Vec<TrackRow>,
    playback_queue: Vec<TrackRow>,
    current_queue_index: Option<usize>,
    player: Option<Player>,
    mpris: Option<MprisService>,
    hardware_volume: Option<HardwareVolume>,
    library_watcher: Option<LibraryWatcher>,
    library_rescan_pending: bool,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            status: QString::from("曲库待扫描"),
            track_count: 0,
            album_count: 0,
            selected_album_index: -1,
            selected_track_count: 0,
            album_open: false,
            scanning: false,
            playback_initializing: false,
            has_current_track: false,
            seekable: false,
            playing: false,
            current_title: QString::default(),
            current_artist: QString::default(),
            current_track_path: QString::default(),
            current_duration_ms: 0,
            position_ms: 0,
            playback_error: QString::default(),
            exclusive_output: false,
            output_switching: false,
            output_status: QString::from("PipeWire"),
            output_error: QString::default(),
            hardware_volume_available: false,
            hardware_volume_percent: 100,
            hardware_muted: false,
            hardware_mute_available: false,
            hardware_volume_error: QString::from("正在检测硬件音量"),
            albums: Vec::new(),
            tracks: Vec::new(),
            selected_tracks: Vec::new(),
            playback_queue: Vec::new(),
            current_queue_index: None,
            player: None,
            mpris: None,
            hardware_volume: None,
            library_watcher: None,
            library_rescan_pending: false,
        }
    }
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status)]
        #[qproperty(i32, track_count, cxx_name = "trackCount")]
        #[qproperty(i32, album_count, cxx_name = "albumCount")]
        #[qproperty(i32, selected_album_index, cxx_name = "selectedAlbumIndex")]
        #[qproperty(i32, selected_track_count, cxx_name = "selectedTrackCount")]
        #[qproperty(bool, album_open, cxx_name = "albumOpen")]
        #[qproperty(bool, scanning)]
        #[qproperty(bool, playback_initializing, cxx_name = "playbackInitializing")]
        #[qproperty(bool, has_current_track, cxx_name = "hasCurrentTrack")]
        #[qproperty(bool, seekable)]
        #[qproperty(bool, playing)]
        #[qproperty(QString, current_title, cxx_name = "currentTitle")]
        #[qproperty(QString, current_artist, cxx_name = "currentArtist")]
        #[qproperty(QString, current_track_path, cxx_name = "currentTrackPath")]
        #[qproperty(i32, current_duration_ms, cxx_name = "currentDurationMs")]
        #[qproperty(i32, position_ms, cxx_name = "positionMs")]
        #[qproperty(QString, playback_error, cxx_name = "playbackError")]
        #[qproperty(bool, exclusive_output, cxx_name = "exclusiveOutput")]
        #[qproperty(bool, output_switching, cxx_name = "outputSwitching")]
        #[qproperty(QString, output_status, cxx_name = "outputStatus")]
        #[qproperty(QString, output_error, cxx_name = "outputError")]
        #[qproperty(bool, hardware_volume_available, cxx_name = "hardwareVolumeAvailable")]
        #[qproperty(i32, hardware_volume_percent, cxx_name = "hardwareVolumePercent")]
        #[qproperty(bool, hardware_muted, cxx_name = "hardwareMuted")]
        #[qproperty(bool, hardware_mute_available, cxx_name = "hardwareMuteAvailable")]
        #[qproperty(QString, hardware_volume_error, cxx_name = "hardwareVolumeError")]
        #[namespace = "liusheng"]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        #[cxx_name = "scanLibrary"]
        fn scan_library(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "albumTitle"]
        fn album_title(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "albumArtist"]
        fn album_artist(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "albumTrackCount"]
        fn album_track_count(&self, index: i32) -> i32;

        #[qinvokable]
        #[cxx_name = "albumYear"]
        fn album_year(&self, index: i32) -> i32;

        #[qinvokable]
        #[cxx_name = "openAlbum"]
        fn open_album(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "closeAlbum"]
        fn close_album(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "selectedTrackTitle"]
        fn selected_track_title(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "selectedTrackArtist"]
        fn selected_track_artist(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "selectedTrackNumber"]
        fn selected_track_number(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "selectedTrackDurationMs"]
        fn selected_track_duration_ms(&self, index: i32) -> i32;

        #[qinvokable]
        #[cxx_name = "selectedTrackPath"]
        fn selected_track_path(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "playSelectedTrack"]
        fn play_selected_track(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "togglePlayback"]
        fn toggle_playback(&self);

        #[qinvokable]
        #[cxx_name = "previousTrack"]
        fn previous_track(&self);

        #[qinvokable]
        #[cxx_name = "nextTrack"]
        fn next_track(&self);

        #[qinvokable]
        #[cxx_name = "seekTo"]
        fn seek_to(self: Pin<&mut Self>, position_ms: i32);

        #[qinvokable]
        #[cxx_name = "requestExclusiveOutput"]
        fn request_exclusive_output(self: Pin<&mut Self>, exclusive: bool);

        #[qinvokable]
        #[cxx_name = "refreshHardwareVolume"]
        fn refresh_hardware_volume(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "requestHardwareVolume"]
        fn request_hardware_volume(self: Pin<&mut Self>, percent: i32);

        #[qinvokable]
        #[cxx_name = "toggleHardwareMute"]
        fn toggle_hardware_mute(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for AppController {}
}

impl qobject::AppController {
    pub fn scan_library(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().ensure_mpris();
        self.as_mut().request_library_scan();
    }

    fn request_library_scan(mut self: core::pin::Pin<&mut Self>) {
        if *self.scanning() {
            self.as_mut().rust_mut().get_mut().library_rescan_pending = true;
            return;
        }
        self.as_mut().rust_mut().get_mut().library_rescan_pending = false;
        self.as_mut().set_scanning(true);
        self.as_mut().set_status(QString::from("正在扫描曲库"));
        let watcher_error = self.as_mut().ensure_library_watcher().err();
        let qt_thread = self.qt_thread();

        std::thread::spawn(move || {
            let result = scan_default_library();
            qt_thread
                .queue(move |mut controller| {
                    controller.as_mut().set_scanning(false);
                    match result {
                        Ok(outcome) => {
                            let mut status = outcome.status_text();
                            if let Some(error) = watcher_error.as_deref() {
                                status.push_str(&format!("；曲库监听失败：{error}"));
                            }
                            let album_count = outcome.albums.len().min(i32::MAX as usize) as i32;
                            controller.as_mut().rust_mut().get_mut().albums = outcome.albums;
                            controller.as_mut().rust_mut().get_mut().tracks = outcome.tracks;
                            controller
                                .as_mut()
                                .rust_mut()
                                .get_mut()
                                .selected_tracks
                                .clear();
                            controller
                                .as_mut()
                                .set_track_count(outcome.track_count.min(i32::MAX as u64) as i32);
                            controller.as_mut().set_album_count(album_count);
                            controller.as_mut().set_selected_album_index(-1);
                            controller.as_mut().set_selected_track_count(0);
                            controller.as_mut().set_album_open(false);
                            controller.as_mut().set_status(QString::from(&status));
                        }
                        Err(message) => {
                            let status = match watcher_error.as_deref() {
                                Some(error) => format!("{message}；曲库监听失败：{error}"),
                                None => message,
                            };
                            controller.as_mut().set_status(QString::from(&status));
                        }
                    }
                    let rescan = std::mem::take(
                        &mut controller
                            .as_mut()
                            .rust_mut()
                            .get_mut()
                            .library_rescan_pending,
                    );
                    if rescan {
                        controller.as_mut().request_library_scan();
                    }
                })
                .ok();
        });
    }

    pub fn album_title(&self, index: i32) -> QString {
        self.album_at(index)
            .map(|album| QString::from(&album.title))
            .unwrap_or_default()
    }

    pub fn album_artist(&self, index: i32) -> QString {
        self.album_at(index)
            .map(|album| QString::from(&album.artist))
            .unwrap_or_default()
    }

    pub fn album_track_count(&self, index: i32) -> i32 {
        self.album_at(index)
            .map(|album| album.track_count.min(i32::MAX as u32) as i32)
            .unwrap_or_default()
    }

    pub fn album_year(&self, index: i32) -> i32 {
        self.album_at(index)
            .and_then(|album| album.year)
            .and_then(|year| i32::try_from(year).ok())
            .unwrap_or_default()
    }

    pub fn open_album(mut self: core::pin::Pin<&mut Self>, index: i32) {
        let Some(key) = self.album_at(index).map(|album| album.key.clone()) else {
            return;
        };
        let selected_tracks: Vec<_> = self
            .rust()
            .tracks
            .iter()
            .filter(|track| track.album == key.album && track.album_artist == key.album_artist)
            .cloned()
            .collect();
        let selected_track_count = selected_tracks.len().min(i32::MAX as usize) as i32;
        self.as_mut().rust_mut().get_mut().selected_tracks = selected_tracks;
        self.as_mut().set_selected_album_index(index);
        self.as_mut().set_selected_track_count(selected_track_count);
        self.as_mut().set_album_open(true);
    }

    pub fn close_album(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().set_album_open(false);
        self.as_mut().set_selected_album_index(-1);
        self.as_mut().set_selected_track_count(0);
        self.as_mut().rust_mut().get_mut().selected_tracks.clear();
    }

    pub fn selected_track_title(&self, index: i32) -> QString {
        self.selected_track_at(index)
            .map(|track| QString::from(&track.title))
            .unwrap_or_default()
    }

    pub fn selected_track_artist(&self, index: i32) -> QString {
        self.selected_track_at(index)
            .map(|track| QString::from(display_artist(&track.artist)))
            .unwrap_or_default()
    }

    pub fn selected_track_number(&self, index: i32) -> QString {
        self.selected_track_at(index)
            .map(|track| match (track.disc_no, track.track_no) {
                (Some(disc), Some(number)) if disc > 1 => format!("{disc}-{number:02}"),
                (_, Some(number)) => format!("{number:02}"),
                _ => format!("{:02}", index + 1),
            })
            .map(|number| QString::from(&number))
            .unwrap_or_default()
    }

    pub fn selected_track_duration_ms(&self, index: i32) -> i32 {
        self.selected_track_at(index)
            .map(|track| track.duration_ms.min(i32::MAX as u64) as i32)
            .unwrap_or_default()
    }

    pub fn selected_track_path(&self, index: i32) -> QString {
        self.selected_track_at(index)
            .map(|track| QString::from(&track.path))
            .unwrap_or_default()
    }

    pub fn play_selected_track(mut self: core::pin::Pin<&mut Self>, index: i32) {
        if *self.playback_initializing() {
            return;
        }
        let Ok(start) = usize::try_from(index) else {
            return;
        };
        let Some(track) = self.rust().selected_tracks.get(start).cloned() else {
            return;
        };
        let playback_queue = self.rust().selected_tracks.clone();
        let paths = playback_queue
            .iter()
            .map(|track| PathBuf::from(&track.path))
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return;
        }

        self.as_mut().rust_mut().get_mut().playback_queue = playback_queue;
        self.as_mut().rust_mut().get_mut().current_queue_index = Some(start);
        self.as_mut().set_current_title(QString::from(&track.title));
        self.as_mut()
            .set_current_artist(QString::from(display_artist(&track.artist)));
        self.as_mut()
            .set_current_track_path(QString::from(&track.path));
        self.as_mut()
            .set_current_duration_ms(track.duration_ms.min(i32::MAX as u64) as i32);
        self.as_mut().set_position_ms(0);
        self.as_mut().set_has_current_track(true);
        self.as_mut().set_seekable(false);
        self.as_mut().set_playback_error(QString::default());
        self.sync_mpris();

        if let Some(player) = self.rust().player.as_ref() {
            player.send(Command::SetQueue { paths, start });
            player.send(Command::Play);
            return;
        }

        self.as_mut().set_playback_initializing(true);
        let exclusive = *self.exclusive_output();
        let initialization_qt_thread = self.qt_thread();
        std::thread::spawn(move || match create_player(exclusive, None) {
            Ok(player) => {
                player.send(Command::SetQueue { paths, start });
                player.send(Command::Play);

                initialization_qt_thread
                    .queue(move |mut controller| {
                        controller.as_mut().attach_player(player);
                        controller.as_mut().set_playback_initializing(false);
                    })
                    .ok();
            }
            Err(error) => {
                initialization_qt_thread
                    .queue(move |mut controller| {
                        controller.as_mut().set_playback_initializing(false);
                        controller.as_mut().set_has_current_track(false);
                        controller.as_mut().set_seekable(false);
                        controller.as_mut().set_playing(false);
                        controller
                            .as_mut()
                            .set_playback_error(QString::from(&format!(
                                "音频输出初始化失败：{error}"
                            )));
                        controller.sync_mpris();
                    })
                    .ok();
            }
        });
    }

    pub fn toggle_playback(&self) {
        let Some(player) = self.rust().player.as_ref() else {
            return;
        };
        player.send(if *self.playing() {
            Command::Pause
        } else {
            Command::Play
        });
    }

    pub fn previous_track(&self) {
        if let Some(player) = self.rust().player.as_ref() {
            player.send(Command::Prev);
        }
    }

    pub fn next_track(&self) {
        if let Some(player) = self.rust().player.as_ref() {
            player.send(Command::Next);
        }
    }

    pub fn seek_to(mut self: core::pin::Pin<&mut Self>, position_ms: i32) {
        if !*self.seekable() {
            return;
        }
        let duration_ms = *self.current_duration_ms();
        if duration_ms <= 0 {
            return;
        }
        let position_ms = position_ms.clamp(0, duration_ms);
        let Some(player) = self.rust().player.as_ref() else {
            return;
        };
        player.send(Command::Seek(position_ms as f64 / 1000.0));
        self.as_mut().set_position_ms(position_ms);
        if let Some(mpris) = self.rust().mpris.as_ref() {
            let _ = mpris.seeked(i64::from(position_ms) * 1000);
        }
        self.sync_mpris();
    }

    pub fn refresh_hardware_volume(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().hardware_volume.is_none() {
            match HardwareVolume::open(HARDWARE_MIXER_DEVICE, HARDWARE_MIXER_ELEMENT) {
                Ok(volume) => {
                    self.as_mut().rust_mut().get_mut().hardware_volume = Some(volume);
                }
                Err(error) => {
                    self.as_mut().apply_hardware_volume_result(Err(error));
                    return;
                }
            }
        }

        let result = self
            .rust()
            .hardware_volume
            .as_ref()
            .expect("硬件音量已打开")
            .state();
        self.as_mut().apply_hardware_volume_result(result);
    }

    pub fn request_hardware_volume(mut self: core::pin::Pin<&mut Self>, percent: i32) {
        if self.rust().hardware_volume.is_none() {
            self.as_mut().refresh_hardware_volume();
        }
        let Some(volume) = self.rust().hardware_volume.as_ref() else {
            return;
        };
        let result = volume.apply(VolumeChange::Percent(percent.clamp(0, 100) as u8));
        self.as_mut().apply_hardware_volume_result(result);
    }

    pub fn toggle_hardware_mute(mut self: core::pin::Pin<&mut Self>) {
        if !*self.hardware_mute_available() {
            return;
        }
        let Some(volume) = self.rust().hardware_volume.as_ref() else {
            return;
        };
        let result = volume.apply(VolumeChange::Muted(!*self.hardware_muted()));
        self.as_mut().apply_hardware_volume_result(result);
    }

    pub fn request_exclusive_output(mut self: core::pin::Pin<&mut Self>, exclusive: bool) {
        if *self.output_switching() || exclusive == *self.exclusive_output() {
            return;
        }

        let previous_exclusive = *self.exclusive_output();
        let resume = self.playback_resume();
        let player = self.as_mut().rust_mut().get_mut().player.take();
        self.as_mut().set_output_switching(true);
        self.as_mut().set_playback_initializing(true);
        self.as_mut().set_seekable(false);
        self.as_mut().set_playing(false);
        self.as_mut().set_output_error(QString::default());
        self.as_mut().set_output_status(QString::from(if exclusive {
            "正在连接 AKG N9"
        } else {
            "正在连接 PipeWire"
        }));
        self.sync_mpris();

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            drop(player);
            let outcome = match create_player(exclusive, resume.as_ref()) {
                Ok(player) => OutputSwitchOutcome::Switched(player),
                Err(target_error) => match create_player(previous_exclusive, resume.as_ref()) {
                    Ok(player) => OutputSwitchOutcome::Restored {
                        player,
                        target_error,
                    },
                    Err(restore_error) => OutputSwitchOutcome::Failed {
                        target_error,
                        restore_error,
                    },
                },
            };
            qt_thread
                .queue(move |mut controller| {
                    match outcome {
                        OutputSwitchOutcome::Switched(player) => {
                            controller.as_mut().attach_player(player);
                            controller.as_mut().set_exclusive_output(exclusive);
                            controller
                                .as_mut()
                                .set_output_status(QString::from(output_status(exclusive)));
                            controller.as_mut().set_output_error(QString::default());
                        }
                        OutputSwitchOutcome::Restored {
                            player,
                            target_error,
                        } => {
                            controller.as_mut().attach_player(player);
                            controller.as_mut().set_exclusive_output(previous_exclusive);
                            controller
                                .as_mut()
                                .set_output_status(QString::from(output_status(
                                    previous_exclusive,
                                )));
                            controller.as_mut().set_output_error(QString::from(&format!(
                                "输出切换失败：{target_error}"
                            )));
                        }
                        OutputSwitchOutcome::Failed {
                            target_error,
                            restore_error,
                        } => {
                            controller
                                .as_mut()
                                .set_output_status(QString::from("输出不可用"));
                            controller.as_mut().set_output_error(QString::from(&format!(
                                "输出切换失败：{target_error}；恢复原模式失败：{restore_error}"
                            )));
                        }
                    }
                    controller.as_mut().set_playback_initializing(false);
                    controller.as_mut().set_output_switching(false);
                    controller.sync_mpris();
                })
                .ok();
        });
    }

    fn apply_hardware_volume_result(
        mut self: core::pin::Pin<&mut Self>,
        result: liusheng_core::error::Result<VolumeState>,
    ) {
        match result {
            Ok(state) => {
                self.as_mut().set_hardware_volume_available(true);
                self.as_mut()
                    .set_hardware_volume_percent(i32::from(state.percent));
                self.as_mut().set_hardware_muted(state.muted);
                self.as_mut().set_hardware_mute_available(state.can_mute);
                self.as_mut().set_hardware_volume_error(QString::default());
            }
            Err(error) => {
                self.as_mut().rust_mut().get_mut().hardware_volume = None;
                self.as_mut().set_hardware_volume_available(false);
                self.as_mut().set_hardware_volume_percent(100);
                self.as_mut().set_hardware_muted(false);
                self.as_mut().set_hardware_mute_available(false);
                self.as_mut()
                    .set_hardware_volume_error(QString::from(&format!(
                        "硬件音量不可用：{error}。请使用耳机按键调节"
                    )));
            }
        }
        self.sync_mpris();
    }

    fn ensure_library_watcher(
        mut self: core::pin::Pin<&mut Self>,
    ) -> std::result::Result<(), String> {
        if self.rust().library_watcher.is_some() {
            return Ok(());
        }

        let watcher =
            LibraryWatcher::start(Path::new(MUSIC_ROOT)).map_err(|error| error.to_string())?;
        let events = watcher.events();
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            while let Ok(event) = events.recv() {
                if qt_thread
                    .queue(move |controller| controller.handle_library_watch_event(event))
                    .is_err()
                {
                    break;
                }
            }
        });
        self.as_mut().rust_mut().get_mut().library_watcher = Some(watcher);
        Ok(())
    }

    fn handle_library_watch_event(mut self: core::pin::Pin<&mut Self>, event: LibraryWatchEvent) {
        match event {
            LibraryWatchEvent::Changed => self.as_mut().request_library_scan(),
            LibraryWatchEvent::Error(error) => self
                .as_mut()
                .set_status(QString::from(&format!("曲库监听错误：{error}"))),
        }
    }

    fn attach_player(mut self: core::pin::Pin<&mut Self>, player: Player) {
        let events = player.events().clone();
        let events_qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            while let Ok(event) = events.recv() {
                if events_qt_thread
                    .queue(move |controller| controller.handle_player_event(event))
                    .is_err()
                {
                    break;
                }
            }
        });
        self.as_mut().rust_mut().get_mut().player = Some(player);
    }

    fn playback_resume(&self) -> Option<PlaybackResume> {
        if !*self.has_current_track() || self.rust().playback_queue.is_empty() {
            return None;
        }
        Some(PlaybackResume {
            paths: self
                .rust()
                .playback_queue
                .iter()
                .map(|track| PathBuf::from(&track.path))
                .collect(),
            start: self.rust().current_queue_index.unwrap_or_default(),
            position_secs: f64::from(*self.position_ms()) / 1000.0,
            playing: *self.playing(),
        })
    }

    fn handle_player_event(mut self: core::pin::Pin<&mut Self>, event: PlayerEvent) {
        match event {
            PlayerEvent::TrackStarted {
                index,
                duration_secs,
                ..
            } => {
                self.as_mut().rust_mut().get_mut().current_queue_index = Some(index);
                if let Some(track) = self.rust().playback_queue.get(index).cloned() {
                    self.as_mut().set_current_title(QString::from(&track.title));
                    self.as_mut()
                        .set_current_artist(QString::from(display_artist(&track.artist)));
                    self.as_mut()
                        .set_current_track_path(QString::from(&track.path));
                    self.as_mut()
                        .set_current_duration_ms(track.duration_ms.min(i32::MAX as u64) as i32);
                } else if let Some(duration_secs) = duration_secs {
                    self.as_mut().set_current_duration_ms(
                        (duration_secs * 1000.0).clamp(0.0, i32::MAX as f64) as i32,
                    );
                }
                self.as_mut().set_position_ms(0);
                self.as_mut().set_has_current_track(true);
                self.as_mut().set_seekable(true);
                self.as_mut().set_playing(true);
                self.as_mut().set_playback_error(QString::default());
            }
            PlayerEvent::Progress { secs } => {
                let position_ms = (secs * 1000.0).clamp(0.0, i32::MAX as f64) as i32;
                let duration_ms = *self.current_duration_ms();
                self.as_mut().set_position_ms(if duration_ms > 0 {
                    position_ms.min(duration_ms)
                } else {
                    position_ms
                });
            }
            PlayerEvent::Paused => self.as_mut().set_playing(false),
            PlayerEvent::Resumed => self.as_mut().set_playing(true),
            PlayerEvent::Stopped | PlayerEvent::QueueFinished => {
                self.as_mut().set_seekable(false);
                self.as_mut().set_playing(false);
            }
            PlayerEvent::TrackError { path, message } => {
                self.as_mut().set_playback_error(QString::from(&format!(
                    "跳过 {}：{message}",
                    path.display()
                )));
            }
            PlayerEvent::EngineError { message } => {
                self.as_mut()
                    .set_playback_error(QString::from(&format!("播放失败：{message}")));
                self.as_mut().set_seekable(false);
                self.as_mut().set_playing(false);
            }
        }
        self.sync_mpris();
    }

    fn ensure_mpris(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().mpris.is_some() {
            return;
        }
        let (service, commands) = match MprisService::start() {
            Ok(started) => started,
            Err(error) => {
                eprintln!("MPRIS 启动失败：{error}");
                return;
            }
        };
        let qt_thread = self.qt_thread();
        self.as_mut().rust_mut().get_mut().mpris = Some(service);
        self.sync_mpris();

        std::thread::spawn(move || {
            while let Ok(command) = commands.recv() {
                if qt_thread
                    .queue(move |controller| controller.handle_mpris_command(command))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    fn handle_mpris_command(mut self: core::pin::Pin<&mut Self>, command: MprisCommand) {
        match command {
            MprisCommand::Next => self.next_track(),
            MprisCommand::Previous => self.previous_track(),
            MprisCommand::Pause => {
                if *self.playing()
                    && let Some(player) = self.rust().player.as_ref()
                {
                    player.send(Command::Pause);
                }
            }
            MprisCommand::PlayPause => self.toggle_playback(),
            MprisCommand::Stop => {
                if let Some(player) = self.rust().player.as_ref() {
                    player.send(Command::Stop);
                }
            }
            MprisCommand::Play => {
                if !*self.playing()
                    && let Some(player) = self.rust().player.as_ref()
                {
                    player.send(Command::Play);
                }
            }
            MprisCommand::SeekRelative(offset_us) => {
                let target_us = i64::from(*self.position_ms())
                    .saturating_mul(1000)
                    .saturating_add(offset_us);
                let target_ms = (target_us / 1000).clamp(0, i64::from(i32::MAX)) as i32;
                self.as_mut().seek_to(target_ms);
            }
            MprisCommand::SeekAbsolute(position_us) => {
                let target_ms = (position_us / 1000).clamp(0, i64::from(i32::MAX)) as i32;
                self.as_mut().seek_to(target_ms);
            }
            MprisCommand::SetVolume(volume) => {
                let percent = (volume.clamp(0.0, 1.0) * 100.0).round() as i32;
                self.as_mut().request_hardware_volume(percent);
            }
        }
    }

    fn sync_mpris(&self) {
        if let Some(mpris) = self.rust().mpris.as_ref() {
            let _ = mpris.publish(self.mpris_snapshot());
        }
    }

    fn mpris_snapshot(&self) -> PlaybackSnapshot {
        let has_track = *self.has_current_track();
        let queue_index = self.rust().current_queue_index.unwrap_or_default();
        let track = self.rust().playback_queue.get(queue_index);
        PlaybackSnapshot {
            status: if *self.playing() {
                MprisPlaybackStatus::Playing
            } else if *self.seekable() {
                MprisPlaybackStatus::Paused
            } else {
                MprisPlaybackStatus::Stopped
            },
            has_track,
            title: track.map(|track| track.title.clone()).unwrap_or_default(),
            artist: track
                .map(|track| display_artist(&track.artist).to_owned())
                .unwrap_or_default(),
            album: track
                .map(|track| {
                    if track.album.trim().is_empty() {
                        "未知专辑".to_owned()
                    } else {
                        track.album.clone()
                    }
                })
                .unwrap_or_default(),
            path: track.map(|track| track.path.clone()).unwrap_or_default(),
            duration_us: i64::from(*self.current_duration_ms()) * 1000,
            position_us: i64::from(*self.position_ms()) * 1000,
            track_number: track.and_then(|track| track.track_no),
            queue_index,
            queue_len: self.rust().playback_queue.len(),
            seekable: *self.seekable(),
            hardware_volume_available: *self.hardware_volume_available(),
            hardware_volume_percent: (*self.hardware_volume_percent()).clamp(0, 100) as u8,
        }
    }

    fn album_at(&self, index: i32) -> Option<&AlbumSummary> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().albums.get(index))
    }

    fn selected_track_at(&self, index: i32) -> Option<&TrackRow> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().selected_tracks.get(index))
    }
}

fn create_player(
    exclusive: bool,
    resume: Option<&PlaybackResume>,
) -> std::result::Result<Player, String> {
    let sink = create_audio_sink(exclusive)?;
    let player = Player::new(sink);
    if let Some(resume) = resume {
        player.send(Command::SetQueue {
            paths: resume.paths.clone(),
            start: resume.start,
        });
        player.send(Command::Play);
        if resume.position_secs > 0.0 {
            player.send(Command::Seek(resume.position_secs));
        }
        if !resume.playing {
            player.send(Command::Pause);
        }
    }
    Ok(player)
}

fn create_audio_sink(exclusive: bool) -> std::result::Result<Box<dyn AudioSink>, String> {
    if !exclusive {
        return PipeWireSink::new()
            .map(|sink| Box::new(sink) as Box<dyn AudioSink>)
            .map_err(|error| error.to_string());
    }

    let deadline = Instant::now() + EXCLUSIVE_OPEN_TIMEOUT;
    loop {
        match AlsaSink::new(EXCLUSIVE_DEVICE) {
            Ok(sink) => {
                return Ok(Box::new(ResamplingSink::new(Box::new(sink))));
            }
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(EXCLUSIVE_OPEN_RETRY);
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn output_status(exclusive: bool) -> &'static str {
    if exclusive {
        "AKG N9 · 48/96 kHz"
    } else {
        "PipeWire"
    }
}

fn display_artist(artist: &str) -> &str {
    if artist.trim().is_empty() {
        "未知艺术家"
    } else {
        artist
    }
}

#[derive(Debug)]
struct ScanOutcome {
    stats: ScanStats,
    track_count: u64,
    albums: Vec<AlbumSummary>,
    tracks: Vec<TrackRow>,
}

impl ScanOutcome {
    fn status_text(&self) -> String {
        let changed = self.stats.added + self.stats.updated + self.stats.removed;
        if changed == 0 {
            format!("扫描完成，{} 首", self.track_count)
        } else {
            format!(
                "扫描完成，{} 首，新增 {}，更新 {}，移除 {}",
                self.track_count, self.stats.added, self.stats.updated, self.stats.removed
            )
        }
    }
}

fn scan_default_library() -> Result<ScanOutcome, String> {
    let root = Path::new(MUSIC_ROOT);
    let db_path = library_db_path()?;
    scan_paths(root, &db_path)
}

fn scan_paths(root: &Path, db_path: &Path) -> Result<ScanOutcome, String> {
    if !root.is_dir() {
        return Err(format!("未找到 {}，请检查音乐目录", root.display()));
    }
    let mut library = Library::open(db_path).map_err(|e| format!("曲库数据库打开失败：{e}"))?;
    let stats = library
        .scan(root)
        .map_err(|e| format!("曲库扫描失败：{e}"))?;
    let track_count = library
        .track_count()
        .map_err(|e| format!("曲目数量读取失败：{e}"))?;
    let albums = library
        .albums()
        .map_err(|e| format!("专辑列表读取失败：{e}"))?;
    let tracks = library
        .all_tracks()
        .map_err(|e| format!("曲目列表读取失败：{e}"))?;
    Ok(ScanOutcome {
        stats,
        track_count,
        albums,
        tracks,
    })
}

fn library_db_path() -> Result<PathBuf, String> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(|| "无法确定用户数据目录".to_string())?;
    let app_dir = data_home.join("liusheng");
    std::fs::create_dir_all(&app_dir).map_err(|e| format!("曲库目录创建失败：{e}"))?;
    Ok(app_dir.join("library.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_library_scan_reports_zero_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("music");
        std::fs::create_dir(&root).unwrap();
        let outcome = scan_paths(&root, &dir.path().join("library.db")).unwrap();
        assert_eq!(outcome.track_count, 0);
        assert!(outcome.albums.is_empty());
        assert!(outcome.tracks.is_empty());
        assert_eq!(outcome.status_text(), "扫描完成，0 首");
    }

    #[test]
    fn missing_music_root_has_actionable_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("missing");
        let error = scan_paths(&root, &dir.path().join("library.db")).unwrap_err();
        assert!(error.contains("请检查音乐目录"));
    }
}
