use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use liusheng_core::library::{AlbumSummary, Library, ScanStats, TrackRow};

pub struct AppControllerRust {
    status: QString,
    track_count: i32,
    album_count: i32,
    selected_album_index: i32,
    selected_track_count: i32,
    album_open: bool,
    scanning: bool,
    albums: Vec<AlbumSummary>,
    tracks: Vec<TrackRow>,
    selected_tracks: Vec<TrackRow>,
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
            albums: Vec::new(),
            tracks: Vec::new(),
            selected_tracks: Vec::new(),
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
            .map(|track| {
                if track.artist.trim().is_empty() {
                    QString::from("未知艺术家")
                } else {
                    QString::from(&track.artist)
                }
            })
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
