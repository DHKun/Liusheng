use super::*;
use std::path::Path;

fn fixture() -> (tempfile::TempDir, AppPaths, AppSettings) {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir(&music).unwrap();
    let mut writer = hound::WavWriter::create(
        music.join("song.wav"),
        hound::WavSpec {
            channels: 2,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..800 {
        writer.write_sample(0i16).unwrap();
    }
    writer.finalize().unwrap();
    let paths = AppPaths {
        database: dir.path().join("library.db"),
        settings: dir.path().join("settings.json"),
        session: dir.path().join("session.json"),
        covers: dir.path().join("covers"),
    };
    let settings = AppSettings {
        music_roots: vec![music],
        ..Default::default()
    };
    settings.save(&paths.settings).unwrap();
    (dir, paths, settings)
}

#[test]
fn blocked_metadata_io_keeps_settings_playlists_checkpoint_and_shutdown_responsive() {
    let (_dir, paths, mut settings) = fixture();
    let (entered, entry) = bounded(1);
    let (release, gate) = bounded(1);
    let (finished, finish) = bounded(1);
    let reader: scan::MetadataReader = Arc::new(move |path: &Path| {
        entered.send(()).unwrap();
        gate.recv_timeout(Duration::from_secs(12))
            .map_err(|_| crate::Error::Interrupted)?;
        let result = super::super::tags::read_meta(path);
        finished.send(()).ok();
        result
    });
    let service = LibraryService::start_inner(paths.clone(), reader).unwrap();
    entry
        .recv_timeout(Duration::from_secs(5))
        .expect("metadata reader entered its controlled barrier");
    let events = service.events();
    settings.check_updates_on_startup = false;
    let start = Instant::now();
    assert!(service.send(LibraryCommand::Settings(settings)));
    assert!(service.send(LibraryCommand::SavePlaylist {
        name: "during scan".into(),
        paths: vec!["/offline/song.flac".into()]
    }));
    let (mut saved, mut playlist) = (false, false);
    while !(saved && playlist) {
        match events.recv_timeout(Duration::from_secs(3)).unwrap() {
            LibraryEvent::SettingsSaved(s) => {
                assert!(!s.check_updates_on_startup);
                saved = true;
            }
            LibraryEvent::Playlists(p) if p.len() == 1 => playlist = true,
            LibraryEvent::Error(e) => panic!("unexpected service error: {e}"),
            _ => {}
        }
    }
    assert!(start.elapsed() < Duration::from_secs(3));
    service.save_session(SavedSession {
        version: 1,
        position_ms: 4321,
        ..Default::default()
    });
    // Commands remain continuously active; the save deadline still progresses.
    let deadline = Instant::now() + Duration::from_secs(4);
    while !paths.session.exists() && Instant::now() < deadline {
        service.send(LibraryCommand::Refresh);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        SavedSession::load(&paths.session).unwrap().position_ms,
        4321
    );
    assert!(service.send(LibraryCommand::CancelScan));
    let shutdown = Instant::now();
    drop(service);
    assert!(shutdown.elapsed() < Duration::from_secs(1));
    release.send(()).unwrap();
    finish.recv_timeout(Duration::from_secs(3)).unwrap();
    // An obsolete reader's late result has no SQLite owner to apply it.
    let library = Library::open(&paths.database).unwrap();
    assert_eq!(library.track_count().unwrap(), 0);
    assert_eq!(library.playlists().unwrap().len(), 1);
}

#[test]
fn checkpoint_write_failure_retains_latest_state_for_retry() {
    let (_dir, mut paths, _) = fixture();
    let (events, _) = unbounded();
    std::fs::write(&paths.session, "parent is a file").unwrap();
    paths.session = paths.session.join("invalid-child.json");
    let pending = Mutex::new(Some(SavedSession {
        version: 1,
        position_ms: 99,
        ..Default::default()
    }));
    flush_session(&paths, &pending, &events);
    assert_eq!(pending.lock().unwrap().as_ref().unwrap().position_ms, 99);
    paths.session = paths.database.with_file_name("recovered-session.json");
    flush_session(&paths, &pending, &events);
    assert!(pending.lock().unwrap().is_none());
    assert_eq!(SavedSession::load(&paths.session).unwrap().position_ms, 99);
}

#[test]
fn unchanged_refresh_emits_completion_without_republishing_track_snapshot() {
    let (_dir, paths, settings) = fixture();
    let mut library = Library::open(&paths.database).unwrap();
    library.scan(&settings.music_roots[0]).unwrap();
    drop(library);
    let service = LibraryService::start(paths).unwrap();
    let mut ready = false;
    loop {
        match service
            .events()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
        {
            LibraryEvent::Ready { .. } => ready = true,
            LibraryEvent::Snapshot { .. } => panic!("unchanged scan rebuilt its snapshot"),
            LibraryEvent::ScanFinished { stats, cancelled } => {
                assert!(ready);
                assert!(!cancelled);
                assert_eq!(stats.unchanged, 1);
                break;
            }
            LibraryEvent::Error(e) => panic!("{e}"),
            _ => {}
        }
    }
}
