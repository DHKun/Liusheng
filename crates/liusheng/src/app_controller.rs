use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use liusheng_core::artwork::CoverCache;
use liusheng_core::audio::hardware_volume::{HardwareVolume, VolumeChange, VolumeState};
use liusheng_core::engine::{PlayerCommand, PlayerEvent};
use liusheng_core::library::pinyin::{normalize, search_blob};
use liusheng_core::library::watcher::{LibraryWatchEvent, LibraryWatcher};
use liusheng_core::library::{AlbumSummary, ArtistSummary, Library, ScanStats, TrackRow};
use liusheng_core::lyrics::Lyrics;
use liusheng_core::output_session::{
    OutputConfig, OutputMode, OutputSession, SessionCommand, SessionEvent,
};

use crate::mpris::{
    Command as MprisCommand, PlaybackSnapshot, PlaybackStatus as MprisPlaybackStatus,
    Service as MprisService,
};

const EXCLUSIVE_DEVICE: &str = "hw:Hybrid,0";
const HARDWARE_MIXER_DEVICE: &str = "hw:Hybrid";
const HARDWARE_MIXER_ELEMENT: &str = "PCM";
const MUSIC_ROOT: &str = "/data/Music";

pub struct AppControllerRust {
    status: QString,
    track_count: i32,
    album_count: i32,
    artist_count: i32,
    library_revision: i32,
    visible_track_count: i32,
    track_filter: QString,
    queue_count: i32,
    current_queue_position: i32,
    queue_revision: i32,
    selected_album_index: i32,
    selected_artist_index: i32,
    selected_track_count: i32,
    album_open: bool,
    artist_open: bool,
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
    current_cover_url: QString,
    lyrics_loading: bool,
    lyrics_synced: bool,
    lyric_line_count: i32,
    current_lyric_index: i32,
    lyrics_revision: i32,
    lyrics_error: QString,
    exclusive_output: bool,
    output_switching: bool,
    output_unavailable: bool,
    output_status: QString,
    output_error: QString,
    hardware_volume_available: bool,
    hardware_volume_percent: i32,
    hardware_muted: bool,
    hardware_mute_available: bool,
    hardware_volume_error: QString,
    albums: Vec<AlbumSummary>,
    album_cover_urls: Vec<String>,
    artists: Vec<ArtistSummary>,
    artist_cover_urls: Vec<String>,
    tracks: Vec<TrackRow>,
    track_search_blobs: Vec<String>,
    visible_track_indices: Vec<usize>,
    selected_tracks: Vec<TrackRow>,
    playback_queue: Vec<TrackRow>,
    current_queue_index: Option<usize>,
    output_session: Option<OutputSession>,
    mpris: Option<MprisService>,
    hardware_volume: Option<HardwareVolume>,
    library_watcher: Option<LibraryWatcher>,
    library_rescan_pending: bool,
    lyrics: Option<Lyrics>,
    lyrics_request_path: Option<PathBuf>,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            status: QString::from("曲库待扫描"),
            track_count: 0,
            album_count: 0,
            artist_count: 0,
            library_revision: 0,
            visible_track_count: 0,
            track_filter: QString::default(),
            queue_count: 0,
            current_queue_position: -1,
            queue_revision: 0,
            selected_album_index: -1,
            selected_artist_index: -1,
            selected_track_count: 0,
            album_open: false,
            artist_open: false,
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
            current_cover_url: QString::default(),
            lyrics_loading: false,
            lyrics_synced: false,
            lyric_line_count: 0,
            current_lyric_index: -1,
            lyrics_revision: 0,
            lyrics_error: QString::default(),
            exclusive_output: false,
            output_switching: false,
            output_unavailable: false,
            output_status: QString::from("PipeWire"),
            output_error: QString::default(),
            hardware_volume_available: false,
            hardware_volume_percent: 100,
            hardware_muted: false,
            hardware_mute_available: false,
            hardware_volume_error: QString::from("正在检测硬件音量"),
            albums: Vec::new(),
            album_cover_urls: Vec::new(),
            artists: Vec::new(),
            artist_cover_urls: Vec::new(),
            tracks: Vec::new(),
            track_search_blobs: Vec::new(),
            visible_track_indices: Vec::new(),
            selected_tracks: Vec::new(),
            playback_queue: Vec::new(),
            current_queue_index: None,
            output_session: None,
            mpris: None,
            hardware_volume: None,
            library_watcher: None,
            library_rescan_pending: false,
            lyrics: None,
            lyrics_request_path: None,
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
        #[qproperty(i32, artist_count, cxx_name = "artistCount")]
        #[qproperty(i32, library_revision, cxx_name = "libraryRevision")]
        #[qproperty(i32, visible_track_count, cxx_name = "visibleTrackCount")]
        #[qproperty(QString, track_filter, cxx_name = "trackFilter")]
        #[qproperty(i32, queue_count, cxx_name = "queueCount")]
        #[qproperty(i32, current_queue_position, cxx_name = "currentQueueIndex")]
        #[qproperty(i32, queue_revision, cxx_name = "queueRevision")]
        #[qproperty(i32, selected_album_index, cxx_name = "selectedAlbumIndex")]
        #[qproperty(i32, selected_artist_index, cxx_name = "selectedArtistIndex")]
        #[qproperty(i32, selected_track_count, cxx_name = "selectedTrackCount")]
        #[qproperty(bool, album_open, cxx_name = "albumOpen")]
        #[qproperty(bool, artist_open, cxx_name = "artistOpen")]
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
        #[qproperty(QString, current_cover_url, cxx_name = "currentCoverUrl")]
        #[qproperty(bool, lyrics_loading, cxx_name = "lyricsLoading")]
        #[qproperty(bool, lyrics_synced, cxx_name = "lyricsSynced")]
        #[qproperty(i32, lyric_line_count, cxx_name = "lyricLineCount")]
        #[qproperty(i32, current_lyric_index, cxx_name = "currentLyricIndex")]
        #[qproperty(i32, lyrics_revision, cxx_name = "lyricsRevision")]
        #[qproperty(QString, lyrics_error, cxx_name = "lyricsError")]
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
        #[cxx_name = "albumCoverUrl"]
        fn album_cover_url(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "openAlbum"]
        fn open_album(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "closeAlbum"]
        fn close_album(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "artistName"]
        fn artist_name(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "artistTrackCount"]
        fn artist_track_count(&self, index: i32) -> i32;

        #[qinvokable]
        #[cxx_name = "artistAlbumCount"]
        fn artist_album_count(&self, index: i32) -> i32;

        #[qinvokable]
        #[cxx_name = "artistCoverUrl"]
        fn artist_cover_url(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "openArtist"]
        fn open_artist(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "closeArtist"]
        fn close_artist(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "selectedTrackTitle"]
        fn selected_track_title(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "selectedTrackArtist"]
        fn selected_track_artist(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "selectedTrackAlbum"]
        fn selected_track_album(&self, index: i32) -> QString;

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
        #[cxx_name = "allTrackTitle"]
        fn all_track_title(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "allTrackArtist"]
        fn all_track_artist(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "allTrackAlbum"]
        fn all_track_album(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "allTrackNumber"]
        fn all_track_number(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "allTrackDurationMs"]
        fn all_track_duration_ms(&self, index: i32) -> i32;

        #[qinvokable]
        #[cxx_name = "allTrackPath"]
        fn all_track_path(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "playAllTrack"]
        fn play_all_track(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "filterTracks"]
        fn filter_tracks(self: Pin<&mut Self>, query: &QString);

        #[qinvokable]
        #[cxx_name = "queueTrackTitle"]
        fn queue_track_title(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "queueTrackArtist"]
        fn queue_track_artist(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "queueTrackAlbum"]
        fn queue_track_album(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "queueTrackNumber"]
        fn queue_track_number(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "queueTrackDurationMs"]
        fn queue_track_duration_ms(&self, index: i32) -> i32;

        #[qinvokable]
        #[cxx_name = "playQueueTrack"]
        fn play_queue_track(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "removeQueueTrack"]
        fn remove_queue_track(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "clearQueue"]
        fn clear_queue(self: Pin<&mut Self>);

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
        #[cxx_name = "lyricText"]
        fn lyric_text(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "lyricTimeMs"]
        fn lyric_time_ms(&self, index: i32) -> i32;

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
        self.as_mut().ensure_output_session();
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
                            let artist_count = outcome.artists.len().min(i32::MAX as usize) as i32;
                            controller.as_mut().rust_mut().get_mut().album_cover_urls =
                                outcome.album_cover_urls;
                            controller.as_mut().rust_mut().get_mut().albums = outcome.albums;
                            controller.as_mut().rust_mut().get_mut().artist_cover_urls =
                                outcome.artist_cover_urls;
                            controller.as_mut().rust_mut().get_mut().artists = outcome.artists;
                            controller.as_mut().replace_tracks(outcome.tracks);
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
                            controller.as_mut().set_artist_count(artist_count);
                            controller.as_mut().set_selected_album_index(-1);
                            controller.as_mut().set_selected_artist_index(-1);
                            controller.as_mut().set_selected_track_count(0);
                            controller.as_mut().set_album_open(false);
                            controller.as_mut().set_artist_open(false);
                            controller.as_mut().set_status(QString::from(&status));
                            controller.as_mut().refresh_current_cover();
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

    pub fn album_cover_url(&self, index: i32) -> QString {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().album_cover_urls.get(index))
            .map(QString::from)
            .unwrap_or_default()
    }

    pub fn open_album(mut self: core::pin::Pin<&mut Self>, index: i32) {
        let Some(key) = self.album_at(index).map(|album| album.key.clone()) else {
            return;
        };
        self.as_mut().close_artist();
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

    pub fn artist_name(&self, index: i32) -> QString {
        self.artist_at(index)
            .map(|artist| QString::from(&artist.name))
            .unwrap_or_default()
    }

    pub fn artist_track_count(&self, index: i32) -> i32 {
        self.artist_at(index)
            .map(|artist| artist.track_count.min(i32::MAX as u32) as i32)
            .unwrap_or_default()
    }

    pub fn artist_album_count(&self, index: i32) -> i32 {
        self.artist_at(index)
            .map(|artist| artist.album_count.min(i32::MAX as u32) as i32)
            .unwrap_or_default()
    }

    pub fn artist_cover_url(&self, index: i32) -> QString {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().artist_cover_urls.get(index))
            .map(QString::from)
            .unwrap_or_default()
    }

    pub fn open_artist(mut self: core::pin::Pin<&mut Self>, index: i32) {
        let Some(key) = self.artist_at(index).map(|artist| artist.key.clone()) else {
            return;
        };
        self.as_mut().close_album();
        let selected_tracks = self
            .rust()
            .tracks
            .iter()
            .filter(|track| track.artist == key)
            .cloned()
            .collect::<Vec<_>>();
        let selected_track_count = selected_tracks.len().min(i32::MAX as usize) as i32;
        self.as_mut().rust_mut().get_mut().selected_tracks = selected_tracks;
        self.as_mut().set_selected_artist_index(index);
        self.as_mut().set_selected_track_count(selected_track_count);
        self.as_mut().set_artist_open(true);
    }

    pub fn close_artist(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().set_artist_open(false);
        self.as_mut().set_selected_artist_index(-1);
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

    pub fn selected_track_album(&self, index: i32) -> QString {
        self.selected_track_at(index)
            .map(|track| QString::from(display_album(&track.album)))
            .unwrap_or_default()
    }

    pub fn selected_track_number(&self, index: i32) -> QString {
        self.selected_track_at(index)
            .map(|track| {
                if *self.artist_open() {
                    return format!("{:02}", index + 1);
                }
                match (track.disc_no, track.track_no) {
                    (Some(disc), Some(number)) if disc > 1 => format!("{disc}-{number:02}"),
                    (_, Some(number)) => format!("{number:02}"),
                    _ => format!("{:02}", index + 1),
                }
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
        if self.rust().selected_tracks.get(start).is_none() {
            return;
        }
        let playback_queue = self.rust().selected_tracks.clone();
        self.as_mut().play_track_queue(playback_queue, start);
    }

    pub fn all_track_title(&self, index: i32) -> QString {
        self.all_track_at(index)
            .map(|track| QString::from(&track.title))
            .unwrap_or_default()
    }

    pub fn all_track_artist(&self, index: i32) -> QString {
        self.all_track_at(index)
            .map(|track| QString::from(display_artist(&track.artist)))
            .unwrap_or_default()
    }

    pub fn all_track_album(&self, index: i32) -> QString {
        self.all_track_at(index)
            .map(|track| QString::from(display_album(&track.album)))
            .unwrap_or_default()
    }

    pub fn all_track_number(&self, index: i32) -> QString {
        usize::try_from(index)
            .ok()
            .filter(|_| self.all_track_at(index).is_some())
            .map(|index| QString::from(&format!("{:02}", index + 1)))
            .unwrap_or_default()
    }

    pub fn all_track_duration_ms(&self, index: i32) -> i32 {
        self.all_track_at(index)
            .map(|track| track.duration_ms.min(i32::MAX as u64) as i32)
            .unwrap_or_default()
    }

    pub fn all_track_path(&self, index: i32) -> QString {
        self.all_track_at(index)
            .map(|track| QString::from(&track.path))
            .unwrap_or_default()
    }

    pub fn play_all_track(mut self: core::pin::Pin<&mut Self>, index: i32) {
        if *self.playback_initializing() {
            return;
        }
        let Ok(start) = usize::try_from(index) else {
            return;
        };
        if self.all_track_at(index).is_none() {
            return;
        }
        let playback_queue = self
            .rust()
            .visible_track_indices
            .iter()
            .filter_map(|index| self.rust().tracks.get(*index).cloned())
            .collect();
        self.as_mut().play_track_queue(playback_queue, start);
    }

    pub fn filter_tracks(mut self: core::pin::Pin<&mut Self>, query: &QString) {
        let normalized = normalize(&query.to_string());
        let visible_track_indices =
            filtered_track_indices(&self.rust().track_search_blobs, &normalized);
        let visible_track_count = visible_track_indices.len().min(i32::MAX as usize) as i32;
        self.as_mut().rust_mut().get_mut().visible_track_indices = visible_track_indices;
        self.as_mut().set_visible_track_count(visible_track_count);
        self.as_mut().set_track_filter(QString::from(&normalized));
        self.as_mut().bump_library_revision();
    }

    pub fn queue_track_title(&self, index: i32) -> QString {
        self.queue_track_at(index)
            .map(|track| QString::from(&track.title))
            .unwrap_or_default()
    }

    pub fn queue_track_artist(&self, index: i32) -> QString {
        self.queue_track_at(index)
            .map(|track| QString::from(display_artist(&track.artist)))
            .unwrap_or_default()
    }

    pub fn queue_track_album(&self, index: i32) -> QString {
        self.queue_track_at(index)
            .map(|track| QString::from(display_album(&track.album)))
            .unwrap_or_default()
    }

    pub fn queue_track_number(&self, index: i32) -> QString {
        usize::try_from(index)
            .ok()
            .filter(|_| self.queue_track_at(index).is_some())
            .map(|index| QString::from(&format!("{:02}", index + 1)))
            .unwrap_or_default()
    }

    pub fn queue_track_duration_ms(&self, index: i32) -> i32 {
        self.queue_track_at(index)
            .map(|track| track.duration_ms.min(i32::MAX as u64) as i32)
            .unwrap_or_default()
    }

    pub fn play_queue_track(mut self: core::pin::Pin<&mut Self>, index: i32) {
        if *self.playback_initializing() {
            return;
        }
        let Ok(start) = usize::try_from(index) else {
            return;
        };
        if self.rust().playback_queue.get(start).is_none() {
            return;
        }
        let playback_queue = self.rust().playback_queue.clone();
        self.as_mut().play_track_queue(playback_queue, start);
    }

    pub fn remove_queue_track(mut self: core::pin::Pin<&mut Self>, index: i32) {
        if *self.playback_initializing() {
            return;
        }
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        if index >= self.rust().playback_queue.len() {
            return;
        }

        let current = self.rust().current_queue_index;
        self.as_mut()
            .rust_mut()
            .get_mut()
            .playback_queue
            .remove(index);
        let queue_len = self.rust().playback_queue.len();
        let next_current = queue_index_after_removal(current, index, queue_len);
        self.as_mut().rust_mut().get_mut().current_queue_index = next_current;
        self.send_player_command(PlayerCommand::RemoveQueueItem(index));

        if queue_len == 0 {
            self.as_mut().reset_empty_queue();
            return;
        }

        self.as_mut()
            .set_queue_count(queue_len.min(i32::MAX as usize) as i32);
        self.as_mut().set_current_queue_position(
            next_current
                .map(|index| index.min(i32::MAX as usize) as i32)
                .unwrap_or(-1),
        );
        self.as_mut().bump_queue_revision();
        self.sync_mpris();
    }

    pub fn clear_queue(mut self: core::pin::Pin<&mut Self>) {
        if *self.playback_initializing() || self.rust().playback_queue.is_empty() {
            return;
        }
        self.send_player_command(PlayerCommand::ClearQueue);
        self.as_mut().reset_empty_queue();
    }

    fn reset_empty_queue(mut self: core::pin::Pin<&mut Self>) {
        let rust = self.as_mut().rust_mut().get_mut();
        rust.playback_queue.clear();
        rust.current_queue_index = None;
        rust.lyrics = None;
        rust.lyrics_request_path = None;
        self.as_mut().set_queue_count(0);
        self.as_mut().set_current_queue_position(-1);
        self.as_mut().bump_queue_revision();
        self.as_mut().set_current_title(QString::default());
        self.as_mut().set_current_artist(QString::default());
        self.as_mut().set_current_track_path(QString::default());
        self.as_mut().set_current_duration_ms(0);
        self.as_mut().set_position_ms(0);
        self.as_mut().set_playback_error(QString::default());
        self.as_mut().set_current_cover_url(QString::default());
        self.as_mut().set_has_current_track(false);
        self.as_mut().set_seekable(false);
        self.as_mut().set_playing(false);
        self.as_mut().set_lyrics_loading(false);
        self.as_mut().set_lyrics_synced(false);
        self.as_mut().set_lyric_line_count(0);
        self.as_mut().set_current_lyric_index(-1);
        self.as_mut().set_lyrics_error(QString::default());
        self.as_mut().bump_lyrics_revision();
        self.sync_mpris();
    }

    fn play_track_queue(
        mut self: core::pin::Pin<&mut Self>,
        playback_queue: Vec<TrackRow>,
        start: usize,
    ) {
        let Some(track) = playback_queue.get(start).cloned() else {
            return;
        };
        let paths = playback_queue
            .iter()
            .map(|track| PathBuf::from(&track.path))
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return;
        }
        let cover_url = self
            .cover_url_for_track(&track)
            .unwrap_or_default()
            .to_owned();

        let queue_count = playback_queue.len().min(i32::MAX as usize) as i32;
        self.as_mut().rust_mut().get_mut().playback_queue = playback_queue;
        self.as_mut().rust_mut().get_mut().current_queue_index = Some(start);
        self.as_mut().set_queue_count(queue_count);
        self.as_mut()
            .set_current_queue_position(start.min(i32::MAX as usize) as i32);
        self.as_mut().bump_queue_revision();
        self.as_mut().set_current_title(QString::from(&track.title));
        self.as_mut()
            .set_current_artist(QString::from(display_artist(&track.artist)));
        self.as_mut()
            .set_current_track_path(QString::from(&track.path));
        self.as_mut()
            .set_current_duration_ms(track.duration_ms.min(i32::MAX as u64) as i32);
        self.as_mut()
            .set_current_cover_url(QString::from(&cover_url));
        self.as_mut().set_position_ms(0);
        self.as_mut().set_has_current_track(true);
        self.as_mut().set_seekable(false);
        self.as_mut().set_playback_error(QString::default());
        self.as_mut()
            .request_lyrics_for_path(PathBuf::from(&track.path));
        self.sync_mpris();

        self.as_mut().ensure_output_session();
        self.send_player_command(PlayerCommand::SetQueue { paths, start });
        self.send_player_command(PlayerCommand::Play);
    }

    pub fn toggle_playback(&self) {
        self.send_player_command(if *self.playing() {
            PlayerCommand::Pause
        } else {
            PlayerCommand::Play
        });
    }

    pub fn previous_track(&self) {
        self.send_player_command(PlayerCommand::Prev);
    }

    pub fn next_track(&self) {
        self.send_player_command(PlayerCommand::Next);
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
        if self.rust().output_session.is_none() {
            return;
        }
        self.send_player_command(PlayerCommand::Seek(position_ms as f64 / 1000.0));
        self.as_mut().set_position_ms(position_ms);
        self.as_mut().update_current_lyric_index();
        if let Some(mpris) = self.rust().mpris.as_ref() {
            let _ = mpris.seeked(i64::from(position_ms) * 1000);
        }
        self.sync_mpris();
    }

    pub fn lyric_text(&self, index: i32) -> QString {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().lyrics.as_ref()?.lines().get(index))
            .map(|line| QString::from(&line.text))
            .unwrap_or_default()
    }

    pub fn lyric_time_ms(&self, index: i32) -> i32 {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().lyrics.as_ref()?.lines().get(index))
            .and_then(|line| line.start_ms)
            .map(|start_ms| start_ms.min(i32::MAX as u64) as i32)
            .unwrap_or(-1)
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
        self.as_mut().ensure_output_session();
        if let Some(session) = self.rust().output_session.as_ref() {
            session.send(SessionCommand::Switch(output_mode(exclusive)));
        }
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

    fn ensure_output_session(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().output_session.is_some() {
            return;
        }
        self.as_mut().set_playback_initializing(true);
        let session = OutputSession::start(OutputConfig {
            initial_mode: output_mode(*self.exclusive_output()),
            exclusive_device: EXCLUSIVE_DEVICE.into(),
        });
        let events = session.events().clone();
        let events_qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            while let Ok(event) = events.recv() {
                if events_qt_thread
                    .queue(move |controller| controller.handle_output_session_event(event))
                    .is_err()
                {
                    break;
                }
            }
        });
        self.as_mut().rust_mut().get_mut().output_session = Some(session);
    }

    fn send_player_command(&self, command: PlayerCommand) {
        if let Some(session) = self.rust().output_session.as_ref() {
            if self.rust().output_unavailable {
                session.send(SessionCommand::RetryOutput);
            }
            session.send(SessionCommand::Playback(command));
        }
    }

    fn handle_output_session_event(mut self: core::pin::Pin<&mut Self>, event: SessionEvent) {
        match event {
            SessionEvent::Playback(event) => {
                self.as_mut().handle_player_event(event);
                return;
            }
            SessionEvent::Switching { to, .. } => {
                self.as_mut().rust_mut().get_mut().output_unavailable = false;
                self.as_mut().set_output_switching(true);
                self.as_mut().set_playback_initializing(true);
                self.as_mut().set_seekable(false);
                self.as_mut().set_playing(false);
                self.as_mut().set_output_error(QString::default());
                self.as_mut()
                    .set_output_status(QString::from(connecting_output_status(to)));
            }
            SessionEvent::Active { mode } => {
                self.as_mut().rust_mut().get_mut().output_unavailable = false;
                let exclusive = mode == OutputMode::Exclusive;
                self.as_mut().set_exclusive_output(exclusive);
                self.as_mut()
                    .set_output_status(QString::from(output_status(exclusive)));
                self.as_mut().set_output_error(QString::default());
                self.as_mut().set_output_switching(false);
                self.as_mut().set_playback_initializing(false);
            }
            SessionEvent::Restored { mode, error } => {
                self.as_mut().rust_mut().get_mut().output_unavailable = false;
                let exclusive = mode == OutputMode::Exclusive;
                self.as_mut().set_exclusive_output(exclusive);
                self.as_mut()
                    .set_output_status(QString::from(output_status(exclusive)));
                self.as_mut()
                    .set_output_error(QString::from(&format!("输出切换失败：{}", error.message)));
                self.as_mut().set_output_switching(false);
                self.as_mut().set_playback_initializing(false);
            }
            SessionEvent::Unavailable {
                target_error,
                restore_error,
            } => {
                self.as_mut().rust_mut().get_mut().output_unavailable = true;
                self.as_mut().set_output_status(QString::from("输出不可用"));
                let message = match restore_error {
                    Some(restore_error) => format!(
                        "输出切换失败：{}；恢复原模式失败：{}",
                        target_error.message, restore_error.message
                    ),
                    None => format!("音频输出初始化失败：{}", target_error.message),
                };
                self.as_mut().set_output_error(QString::from(&message));
                self.as_mut().set_has_current_track(false);
                self.as_mut().set_seekable(false);
                self.as_mut().set_playing(false);
                self.as_mut().set_output_switching(false);
                self.as_mut().set_playback_initializing(false);
            }
        }
        self.sync_mpris();
    }

    fn request_lyrics_for_path(mut self: core::pin::Pin<&mut Self>, path: PathBuf) {
        if self.rust().lyrics_request_path.as_ref() == Some(&path) {
            self.as_mut().update_current_lyric_index();
            return;
        }

        self.as_mut().rust_mut().get_mut().lyrics_request_path = Some(path.clone());
        self.as_mut().rust_mut().get_mut().lyrics = None;
        self.as_mut().set_lyrics_loading(true);
        self.as_mut().set_lyrics_synced(false);
        self.as_mut().set_lyric_line_count(0);
        self.as_mut().set_current_lyric_index(-1);
        self.as_mut().set_lyrics_error(QString::default());
        self.as_mut().bump_lyrics_revision();

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = Lyrics::load(&path).map_err(|error| error.to_string());
            qt_thread
                .queue(move |mut controller| {
                    if controller.rust().lyrics_request_path.as_ref() != Some(&path) {
                        return;
                    }
                    controller.as_mut().set_lyrics_loading(false);
                    match result {
                        Ok(Some(lyrics)) => {
                            let line_count = lyrics.lines().len().min(i32::MAX as usize) as i32;
                            controller
                                .as_mut()
                                .set_lyrics_synced(lyrics.is_synchronized());
                            controller.as_mut().set_lyric_line_count(line_count);
                            controller.as_mut().set_lyrics_error(QString::default());
                            controller.as_mut().rust_mut().get_mut().lyrics = Some(lyrics);
                        }
                        Ok(None) => {
                            controller.as_mut().set_lyrics_synced(false);
                            controller.as_mut().set_lyric_line_count(0);
                            controller.as_mut().rust_mut().get_mut().lyrics = None;
                        }
                        Err(error) => {
                            controller.as_mut().set_lyrics_synced(false);
                            controller.as_mut().set_lyric_line_count(0);
                            controller
                                .as_mut()
                                .set_lyrics_error(QString::from(&format!("歌词读取失败：{error}")));
                            controller.as_mut().rust_mut().get_mut().lyrics = None;
                        }
                    }
                    controller.as_mut().bump_lyrics_revision();
                    controller.as_mut().update_current_lyric_index();
                })
                .ok();
        });
    }

    fn update_current_lyric_index(mut self: core::pin::Pin<&mut Self>) {
        let position_ms = (*self.position_ms()).max(0) as u64;
        let index = self
            .rust()
            .lyrics
            .as_ref()
            .and_then(|lyrics| lyrics.active_index(position_ms))
            .map(|index| index.min(i32::MAX as usize) as i32)
            .unwrap_or(-1);
        self.as_mut().set_current_lyric_index(index);
    }

    fn bump_lyrics_revision(mut self: core::pin::Pin<&mut Self>) {
        let revision = if *self.lyrics_revision() == i32::MAX {
            0
        } else {
            *self.lyrics_revision() + 1
        };
        self.as_mut().set_lyrics_revision(revision);
    }

    fn handle_player_event(mut self: core::pin::Pin<&mut Self>, event: PlayerEvent) {
        match event {
            PlayerEvent::TrackStarted {
                index,
                path,
                duration_secs,
                ..
            } => {
                if self.rust().playback_queue.is_empty() {
                    return;
                }
                self.as_mut().rust_mut().get_mut().current_queue_index = Some(index);
                self.as_mut()
                    .set_current_queue_position(index.min(i32::MAX as usize) as i32);
                if let Some(track) = self.rust().playback_queue.get(index).cloned() {
                    let cover_url = self
                        .cover_url_for_track(&track)
                        .unwrap_or_default()
                        .to_owned();
                    self.as_mut().set_current_title(QString::from(&track.title));
                    self.as_mut()
                        .set_current_artist(QString::from(display_artist(&track.artist)));
                    self.as_mut()
                        .set_current_track_path(QString::from(&track.path));
                    self.as_mut()
                        .set_current_duration_ms(track.duration_ms.min(i32::MAX as u64) as i32);
                    self.as_mut()
                        .set_current_cover_url(QString::from(&cover_url));
                } else {
                    self.as_mut()
                        .set_current_track_path(QString::from(path.to_string_lossy().as_ref()));
                    self.as_mut().set_current_cover_url(QString::default());
                    if let Some(duration_secs) = duration_secs {
                        self.as_mut().set_current_duration_ms(
                            (duration_secs * 1000.0).clamp(0.0, i32::MAX as f64) as i32,
                        );
                    }
                }
                self.as_mut().set_position_ms(0);
                self.as_mut().set_has_current_track(true);
                self.as_mut().set_seekable(true);
                self.as_mut().set_playing(true);
                self.as_mut().set_playback_error(QString::default());
                self.as_mut().request_lyrics_for_path(path);
            }
            PlayerEvent::Progress { secs } => {
                let Some(position_ms) = progress_position_ms(
                    *self.has_current_track(),
                    secs,
                    *self.current_duration_ms(),
                ) else {
                    return;
                };
                self.as_mut().set_position_ms(position_ms);
                self.as_mut().update_current_lyric_index();
            }
            PlayerEvent::Paused if *self.has_current_track() => {
                self.as_mut().set_playing(false);
            }
            PlayerEvent::Resumed if *self.has_current_track() => {
                self.as_mut().set_playing(true);
            }
            PlayerEvent::Paused | PlayerEvent::Resumed => return,
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
                if *self.playing() && self.rust().output_session.is_some() {
                    self.send_player_command(PlayerCommand::Pause);
                }
            }
            MprisCommand::PlayPause => self.toggle_playback(),
            MprisCommand::Stop => {
                self.send_player_command(PlayerCommand::Stop);
            }
            MprisCommand::Play => {
                if !*self.playing() && self.rust().output_session.is_some() {
                    self.send_player_command(PlayerCommand::Play);
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
            art_url: track
                .and_then(|track| self.cover_url_for_track(track))
                .unwrap_or_default()
                .to_owned(),
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

    fn refresh_current_cover(mut self: core::pin::Pin<&mut Self>) {
        let track = self
            .rust()
            .current_queue_index
            .and_then(|index| self.rust().playback_queue.get(index))
            .cloned();
        let cover_url = track
            .as_ref()
            .and_then(|track| self.cover_url_for_track(track))
            .unwrap_or_default()
            .to_owned();
        self.as_mut()
            .set_current_cover_url(QString::from(&cover_url));
        self.sync_mpris();
    }

    fn cover_url_for_track(&self, track: &TrackRow) -> Option<&str> {
        self.rust()
            .albums
            .iter()
            .position(|album| {
                album.key.album == track.album && album.key.album_artist == track.album_artist
            })
            .and_then(|index| self.rust().album_cover_urls.get(index))
            .map(String::as_str)
            .filter(|url| !url.is_empty())
    }

    fn album_at(&self, index: i32) -> Option<&AlbumSummary> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().albums.get(index))
    }

    fn artist_at(&self, index: i32) -> Option<&ArtistSummary> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().artists.get(index))
    }

    fn selected_track_at(&self, index: i32) -> Option<&TrackRow> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().selected_tracks.get(index))
    }

    fn queue_track_at(&self, index: i32) -> Option<&TrackRow> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().playback_queue.get(index))
    }

    fn all_track_at(&self, index: i32) -> Option<&TrackRow> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().visible_track_indices.get(index))
            .and_then(|index| self.rust().tracks.get(*index))
    }

    fn replace_tracks(mut self: core::pin::Pin<&mut Self>, tracks: Vec<TrackRow>) {
        let search_blobs = tracks.iter().map(track_search_blob).collect::<Vec<_>>();
        let normalized = normalize(&self.track_filter().to_string());
        let visible_track_indices = filtered_track_indices(&search_blobs, &normalized);
        let visible_track_count = visible_track_indices.len().min(i32::MAX as usize) as i32;

        self.as_mut().rust_mut().get_mut().tracks = tracks;
        self.as_mut().rust_mut().get_mut().track_search_blobs = search_blobs;
        self.as_mut().rust_mut().get_mut().visible_track_indices = visible_track_indices;
        self.as_mut().set_visible_track_count(visible_track_count);
        self.as_mut().bump_library_revision();
    }

    fn bump_library_revision(mut self: core::pin::Pin<&mut Self>) {
        let next_revision = (*self.library_revision()).wrapping_add(1);
        self.as_mut().set_library_revision(next_revision);
    }

    fn bump_queue_revision(mut self: core::pin::Pin<&mut Self>) {
        let next_revision = (*self.queue_revision()).wrapping_add(1);
        self.as_mut().set_queue_revision(next_revision);
    }
}

fn output_status(exclusive: bool) -> &'static str {
    if exclusive {
        "AKG N9 · 48/96 kHz"
    } else {
        "PipeWire"
    }
}

fn output_mode(exclusive: bool) -> OutputMode {
    if exclusive {
        OutputMode::Exclusive
    } else {
        OutputMode::Shared
    }
}

fn connecting_output_status(mode: OutputMode) -> &'static str {
    match mode {
        OutputMode::Shared => "正在连接 PipeWire",
        OutputMode::Exclusive => "正在连接 AKG N9",
    }
}

fn display_artist(artist: &str) -> &str {
    if artist.trim().is_empty() {
        "未知艺术家"
    } else {
        artist
    }
}

fn display_album(album: &str) -> &str {
    if album.trim().is_empty() {
        "未知专辑"
    } else {
        album
    }
}

fn track_search_blob(track: &TrackRow) -> String {
    [
        search_blob(&track.title),
        search_blob(&track.artist),
        search_blob(&track.album),
        search_blob(&track.album_artist),
    ]
    .join("\n")
}

fn filtered_track_indices(search_blobs: &[String], normalized_query: &str) -> Vec<usize> {
    if normalized_query.is_empty() {
        return (0..search_blobs.len()).collect();
    }
    search_blobs
        .iter()
        .enumerate()
        .filter_map(|(index, blob)| blob.contains(normalized_query).then_some(index))
        .collect()
}

fn queue_index_after_removal(
    current: Option<usize>,
    removed: usize,
    remaining: usize,
) -> Option<usize> {
    if remaining == 0 {
        return None;
    }
    current.map(|current| {
        if removed < current {
            current - 1
        } else if removed == current {
            current.min(remaining - 1)
        } else {
            current
        }
    })
}

fn progress_position_ms(has_current_track: bool, secs: f64, duration_ms: i32) -> Option<i32> {
    if !has_current_track {
        return None;
    }
    let position_ms = (secs * 1000.0).clamp(0.0, i32::MAX as f64) as i32;
    Some(if duration_ms > 0 {
        position_ms.min(duration_ms)
    } else {
        position_ms
    })
}

#[derive(Debug)]
struct ScanOutcome {
    stats: ScanStats,
    track_count: u64,
    albums: Vec<AlbumSummary>,
    album_cover_urls: Vec<String>,
    artists: Vec<ArtistSummary>,
    artist_cover_urls: Vec<String>,
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
    let cover_cache_path = cover_cache_path()?;
    scan_paths(root, &db_path, &cover_cache_path)
}

fn scan_paths(root: &Path, db_path: &Path, cover_cache_path: &Path) -> Result<ScanOutcome, String> {
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
    let artists = library
        .artists()
        .map_err(|e| format!("艺术家列表读取失败：{e}"))?;
    let tracks = library
        .all_tracks()
        .map_err(|e| format!("曲目列表读取失败：{e}"))?;
    let album_cover_urls = resolve_album_cover_urls(&albums, &tracks, cover_cache_path);
    let artist_cover_urls =
        resolve_artist_cover_urls(&artists, &tracks, &albums, &album_cover_urls);
    Ok(ScanOutcome {
        stats,
        track_count,
        albums,
        album_cover_urls,
        artists,
        artist_cover_urls,
        tracks,
    })
}

fn resolve_album_cover_urls(
    albums: &[AlbumSummary],
    tracks: &[TrackRow],
    cache_path: &Path,
) -> Vec<String> {
    let Ok(cache) = CoverCache::new(cache_path) else {
        return vec![String::new(); albums.len()];
    };

    albums
        .iter()
        .map(|album| {
            let album_tracks = tracks
                .iter()
                .filter(|track| {
                    track.album == album.key.album && track.album_artist == album.key.album_artist
                })
                .map(|track| PathBuf::from(&track.path))
                .collect::<Vec<_>>();
            cache
                .cover_for_album(&album_tracks)
                .ok()
                .flatten()
                .map(|path| format!("file:{}", path.to_string_lossy()))
                .unwrap_or_default()
        })
        .collect()
}

fn resolve_artist_cover_urls(
    artists: &[ArtistSummary],
    tracks: &[TrackRow],
    albums: &[AlbumSummary],
    album_cover_urls: &[String],
) -> Vec<String> {
    artists
        .iter()
        .map(|artist| {
            tracks
                .iter()
                .filter(|track| track.artist == artist.key)
                .filter_map(|track| {
                    albums
                        .iter()
                        .position(|album| {
                            album.key.album == track.album
                                && album.key.album_artist == track.album_artist
                        })
                        .and_then(|index| album_cover_urls.get(index))
                        .filter(|url| !url.is_empty())
                })
                .next()
                .cloned()
                .unwrap_or_default()
        })
        .collect()
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

fn cover_cache_path() -> Result<PathBuf, String> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .map(|cache_home| cache_home.join("liusheng/covers"))
        .ok_or_else(|| "无法确定用户缓存目录".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artist: &str, album: &str, album_artist: &str) -> TrackRow {
        TrackRow {
            id: 0,
            path: format!("/{title}.flac"),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            album_artist: album_artist.into(),
            track_no: None,
            disc_no: None,
            year: None,
            genre: String::new(),
            duration_ms: 0,
            sample_rate: 44_100,
            bit_depth: Some(16),
            channels: 2,
        }
    }

    #[test]
    fn empty_library_scan_reports_zero_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("music");
        std::fs::create_dir(&root).unwrap();
        let outcome = scan_paths(
            &root,
            &dir.path().join("library.db"),
            &dir.path().join("covers"),
        )
        .unwrap();
        assert_eq!(outcome.track_count, 0);
        assert!(outcome.albums.is_empty());
        assert!(outcome.album_cover_urls.is_empty());
        assert!(outcome.artists.is_empty());
        assert!(outcome.artist_cover_urls.is_empty());
        assert!(outcome.tracks.is_empty());
        assert_eq!(outcome.status_text(), "扫描完成，0 首");
    }

    #[test]
    fn missing_music_root_has_actionable_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("missing");
        let error = scan_paths(
            &root,
            &dir.path().join("library.db"),
            &dir.path().join("covers"),
        )
        .unwrap_err();
        assert!(error.contains("请检查音乐目录"));
    }

    #[test]
    fn track_filter_matches_original_text_full_pinyin_and_initials() {
        let tracks = [
            track("江南", "林俊杰", "第二天堂", "林俊杰"),
            track("晴天", "周杰伦", "叶惠美", "周杰伦"),
        ];
        let blobs = tracks.iter().map(track_search_blob).collect::<Vec<_>>();

        for query in ["江南", "jiangnan", "ljj", "第二天堂", "dett"] {
            assert_eq!(filtered_track_indices(&blobs, &normalize(query)), vec![0]);
        }
        assert_eq!(filtered_track_indices(&blobs, ""), vec![0, 1]);
        assert!(filtered_track_indices(&blobs, "nomatch").is_empty());
    }

    #[test]
    fn queue_index_tracks_removals_before_at_and_after_the_current_track() {
        assert_eq!(queue_index_after_removal(Some(2), 0, 3), Some(1));
        assert_eq!(queue_index_after_removal(Some(2), 2, 3), Some(2));
        assert_eq!(queue_index_after_removal(Some(2), 2, 2), Some(1));
        assert_eq!(queue_index_after_removal(Some(1), 2, 2), Some(1));
        assert_eq!(queue_index_after_removal(Some(0), 0, 0), None);
        assert_eq!(queue_index_after_removal(None, 0, 2), None);
    }

    #[test]
    fn progress_is_ignored_after_the_queue_has_been_cleared() {
        assert_eq!(progress_position_ms(false, 3.0, 0), None);
        assert_eq!(progress_position_ms(true, 3.0, 5_000), Some(3_000));
        assert_eq!(progress_position_ms(true, 7.0, 5_000), Some(5_000));
    }
}
