mod common;

use liusheng_core::library::Library;

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
