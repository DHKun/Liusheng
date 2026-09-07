//! Deterministic reducer tests prove exact batches. Native tests verify eventual
//! index state and accept the additional scopes/duplicate batches allowed by OSes.
use crossbeam_channel::{RecvTimeoutError, bounded};
use notify::event::{AccessKind, AccessMode, DataChange, Flag, RenameMode};

use super::*;
use crate::library::{Library, TrackRow};

fn audio_event(path: &str) -> Event {
    Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content))).add_path(path.into())
}

#[test]
fn canonical_backend_paths_use_configured_database_keys() {
    let roots = [(
        PathBuf::from("/private/var/music"),
        PathBuf::from("/var/music"),
    )];
    let mapped = map_event_roots(audio_event("/private/var/music/album/track.wav"), &roots);
    assert_eq!(
        mapped.paths,
        vec![PathBuf::from("/var/music/album/track.wav")]
    );
    assert!(affects_library(&mapped));
}

#[test]
fn root_mapping_preserves_directory_identity_and_rescan_flags() {
    let roots = [(
        PathBuf::from("/physical/music"),
        PathBuf::from("/music-link"),
    )];
    let event = Event::new(EventKind::Other)
        .add_path("/physical/music/".into())
        .set_flag(Flag::Rescan);
    let mapped = map_event_roots(event, &roots);
    assert_eq!(mapped.paths, vec![PathBuf::from("/music-link")]);
    assert!(mapped.need_rescan());
}

#[test]
fn deleted_paths_map_without_canonicalizing_the_missing_file() {
    let roots = [(
        PathBuf::from("/physical/music"),
        PathBuf::from("/music-link"),
    )];
    let event = Event::new(EventKind::Remove(RemoveKind::Folder))
        .add_path("/physical/music/deleted-album".into());
    let mapped = map_event_roots(event, &roots);
    assert_eq!(
        mapped.paths,
        vec![PathBuf::from("/music-link/deleted-album")]
    );
    assert_eq!(mapped.kind, EventKind::Remove(RemoveKind::Folder));
}

#[test]
fn path_mapping_respects_components_and_multiple_root_aliases() {
    let roots = [
        (PathBuf::from("/physical/music"), PathBuf::from("/music-a")),
        (PathBuf::from("/physical/music"), PathBuf::from("/music-b")),
    ];
    let event = Event::new(EventKind::Any)
        .add_path("/physical/music/track.wav".into())
        .add_path("/physical/music-backup/other.wav".into());
    assert_eq!(
        map_event_roots(event, &roots).paths,
        vec![
            PathBuf::from("/music-a/track.wav"),
            PathBuf::from("/music-b/track.wav"),
            PathBuf::from("/physical/music-backup/other.wav"),
        ]
    );
}

#[test]
fn rename_mapping_keeps_both_endpoints() {
    let roots = [(
        PathBuf::from("/physical/music"),
        PathBuf::from("/music-link"),
    )];
    let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
        .add_path("/physical/music/old-album".into())
        .add_path("/physical/music/new-album".into());
    let mapped = map_event_roots(event, &roots);
    assert_eq!(
        mapped.paths,
        vec![
            PathBuf::from("/music-link/old-album"),
            PathBuf::from("/music-link/new-album")
        ]
    );
    assert!(affects_library(&mapped));
}

#[test]
fn filters_access_and_library_events() {
    let access = Event::new(EventKind::Access(AccessKind::Close(AccessMode::Write)))
        .add_path("track.wav".into());
    assert!(!affects_library(&access));
    for path in ["cover.jpg", "TRACK.FLAC", "track.LRC"] {
        assert!(affects_library(&audio_event(path)), "missing {path}");
    }
    let folder = Event::new(EventKind::Remove(RemoveKind::Folder)).add_path("deleted-album".into());
    assert!(affects_library(&folder));
}

#[test]
fn ignores_unsupported_file_changes() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    for kind in [
        EventKind::Create(CreateKind::File),
        EventKind::Modify(ModifyKind::Data(DataChange::Content)),
        EventKind::Remove(RemoveKind::File),
    ] {
        let event = Event::new(kind).add_path("booklet.pdf".into());
        assert!(!affects_library(&event));
        batch.push(event, now);
    }
    assert!(batch.deadline.is_none());
    assert!(batch.finish().is_none());
}

#[test]
fn coalesces_a_burst_of_audio_changes() {
    let start = Instant::now();
    let mut batch = PendingChanges::default();
    batch.push(
        Event::new(EventKind::Create(CreateKind::File)).add_path("track.wav".into()),
        start,
    );
    batch.push(audio_event("track.wav"), start + Duration::from_millis(10));
    batch.push(audio_event("track.wav"), start + Duration::from_millis(20));
    assert!(batch.take_due(start + Duration::from_millis(519)).is_none());
    assert_eq!(
        batch.take_due(start + Duration::from_millis(520)),
        Some(LibraryWatchEvent::PathsChanged(vec!["track.wav".into()]))
    );
    assert!(batch.take_due(start + Duration::from_secs(20)).is_none());
}

#[test]
fn directory_and_file_notifications_preserve_both_scopes() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    // Replay the extra parent directory seen in macOS arm64 CI. Dropping it can
    // lose changes to other children that FSEvents coalesced into this scope.
    batch.push(
        Event::new(EventKind::Create(CreateKind::Folder)).add_path("/music".into()),
        now,
    );
    batch.push(audio_event("/music/track.wav"), now);
    batch.push(audio_event("/music/track.wav"), now);
    assert_eq!(
        batch.take_due(now + CHANGE_DEBOUNCE),
        Some(LibraryWatchEvent::PathsChanged(vec![
            PathBuf::from("/music"),
            PathBuf::from("/music/track.wav")
        ]))
    );
}

#[test]
fn directory_only_and_removed_imprecise_scopes_are_kept() {
    let now = Instant::now();
    for kind in [
        EventKind::Create(CreateKind::Folder),
        EventKind::Remove(RemoveKind::Folder),
        EventKind::Remove(RemoveKind::Any),
        EventKind::Any,
    ] {
        let mut batch = PendingChanges::default();
        batch.push(Event::new(kind).add_path("deleted-album".into()), now);
        assert_eq!(
            batch.finish(),
            Some(LibraryWatchEvent::PathsChanged(vec![
                "deleted-album".into()
            ]))
        );
    }
}

#[test]
fn rescan_flag_precedes_kind_and_path_filters() {
    let now = Instant::now();
    for event in [
        Event::new(EventKind::Other).set_flag(Flag::Rescan),
        Event::new(EventKind::Remove(RemoveKind::File))
            .add_path("booklet.pdf".into())
            .set_flag(Flag::Rescan),
        Event::new(EventKind::Access(AccessKind::Any)).set_flag(Flag::Rescan),
    ] {
        let mut batch = PendingChanges::default();
        assert!(affects_library(&event));
        batch.push(event, now);
        assert_eq!(
            batch.take_due(now + CHANGE_DEBOUNCE),
            Some(LibraryWatchEvent::Changed)
        );
    }
}

#[test]
fn pathless_change_requests_reconciliation() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    batch.push(Event::new(EventKind::Any), now);
    assert_eq!(batch.finish(), Some(LibraryWatchEvent::Changed));
    assert!(!affects_library(&Event::new(EventKind::Access(
        AccessKind::Any
    ))));
}

#[test]
fn first_oversized_notification_requests_full_refresh() {
    let now = Instant::now();
    let mut event = Event::new(EventKind::Create(CreateKind::File));
    event.paths = (0..MAX_CHANGED_PATHS + 100)
        .map(|i| PathBuf::from(format!("/{i}.wav")))
        .collect();
    let mut batch = PendingChanges::default();
    batch.push(event, now);
    assert!(batch.paths.is_empty());
    assert_eq!(
        batch.take_due(now + CHANGE_DEBOUNCE),
        Some(LibraryWatchEvent::Changed)
    );
}

#[test]
fn path_limit_applies_across_batches_and_counts_unique_paths() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    for i in 0..MAX_CHANGED_PATHS {
        batch.push(audio_event(&format!("/{i}.wav")), now);
    }
    assert_eq!(batch.paths.len(), MAX_CHANGED_PATHS);
    batch.push(audio_event("/0.wav"), now);
    assert!(!batch.rescan, "a duplicate at capacity must stay precise");
    batch.push(audio_event("/overflow.wav"), now);
    assert!(batch.paths.is_empty());
    assert_eq!(batch.finish(), Some(LibraryWatchEvent::Changed));
}

#[test]
fn full_refresh_dominates_later_paths_and_next_batch_resets() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    batch.push(audio_event("before.wav"), now);
    batch.push(Event::new(EventKind::Other).set_flag(Flag::Rescan), now);
    batch.push(audio_event("after.wav"), now);
    assert!(batch.paths.is_empty());
    assert_eq!(batch.finish(), Some(LibraryWatchEvent::Changed));
    batch.push(audio_event("new.wav"), now + MAX_BATCH_DELAY);
    assert_eq!(
        batch.deadline,
        Some(now + MAX_BATCH_DELAY + CHANGE_DEBOUNCE)
    );
    assert_eq!(
        batch.finish(),
        Some(LibraryWatchEvent::PathsChanged(vec!["new.wav".into()]))
    );
}

#[test]
fn batches_sort_deduplicate_and_keep_rename_endpoints() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    batch.push(audio_event("z.wav"), now);
    batch.push(
        Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path("z.wav".into())
            .add_path("a.wav".into()),
        now,
    );
    batch.push(audio_event("a.wav"), now);
    assert_eq!(
        batch.finish(),
        Some(LibraryWatchEvent::PathsChanged(vec![
            "a.wav".into(),
            "z.wav".into()
        ]))
    );
}

#[test]
fn irrelevant_input_leaves_pending_deadline_unchanged() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    batch.push(audio_event("track.wav"), now);
    batch.push(
        Event::new(EventKind::Access(AccessKind::Any)).add_path("track.wav".into()),
        now + Duration::from_millis(400),
    );
    assert_eq!(batch.deadline, Some(now + CHANGE_DEBOUNCE));
    assert_eq!(
        batch.take_due(now + CHANGE_DEBOUNCE),
        Some(LibraryWatchEvent::PathsChanged(vec!["track.wav".into()]))
    );
}

#[test]
fn continuous_changes_stop_extending_at_maximum_deadline() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    for milliseconds in (0..2000).step_by(100) {
        batch.push(
            audio_event("track.wav"),
            now + Duration::from_millis(milliseconds),
        );
        assert!(
            batch
                .take_due(now + Duration::from_millis(milliseconds))
                .is_none()
        );
    }
    assert_eq!(batch.deadline, Some(now + MAX_BATCH_DELAY));
    assert_eq!(
        batch.take_due(now + MAX_BATCH_DELAY),
        Some(LibraryWatchEvent::PathsChanged(vec!["track.wav".into()]))
    );
}

#[test]
fn separate_native_deliveries_can_form_separate_valid_batches() {
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    for at in [now, now + Duration::from_secs(5)] {
        batch.push(audio_event("track.wav"), at);
        assert_eq!(
            batch.take_due(at + CHANGE_DEBOUNCE),
            Some(LibraryWatchEvent::PathsChanged(vec!["track.wav".into()]))
        );
    }
}

struct EventLoopHarness {
    raw: Sender<notify::Result<Event>>,
    stop: Sender<()>,
    events: Receiver<LibraryWatchEvent>,
    worker: Option<JoinHandle<()>>,
}
impl EventLoopHarness {
    fn new() -> Self {
        let (raw, raw_rx) = unbounded();
        let (stop, stop_rx) = bounded(1);
        let (events_tx, events) = unbounded();
        let worker = std::thread::spawn(move || run_event_loop(raw_rx, stop_rx, events_tx));
        Self {
            raw,
            stop,
            events,
            worker: Some(worker),
        }
    }
}
impl Drop for EventLoopHarness {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

#[test]
fn event_loop_delivers_pathless_rescan_and_reports_backend_errors() {
    let harness = EventLoopHarness::new();
    harness
        .raw
        .send(Err(notify::Error::generic("watch backend failed")))
        .unwrap();
    let error = harness.events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        matches!(error, LibraryWatchEvent::Error(ref message) if message.contains("watch backend failed"))
    );
    harness
        .raw
        .send(Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan)))
        .unwrap();
    assert_eq!(
        harness.events.recv_timeout(Duration::from_secs(5)).unwrap(),
        LibraryWatchEvent::Changed
    );
}

#[test]
fn native_channel_close_flushes_last_batch() {
    let (raw_tx, raw_rx) = unbounded();
    let (_stop_tx, stop_rx) = bounded(1);
    let (events_tx, events) = unbounded();
    raw_tx.send(Ok(audio_event("track.wav"))).unwrap();
    drop(raw_tx);
    run_event_loop(raw_rx, stop_rx, events_tx);
    assert_eq!(
        events.recv().unwrap(),
        LibraryWatchEvent::PathsChanged(vec!["track.wav".into()])
    );
    assert!(events.recv().is_err());
}

#[test]
fn queued_shutdown_precedes_ready_native_events() {
    let (raw_tx, raw_rx) = unbounded();
    let (stop_tx, stop_rx) = bounded(1);
    let (events_tx, events) = unbounded();
    raw_tx.send(Ok(audio_event("track.wav"))).unwrap();
    stop_tx.send(()).unwrap();
    run_event_loop(raw_rx, stop_rx, events_tx);
    assert!(events.recv().is_err());
}

/// At-least-once invalidations are consumed exactly like LibraryService. A late
/// parent event may precede the leaf event; only actual index convergence passes.
fn await_index(
    events: &Receiver<LibraryWatchEvent>,
    library: &mut Library,
    root: &Path,
    expected: impl Fn(&[TrackRow]) -> bool,
) -> Vec<LibraryWatchEvent> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut seen = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = events.recv_timeout(remaining).unwrap_or_else(|error| {
            panic!(
                "index did not converge: {error:?}; received {seen:?}; rows {:?}",
                library.all_tracks().unwrap()
            );
        });
        seen.push(event.clone());
        let stats = match event {
            LibraryWatchEvent::PathsChanged(paths) => {
                assert!(!paths.is_empty());
                assert!(
                    paths.windows(2).all(|p| p[0] < p[1]),
                    "batch must be sorted and unique"
                );
                library
                    .update_paths(&[root.to_owned()], &[], &paths)
                    .unwrap()
            }
            LibraryWatchEvent::Changed => library.scan(root).unwrap(),
            LibraryWatchEvent::Error(error) => panic!("watch error: {error}; received {seen:?}"),
        };
        // The native backend may notify while a file is still being written.
        // The predicate also validates final decoded metadata, so partial files
        // never count as success; subsequent invalidations must reconcile them.
        let rows = library.all_tracks().unwrap();
        if expected(&rows) {
            assert_eq!(stats.failed, 0, "final refresh failed: {stats:?}");
            return seen;
        }
    }
}

fn write_wav(path: &Path, frames: usize) {
    let mut writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for sample in 0..frames * 2 {
        writer.write_sample(sample as i16).unwrap();
    }
    writer.finalize().unwrap();
}

#[test]
fn native_files_converge_after_create_modify_rename_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    let watcher = LibraryWatcher::start(&root).unwrap();
    let events = watcher.events();
    let track = root.join("track.wav");
    write_wav(&track, 800);
    await_index(&events, &mut library, &root, |rows| {
        rows.len() == 1 && rows[0].path == track.to_string_lossy() && rows[0].duration_ms == 100
    });
    write_wav(&track, 1600);
    await_index(&events, &mut library, &root, |rows| {
        rows.len() == 1 && rows[0].duration_ms == 200
    });
    let renamed = root.join("renamed.wav");
    std::fs::rename(&track, &renamed).unwrap();
    await_index(&events, &mut library, &root, |rows| {
        rows.len() == 1 && rows[0].path == renamed.to_string_lossy()
    });
    std::fs::remove_file(&renamed).unwrap();
    await_index(&events, &mut library, &root, |rows| rows.is_empty());
}

#[cfg(unix)]
#[test]
fn watcher_keeps_symlink_root_spelling_for_create_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let physical = dir.path().join("physical");
    let alias = dir.path().join("music-link");
    std::fs::create_dir(&physical).unwrap();
    std::os::unix::fs::symlink(&physical, &alias).unwrap();
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    let watcher = LibraryWatcher::start(&alias).unwrap();
    let events = watcher.events();
    let track = alias.join("track.wav");
    write_wav(&track, 800);
    await_index(&events, &mut library, &alias, |rows| {
        rows.len() == 1 && rows[0].path == track.to_string_lossy()
    });
    std::fs::remove_file(&track).unwrap();
    await_index(&events, &mut library, &alias, |rows| rows.is_empty());
}

#[test]
fn native_populated_directory_move_rename_and_remove_converge() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    let staged = dir.path().join("incoming");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&staged).unwrap();
    write_wav(&staged.join("track.wav"), 800);
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    let watcher = LibraryWatcher::start(&root).unwrap();
    let events = watcher.events();
    let album = root.join("album");
    std::fs::rename(&staged, &album).unwrap();
    await_index(&events, &mut library, &root, |rows| {
        rows.len() == 1 && rows[0].path == album.join("track.wav").to_string_lossy()
    });
    let renamed = root.join("renamed-album");
    std::fs::rename(&album, &renamed).unwrap();
    await_index(&events, &mut library, &root, |rows| {
        rows.len() == 1 && rows[0].path == renamed.join("track.wav").to_string_lossy()
    });
    std::fs::remove_dir_all(&renamed).unwrap();
    await_index(&events, &mut library, &root, |rows| rows.is_empty());
}

#[test]
fn native_watcher_shutdown_disconnects_consumer() {
    let dir = tempfile::tempdir().unwrap();
    let watcher = LibraryWatcher::start(dir.path()).unwrap();
    let events = watcher.events();
    drop(watcher);
    loop {
        match events.recv_timeout(Duration::from_secs(5)) {
            Ok(_) => {}
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("watcher did not disconnect after drop"),
        }
    }
}

#[test]
fn directory_scope_reconciles_children_missing_from_leaf_notifications() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    let a = root.join("a.wav");
    let b = root.join("b.wav");
    write_wav(&a, 800);
    write_wav(&b, 800);
    let now = Instant::now();
    let mut batch = PendingChanges::default();
    batch.push(
        Event::new(EventKind::Create(CreateKind::Folder)).add_path(root.clone()),
        now,
    );
    // The other child's detail may be coalesced into the directory notification.
    batch.push(
        Event::new(EventKind::Create(CreateKind::File)).add_path(a.clone()),
        now,
    );
    let Some(LibraryWatchEvent::PathsChanged(paths)) = batch.finish() else {
        panic!("expected paths");
    };
    library
        .update_paths(std::slice::from_ref(&root), &[], &paths)
        .unwrap();
    assert_eq!(library.track_count().unwrap(), 2);
    std::fs::remove_file(&b).unwrap();
    batch.push(Event::new(EventKind::Any).add_path(root.clone()), now);
    let Some(LibraryWatchEvent::PathsChanged(paths)) = batch.finish() else {
        panic!("expected directory");
    };
    library
        .update_paths(std::slice::from_ref(&root), &[], &paths)
        .unwrap();
    let rows = library.all_tracks().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, a.to_string_lossy());
}
