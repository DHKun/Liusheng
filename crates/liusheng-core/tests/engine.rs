mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use liusheng_core::audio::PcmSpec;
use liusheng_core::audio::resampling_sink::ResamplingSink;
use liusheng_core::audio::sink::{AudioSink, WavSink};
use liusheng_core::engine::{Player, PlayerCommand as Command, PlayerEvent};
use liusheng_core::error::Result;

struct FirstWriteGateSink {
    reached: Option<Sender<()>>,
    resume: Receiver<()>,
    samples: Arc<AtomicU64>,
    discards: Arc<AtomicU64>,
}

struct ShutdownProbeSink {
    discards: Arc<AtomicU64>,
    flushes: Arc<AtomicU64>,
}

impl AudioSink for ShutdownProbeSink {
    fn write(&mut self, _spec: PcmSpec, _samples: &[i32]) -> Result<()> {
        Ok(())
    }

    fn discard(&mut self) -> Result<()> {
        self.discards.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.flushes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

impl AudioSink for FirstWriteGateSink {
    fn write(&mut self, _spec: PcmSpec, samples: &[i32]) -> Result<()> {
        if let Some(reached) = self.reached.take() {
            let _ = reached.send(());
            let _ = self.resume.recv();
        }
        self.samples
            .fetch_add(samples.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    fn discard(&mut self) -> Result<()> {
        self.discards.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

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
fn dropping_player_discards_output_without_draining() {
    let discards = Arc::new(AtomicU64::new(0));
    let flushes = Arc::new(AtomicU64::new(0));
    let player = Player::new(Box::new(ShutdownProbeSink {
        discards: discards.clone(),
        flushes: flushes.clone(),
    }));

    drop(player);

    assert_eq!(discards.load(Ordering::Relaxed), 1);
    assert_eq!(flushes.load(Ordering::Relaxed), 0);
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

#[test]
fn next_track_is_preloaded_before_current_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("b.wav");
    common::write_ramp_wav16(&a, 8000, 2000, 0);
    common::write_ramp_wav16(&b, 8000, 2000, 10000);

    let (reached_tx, reached_rx) = crossbeam_channel::bounded(1);
    let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
    let samples = Arc::new(AtomicU64::new(0));
    let sink = FirstWriteGateSink {
        reached: Some(reached_tx),
        resume: resume_rx,
        samples: samples.clone(),
        discards: Arc::new(AtomicU64::new(0)),
    };
    let player = Player::new(Box::new(sink));
    player.send(Command::SetQueue {
        paths: vec![a, b.clone()],
        start: 0,
    });
    player.send(Command::Play);

    reached_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("播放引擎未写入第一块样本");
    // 引擎此时仍阻塞在第一曲的首次写入；删除路径后第二曲仍应由已打开的解码器播放。
    std::fs::remove_file(b).unwrap();
    resume_tx.send(()).unwrap();

    let events = wait_for(&player, Duration::from_secs(10), |e| {
        matches!(e, PlayerEvent::QueueFinished)
    });
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, PlayerEvent::TrackStarted { .. }))
            .count(),
        2
    );
    assert!(!events.iter().any(|e| matches!(
        e,
        PlayerEvent::TrackError { .. } | PlayerEvent::EngineError { .. }
    )));
    assert_eq!(samples.load(Ordering::Relaxed), 4000 * 2);
}

#[test]
fn seek_reports_the_actual_playback_position() {
    let dir = tempfile::tempdir().unwrap();
    let track = dir.path().join("track.wav");
    common::write_ramp_wav16(&track, 8000, 24_000, 0);

    let (reached_tx, reached_rx) = crossbeam_channel::bounded(1);
    let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
    let sink = FirstWriteGateSink {
        reached: Some(reached_tx),
        resume: resume_rx,
        samples: Arc::new(AtomicU64::new(0)),
        discards: Arc::new(AtomicU64::new(0)),
    };
    let player = Player::new(Box::new(sink));
    player.send(Command::SetQueue {
        paths: vec![track],
        start: 0,
    });
    player.send(Command::Play);

    wait_for(&player, Duration::from_secs(5), |event| {
        matches!(event, PlayerEvent::TrackStarted { .. })
    });
    reached_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("播放引擎未写入第一块样本");
    player.send(Command::Seek(1.25));
    resume_tx.send(()).unwrap();

    let events = wait_for(
        &player,
        Duration::from_secs(5),
        |event| matches!(event, PlayerEvent::Progress { secs } if *secs >= 1.0),
    );
    let actual = events
        .iter()
        .find_map(|event| match event {
            PlayerEvent::Progress { secs } if *secs >= 1.0 => Some(*secs),
            _ => None,
        })
        .unwrap();
    assert!(
        (actual - 1.25).abs() <= 0.1,
        "实际定位到 {actual} 秒，超出一个解码包"
    );

    player.send(Command::Stop);
    wait_for(&player, Duration::from_secs(5), |event| {
        matches!(event, PlayerEvent::Stopped)
    });
}

#[test]
fn removing_a_queued_track_keeps_the_current_track_and_updates_the_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.wav");
    let removed = dir.path().join("removed.wav");
    let last = dir.path().join("last.wav");
    common::write_ramp_wav16(&first, 8000, 2000, 0);
    common::write_ramp_wav16(&removed, 8000, 2000, 5000);
    common::write_ramp_wav16(&last, 8000, 2000, 10000);

    let (reached_tx, reached_rx) = crossbeam_channel::bounded(1);
    let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
    let samples = Arc::new(AtomicU64::new(0));
    let player = Player::new(Box::new(FirstWriteGateSink {
        reached: Some(reached_tx),
        resume: resume_rx,
        samples: samples.clone(),
        discards: Arc::new(AtomicU64::new(0)),
    }));
    player.send(Command::SetQueue {
        paths: vec![first.clone(), removed, last.clone()],
        start: 0,
    });
    player.send(Command::Play);
    wait_for(
        &player,
        Duration::from_secs(5),
        |event| matches!(event, PlayerEvent::TrackStarted { path, .. } if *path == first),
    );
    reached_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("播放引擎未写入第一块样本");

    player.send(Command::RemoveQueueItem(1));
    resume_tx.send(()).unwrap();

    let events = wait_for(&player, Duration::from_secs(10), |event| {
        matches!(event, PlayerEvent::QueueFinished)
    });
    let starts = events
        .iter()
        .filter_map(|event| match event {
            PlayerEvent::TrackStarted { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(starts, vec![last]);
    assert_eq!(samples.load(Ordering::Relaxed), 4000 * 2);
}

#[test]
fn removing_the_current_track_starts_the_next_track() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.wav");
    let next = dir.path().join("next.wav");
    common::write_ramp_wav16(&first, 8000, 8000, 0);
    common::write_ramp_wav16(&next, 8000, 8000, 10000);

    let (reached_tx, reached_rx) = crossbeam_channel::bounded(1);
    let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
    let player = Player::new(Box::new(FirstWriteGateSink {
        reached: Some(reached_tx),
        resume: resume_rx,
        samples: Arc::new(AtomicU64::new(0)),
        discards: Arc::new(AtomicU64::new(0)),
    }));
    player.send(Command::SetQueue {
        paths: vec![first.clone(), next.clone()],
        start: 0,
    });
    player.send(Command::Play);
    wait_for(
        &player,
        Duration::from_secs(5),
        |event| matches!(event, PlayerEvent::TrackStarted { path, .. } if *path == first),
    );
    reached_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("播放引擎未写入第一块样本");

    player.send(Command::RemoveQueueItem(0));
    resume_tx.send(()).unwrap();

    wait_for(
        &player,
        Duration::from_secs(5),
        |event| matches!(event, PlayerEvent::TrackStarted { path, index, .. } if *path == next && *index == 0),
    );
    player.send(Command::Stop);
    wait_for(&player, Duration::from_secs(5), |event| {
        matches!(event, PlayerEvent::Stopped)
    });
}

#[test]
fn clearing_the_queue_stops_and_forgets_the_previous_paths() {
    let dir = tempfile::tempdir().unwrap();
    let track = dir.path().join("track.wav");
    common::write_ramp_wav16(&track, 8000, 8000, 0);

    let (reached_tx, reached_rx) = crossbeam_channel::bounded(1);
    let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
    let player = Player::new(Box::new(FirstWriteGateSink {
        reached: Some(reached_tx),
        resume: resume_rx,
        samples: Arc::new(AtomicU64::new(0)),
        discards: Arc::new(AtomicU64::new(0)),
    }));
    player.send(Command::SetQueue {
        paths: vec![track.clone()],
        start: 0,
    });
    player.send(Command::Play);
    wait_for(
        &player,
        Duration::from_secs(5),
        |event| matches!(event, PlayerEvent::TrackStarted { path, .. } if *path == track),
    );
    reached_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("播放引擎未写入第一块样本");

    player.send(Command::ClearQueue);
    resume_tx.send(()).unwrap();
    wait_for(&player, Duration::from_secs(5), |event| {
        matches!(event, PlayerEvent::Stopped)
    });

    player.send(Command::Play);
    let events = wait_for(&player, Duration::from_secs(5), |event| {
        matches!(event, PlayerEvent::QueueFinished)
    });
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, PlayerEvent::TrackStarted { .. }))
    );
}

#[test]
fn player_streams_cd_audio_through_the_exclusive_resampler() {
    let dir = tempfile::tempdir().unwrap();
    let track = dir.path().join("cd.wav");
    let out = dir.path().join("resampled.wav");
    let input_frames = 4_410usize;
    common::write_ramp_wav16(&track, 44_100, input_frames, 0);

    let sink = ResamplingSink::new(Box::new(WavSink::create(&out)));
    let player = Player::new(Box::new(sink));
    player.send(Command::SetQueue {
        paths: vec![track],
        start: 0,
    });
    player.send(Command::Play);
    let events = wait_for(&player, Duration::from_secs(10), |event| {
        matches!(event, PlayerEvent::QueueFinished)
    });
    drop(player);

    assert!(events.iter().any(|event| matches!(
        event,
        PlayerEvent::TrackStarted {
            spec: PcmSpec { rate: 44_100, .. },
            ..
        }
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        PlayerEvent::TrackError { .. } | PlayerEvent::EngineError { .. }
    )));

    let mut reader = hound::WavReader::open(out).unwrap();
    assert_eq!(reader.spec().sample_rate, 96_000);
    assert_eq!(reader.spec().bits_per_sample, 24);
    let expected_frames = (input_frames as u64 * 96_000).div_ceil(44_100) as usize;
    assert_eq!(reader.samples::<i32>().count(), expected_frames * 2);
}
