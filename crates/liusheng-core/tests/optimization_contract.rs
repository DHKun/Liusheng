mod common;

use liusheng_core::{library::Library, lyrics::Lyrics};
use lofty::{
    config::WriteOptions,
    prelude::{Accessor, TagExt},
    tag::{ItemKey, Tag, TagType},
};
use std::path::Path;

fn tagged_song(path: &Path) {
    common::write_ramp_wav16(path, 8000, 800, 0);
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title("晴天（Live）".into());
    tag.set_artist("周杰伦".into());
    tag.set_album("叶惠美".into());
    tag.insert_text(ItemKey::UnsyncLyrics, "内嵌第一行\n内嵌第二行".into());
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

#[test]
fn combined_search_matches_artist_and_title_in_either_order() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir(&music).unwrap();
    tagged_song(&music.join("song.wav"));
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    library.scan(&music).unwrap();
    for query in ["周杰伦 晴天", "晴天 周杰伦", "zjl qingtian", "Live 叶惠美"] {
        assert_eq!(library.search(query, 10).unwrap().len(), 1, "{query}");
    }
    assert!(library.search("周杰伦 江南", 10).unwrap().is_empty());
}

#[test]
fn corrupt_sidecar_falls_back_to_embedded_lyrics() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("song.wav");
    tagged_song(&audio);
    std::fs::write(audio.with_extension("lrc"), [0xff, 0x80, 0x81]).unwrap();
    let lyrics = Lyrics::load(&audio).unwrap().unwrap();
    assert_eq!(lyrics.lines()[0].text, "内嵌第一行");
}

#[test]
fn directory_change_only_reconciles_its_subtree() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    let changed = music.join("album");
    let sibling = music.join("album-extra");
    std::fs::create_dir_all(&changed).unwrap();
    std::fs::create_dir_all(&sibling).unwrap();
    common::write_ramp_wav16(&changed.join("one.wav"), 8000, 80, 0);
    common::write_ramp_wav16(&sibling.join("two.wav"), 8000, 80, 0);
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    library.scan(&music).unwrap();
    common::write_ramp_wav16(&changed.join("new.wav"), 8000, 80, 0);
    let stats = library.update_paths(&[music], &[], &[changed]).unwrap();
    assert_eq!((stats.added, stats.unchanged, stats.failed), (1, 1, 0));
    assert_eq!(library.track_count().unwrap(), 3);
}

#[test]
fn utf16_sidecars_and_oversized_sidecars_have_safe_fallbacks() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("song.wav");
    tagged_song(&audio);
    let lrc = audio.with_extension("lrc");
    for little in [true, false] {
        let mut bytes = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for word in "[00:01.00]你好 · 世界".encode_utf16() {
            bytes.extend(if little {
                word.to_le_bytes()
            } else {
                word.to_be_bytes()
            });
        }
        std::fs::write(&lrc, bytes).unwrap();
        let lyrics = Lyrics::load(&audio).unwrap().unwrap();
        assert_eq!(lyrics.lines()[0].start_ms, Some(1000));
        assert_eq!(lyrics.lines()[0].text, "你好 · 世界");
    }
    for text in [
        "x".repeat(1024 * 1024 + 1),
        "x\n".repeat(10_001),
        "x".repeat(8193),
    ] {
        std::fs::write(&lrc, text).unwrap();
        assert_eq!(
            Lyrics::load(&audio).unwrap().unwrap().lines()[0].text,
            "内嵌第一行"
        );
    }
}

#[test]
fn deleted_nested_scope_keeps_its_prefix_sibling_and_offline_cache() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    let album = root.join("A_%[日本語]");
    let sibling = root.join("A_%[日本語]-live");
    std::fs::create_dir_all(&album).unwrap();
    std::fs::create_dir_all(&sibling).unwrap();
    common::write_ramp_wav16(&album.join("one.wav"), 8000, 80, 0);
    common::write_ramp_wav16(&sibling.join("two.wav"), 8000, 80, 0);
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    library.scan(&root).unwrap();
    std::fs::remove_dir_all(&album).unwrap();
    let stats = library
        .update_paths(std::slice::from_ref(&root), &[], &[album])
        .unwrap();
    assert_eq!((stats.removed, stats.state_rows), (1, 1));
    std::fs::rename(&root, dir.path().join("unmounted")).unwrap();
    let stats = library
        .update_paths(std::slice::from_ref(&root), &[], &[sibling])
        .unwrap();
    assert!(stats.failed > 0);
    assert_eq!(library.track_count().unwrap(), 1);
}

#[test]
fn cancelling_a_path_reconciliation_keeps_unvisited_records() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    std::fs::create_dir(&root).unwrap();
    common::write_ramp_wav16(&root.join("one.wav"), 8000, 80, 0);
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    library.scan(&root).unwrap();
    std::fs::remove_file(root.join("one.wav")).unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(true);
    assert!(
        library
            .update_paths_controlled(
                std::slice::from_ref(&root),
                &[],
                std::slice::from_ref(&root),
                &cancel
            )
            .is_err()
    );
    assert_eq!(library.track_count().unwrap(), 1);
}

#[test]
fn one_file_update_reads_one_state_row_and_shares_unchanged_snapshot_rows() {
    use liusheng_core::library::{db, tags::TrackMeta};
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    let album = root.join("album");
    std::fs::create_dir_all(&album).unwrap();
    let file = album.join("one.wav");
    common::write_ramp_wav16(&file, 8000, 80, 0);
    let database = dir.path().join("library.db");
    {
        let mut conn = db::open(&database).unwrap();
        let tx = conn.transaction().unwrap();
        let meta = TrackMeta {
            title: "cached".into(),
            album: "other".into(),
            ..Default::default()
        };
        for i in 0..10_000 {
            db::upsert_track(
                &tx,
                &root
                    .join(format!("unreported/{i:05}.flac"))
                    .to_string_lossy(),
                0,
                &meta,
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    let mut library = Library::open(&database).unwrap();
    library.import_files(std::slice::from_ref(&file)).unwrap();
    let before = library.snapshot().unwrap();
    let cached = library.snapshot().unwrap();
    assert!(Arc::ptr_eq(&before.tracks, &cached.tracks));
    common::write_ramp_wav16(&file, 8000, 160, 0);
    let stats = library
        .update_paths(&[root], &[], std::slice::from_ref(&file))
        .unwrap();
    assert_eq!((stats.updated, stats.visited, stats.state_rows), (1, 1, 1));
    let after = library.snapshot().unwrap();
    assert_eq!(after.tracks.len(), 10_001);
    let old = before.tracks.iter().find(|t| t.album == "other").unwrap();
    let new = after.tracks.iter().find(|t| t.path == old.path).unwrap();
    assert!(Arc::ptr_eq(old, new));
    assert_eq!(
        after
            .tracks
            .iter()
            .find(|t| t.path == file.to_string_lossy())
            .unwrap()
            .duration_ms,
        20
    );
}

#[test]
fn sqlite_connections_explicitly_enforce_playlist_parent_integrity() {
    use liusheng_core::library::{db, playlists};
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("library.db")).unwrap();
    playlists::initialize(&conn).unwrap();
    assert_eq!(
        conn.pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(
        conn.execute(
            "INSERT INTO playlist_entries(playlist_id,position,path) VALUES (123,0,'/offline.wav')",
            []
        )
        .is_err()
    );
}

#[test]
fn real_v010_schema_migrates_atomically_and_retains_metadata() {
    use liusheng_core::library::db;
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("legacy.db");
    {
        let conn = rusqlite::Connection::open(&database).unwrap();
        conn.execute_batch(include_str!("fixtures/schema-v0.1.0.sql"))
            .unwrap();
        conn.execute("INSERT INTO tracks(path,mtime,title,artist,title_search,artist_search) VALUES('/offline.wav',0,'晴天','周杰伦','晴天','周杰伦')", []).unwrap();
    }
    let conn = db::open(&database).unwrap();
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(db::search(&conn, "周杰伦 晴天", 10).unwrap().len(), 1);
    assert_eq!(
        db::file_state_for_path(&conn, "/offline.wav").unwrap(),
        Some((0, -1))
    );
    assert_eq!(
        conn.pragma_query_value(None, "integrity_check", |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    drop(conn);
    assert_eq!(
        Library::open(&database)
            .unwrap()
            .snapshot()
            .unwrap()
            .tracks
            .len(),
        1
    );
}
