mod common;

use std::time::{Duration, Instant};

use liusheng_core::audio::sink::WavSink;
use liusheng_core::engine::{Command, Player, PlayerEvent};

/// 在超时前等到谓词命中的事件，返回收到的全部事件。
fn wait_for(
    player: &Player,
    timeout: Duration,
    pred: impl Fn(&PlayerEvent) -> bool,
) -> Vec<PlayerEvent> {
    let deadline = Instant::now() + timeout;
    let mut seen = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("等待事件超时");
        let ev = player
            .events()
            .recv_timeout(remaining)
            .expect("事件通道关闭");
        let hit = pred(&ev);
        seen.push(ev);
        if hit {
            return seen;
        }
    }
}

#[test]
fn queue_plays_gapless_and_concatenates_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("b.wav");
    let out = dir.path().join("out.wav");
    common::write_ramp_wav16(&a, 8000, 2000, 0);
    common::write_ramp_wav16(&b, 8000, 2000, 10000);

    let player = Player::new(Box::new(WavSink::create(&out)));
    player.send(Command::SetQueue {
        paths: vec![a, b],
        start: 0,
    });
    player.send(Command::Play);
    let events = wait_for(&player, Duration::from_secs(10), |e| {
        matches!(e, PlayerEvent::QueueFinished)
    });
    drop(player);

    let starts = events
        .iter()
        .filter(|e| matches!(e, PlayerEvent::TrackStarted { .. }))
        .count();
    assert_eq!(starts, 2);
    assert!(
        !events.iter().any(|e| matches!(
            e,
            PlayerEvent::TrackError { .. } | PlayerEvent::EngineError { .. }
        )),
        "不应有错误事件: {events:?}"
    );

    let mut reader = hound::WavReader::open(&out).unwrap();
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    // 两曲逐帧首尾相接，无缝也无多余样本
    assert_eq!(samples.len(), 4000 * 2);
    for (i, &s) in samples.iter().enumerate() {
        let frame = i / 2;
        let expected = if frame < 2000 {
            frame as i32
        } else {
            10000 + (frame - 2000) as i32
        };
        assert_eq!(s as i32, expected, "frame {frame}");
    }
}

#[test]
fn bad_file_is_skipped_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.wav");
    let good = dir.path().join("good.wav");
    std::fs::write(&bad, b"not audio at all").unwrap();
    common::write_ramp_wav16(&good, 8000, 100, 7);
    let out = dir.path().join("out.wav");

    let player = Player::new(Box::new(WavSink::create(&out)));
    player.send(Command::SetQueue {
        paths: vec![bad.clone(), good],
        start: 0,
    });
    player.send(Command::Play);
    let events = wait_for(&player, Duration::from_secs(10), |e| {
        matches!(e, PlayerEvent::QueueFinished)
    });
    drop(player);

    assert!(
        events
            .iter()
            .any(|e| matches!(e, PlayerEvent::TrackError { path, .. } if *path == bad))
    );
    let mut reader = hound::WavReader::open(&out).unwrap();
    assert_eq!(reader.samples::<i16>().count(), 100 * 2);
}
