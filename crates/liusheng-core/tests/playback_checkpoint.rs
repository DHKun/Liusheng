mod common;
use liusheng_core::{
    audio::sink::NullSink,
    engine::{Player, PlayerCommand as Command},
    queue_order::{NavigationState, PlaybackOrder},
    settings::SavedSession,
};
use std::sync::Arc;

fn player() -> Player {
    Player::new(Box::new(NullSink::new().0))
}

#[test]
fn fifo_handoff_preserves_shuffle_pause_seek_and_duplicate_entries() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("song.wav");
    common::write_ramp_wav16(&audio, 8000, 32_000, 0);
    // Repeated paths are distinct queue entries, identified by their positions.
    let actual_seek = liusheng_core::audio::decode::AudioFileDecoder::open(&audio)
        .unwrap()
        .seek_secs(1.25)
        .unwrap();
    let paths = vec![audio; 8];
    let mut order = PlaybackOrder::default();
    order.rebuild(paths.len(), 7, true);
    let expected_next = order.next(7, 0, true).unwrap();
    let p = player();
    p.send(Command::SetPlaybackMode {
        repeat: 0,
        shuffle: true,
    });
    p.send(Command::SetQueueOrdered {
        paths: paths.clone(),
        start: 7,
        order: order.clone(),
    });
    // Next while paused opens the target decoder without writing samples.
    p.send(Command::Next);
    p.send(Command::Seek(1.25));
    let checkpoint = p.into_checkpoint().unwrap();
    assert_eq!(checkpoint.navigation.order, order);
    assert_eq!(checkpoint.navigation.current, expected_next);
    assert!(checkpoint.has_current);
    assert!(!checkpoint.playing);
    // The decoder reports the actual packet-aligned seek position. Handoff
    // preserves that position, including commands still in the FIFO.
    assert!(
        (checkpoint.position_secs - actual_seek).abs() <= 1.0 / 8000.0,
        "{checkpoint:?}"
    );
    let restored = Player::from_checkpoint(Box::new(NullSink::new().0), checkpoint.clone());
    let again = restored.into_checkpoint().unwrap();
    assert_eq!(again.navigation, checkpoint.navigation);
    assert_eq!(again.paths, paths);
    assert!(!again.playing);
    assert!((again.position_secs - checkpoint.position_secs).abs() < 0.02);
}

#[test]
fn repeat_only_mode_change_and_queue_edits_keep_shuffle_history() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("song.wav");
    common::write_ramp_wav16(&audio, 8000, 8000, 0);
    let paths = vec![audio.clone(); 6];
    let mut order = PlaybackOrder::default();
    order.rebuild(6, 5, true);
    let p = player();
    p.send(Command::SetPlaybackMode {
        repeat: 0,
        shuffle: true,
    });
    p.send(Command::SetQueueOrdered {
        paths,
        start: 5,
        order: order.clone(),
    });
    p.send(Command::SetPlaybackMode {
        repeat: 2,
        shuffle: true,
    });
    p.send(Command::AppendQueueItem(audio));
    order.insert(6, 5, true, false);
    p.send(Command::MoveQueueItem { from: 1, to: 3 });
    order.move_item(1, 3, true);
    let checkpoint = p.into_checkpoint().unwrap();
    assert_eq!(checkpoint.navigation.order, order);
    assert_eq!(checkpoint.navigation.repeat, 2);
    assert_eq!(checkpoint.paths.len(), 7);
}

#[test]
fn deleting_current_entry_follows_actual_shuffle_order() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("song.wav");
    common::write_ramp_wav16(&audio, 8000, 8000, 0);
    let mut order = PlaybackOrder::default();
    order.rebuild(8, 7, true);
    let expected = order.current_after_removal(7, 7, 0).unwrap();
    let p = player();
    p.send(Command::SetPlaybackMode {
        repeat: 0,
        shuffle: true,
    });
    p.send(Command::SetQueueOrdered {
        paths: vec![audio; 8],
        start: 7,
        order,
    });
    p.send(Command::RemoveQueueItem(7));
    let checkpoint = p.into_checkpoint().unwrap();
    assert_eq!(checkpoint.navigation.current, expected);
    assert!(checkpoint.navigation.order.is_valid_for(7));
}

#[test]
fn repeat_all_next_and_previous_are_consistent_at_handoff() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("song.wav");
    common::write_ramp_wav16(&audio, 8000, 8000, 0);
    let p = player();
    p.send(Command::SetQueue {
        paths: vec![audio; 3],
        start: 2,
    });
    p.send(Command::SetPlaybackMode {
        repeat: 2,
        shuffle: false,
    });
    p.send(Command::Next);
    let checkpoint = p.into_checkpoint().unwrap();
    assert_eq!(checkpoint.navigation.current, 0);
    let p = Player::from_checkpoint(Box::new(NullSink::new().0), checkpoint);
    p.send(Command::Prev);
    assert_eq!(p.into_checkpoint().unwrap().navigation.current, 2);
}

#[test]
fn session_roundtrip_retains_order_and_legacy_session_keeps_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let mut order = PlaybackOrder::default();
    order.rebuild(3, 2, true);
    let saved = SavedSession {
        version: 1,
        queue: Arc::new(vec!["/a".into(), "/a".into(), "/b".into()]),
        current_index: 2,
        shuffle: true,
        playback_order: Some(order.clone()),
        ..Default::default()
    };
    let path = dir.path().join("session.json");
    saved.save(&path).unwrap();
    assert_eq!(
        SavedSession::load(&path).unwrap().playback_order,
        Some(order)
    );
    std::fs::write(&path, r#"{"version":1,"queue":["/a"],"current_index":0}"#).unwrap();
    assert!(SavedSession::load(&path).unwrap().playback_order.is_none());
    let nav = NavigationState::default();
    assert!(!nav.can_next());
}
