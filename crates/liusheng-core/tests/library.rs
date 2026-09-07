mod common;
use std::time::Duration;

use liusheng_core::library::Library;
use liusheng_core::library::watcher::{LibraryWatchEvent, LibraryWatcher};

#[test]
fn scan_search_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir(&music).unwrap();
    let han = music.join("林俊杰 - 江南.wav");
    let latin = music.join("plain-song.wav");
    common::write_ramp_wav16(&han, 8000, 800, 0);
    common::write_ramp_wav16(&latin, 8000, 800, 0);

    let db = dir.path().join("lib.sqlite3");
    let mut lib = Library::open(&db).unwrap();

    let stats = lib.scan(&music).unwrap();
    assert_eq!((stats.added, stats.failed), (2, 0), "{stats:?}");
    assert_eq!(lib.track_count().unwrap(), 2);

    // 重复扫描全部命中缓存
    let stats = lib.scan(&music).unwrap();
    assert_eq!((stats.added, stats.unchanged), (0, 2), "{stats:?}");

    // 运行时监听会在写入后立即触发扫描，纳秒级 mtime 必须识别同一秒内的修改。
    let previous_mtime = liusheng_core::library::tags::file_mtime_nanos(&latin);
    std::thread::sleep(std::time::Duration::from_millis(2));
    common::write_ramp_wav16(&latin, 8000, 900, 0);
    let current_mtime = liusheng_core::library::tags::file_mtime_nanos(&latin);
    assert_ne!(previous_mtime, current_mtime);
    let stats = lib.scan(&music).unwrap();
    assert_eq!(stats.updated, 1, "{stats:?}");

    // 无标签 wav 的标题回退为文件名，拼音索引应生效
    for query in ["ljj", "linjunjie", "jiangnan", "江南"] {
        let hits = lib.search(query, 10).unwrap();
        assert_eq!(hits.len(), 1, "query = {query}");
        assert!(hits[0].title.contains("江南"), "query = {query}");
    }
    let hits = lib.search("plain", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(lib.search("不存在的歌", 10).unwrap().is_empty());

    // 文件删除后扫描应移除对应记录
    std::fs::remove_file(&latin).unwrap();
    let stats = lib.scan(&music).unwrap();
    assert_eq!(stats.removed, 1, "{stats:?}");
    assert_eq!(lib.track_count().unwrap(), 1);
}

#[test]
fn technical_properties_are_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir(&music).unwrap();
    common::write_ramp_wav16(&music.join("t.wav"), 8000, 4000, 0);

    let db = dir.path().join("lib.sqlite3");
    let mut lib = Library::open(&db).unwrap();
    lib.scan(&music).unwrap();

    let rows = lib.all_tracks().unwrap();
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.sample_rate, 8000);
    assert_eq!(r.bit_depth, Some(16));
    assert_eq!(r.channels, 2);
    // 4000 帧 @8000Hz = 500ms
    assert!(
        (r.duration_ms as i64 - 500).abs() <= 10,
        "duration = {}",
        r.duration_ms
    );
}

#[test]
fn watcher_changes_drive_incremental_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir(&music).unwrap();
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    let watcher = LibraryWatcher::start(&music).unwrap();
    let events = watcher.events();
    let track = music.join("live.wav");

    common::write_ramp_wav16(&track, 8000, 800, 0);
    assert_eq!(
        events
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap(),
        LibraryWatchEvent::PathsChanged(vec![track.clone()])
    );
    let stats = library.scan(&music).unwrap();
    assert_eq!((stats.added, library.track_count().unwrap()), (1, 1));

    std::fs::remove_file(&track).unwrap();
    assert_eq!(
        events
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap(),
        LibraryWatchEvent::PathsChanged(vec![track.clone()])
    );
    let stats = library.scan(&music).unwrap();
    assert_eq!((stats.removed, library.track_count().unwrap()), (1, 0));
}

#[test]
fn scanning_one_root_preserves_other_roots_and_offline_cache() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    common::write_ramp_wav16(&a.join("a.wav"), 44_100, 882, 0);
    common::write_ramp_wav16(&b.join("b.wav"), 44_100, 882, 0);
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    library.scan(&a).unwrap();
    library.scan(&b).unwrap();
    assert_eq!(library.track_count().unwrap(), 2);
    std::fs::remove_file(a.join("a.wav")).unwrap();
    library.scan(&a).unwrap();
    assert_eq!(library.track_count().unwrap(), 1);
    std::fs::rename(&b, dir.path().join("offline")).unwrap();
    assert!(library.scan(&b).is_err());
    assert_eq!(library.track_count().unwrap(), 1);
}

#[test]
fn path_update_changes_only_the_requested_audio_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let a = root.join("a.wav");
    let b = root.join("b.wav");
    common::write_ramp_wav16(&a, 44_100, 882, 0);
    common::write_ramp_wav16(&b, 44_100, 882, 0);
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    library.scan(&root).unwrap();
    common::write_ramp_wav16(&a, 44_100, 1764, 0);
    let stats = library
        .update_paths(std::slice::from_ref(&root), &[], std::slice::from_ref(&a))
        .unwrap();
    assert_eq!(stats.updated, 1);
    assert_eq!(stats.unchanged, 0);
    assert_eq!(stats.removed, 0);
    assert_eq!(library.snapshot().unwrap().tracks.len(), 2);
}

#[test]
fn cancelled_scan_keeps_unvisited_records() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let a = root.join("a.wav");
    common::write_ramp_wav16(&a, 44_100, 882, 0);
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    library.scan(&root).unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    assert!(
        library
            .scan_controlled(&root, &[], &cancelled, |_, _| {})
            .is_err()
    );
    assert_eq!(library.track_count().unwrap(), 1);
}

#[test]
fn cached_library_is_available_before_offline_root_scan() {
    use liusheng_core::library::service::{LibraryEvent, LibraryService};
    use liusheng_core::settings::{AppPaths, AppSettings};
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        database: dir.path().join("library.db"),
        settings: dir.path().join("settings.json"),
        session: dir.path().join("session.json"),
        covers: dir.path().join("covers"),
    };
    let settings = AppSettings {
        music_roots: vec![dir.path().join("offline")],
        ..Default::default()
    };
    settings.save(&paths.settings).unwrap();
    let service = LibraryService::start(paths).unwrap();
    assert!(matches!(
        service
            .events()
            .recv_timeout(Duration::from_secs(3))
            .unwrap(),
        LibraryEvent::Ready { .. }
    ));
}
