//! Local identity and persisted resources for online enrichment. Audio stays read-only.
//! Network requests are owned by the desktop service; this module also works offline.
use crate::{Error, Result, library::TrackRow, lyrics::Lyrics};
use lofty::prelude::TaggedFileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn identity(parts: &[&str]) -> String {
    // Length-delimited JSON avoids collisions from delimiters in user metadata.
    digest(
        serde_json::to_string(parts)
            .expect("strings serialize")
            .as_bytes(),
    )
}
pub fn valid_key(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn known(value: &str) -> bool {
    !matches!(
        value.trim().to_lowercase().as_str(),
        "" | "unknown" | "unknown album" | "unknown artist" | "未知专辑" | "未知艺术家"
    )
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub track_key: String,
    pub album_key: String,
    pub identity: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub duration: f64,
    pub scope: String,
    #[serde(default)]
    pub local_cover: bool,
    #[serde(default)]
    pub local_lyrics: bool,
}
impl Context {
    pub fn from_track(track: &TrackRow) -> Self {
        let directory = Path::new(&track.path)
            .parent()
            .unwrap_or(Path::new(""))
            .to_string_lossy();
        let artist = if known(&track.album_artist) {
            &track.album_artist
        } else {
            &track.artist
        };
        let album_key = if known(&track.album) && known(artist) {
            identity(&["album-v1", &directory, track.album.trim(), artist.trim()])
        } else {
            String::new()
        };
        Self {
            track_key: identity(&["track-v1", &track.path]),
            album_key,
            // Avoid packet rounding and mtime changes invalidating a user's choice.
            // Replacing a file with a different named work invalidates this binding.
            identity: identity(&[
                "metadata-v1",
                &track.path,
                &track.title,
                &track.artist,
                &track.album,
                &track.album_artist,
            ]),
            path: track.path.clone(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            album_artist: track.album_artist.clone(),
            duration: track.duration_ms as f64 / 1000.0,
            scope: "track".into(),
            local_cover: false,
            local_lyrics: false,
        }
    }
    pub fn cover_key(&self) -> &str {
        if self.scope == "album" && valid_key(&self.album_key) {
            &self.album_key
        } else {
            &self.track_key
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Binding {
    pub version: u32,
    pub key: String,
    pub identity: String,
    pub kind: String,
    pub blob: String,
    pub pinned: bool,
    pub disabled: bool,
    pub instrumental: bool,
    pub provider: String,
    pub source_id: String,
    pub source_url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub saved_at: String,
    pub accent: String,
}
impl Binding {
    pub fn applies(&self, context: &Context) -> bool {
        if self.key == context.track_key {
            self.identity == context.identity
        } else {
            !context.album_key.is_empty() && self.key == context.album_key && self.kind == "cover"
        }
    }
    pub fn file(&self, root: &Path) -> Option<PathBuf> {
        let ext = match self.kind.as_str() {
            "cover" => ".jpg",
            "lyrics" => ".lrc",
            _ => return None,
        };
        let hash = self.blob.strip_suffix(ext)?;
        if !valid_key(hash) {
            return None;
        }
        let path = root.join("objects").join(&self.blob);
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        if !metadata.is_file() || metadata.len() > 8 * 1024 * 1024 {
            return None;
        }
        if digest(&std::fs::read(&path).ok()?) != hash {
            return None;
        }
        Some(path)
    }
    pub fn cover_url(&self, root: &Path) -> Option<String> {
        if self.kind != "cover" || self.disabled {
            return None;
        }
        url::Url::from_file_path(self.file(root)?)
            .ok()
            .map(Into::into)
    }
    pub fn offset_key(&self, context: &Context) -> String {
        format!("{}#online:{}", context.path, self.blob)
    }
}

/// Read-only existence probe: batch preparation shares no cache writers with artwork jobs.
pub fn has_local_cover(path: &Path) -> bool {
    if let Ok(file) = lofty::read_from_path(path)
        && file.tags().iter().any(|tag| !tag.pictures().is_empty())
    {
        return true;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };
    entries.filter_map(std::result::Result::ok).any(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        let p = Path::new(&name);
        p.file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| matches!(s, "cover" | "folder" | "front"))
            && p.extension().and_then(|s| s.to_str()).is_some_and(|s| {
                matches!(
                    s,
                    "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tif" | "tiff"
                )
            })
    })
}

pub fn root() -> Result<PathBuf> {
    let paths = crate::settings::AppPaths::discover()?;
    Ok(paths
        .database
        .parent()
        .ok_or_else(|| Error::Other("曲库路径缺少父目录".into()))?
        .join("online-assets"))
}
pub fn read_binding(root: &Path, key: &str, kind: &str) -> Option<Binding> {
    if !valid_key(key) || !matches!(kind, "cover" | "lyrics") {
        return None;
    }
    let path = root.join("bindings").join(format!("{key}.{kind}.json"));
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > 32768 {
        return None;
    }
    let binding: Binding = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (binding.version == 1 && binding.key == key && binding.kind == kind).then_some(binding)
}

/// Called off the GUI thread once; later resource changes update just one binding.
pub fn load_covers(root: &Path) -> HashMap<String, Binding> {
    let Ok(entries) = std::fs::read_dir(root.join("bindings")) else {
        return HashMap::new();
    };
    entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let key = name.strip_suffix(".cover.json")?;
            let value = read_binding(root, key, "cover")?;
            if !value.disabled && !value.pinned && value.cover_url(root).is_none() {
                return None;
            }
            Some((key.to_owned(), value))
        })
        .collect()
}

pub struct ResolvedLyrics {
    pub lyrics: Option<Lyrics>,
    pub source: String,
    pub instrumental: bool,
    pub offset_key: String,
}
pub fn load_lyrics(root: &Path, context: &Context) -> Result<ResolvedLyrics> {
    let binding = read_binding(root, &context.track_key, "lyrics")
        .filter(|b| b.applies(context) && !b.disabled);
    let online = || -> Option<ResolvedLyrics> {
        let b = binding.as_ref()?;
        if b.instrumental {
            return Some(ResolvedLyrics {
                lyrics: None,
                source: b.provider.clone(),
                instrumental: true,
                offset_key: b.offset_key(context),
            });
        }
        let mut data = String::new();
        std::fs::File::open(b.file(root)?)
            .ok()?
            .take(1024 * 1024 + 1)
            .read_to_string(&mut data)
            .ok()?;
        let lyrics = Lyrics::from_text(&data).ok()?;
        Some(ResolvedLyrics {
            lyrics: Some(lyrics),
            source: b.provider.clone(),
            instrumental: false,
            offset_key: b.offset_key(context),
        })
    };
    if binding.as_ref().is_some_and(|b| b.pinned)
        && let Some(result) = online()
    {
        return Ok(result);
    }
    let local = Lyrics::load(Path::new(&context.path));
    if let Ok(Some(lyrics)) = local {
        return Ok(ResolvedLyrics {
            lyrics: Some(lyrics),
            source: "本地歌词".into(),
            instrumental: false,
            offset_key: context.path.clone(),
        });
    }
    if let Some(result) = online() {
        return Ok(result);
    }
    local.map(|lyrics| ResolvedLyrics {
        lyrics,
        source: String::new(),
        instrumental: false,
        offset_key: context.path.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(path: &str) -> TrackRow {
        serde_json::from_value(serde_json::json!({"id":1,"path":path,"title":"曲目","artist":"歌手","album":"","album_artist":"","track_no":null,"disc_no":null,"year":null,"genre":"","duration_ms":100000,"sample_rate":44100,"bit_depth":16,"channels":2})).unwrap()
    }
    #[test]
    fn unknown_albums_and_separate_editions_have_distinct_identities() {
        let mut a = track("/music/a.wav");
        let b = track("/music/b.wav");
        assert_ne!(
            Context::from_track(&a).track_key,
            Context::from_track(&b).track_key
        );
        assert!(Context::from_track(&a).album_key.is_empty());
        a.album = "专辑".into();
        let mut edition = a.clone();
        edition.path = "/deluxe/a.wav".into();
        assert_ne!(
            Context::from_track(&a).album_key,
            Context::from_track(&edition).album_key
        );
    }
    #[test]
    fn identity_preserves_choice_on_timestamp_changes_and_rejects_replaced_song() {
        let a = track("/music/a.wav");
        let mut b = a.clone();
        b.mtime = 999;
        b.duration_ms += 2;
        assert_eq!(
            Context::from_track(&a).identity,
            Context::from_track(&b).identity
        );
        b.title = "另外一首".into();
        assert_ne!(
            Context::from_track(&a).identity,
            Context::from_track(&b).identity
        );
    }
    #[test]
    fn binding_rejects_path_escape_symlink_wrong_version_and_scope() {
        let dir = tempfile::tempdir().unwrap();
        let context = Context::from_track(&track("/music/a.wav"));
        let mut b = Binding {
            version: 1,
            key: context.track_key.clone(),
            identity: context.identity.clone(),
            kind: "cover".into(),
            blob: "../../outside.jpg".into(),
            ..Default::default()
        };
        assert!(b.file(dir.path()).is_none());
        assert!(b.applies(&context));
        assert!(!b.applies(&Context::from_track(&track("/music/b.wav"))));
        b.blob = format!("{}.jpg", digest(b"image"));
        std::fs::create_dir(dir.path().join("objects")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", dir.path().join("objects").join(&b.blob))
                .unwrap();
            assert!(b.file(dir.path()).is_none());
        }
        assert!(read_binding(dir.path(), "../other", "cover").is_none());
    }
    #[test]
    fn persisted_online_lyrics_reuses_parser_and_resource_specific_offset() {
        let dir = tempfile::tempdir().unwrap();
        let c = Context::from_track(&track("/missing.wav"));
        std::fs::create_dir(dir.path().join("objects")).unwrap();
        std::fs::create_dir(dir.path().join("bindings")).unwrap();
        let text = "[00:01.00]测试\n[00:02.00]歌词";
        let name = format!("{}.lrc", digest(text.as_bytes()));
        std::fs::write(dir.path().join("objects").join(&name), text).unwrap();
        let b = Binding {
            version: 1,
            key: c.track_key.clone(),
            identity: c.identity.clone(),
            kind: "lyrics".into(),
            blob: name.clone(),
            provider: "LRCLIB".into(),
            pinned: true,
            ..Default::default()
        };
        crate::settings::save_json(
            &dir.path()
                .join("bindings")
                .join(format!("{}.lyrics.json", c.track_key)),
            &b,
        )
        .unwrap();
        let lyrics = load_lyrics(dir.path(), &c).unwrap();
        assert!(lyrics.lyrics.unwrap().is_synchronized());
        assert!(lyrics.offset_key.ends_with(&name));
        assert_eq!(lyrics.source, "LRCLIB");
    }
    fn persist(root: &Path, b: &Binding) {
        crate::settings::save_json(
            &root
                .join("bindings")
                .join(format!("{}.{}.json", b.key, b.kind)),
            b,
        )
        .unwrap();
    }
    fn audio_fixture(root: &Path) -> Context {
        let path = root.join("song.wav");
        let mut wav = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        wav.write_sample(0_i16).unwrap();
        wav.finalize().unwrap();
        Context::from_track(&track(path.to_str().unwrap()))
    }
    #[test]
    fn local_priority_manual_pin_reset_and_corrupt_blob_keep_their_contract() {
        let dir = tempfile::tempdir().unwrap();
        let c = audio_fixture(dir.path());
        let lrc = Path::new(&c.path).with_extension("lrc");
        std::fs::write(&lrc, "[00:01.00]local fixture").unwrap();
        let online = "[00:02.00]online fixture";
        let blob = format!("{}.lrc", digest(online.as_bytes()));
        std::fs::create_dir(dir.path().join("objects")).unwrap();
        std::fs::write(dir.path().join("objects").join(&blob), online).unwrap();
        let mut binding = Binding {
            version: 1,
            key: c.track_key.clone(),
            identity: c.identity.clone(),
            kind: "lyrics".into(),
            blob: blob.clone(),
            provider: "fixture source".into(),
            ..Default::default()
        };
        persist(dir.path(), &binding);
        assert_eq!(load_lyrics(dir.path(), &c).unwrap().source, "本地歌词");
        std::fs::remove_file(&lrc).unwrap();
        assert_eq!(
            load_lyrics(dir.path(), &c).unwrap().source,
            "fixture source"
        );
        std::fs::write(&lrc, "[00:01.00]local fixture").unwrap();
        binding.pinned = true;
        persist(dir.path(), &binding);
        let chosen = load_lyrics(dir.path(), &c).unwrap();
        assert_eq!(chosen.source, "fixture source");
        assert_ne!(chosen.offset_key, c.path);
        binding.disabled = true;
        persist(dir.path(), &binding);
        assert_eq!(load_lyrics(dir.path(), &c).unwrap().offset_key, c.path);
        binding.disabled = false;
        persist(dir.path(), &binding);
        std::fs::write(dir.path().join("objects").join(&blob), "changed bytes").unwrap();
        assert!(binding.file(dir.path()).is_none());
        assert_eq!(load_lyrics(dir.path(), &c).unwrap().source, "本地歌词");
        assert!(
            read_binding(dir.path(), &c.track_key, "lyrics")
                .unwrap()
                .pinned
        );
    }
    #[test]
    fn catalog_preserves_reset_and_unavailable_manual_pins_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let c = Context::from_track(&track("/music/a.wav"));
        let mut binding = Binding {
            version: 1,
            key: c.track_key.clone(),
            identity: c.identity.clone(),
            kind: "cover".into(),
            pinned: true,
            disabled: true,
            ..Default::default()
        };
        persist(dir.path(), &binding);
        assert!(load_covers(dir.path())[&c.track_key].disabled);
        binding.disabled = false;
        binding.blob = format!("{}.jpg", digest(b"missing-image"));
        persist(dir.path(), &binding);
        assert!(load_covers(dir.path())[&c.track_key].pinned);
        binding.pinned = false;
        persist(dir.path(), &binding);
        assert!(load_covers(dir.path()).is_empty());
    }
    #[test]
    fn instrumental_is_an_explicit_resource_state_with_its_source() {
        let dir = tempfile::tempdir().unwrap();
        let c = audio_fixture(dir.path());
        let binding = Binding {
            version: 1,
            key: c.track_key.clone(),
            identity: c.identity.clone(),
            kind: "lyrics".into(),
            instrumental: true,
            provider: "LRCLIB".into(),
            ..Default::default()
        };
        persist(dir.path(), &binding);
        let resolved = load_lyrics(dir.path(), &c).unwrap();
        assert!(resolved.instrumental);
        assert!(resolved.lyrics.is_none());
        assert_eq!(resolved.source, "LRCLIB");
    }
    #[test]
    fn online_privacy_preferences_default_to_explicit_opt_in() {
        let defaults: crate::settings::AppSettings = serde_json::from_str("{}").unwrap();
        assert!(!defaults.online_covers && !defaults.online_lyrics);
        let mut enabled = defaults;
        enabled.online_lyrics = true;
        let roundtrip: crate::settings::AppSettings =
            serde_json::from_str(&serde_json::to_string(&enabled).unwrap()).unwrap();
        assert!(roundtrip.online_lyrics && !roundtrip.online_covers);
    }
}
