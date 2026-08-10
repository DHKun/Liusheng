use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, mpsc};

use zbus::blocking::{Connection, connection};
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const BUS_NAME: &str = "org.mpris.MediaPlayer2.io.github.dhkun.Liusheng";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const NO_TRACK_PATH: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    #[default]
    Stopped,
}

impl PlaybackStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::Stopped => "Stopped",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaybackSnapshot {
    pub status: PlaybackStatus,
    pub has_track: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub path: String,
    pub duration_us: i64,
    pub position_us: i64,
    pub track_number: Option<u32>,
    pub queue_index: usize,
    pub queue_len: usize,
    pub seekable: bool,
}

impl PlaybackSnapshot {
    fn track_id(&self) -> OwnedObjectPath {
        if !self.has_track {
            return OwnedObjectPath::try_from(NO_TRACK_PATH).expect("固定 MPRIS 路径有效");
        }
        let mut hasher = DefaultHasher::new();
        self.path.hash(&mut hasher);
        OwnedObjectPath::try_from(format!(
            "/io/github/dhkun/Liusheng/track/{:016x}",
            hasher.finish()
        ))
        .expect("哈希生成的 MPRIS 路径有效")
    }

    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let mut metadata = HashMap::new();
        metadata.insert(
            "mpris:trackid".into(),
            Value::from(self.track_id())
                .try_into()
                .expect("对象路径可转换为 D-Bus 值"),
        );
        if !self.has_track {
            return metadata;
        }
        metadata.insert(
            "xesam:title".into(),
            Value::from(self.title.as_str())
                .try_into()
                .expect("字符串可转换为 D-Bus 值"),
        );
        metadata.insert(
            "xesam:artist".into(),
            Value::from(vec![self.artist.as_str()])
                .try_into()
                .expect("艺术家列表可转换为 D-Bus 值"),
        );
        if !self.album.is_empty() {
            metadata.insert(
                "xesam:album".into(),
                Value::from(self.album.as_str())
                    .try_into()
                    .expect("字符串可转换为 D-Bus 值"),
            );
        }
        if self.duration_us > 0 {
            metadata.insert("mpris:length".into(), self.duration_us.into());
        }
        if let Some(track_number) = self.track_number {
            metadata.insert("xesam:trackNumber".into(), track_number.into());
        }
        metadata
    }

    fn can_go_next(&self) -> bool {
        self.has_track && self.queue_index + 1 < self.queue_len
    }

    fn can_go_previous(&self) -> bool {
        self.has_track
    }
}

#[derive(Debug)]
pub enum Command {
    Next,
    Previous,
    Pause,
    PlayPause,
    Stop,
    Play,
    SeekRelative(i64),
    SeekAbsolute(i64),
}

pub struct Service {
    connection: Connection,
    state: Arc<Mutex<PlaybackSnapshot>>,
}

impl Service {
    pub fn start() -> zbus::Result<(Self, mpsc::Receiver<Command>)> {
        let state = Arc::new(Mutex::new(PlaybackSnapshot::default()));
        let (commands, receiver) = mpsc::channel();
        let connection = connection::Builder::session()?
            .serve_at(OBJECT_PATH, RootInterface)?
            .serve_at(
                OBJECT_PATH,
                PlayerInterface {
                    state: state.clone(),
                    commands,
                },
            )?
            .name(BUS_NAME)?
            .build()?;
        Ok((Self { connection, state }, receiver))
    }

    pub fn publish(&self, snapshot: PlaybackSnapshot) -> zbus::Result<()> {
        let previous = {
            let mut state = self.state.lock().expect("MPRIS 状态锁未损坏");
            std::mem::replace(&mut *state, snapshot.clone())
        };
        if previous == snapshot {
            return Ok(());
        }

        let status_changed = previous.status != snapshot.status;
        let metadata_changed = metadata_changed(&previous, &snapshot);
        let can_go_next_changed = previous.can_go_next() != snapshot.can_go_next();
        let can_go_previous_changed = previous.can_go_previous() != snapshot.can_go_previous();
        let track_availability_changed = previous.has_track != snapshot.has_track;
        let can_seek_changed = previous.seekable != snapshot.seekable;
        if !(status_changed
            || metadata_changed
            || can_go_next_changed
            || can_go_previous_changed
            || track_availability_changed
            || can_seek_changed)
        {
            return Ok(());
        }

        let interface_ref = self
            .connection
            .object_server()
            .interface::<_, PlayerInterface>(OBJECT_PATH)?;
        let interface = interface_ref.get();
        let emitter = interface_ref.signal_emitter();
        if status_changed {
            zbus::block_on(interface.playback_status_changed(emitter))?;
        }
        if metadata_changed {
            zbus::block_on(interface.metadata_changed(emitter))?;
        }
        if can_go_next_changed {
            zbus::block_on(interface.can_go_next_changed(emitter))?;
        }
        if can_go_previous_changed {
            zbus::block_on(interface.can_go_previous_changed(emitter))?;
        }
        if track_availability_changed {
            zbus::block_on(interface.can_play_changed(emitter))?;
            zbus::block_on(interface.can_pause_changed(emitter))?;
        }
        if can_seek_changed {
            zbus::block_on(interface.can_seek_changed(emitter))?;
        }
        Ok(())
    }

    pub fn seeked(&self, position_us: i64) -> zbus::Result<()> {
        let interface_ref = self
            .connection
            .object_server()
            .interface::<_, PlayerInterface>(OBJECT_PATH)?;
        zbus::block_on(PlayerInterface::seeked(
            interface_ref.signal_emitter(),
            position_us.max(0),
        ))
    }
}

fn metadata_changed(previous: &PlaybackSnapshot, current: &PlaybackSnapshot) -> bool {
    previous.has_track != current.has_track
        || previous.title != current.title
        || previous.artist != current.artist
        || previous.album != current.album
        || previous.path != current.path
        || previous.duration_us != current.duration_us
        || previous.track_number != current.track_number
}

struct RootInterface;

#[interface(name = "org.mpris.MediaPlayer2")]
impl RootInterface {
    fn raise(&self) {}

    fn quit(&self) {}

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn fullscreen(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_set_fullscreen(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn identity(&self) -> String {
        "留声".into()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn desktop_entry(&self) -> String {
        "io.github.dhkun.Liusheng".into()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_mime_types(&self) -> Vec<String> {
        [
            "audio/flac",
            "audio/mpeg",
            "audio/mp4",
            "audio/ogg",
            "audio/wav",
            "audio/x-aiff",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
}

struct PlayerInterface {
    state: Arc<Mutex<PlaybackSnapshot>>,
    commands: mpsc::Sender<Command>,
}

impl PlayerInterface {
    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    fn snapshot(&self) -> PlaybackSnapshot {
        self.state.lock().expect("MPRIS 状态锁未损坏").clone()
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerInterface {
    fn next(&self) {
        self.send(Command::Next);
    }

    fn previous(&self) {
        self.send(Command::Previous);
    }

    fn pause(&self) {
        self.send(Command::Pause);
    }

    fn play_pause(&self) {
        self.send(Command::PlayPause);
    }

    fn stop(&self) {
        self.send(Command::Stop);
    }

    fn play(&self) {
        self.send(Command::Play);
    }

    fn seek(&self, offset: i64) {
        self.send(Command::SeekRelative(offset));
    }

    fn set_position(&self, track_id: OwnedObjectPath, position: i64) {
        if track_id == self.snapshot().track_id() {
            self.send(Command::SeekAbsolute(position));
        }
    }

    fn open_uri(&self, _uri: &str) {}

    #[zbus(signal)]
    async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.snapshot().status.as_str().into()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn loop_status(&self) -> String {
        "None".into()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn shuffle(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        self.snapshot().metadata()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn volume(&self) -> f64 {
        1.0
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        self.snapshot().position_us.max(0)
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.snapshot().can_go_next()
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.snapshot().can_go_previous()
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.snapshot().has_track
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.snapshot().has_track
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.snapshot().seekable
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_control(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_contains_mpris_and_xesam_fields() {
        let snapshot = PlaybackSnapshot {
            status: PlaybackStatus::Playing,
            has_track: true,
            title: "起风了".into(),
            artist: "林俊杰".into(),
            album: "未知专辑".into(),
            path: "/data/Music/track.flac".into(),
            duration_us: 315_000_000,
            position_us: 12_000_000,
            track_number: Some(1),
            queue_index: 0,
            queue_len: 2,
            seekable: true,
        };
        let mut metadata = snapshot.metadata();
        assert_eq!(
            String::try_from(metadata.remove("xesam:title").unwrap()).unwrap(),
            "起风了"
        );
        assert_eq!(
            Vec::<String>::try_from(metadata.remove("xesam:artist").unwrap()).unwrap(),
            vec!["林俊杰"]
        );
        assert_eq!(
            i64::try_from(metadata.remove("mpris:length").unwrap()).unwrap(),
            315_000_000
        );
        assert!(snapshot.can_go_next());
        assert!(snapshot.can_go_previous());
    }

    #[test]
    fn empty_snapshot_uses_the_standard_no_track_id() {
        let snapshot = PlaybackSnapshot::default();
        assert_eq!(snapshot.track_id().as_str(), NO_TRACK_PATH);
        assert!(!snapshot.can_go_next());
        assert!(!snapshot.can_go_previous());
    }

    #[test]
    fn set_position_ignores_a_stale_track_id() {
        let snapshot = PlaybackSnapshot {
            has_track: true,
            path: "/data/Music/current.flac".into(),
            ..PlaybackSnapshot::default()
        };
        let current_track_id = snapshot.track_id();
        let (commands, receiver) = mpsc::channel();
        let player = PlayerInterface {
            state: Arc::new(Mutex::new(snapshot)),
            commands,
        };

        player.set_position(
            OwnedObjectPath::try_from("/io/github/dhkun/Liusheng/track/stale").unwrap(),
            42_000_000,
        );
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));

        player.set_position(current_track_id, 42_000_000);
        assert!(matches!(
            receiver.try_recv(),
            Ok(Command::SeekAbsolute(42_000_000))
        ));
    }
}
