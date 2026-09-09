//! macOS system Now Playing adapter. The UI thread owns the service; native
//! remote callbacks only enqueue commands, so they never touch the audio engine.
pub use crate::media_types::{Command, PlaybackSnapshot, PlaybackStatus};
use serde_json::{Value, json};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

/// Apple advances elapsed time from playback rate; publish boundaries immediately
/// and refresh progress periodically rather than rebuilding metadata every tick.
#[derive(Default)]
struct PublicationGate {
    previous: Option<PlaybackSnapshot>,
    anchor: Duration,
    force: bool,
}
impl PublicationGate {
    fn should_publish(&mut self, snapshot: &PlaybackSnapshot, now: Duration) -> bool {
        let changed = match &self.previous {
            None => true,
            Some(previous) => {
                let elapsed = now.saturating_sub(self.anchor);
                let advance = if previous.status == PlaybackStatus::Playing {
                    elapsed.as_micros().min(i64::MAX as u128) as i64
                } else {
                    0
                };
                let expected = previous.position_us.saturating_add(advance);
                let discontinuity = snapshot.position_us.abs_diff(expected) > 1_000_000;
                let mut metadata = previous.clone();
                metadata.position_us = snapshot.position_us;
                metadata != *snapshot
                    || discontinuity
                    || (snapshot.has_track
                        && snapshot.status == PlaybackStatus::Playing
                        && elapsed >= Duration::from_secs(5))
            }
        };
        if changed || self.force {
            self.previous = Some(snapshot.clone());
            self.anchor = now;
            self.force = false;
            true
        } else {
            false
        }
    }
}

fn payload(snapshot: &PlaybackSnapshot) -> Value {
    let duration = snapshot.duration_us.max(0);
    let position = if duration > 0 {
        snapshot.position_us.clamp(0, duration)
    } else {
        snapshot.position_us.max(0)
    };
    let mut hash = DefaultHasher::new();
    snapshot.path.hash(&mut hash);
    json!({
        "hasTrack": snapshot.has_track,
        "status": match snapshot.status { PlaybackStatus::Playing => 1, PlaybackStatus::Paused => 2, PlaybackStatus::Stopped => 0 },
        "title": snapshot.title, "artist": snapshot.artist, "album": snapshot.album,
        "id": format!("{:016x}", hash.finish()), "artUrl": snapshot.art_url,
        "duration": duration as f64 / 1_000_000.0, "position": position as f64 / 1_000_000.0,
        "queueIndex": snapshot.queue_index, "queueLength": snapshot.queue_len,
        "canSeek": snapshot.has_track && snapshot.seekable && duration > 0,
        "canNext": snapshot.has_track && (snapshot.queue_index.saturating_add(1) < snapshot.queue_len || snapshot.repeat_mode == 2 || (snapshot.shuffle && snapshot.queue_len > 1)),
        "repeat": snapshot.repeat_mode.min(2), "shuffle": snapshot.shuffle,
    })
}

// Native integer codes are declared in macos_media.h. Floating values are always
// checked before crossing into the integer-microsecond controller interface.
fn decode_command(code: i32, value: f64) -> Option<Command> {
    match code {
        1 => Some(Command::Play),
        2 => Some(Command::Pause),
        3 => Some(Command::PlayPause),
        4 => Some(Command::Next),
        5 => Some(Command::Previous),
        6 => Some(Command::Stop),
        7 if value.is_finite() && (0.0..=i64::MAX as f64 / 1_000_000.0).contains(&value) => {
            Some(Command::SeekAbsolute((value * 1_000_000.0).round() as i64))
        }
        8 if value.is_finite() && value.abs() <= i64::MAX as f64 / 1_000_000.0 => {
            Some(Command::SeekRelative((value * 1_000_000.0).round() as i64))
        }
        9 if value == 0.0 || value == 1.0 || value == 2.0 => {
            Some(Command::SetRepeatMode(value as u8))
        }
        10 if value == 0.0 || value == 1.0 => Some(Command::SetShuffle(value != 0.0)),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::cell::RefCell;
    use std::sync::{Mutex, mpsc};
    use std::time::Instant;

    static COMMANDS: Mutex<Option<mpsc::Sender<Command>>> = Mutex::new(None);
    unsafe extern "C" {
        fn liusheng_media_start(callback: extern "C" fn(i32, f64) -> bool) -> bool;
        fn liusheng_media_publish(bytes: *const u8, length: usize) -> bool;
        fn liusheng_media_stop();
    }
    extern "C" fn receive(code: i32, value: f64) -> bool {
        let Some(command) = decode_command(code, value) else {
            return false;
        };
        COMMANDS
            .lock()
            .ok()
            .and_then(|sender| sender.as_ref().map(|s| s.send(command).is_ok()))
            .unwrap_or(false)
    }
    pub struct Service {
        gate: RefCell<PublicationGate>,
        started: Instant,
    }
    impl Service {
        pub fn start() -> Result<(Self, mpsc::Receiver<Command>), String> {
            let (sender, receiver) = mpsc::channel();
            {
                let mut slot = COMMANDS.lock().map_err(|e| e.to_string())?;
                if slot.is_some() {
                    return Err("Now Playing service already registered".into());
                }
                *slot = Some(sender);
            }
            // SAFETY: callback is a process-lifetime function and only sends owned
            // values through a mutex-protected channel. Native registration/removal
            // is serialized on AppKit's main thread.
            if !unsafe { liusheng_media_start(receive) } {
                *COMMANDS.lock().map_err(|e| e.to_string())? = None;
                return Err("MediaPlayer command registration failed".into());
            }
            Ok((
                Self {
                    gate: RefCell::new(PublicationGate::default()),
                    started: Instant::now(),
                },
                receiver,
            ))
        }
        pub fn publish(&self, snapshot: PlaybackSnapshot) -> Result<(), String> {
            if !self
                .gate
                .borrow_mut()
                .should_publish(&snapshot, self.started.elapsed())
            {
                return Ok(());
            }
            let json = serde_json::to_vec(&payload(&snapshot)).map_err(|e| e.to_string())?;
            // SAFETY: native code copies the JSON bytes before returning. The
            // slice remains valid for this synchronous, main-thread marshalled call.
            if unsafe { liusheng_media_publish(json.as_ptr(), json.len()) } {
                Ok(())
            } else {
                self.gate.borrow_mut().force = true;
                Err("Now Playing update rejected".into())
            }
        }
        #[allow(clippy::unnecessary_wraps)]
        pub fn seeked(&self, _position_us: i64) -> Result<(), String> {
            self.gate.borrow_mut().force = true;
            Ok(())
        }
    }
    impl Drop for Service {
        fn drop(&mut self) {
            // Remove the sender first; a racing callback then reports command
            // failure rather than accepting a request after shutdown.
            if let Ok(mut sender) = COMMANDS.lock() {
                *sender = None;
            }
            // SAFETY: removes this adapter's targets and clears native metadata.
            unsafe {
                liusheng_media_stop();
            }
        }
    }
}
#[cfg(target_os = "macos")]
pub use native::Service;

#[cfg(test)]
mod tests {
    use super::*;
    fn track() -> PlaybackSnapshot {
        PlaybackSnapshot {
            has_track: true,
            status: PlaybackStatus::Playing,
            title: "原文 · title".into(),
            path: "/private/music/file.flac".into(),
            duration_us: 100_000_000,
            seekable: true,
            queue_len: 2,
            ..Default::default()
        }
    }
    #[test]
    fn accepts_transport_and_rejects_invalid_commands() {
        assert_eq!(decode_command(1, 0.0), Some(Command::Play));
        assert_eq!(decode_command(2, 0.0), Some(Command::Pause));
        assert_eq!(decode_command(3, 0.0), Some(Command::PlayPause));
        assert_eq!(decode_command(4, 0.0), Some(Command::Next));
        assert_eq!(decode_command(5, 0.0), Some(Command::Previous));
        assert_eq!(decode_command(6, 0.0), Some(Command::Stop));
        assert_eq!(decode_command(0, 0.0), None);
    }
    #[test]
    fn seeks_use_checked_microseconds() {
        assert_eq!(
            decode_command(7, 1.234567),
            Some(Command::SeekAbsolute(1_234_567))
        );
        assert_eq!(
            decode_command(8, -15.0),
            Some(Command::SeekRelative(-15_000_000))
        );
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 1e30] {
            assert!(decode_command(7, value).is_none());
        }
        assert_eq!(decode_command(9, 2.0), Some(Command::SetRepeatMode(2)));
        assert_eq!(decode_command(10, 1.0), Some(Command::SetShuffle(true)));
        assert!(decode_command(9, 1.5).is_none());
        assert!(decode_command(10, 2.0).is_none());
    }
    #[test]
    fn payload_clamps_time_and_keeps_paths_out_of_identifiers() {
        let mut snapshot = track();
        snapshot.position_us = 500_000_000;
        let data = payload(&snapshot);
        assert_eq!(data["position"], 100.0);
        assert_eq!(data["duration"], 100.0);
        assert_eq!(data["title"], "原文 · title");
        assert!(!data["id"].as_str().unwrap().contains("/private/"));
        snapshot.has_track = false;
        assert_eq!(payload(&snapshot)["canSeek"], false);
    }
    #[test]
    fn queue_end_respects_repeat_and_shuffle() {
        let mut s = track();
        s.queue_index = 1;
        assert_eq!(payload(&s)["canNext"], false);
        s.repeat_mode = 2;
        assert_eq!(payload(&s)["canNext"], true);
        s.repeat_mode = 0;
        s.shuffle = true;
        assert_eq!(payload(&s)["canNext"], true);
    }
    #[test]
    fn progress_is_throttled_but_pause_seek_and_metadata_are_immediate() {
        let mut gate = PublicationGate::default();
        let mut s = track();
        assert!(gate.should_publish(&s, Duration::ZERO));
        s.position_us = 500_000;
        assert!(!gate.should_publish(&s, Duration::from_millis(500)));
        s.status = PlaybackStatus::Paused;
        assert!(gate.should_publish(&s, Duration::from_millis(600)));
        assert!(!gate.should_publish(&s, Duration::from_secs(1)));
        s.position_us = 10_000_000;
        assert!(gate.should_publish(&s, Duration::from_secs(2)));
        gate.force = true;
        assert!(gate.should_publish(&s, Duration::from_secs(2)));
        s.art_url = "file:///cache/cover.png".into();
        assert!(gate.should_publish(&s, Duration::from_secs(3)));
        assert!(!gate.should_publish(&s, Duration::from_secs(8)));
        s.status = PlaybackStatus::Playing;
        assert!(gate.should_publish(&s, Duration::from_secs(8)));
        s.position_us = 15_000_000;
        assert!(gate.should_publish(&s, Duration::from_secs(13)));
        s.has_track = false;
        assert!(gate.should_publish(&s, Duration::from_secs(13)));
    }
}
