use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use liusheng_core::library::{AlbumSummary, Library, ScanStats};

pub struct AppControllerRust {
    status: QString,
    track_count: i32,
    album_count: i32,
    scanning: bool,
    albums: Vec<AlbumSummary>,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            status: QString::from("曲库待扫描"),
            track_count: 0,
            album_count: 0,
            scanning: false,
            albums: Vec::new(),
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
                            controller
                                .as_mut()
                                .set_track_count(outcome.track_count.min(i32::MAX as u64) as i32);
                            controller.as_mut().set_album_count(album_count);
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

    fn album_at(&self, index: i32) -> Option<&AlbumSummary> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().albums.get(index))
    }
}

#[derive(Debug)]
struct ScanOutcome {
    stats: ScanStats,
    track_count: u64,
    albums: Vec<AlbumSummary>,
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
    Ok(ScanOutcome {
        stats,
        track_count,
        albums,
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
