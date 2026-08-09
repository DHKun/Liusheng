use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use liusheng_core::audio::pipewire_sink::PipeWireSink;
use liusheng_core::engine::{Command, Player, PlayerEvent};
use liusheng_core::library::{AlbumSummary, Library, ScanStats, TrackRow};

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
    playing: bool,
    current_title: QString,
    current_artist: QString,
    current_track_path: QString,
    current_duration_ms: i32,
    position_ms: i32,
    playback_error: QString,
    albums: Vec<AlbumSummary>,
    tracks: Vec<TrackRow>,
    selected_tracks: Vec<TrackRow>,
    playback_queue: Vec<TrackRow>,
    player: Option<Player>,
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
            playing: false,
            current_title: QString::default(),
            current_artist: QString::default(),
            current_track_path: QString::default(),
            current_duration_ms: 0,
            position_ms: 0,
            playback_error: QString::default(),
            albums: Vec::new(),
            tracks: Vec::new(),
            selected_tracks: Vec::new(),
            playback_queue: Vec::new(),
            player: None,
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
        #[qproperty(bool, playing)]
        #[qproperty(QString, current_title, cxx_name = "currentTitle")]
        #[qproperty(QString, current_artist, cxx_name = "currentArtist")]
        #[qproperty(QString, current_track_path, cxx_name = "currentTrackPath")]
        #[qproperty(i32, current_duration_ms, cxx_name = "currentDurationMs")]
        #[qproperty(i32, position_ms, cxx_name = "positionMs")]
        #[qproperty(QString, playback_error, cxx_name = "playbackError")]
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
    }

    impl cxx_qt::Threading for AppController {}
}

impl qobject::AppController {
    pub fn scan_library(mut self: core::pin::Pin<&mut Self>) {
        if *self.scanning() {
            return;
        }
        self.as_mut().set_scanning(true);
        self.as_mut().set_status(QString::from("正在扫描曲库"));
        let qt_thread = self.qt_thread();

        std::thread::spawn(move || {
            let result = scan_default_library();
            qt_thread
                .queue(move |mut controller| {
                    controller.as_mut().set_scanning(false);
                    match result {
                        Ok(outcome) => {
                            let status = outcome.status_text();
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
                            controller.as_mut().set_status(QString::from(&message));
                        }
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
        self.as_mut().set_current_title(QString::from(&track.title));
        self.as_mut()
            .set_current_artist(QString::from(display_artist(&track.artist)));
        self.as_mut()
            .set_current_track_path(QString::from(&track.path));
        self.as_mut()
            .set_current_duration_ms(track.duration_ms.min(i32::MAX as u64) as i32);
        self.as_mut().set_position_ms(0);
        self.as_mut().set_has_current_track(true);
        self.as_mut().set_playback_error(QString::default());

        if let Some(player) = self.rust().player.as_ref() {
            player.send(Command::SetQueue { paths, start });
            player.send(Command::Play);
            return;
        }

        self.as_mut().set_playback_initializing(true);
        let initialization_qt_thread = self.qt_thread();
        let events_qt_thread = self.qt_thread();
        std::thread::spawn(move || match PipeWireSink::new() {
            Ok(sink) => {
                let player = Player::new(Box::new(sink));
                let events = player.events().clone();
                player.send(Command::SetQueue { paths, start });
                player.send(Command::Play);

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

                initialization_qt_thread
                    .queue(move |mut controller| {
                        controller.as_mut().rust_mut().get_mut().player = Some(player);
                        controller.as_mut().set_playback_initializing(false);
                    })
                    .ok();
            }
            Err(error) => {
                initialization_qt_thread
                    .queue(move |mut controller| {
                        controller.as_mut().set_playback_initializing(false);
                        controller.as_mut().set_has_current_track(false);
                        controller.as_mut().set_playing(false);
                        controller
                            .as_mut()
                            .set_playback_error(QString::from(&format!(
                                "音频输出初始化失败：{error}"
                            )));
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

    fn handle_player_event(mut self: core::pin::Pin<&mut Self>, event: PlayerEvent) {
        match event {
            PlayerEvent::TrackStarted {
                index,
                duration_secs,
                ..
            } => {
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
                self.as_mut().set_playing(false);
            }
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
    let root = Path::new("/data/Music");
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
