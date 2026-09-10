mod services;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::search_service::{SearchJob, SearchService};
use crate::volume_service::{VolumeCommand, VolumeService};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use liusheng_core::artwork_service::{
    ArtworkEvent, ArtworkRequest, ArtworkService, CachedArtwork, album_cache_key,
};
use liusheng_core::audio::hardware_volume::{VolumeChange, VolumeState};
use liusheng_core::engine::{PlayerCommand, PlayerEvent};
use liusheng_core::library::pinyin::normalize_query;
use liusheng_core::library::service::{LibraryCommand, LibraryEvent, LibraryService};
use liusheng_core::library::{AlbumKey, LibrarySnapshot};
use liusheng_core::library::{AlbumSummary, ArtistSummary, TrackRow};
use liusheng_core::lyrics::Lyrics;
use liusheng_core::output_session::{
    OutputConfig, OutputMode, OutputSession, SessionCommand, SessionEvent,
};
use liusheng_core::queue_order::NavigationState;
use liusheng_core::settings::{AppPaths, AppSettings, SavedSession};

use crate::mpris::{
    Command as MprisCommand, PlaybackSnapshot, PlaybackStatus as MprisPlaybackStatus,
    Service as MprisService,
};

const EXCLUSIVE_DEVICE: &str = "hw:Hybrid,0";

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
    queue_notice: QString,
    queue_notice_revision: i32,
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
    lyric_cues_json: QString,
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
    albums: Arc<Vec<AlbumSummary>>,
    album_cover_urls: Vec<String>,
    artists: Arc<Vec<ArtistSummary>>,
    artist_cover_urls: Vec<String>,
    tracks: Arc<Vec<Arc<TrackRow>>>,
    album_search: Arc<HashMap<AlbumKey, Arc<str>>>,
    artist_search: Arc<HashMap<String, Arc<str>>>,
    artist_indices: Arc<HashMap<String, usize>>,
    visible_track_indices: Vec<usize>,
    selected_tracks: Vec<Arc<TrackRow>>,
    playback_queue: Vec<Arc<TrackRow>>,
    current_queue_index: Option<usize>,
    navigation: NavigationState,
    output_session: Option<OutputSession>,
    mpris: Option<MprisService>,
    volume_service: Option<VolumeService>,
    library_service: Option<LibraryService>,
    artwork_service: Option<ArtworkService>,
    search_service: Option<SearchService>,
    settings: Option<AppSettings>,
    saved_session: SavedSession,
    saved_queue_revision: i32,
    restore_pending: bool,
    library_ready: bool,
    settings_json: QString,
    devices_json: QString,
    saved_ui_json: QString,
    scan_errors: QString,
    playlist_count: i32,
    playlist_revision: i32,
    playlists: Vec<liusheng_core::library::playlists::Playlist>,
    shuffle_enabled: bool,
    repeat_mode: i32,
    lyrics_offset_ms: i32,
    current_accent: QString,
    audio_details: QString,
    backend_details: String,
    artwork_revision: i32,
    searching: bool,
    sort_order: i32,
    format_filter: String,
    search_generation: u64,
    artwork_generation: u64,
    album_indices: Arc<HashMap<AlbumKey, usize>>,
    album_tracks: Arc<HashMap<AlbumKey, Vec<usize>>>,
    artist_tracks: Arc<HashMap<String, Vec<usize>>>,
    artwork_cache: HashMap<String, CachedArtwork>,
    artwork_requested: HashSet<(AlbumKey, u32, u64)>,
    pending_open_files: Vec<PathBuf>,
    lyrics: Option<Lyrics>,
    lyrics_request_path: Option<PathBuf>,
    lyrics_generation: u64,
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
            queue_notice: QString::default(),
            queue_notice_revision: 0,
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
            lyric_cues_json: QString::from("[]"),
            lyrics_error: QString::default(),
            exclusive_output: false,
            output_switching: false,
            output_unavailable: false,
            output_status: QString::from(shared_output_name()),
            output_error: QString::default(),
            hardware_volume_available: false,
            hardware_volume_percent: 100,
            hardware_muted: false,
            hardware_mute_available: false,
            hardware_volume_error: QString::from(initial_hardware_volume_status()),
            albums: Arc::new(Vec::new()),
            album_cover_urls: Vec::new(),
            artists: Arc::new(Vec::new()),
            artist_cover_urls: Vec::new(),
            tracks: Arc::new(Vec::new()),
            album_search: Arc::new(HashMap::new()),
            artist_search: Arc::new(HashMap::new()),
            artist_indices: Arc::new(HashMap::new()),
            visible_track_indices: Vec::new(),
            selected_tracks: Vec::new(),
            playback_queue: Vec::new(),
            current_queue_index: None,
            navigation: NavigationState::default(),
            output_session: None,
            mpris: None,
            volume_service: None,
            library_service: None,
            artwork_service: None,
            search_service: None,
            settings: None,
            saved_session: SavedSession::default(),
            saved_queue_revision: i32::MIN,
            restore_pending: false,
            library_ready: false,
            settings_json: QString::from("{}"),
            devices_json: QString::from("[]"),
            saved_ui_json: QString::from("{}"),
            scan_errors: QString::default(),
            playlist_count: 0,
            playlist_revision: 0,
            playlists: Vec::new(),
            shuffle_enabled: false,
            repeat_mode: 0,
            lyrics_offset_ms: 0,
            current_accent: QString::from("#6f9d99"),
            audio_details: QString::default(),
            backend_details: String::new(),
            artwork_revision: 0,
            searching: false,
            sort_order: 0,
            format_filter: String::new(),
            search_generation: 0,
            artwork_generation: 0,
            album_indices: Arc::new(HashMap::new()),
            album_tracks: Arc::new(HashMap::new()),
            artist_tracks: Arc::new(HashMap::new()),
            artwork_cache: HashMap::new(),
            artwork_requested: HashSet::new(),
            pending_open_files: Vec::new(),
            lyrics: None,
            lyrics_request_path: None,
            lyrics_generation: 0,
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
        #[qproperty(bool, library_ready, cxx_name = "libraryReady")]
        #[qproperty(QString, devices_json, cxx_name = "devicesJson")]
        #[qproperty(QString, settings_json, cxx_name = "settingsJson")]
        #[qproperty(QString, saved_ui_json, cxx_name = "savedUiJson")]
        #[qproperty(QString, scan_errors, cxx_name = "scanErrors")]
        #[qproperty(i32, playlist_count, cxx_name = "playlistCount")]
        #[qproperty(i32, playlist_revision, cxx_name = "playlistRevision")]
        #[qproperty(bool, shuffle_enabled, cxx_name = "shuffleEnabled")]
        #[qproperty(i32, repeat_mode, cxx_name = "repeatMode")]
        #[qproperty(i32, lyrics_offset_ms, cxx_name = "lyricsOffsetMs")]
        #[qproperty(QString, current_accent, cxx_name = "currentAccent")]
        #[qproperty(QString, audio_details, cxx_name = "audioDetails")]
        #[qproperty(i32, artwork_revision, cxx_name = "artworkRevision")]
        #[qproperty(bool, searching)]
        #[qproperty(i32, sort_order, cxx_name = "sortOrder")]
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
        #[qproperty(QString, queue_notice, cxx_name = "queueNotice")]
        #[qproperty(i32, queue_notice_revision, cxx_name = "queueNoticeRevision")]
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
        #[qproperty(QString, lyric_cues_json, cxx_name = "lyricCuesJson")]
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
        #[cxx_name = "requestAlbumCover"]
        fn request_album_cover(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "requestArtistCover"]
        fn request_artist_cover(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "applySettings"]
        fn apply_settings(self: Pin<&mut Self>, json: &QString);

        #[qinvokable]
        #[cxx_name = "saveUiState"]
        fn save_ui_state(self: Pin<&mut Self>, json: &QString);

        #[qinvokable]
        #[cxx_name = "saveQueuePlaylist"]
        fn save_queue_playlist(self: Pin<&mut Self>, name: &QString);

        #[qinvokable]
        #[cxx_name = "playlistName"]
        fn playlist_name(&self, index: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "playPlaylist"]
        fn play_playlist(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "deletePlaylist"]
        fn delete_playlist(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "renamePlaylist"]
        fn rename_playlist(self: Pin<&mut Self>, index: i32, name: &QString);

        #[qinvokable]
        #[cxx_name = "importPlaylist"]
        fn import_playlist(self: Pin<&mut Self>, path: &QString);

        #[qinvokable]
        #[cxx_name = "exportQueue"]
        fn export_queue(self: Pin<&mut Self>, path: &QString);

        #[qinvokable]
        #[cxx_name = "openFiles"]
        fn open_files(self: Pin<&mut Self>, json: &QString);

        #[qinvokable]
        #[cxx_name = "requestPlaybackMode"]
        fn request_playback_mode(self: Pin<&mut Self>, repeat: i32, shuffle: bool);

        #[qinvokable]
        #[cxx_name = "moveQueueTrack"]
        fn move_queue_track(self: Pin<&mut Self>, from: i32, to: i32);

        #[qinvokable]
        #[cxx_name = "requestLyricsOffset"]
        fn request_lyrics_offset(self: Pin<&mut Self>, offset: i32);

        #[qinvokable]
        #[cxx_name = "sortTracks"]
        fn sort_tracks(self: Pin<&mut Self>, order: i32, format: &QString);

        #[qsignal]
        #[cxx_name = "raiseRequested"]
        fn raise_requested(self: Pin<&mut Self>);
        #[qsignal]
        #[cxx_name = "quitRequested"]
        fn quit_requested(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "refreshDevices"]
        fn refresh_devices(self: Pin<&mut Self>);
        #[qinvokable]
        #[cxx_name = "scanLibrary"]
        fn scan_library(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "cancelScan"]
        fn cancel_scan(self: Pin<&mut Self>);

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
        #[cxx_name = "enqueueSelectedTrack"]
        fn enqueue_selected_track(self: Pin<&mut Self>, index: i32, play_next: bool);

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
        #[cxx_name = "enqueueAllTrack"]
        fn enqueue_all_track(self: Pin<&mut Self>, index: i32, play_next: bool);

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
        fn toggle_playback(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "previousTrack"]
        fn previous_track(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "nextTrack"]
        fn next_track(self: Pin<&mut Self>);

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
    pub fn cancel_scan(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().send_library(LibraryCommand::CancelScan);
    }

    pub fn scan_library(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().library_service.is_none() {
            self.as_mut().start_services();
        } else {
            self.as_mut().send_library(LibraryCommand::Refresh);
        }
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
            .album_tracks
            .get(&key)
            .into_iter()
            .flatten()
            .filter_map(|i| self.rust().tracks.get(*i))
            .cloned()
            .collect();
        let selected_track_count = selected_tracks.len().min(i32::MAX as usize) as i32;
        self.as_mut().rust_mut().get_mut().selected_tracks = selected_tracks;
        self.as_mut().set_selected_album_index(index);
        self.as_mut().set_selected_track_count(selected_track_count);
        self.publish_selected_model();
        self.as_mut().set_album_open(true);
        self.as_mut().bump_library_revision();
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
            .artist_tracks
            .get(&key)
            .into_iter()
            .flatten()
            .filter_map(|i| self.rust().tracks.get(*i))
            .cloned()
            .collect::<Vec<_>>();
        let selected_track_count = selected_tracks.len().min(i32::MAX as usize) as i32;
        self.as_mut().rust_mut().get_mut().selected_tracks = selected_tracks;
        self.as_mut().set_selected_artist_index(index);
        self.as_mut().set_selected_track_count(selected_track_count);
        self.publish_selected_model();
        self.as_mut().set_artist_open(true);
        self.as_mut().bump_library_revision();
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

    pub fn enqueue_selected_track(
        mut self: core::pin::Pin<&mut Self>,
        index: i32,
        play_next: bool,
    ) {
        let Some(track) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().selected_tracks.get(i))
            .cloned()
        else {
            return;
        };
        self.as_mut().enqueue_track(track, play_next);
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

    pub fn enqueue_all_track(mut self: core::pin::Pin<&mut Self>, index: i32, play_next: bool) {
        let Some(track) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().visible_track_indices.get(i))
            .and_then(|i| self.rust().tracks.get(*i))
            .cloned()
        else {
            return;
        };
        self.as_mut().enqueue_track(track, play_next);
    }

    pub fn filter_tracks(mut self: core::pin::Pin<&mut Self>, query: &QString) {
        self.as_mut()
            .set_track_filter(QString::from(&normalize_query(&query.to_string())));
        self.as_mut().submit_search();
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
        let ordered_current = current.and_then(|i| {
            self.rust()
                .navigation
                .order
                .current_after_removal(i, index, *self.repeat_mode() as u8)
        });
        self.as_mut()
            .rust_mut()
            .get_mut()
            .playback_queue
            .remove(index);
        let queue_len = self.rust().playback_queue.len();
        let next_current =
            ordered_current.or_else(|| queue_index_after_removal(current, index, queue_len));
        self.as_mut().rust_mut().get_mut().current_queue_index = next_current;
        if self.rust().restore_pending {
            let navigation = &mut self.as_mut().rust_mut().get_mut().navigation;
            navigation.order.remove(index, navigation.shuffle);
            navigation.current = next_current.unwrap_or(0);
        }
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
        if self.rust().restore_pending
            && current == Some(index)
            && let Some(next) = next_current
        {
            self.as_mut().select_restored_track(next);
        }
        self.as_mut().checkpoint_session();
        self.sync_mpris();
    }

    pub fn clear_queue(mut self: core::pin::Pin<&mut Self>) {
        if *self.playback_initializing() || self.rust().playback_queue.is_empty() {
            return;
        }
        self.send_player_command(PlayerCommand::ClearQueue);
        self.as_mut().reset_empty_queue();
    }

    fn enqueue_track(mut self: core::pin::Pin<&mut Self>, track: Arc<TrackRow>, play_next: bool) {
        if *self.playback_initializing() {
            return;
        }
        if self.rust().playback_queue.is_empty() || !*self.has_current_track() || !*self.seekable()
        {
            self.as_mut().play_track_queue(vec![track], 0);
            self.as_mut().show_queue_notice("已开始播放");
            return;
        }

        let path = PathBuf::from(&track.path);
        let insertion = queue_insertion_index(
            self.rust().current_queue_index,
            self.rust().playback_queue.len(),
            play_next,
        );
        self.as_mut()
            .rust_mut()
            .get_mut()
            .playback_queue
            .insert(insertion, track);
        if self.rust().restore_pending {
            let navigation = &mut self.as_mut().rust_mut().get_mut().navigation;
            navigation
                .order
                .insert(insertion, navigation.current, navigation.shuffle, play_next);
        }
        if play_next {
            self.send_player_command(PlayerCommand::InsertNext(path));
        } else {
            self.send_player_command(PlayerCommand::AppendQueueItem(path));
        }

        let queue_count = self.rust().playback_queue.len().min(i32::MAX as usize) as i32;
        self.as_mut().set_queue_count(queue_count);
        self.as_mut().bump_queue_revision();
        self.as_mut().show_queue_notice(if play_next {
            "已设为下一首"
        } else {
            "已加入队列"
        });
        self.sync_mpris();
    }

    fn show_queue_notice(mut self: core::pin::Pin<&mut Self>, notice: &str) {
        self.as_mut().set_queue_notice(QString::from(notice));
        let revision = if *self.queue_notice_revision() == i32::MAX {
            0
        } else {
            *self.queue_notice_revision() + 1
        };
        self.as_mut().set_queue_notice_revision(revision);
    }

    fn reset_empty_queue(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().rust_mut().get_mut().navigation = NavigationState::default();
        let rust = self.as_mut().rust_mut().get_mut();
        rust.playback_queue.clear();
        rust.restore_pending = false;
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
        self.as_mut().set_lyric_cues_json(QString::from("[]"));
        self.as_mut().set_current_lyric_index(-1);
        self.as_mut().set_lyrics_error(QString::default());
        self.as_mut().bump_lyrics_revision();
        self.sync_mpris();
    }

    fn play_track_queue(
        mut self: core::pin::Pin<&mut Self>,
        playback_queue: Vec<Arc<TrackRow>>,
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

        let mut navigation = self.rust().navigation.clone();
        let same_queue = playback_queue.iter().map(|t| &t.path).eq(self
            .rust()
            .playback_queue
            .iter()
            .map(|t| &t.path));
        if !same_queue
            || !navigation.order.is_valid_for(playback_queue.len())
            || navigation.shuffle != *self.shuffle_enabled()
        {
            navigation
                .order
                .rebuild(playback_queue.len(), start, *self.shuffle_enabled());
        }
        navigation.current = start;
        navigation.repeat = *self.repeat_mode() as u8;
        navigation.shuffle = *self.shuffle_enabled();
        let order = navigation.order.clone();
        self.as_mut().rust_mut().get_mut().navigation = navigation;
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

        self.as_mut().rust_mut().get_mut().restore_pending = false;
        self.as_mut().rust_mut().get_mut().backend_details.clear();
        self.as_mut().update_track_extras();
        self.as_mut().ensure_output_session();
        self.send_player_command(PlayerCommand::SetQueueOrdered {
            paths,
            start,
            order,
        });
        self.send_player_command(PlayerCommand::Play);
    }

    pub fn toggle_playback(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().restore_pending {
            self.as_mut().resume_saved_queue();
            return;
        }
        self.send_player_command(if *self.playing() {
            PlayerCommand::Pause
        } else {
            PlayerCommand::Play
        });
    }

    pub fn previous_track(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().restore_pending {
            if let Some(index) = self.rust().navigation.previous() {
                self.as_mut().select_restored_track(index);
            }
            return;
        }
        self.send_player_command(PlayerCommand::Prev);
    }

    pub fn next_track(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().restore_pending {
            if let Some(index) = self.rust().navigation.next() {
                self.as_mut().select_restored_track(index);
            }
            return;
        }
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
        if self.rust().output_session.is_none() && !self.rust().restore_pending {
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
        self.as_mut().ensure_volume_service();
        if let (Some(service), Some(settings)) =
            (&self.rust().volume_service, &self.rust().settings)
        {
            service.send(VolumeCommand::Refresh(
                settings.mixer_device.clone(),
                settings.mixer_element.clone(),
            ));
        }
    }
    pub fn request_hardware_volume(self: core::pin::Pin<&mut Self>, percent: i32) {
        if let Some(service) = &self.rust().volume_service {
            service.send(VolumeCommand::Change(VolumeChange::Percent(
                percent.clamp(0, 100) as u8,
            )));
        }
    }
    pub fn toggle_hardware_mute(self: core::pin::Pin<&mut Self>) {
        if *self.hardware_mute_available()
            && let Some(service) = &self.rust().volume_service
        {
            service.send(VolumeCommand::Change(VolumeChange::Muted(
                !*self.hardware_muted(),
            )));
        }
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

    fn ensure_output_session(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().output_session.is_some() {
            return;
        }
        self.as_mut().set_playback_initializing(true);
        let session = OutputSession::start(OutputConfig {
            initial_mode: output_mode(*self.exclusive_output()),
            exclusive_device: self
                .rust()
                .settings
                .as_ref()
                .map(|s| s.exclusive_device.clone())
                .unwrap_or_else(|| EXCLUSIVE_DEVICE.into()),
        });
        session.send(SessionCommand::Playback(PlayerCommand::SetPlaybackMode {
            repeat: *self.repeat_mode() as u8,
            shuffle: *self.shuffle_enabled(),
        }));
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

        let generation = self.rust().lyrics_generation.wrapping_add(1);
        self.as_mut().rust_mut().get_mut().lyrics_generation = generation;
        self.as_mut().rust_mut().get_mut().lyrics_request_path = Some(path.clone());
        self.as_mut().rust_mut().get_mut().lyrics = None;
        self.as_mut().set_lyrics_loading(true);
        self.as_mut().set_lyrics_synced(false);
        self.as_mut().set_lyric_line_count(0);
        self.as_mut().set_lyric_cues_json(QString::from("[]"));
        self.as_mut().set_current_lyric_index(-1);
        self.as_mut().set_lyrics_error(QString::default());
        self.as_mut().bump_lyrics_revision();

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = Lyrics::load(&path)
                .map(|lyrics| {
                    let json = lyrics
                        .as_ref()
                        .map(|value| {
                            serde_json::to_string(&value.display_cues())
                                .unwrap_or_else(|_| "[]".into())
                        })
                        .unwrap_or_else(|| "[]".into());
                    (lyrics, json)
                })
                .map_err(|error| error.to_string());
            qt_thread
                .queue(move |mut controller| {
                    if controller.rust().lyrics_generation != generation
                        || controller.rust().lyrics_request_path.as_ref() != Some(&path)
                    {
                        return;
                    }
                    controller.as_mut().set_lyrics_loading(false);
                    match result {
                        Ok((Some(lyrics), cues)) => {
                            controller
                                .as_mut()
                                .set_lyric_cues_json(QString::from(&cues));
                            let line_count = lyrics.lines().len().min(i32::MAX as usize) as i32;
                            controller
                                .as_mut()
                                .set_lyrics_synced(lyrics.is_synchronized());
                            controller.as_mut().set_lyric_line_count(line_count);
                            controller.as_mut().set_lyrics_error(QString::default());
                            controller.as_mut().rust_mut().get_mut().lyrics = Some(lyrics);
                        }
                        Ok((None, _)) => {
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
        let position_ms =
            u64::try_from(i64::from(*self.position_ms()) - i64::from(*self.lyrics_offset_ms()))
                .ok();
        let index = self
            .rust()
            .lyrics
            .as_ref()
            .and_then(|lyrics| position_ms.and_then(|position| lyrics.active_index(position)))
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
            PlayerEvent::NavigationChanged(navigation) => {
                if navigation.order.sequence().len() == self.rust().playback_queue.len() {
                    self.as_mut().rust_mut().get_mut().navigation = navigation;
                }
            }
            PlayerEvent::PreloadReady { .. } => return,
            PlayerEvent::OutputInfo { description } => {
                self.as_mut().rust_mut().get_mut().backend_details = description;
                self.as_mut().update_track_extras();
                return;
            }
            PlayerEvent::TrackStarted {
                index,
                path,
                duration_secs,
                spec,
            } => {
                if self.rust().playback_queue.is_empty() {
                    return;
                }
                self.as_mut().rust_mut().get_mut().backend_details.clear();
                self.as_mut().rust_mut().get_mut().current_queue_index = Some(index);
                if let Some(track) = self
                    .as_mut()
                    .rust_mut()
                    .get_mut()
                    .playback_queue
                    .get_mut(index)
                {
                    let track = Arc::make_mut(track);
                    track.sample_rate = spec.rate;
                    track.channels = spec.channels.min(u8::MAX as u16) as u8;
                    track.bit_depth = Some(spec.bits.min(u8::MAX as u16) as u8);
                    if let Some(duration) = duration_secs {
                        track.duration_ms = (duration * 1000.0).max(0.0) as u64;
                    }
                }
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
                self.as_mut().update_track_extras();
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
        self.as_mut().checkpoint_session();
        self.sync_mpris();
    }

    fn ensure_mpris(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().mpris.is_some() {
            return;
        }
        let (service, commands) = match MprisService::start() {
            Ok(started) => started,
            Err(error) => {
                eprintln!("系统媒体控制启动失败：{error}");
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
            MprisCommand::SetRepeatMode(repeat) => {
                let shuffle = *self.shuffle_enabled();
                self.as_mut()
                    .request_playback_mode(i32::from(repeat), shuffle);
            }
            MprisCommand::SetShuffle(shuffle) => {
                let repeat = *self.repeat_mode();
                self.as_mut().request_playback_mode(repeat, shuffle);
            }
            MprisCommand::OpenUri(uri) => self.as_mut().open_files(&QString::from(
                &serde_json::to_string(&vec![uri]).unwrap_or_default(),
            )),
            MprisCommand::Raise => self.as_mut().raise_requested(),
            MprisCommand::Quit => {
                self.as_mut().checkpoint_session();
                self.as_mut().quit_requested();
            }
            MprisCommand::ServiceError(error) => {
                eprintln!("系统媒体控制：{error}");
            }
            MprisCommand::Next => self.next_track(),
            MprisCommand::Previous => self.previous_track(),
            MprisCommand::Pause => {
                if *self.playing() && self.rust().output_session.is_some() {
                    self.send_player_command(PlayerCommand::Pause);
                }
            }
            MprisCommand::PlayPause => self.toggle_playback(),
            MprisCommand::Stop => {
                if self.rust().restore_pending {
                    self.as_mut().set_position_ms(0);
                    self.as_mut().checkpoint_session();
                    self.sync_mpris();
                } else {
                    self.send_player_command(PlayerCommand::Stop);
                }
            }
            MprisCommand::Play => {
                if self.rust().restore_pending {
                    self.as_mut().resume_saved_queue();
                } else if !*self.playing() && self.rust().output_session.is_some() {
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
            repeat_mode: *self.repeat_mode() as u8,
            shuffle: *self.shuffle_enabled(),
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
            art_url: if has_track {
                self.current_cover_url().to_string()
            } else {
                String::new()
            },
            path: track.map(|track| track.path.clone()).unwrap_or_default(),
            duration_us: i64::from(*self.current_duration_ms()) * 1000,
            position_us: i64::from(*self.position_ms()) * 1000,
            track_number: track.and_then(|track| track.track_no),
            queue_index,
            queue_len: self.rust().playback_queue.len(),
            can_next: self.rust().navigation.can_next(),
            can_previous: self.rust().navigation.previous().is_some(),
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
            .and_then(|track| {
                let key = AlbumKey {
                    album: track.album.clone(),
                    album_artist: track.album_artist.clone(),
                };
                self.rust()
                    .artwork_cache
                    .get(&album_cache_key(&key))
                    .and_then(|cache| cache.variants.get(&768))
                    .map(String::as_str)
                    .filter(|s| !s.is_empty())
                    .or_else(|| self.cover_url_for_track(track))
            })
            .unwrap_or_default()
            .to_owned();
        self.as_mut()
            .set_current_cover_url(QString::from(&cover_url));
        self.sync_mpris();
    }

    fn cover_url_for_track(&self, track: &TrackRow) -> Option<&str> {
        let key = AlbumKey {
            album: track.album.clone(),
            album_artist: track.album_artist.clone(),
        };
        self.rust()
            .album_indices
            .get(&key)
            .and_then(|i| self.rust().album_cover_urls.get(*i))
            .map(String::as_str)
            .filter(|s| !s.is_empty())
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
            .map(Arc::as_ref)
    }

    fn queue_track_at(&self, index: i32) -> Option<&TrackRow> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().playback_queue.get(index))
            .map(Arc::as_ref)
    }

    fn all_track_at(&self, index: i32) -> Option<&TrackRow> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().visible_track_indices.get(index))
            .and_then(|index| self.rust().tracks.get(*index))
            .map(Arc::as_ref)
    }

    fn bump_library_revision(mut self: core::pin::Pin<&mut Self>) {
        self.publish_track_model();
        self.publish_selected_model();
        let next_revision = (*self.library_revision()).wrapping_add(1);
        self.as_mut().set_library_revision(next_revision);
    }

    fn bump_queue_revision(mut self: core::pin::Pin<&mut Self>) {
        self.publish_queue_model();
        let next_revision = (*self.queue_revision()).wrapping_add(1);
        self.as_mut().set_queue_revision(next_revision);
    }
}

fn output_status(exclusive: bool) -> &'static str {
    if exclusive {
        "ALSA 独占"
    } else {
        shared_output_name()
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
        OutputMode::Shared => connecting_shared_output_status(),
        OutputMode::Exclusive => "正在连接独占设备",
    }
}

#[cfg(target_os = "linux")]
fn shared_output_name() -> &'static str {
    "PipeWire"
}

#[cfg(target_os = "macos")]
fn shared_output_name() -> &'static str {
    "CoreAudio"
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn shared_output_name() -> &'static str {
    "系统输出"
}

#[cfg(target_os = "linux")]
fn connecting_shared_output_status() -> &'static str {
    "正在连接 PipeWire"
}

#[cfg(target_os = "macos")]
fn connecting_shared_output_status() -> &'static str {
    "正在连接 CoreAudio"
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn connecting_shared_output_status() -> &'static str {
    "正在连接系统输出"
}

#[cfg(target_os = "linux")]
fn initial_hardware_volume_status() -> &'static str {
    "正在检测硬件音量"
}

#[cfg(not(target_os = "linux"))]
fn initial_hardware_volume_status() -> &'static str {
    "请使用系统音量控制"
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

fn queue_insertion_index(current: Option<usize>, queue_len: usize, play_next: bool) -> usize {
    if play_next {
        current
            .map(|index| index.saturating_add(1))
            .unwrap_or(queue_len)
            .min(queue_len)
    } else {
        queue_len
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn queued_tracks_are_inserted_next_or_appended() {
        assert_eq!(queue_insertion_index(Some(1), 4, true), 2);
        assert_eq!(queue_insertion_index(Some(3), 4, true), 4);
        assert_eq!(queue_insertion_index(None, 4, true), 4);
        assert_eq!(queue_insertion_index(Some(1), 4, false), 4);
    }
}
