mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use liusheng_core::audio::PcmSpec;
use liusheng_core::audio::resampling_sink::ResamplingSink;
use liusheng_core::audio::sink::{AudioSink, WavSink};
use liusheng_core::engine::{Command, Player, PlayerEvent};
use liusheng_core::error::Result;

struct FirstWriteGateSink {
    reached: Option<Sender<()>>,
    resume: Receiver<()>,
    samples: Arc<AtomicU64>,
    discards: Arc<AtomicU64>,
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

struct CountingSink {
    samples: Arc<AtomicU64>,
}

impl AudioSink for CountingSink {
    fn write(&mut self, _spec: PcmSpec, samples: &[i32]) -> Result<()> {
        self.samples
            .fetch_add(samples.len() as u64, Ordering::Relaxed);
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
fn output_sink_can_be_replaced_without_restarting_the_track() {
    let dir = tempfile::tempdir().unwrap();
    let track = dir.path().join("track.wav");
    common::write_ramp_wav16(&track, 8000, 80_000, 0);

    let (reached_tx, reached_rx) = crossbeam_channel::bounded(1);
    let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
    let old_samples = Arc::new(AtomicU64::new(0));
    let old_discards = Arc::new(AtomicU64::new(0));
    let old_sink = FirstWriteGateSink {
        reached: Some(reached_tx),
        resume: resume_rx,
        samples: old_samples.clone(),
        discards: old_discards.clone(),
    };
    let new_samples = Arc::new(AtomicU64::new(0));
    let player = Player::new(Box::new(old_sink));
    player.send(Command::SetQueue {
        paths: vec![track],
        start: 0,
    });
    player.send(Command::Play);

    reached_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("旧输出端未收到第一块样本");
    player.send(Command::ReplaceSink(Box::new(CountingSink {
        samples: new_samples.clone(),
    })));
    resume_tx.send(()).unwrap();

    let events = wait_for(&player, Duration::from_secs(10), |event| {
        matches!(event, PlayerEvent::QueueFinished)
    });
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, PlayerEvent::TrackStarted { .. }))
            .count(),
        1
    );
    assert!(!events.iter().any(|event| matches!(
        event,
        PlayerEvent::TrackError { .. } | PlayerEvent::EngineError { .. }
    )));
    assert!(old_samples.load(Ordering::Relaxed) > 0);
    assert!(new_samples.load(Ordering::Relaxed) > 0);
    assert!(old_discards.load(Ordering::Relaxed) >= 2);
    assert_eq!(
        old_samples.load(Ordering::Relaxed) + new_samples.load(Ordering::Relaxed),
        80_000 * 2
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
