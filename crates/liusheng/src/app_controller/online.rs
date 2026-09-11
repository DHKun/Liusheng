//! Integrates offline online-resource bindings without changing the audio pipeline.
use super::*;
use liusheng_core::online_assets::{self, Binding, Context};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Cover {
    pub binding: Binding,
    pub url: String,
}

fn resolved(binding: Binding, root: &Path) -> Cover {
    let url = binding.cover_url(root).unwrap_or_default();
    Cover { binding, url }
}

impl qobject::AppController {
    pub fn prepare_lyric_import(
        self: core::pin::Pin<&mut Self>,
        request_id: &QString,
        file_url: &QString,
    ) {
        let request_id = request_id.to_string();
        let file_url = file_url.to_string();
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let parsed = (|| -> liusheng_core::Result<(Lyrics, String)> {
                let path = url::Url::parse(&file_url)
                    .ok()
                    .and_then(|u| u.to_file_path().ok())
                    .ok_or_else(|| liusheng_core::Error::Other("请选择本地歌词文件".into()))?;
                if !path.is_file() {
                    return Err(liusheng_core::Error::Other("歌词路径须为普通文件".into()));
                }
                Lyrics::read_file(&path)
            })();
            let (text, word_timed, error) = match parsed {
                Ok((lyrics, text)) => (text, lyrics.has_word_timing(), String::new()),
                Err(error) => (String::new(), false, error.to_string()),
            };
            let _ = qt.queue(move |controller| {
                controller.lyric_import_ready(
                    QString::from(&request_id),
                    QString::from(&text),
                    word_timed,
                    QString::from(&error),
                );
            });
        });
    }

    pub fn online_root(&self) -> QString {
        online_assets::root()
            .map(|p| QString::from(p.to_string_lossy().as_ref()))
            .unwrap_or_default()
    }
    fn online_track(&self, mode: &str, index: i32) -> Option<&TrackRow> {
        match mode {
            "current" => self
                .rust()
                .current_queue_index
                .and_then(|i| self.rust().playback_queue.get(i))
                .map(Arc::as_ref),
            "queue" => self.queue_track_at(index),
            "selected" | "album" => self.selected_track_at(index),
            _ => self.all_track_at(index),
        }
    }
    pub fn request_online_details(
        mut self: core::pin::Pin<&mut Self>,
        mode: &QString,
        index: i32,
        kind: &QString,
    ) {
        let mode = mode.to_string();
        let Some(track) = self.online_track(&mode, index) else {
            return;
        };
        let mut context = Context::from_track(track);
        if mode == "album" && !context.album_key.is_empty() {
            context.scope = "album".into();
        }
        if let Ok(json) = serde_json::to_string(&context) {
            self.as_mut()
                .online_details_requested(QString::from(&json), kind.clone());
        }
    }
    pub fn load_online_covers(self: core::pin::Pin<&mut Self>) {
        let Ok(root) = online_assets::root() else {
            return;
        };
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let covers = online_assets::load_covers(&root)
                .into_iter()
                .map(|(key, value)| (key, resolved(value, &root)))
                .collect::<HashMap<_, _>>();
            if covers.is_empty() {
                return;
            }
            let _ = qt.queue(move |mut controller| {
                for (key, cover) in covers {
                    controller
                        .as_mut()
                        .rust_mut()
                        .get_mut()
                        .online_covers
                        .entry(key)
                        .or_insert(cover);
                }
                controller.as_mut().refresh_artwork_arrays();
            });
        });
    }
    pub fn online_assets_changed(
        mut self: core::pin::Pin<&mut Self>,
        key: &QString,
        kind: &QString,
    ) {
        let key = key.to_string();
        let kind = kind.to_string();
        if !online_assets::valid_key(&key) {
            return;
        }
        if kind == "lyrics" {
            if let Some(track) = self.online_track("current", 0)
                && Context::from_track(track).track_key == key
            {
                let path = PathBuf::from(&track.path);
                self.as_mut().rust_mut().get_mut().lyrics_request_path = None;
                self.as_mut().request_lyrics_for_path(path);
            }
            return;
        }
        if kind != "cover" {
            return;
        }
        let Ok(root) = online_assets::root() else {
            return;
        };
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let value = online_assets::read_binding(&root, &key, "cover")
                .map(|binding| resolved(binding, &root));
            let _ = qt.queue(move |mut controller| {
                if let Some(value) = value {
                    controller
                        .as_mut()
                        .rust_mut()
                        .get_mut()
                        .online_covers
                        .insert(key.clone(), value);
                } else {
                    controller
                        .as_mut()
                        .rust_mut()
                        .get_mut()
                        .online_covers
                        .remove(&key);
                }
                // Online writes are rare and update model cover roles only; no metadata scan.
                controller.as_mut().refresh_artwork_arrays();
            });
        });
    }
    pub fn online_cover_for_track(&self, track: &TrackRow) -> Option<&Cover> {
        if self.rust().online_covers.is_empty() {
            return None;
        }
        let context = Context::from_track(track);
        if self
            .rust()
            .online_covers
            .get(&context.track_key)
            .is_some_and(|c| {
                c.binding.applies(&context)
                    && (c.binding.disabled || (c.binding.pinned && c.url.is_empty()))
            })
        {
            return None;
        }
        let track_cover = self
            .rust()
            .online_covers
            .get(&context.track_key)
            .filter(|c| c.binding.applies(&context) && !c.url.is_empty());
        let album_cover = self
            .rust()
            .online_covers
            .get(&context.album_key)
            .filter(|c| c.binding.applies(&context) && !c.url.is_empty());
        track_cover
            .filter(|c| c.binding.pinned)
            .or_else(|| album_cover.filter(|c| c.binding.pinned))
            .or_else(|| {
                let local = self.local_cover_for_track(track);
                if local.is_none() {
                    track_cover.or(album_cover)
                } else {
                    None
                }
            })
    }
    pub fn local_cover_for_track(&self, track: &TrackRow) -> Option<&str> {
        let key = AlbumKey {
            album: track.album.clone(),
            album_artist: track.album_artist.clone(),
        };
        self.rust()
            .artwork_cache
            .get(&album_cache_key(&key))
            .and_then(|c| {
                c.variants
                    .get(&768)
                    .filter(|s| !s.is_empty())
                    .or_else(|| c.variants.get(&256))
            })
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }
    pub fn grid_cover(&self, album: &AlbumSummary) -> String {
        let local = self
            .rust()
            .artwork_cache
            .get(&album_cache_key(&album.key))
            .and_then(|c| c.variants.get(&256))
            .cloned()
            .unwrap_or_default();
        if self.rust().online_covers.is_empty() {
            return local;
        }
        let Some(indices) = self.rust().album_tracks.get(&album.key) else {
            return local;
        };
        let Some(first) = indices.first().and_then(|i| self.rust().tracks.get(*i)) else {
            return local;
        };
        let context = Context::from_track(first);
        if context.album_key.is_empty() {
            return local;
        }
        let Some(cover) = self
            .rust()
            .online_covers
            .get(&context.album_key)
            .filter(|c| !c.url.is_empty())
        else {
            return local;
        };
        if !cover.binding.pinned && !local.is_empty() {
            return local;
        }
        // A collapsed view containing multiple physical editions keeps its local art.
        if indices
            .iter()
            .any(|i| Context::from_track(&self.rust().tracks[*i]).album_key != context.album_key)
        {
            return local;
        }
        cover.url.clone()
    }
    pub fn prepare_online_automatic(mut self: core::pin::Pin<&mut Self>) {
        let Some(track) = self.online_track("current", 0).cloned() else {
            return;
        };
        let mut context = Context::from_track(&track);
        if !context.album_key.is_empty() {
            context.scope = "album".into();
        }
        context.local_cover = self.local_cover_for_track(&track).is_some()
            || self.online_cover_for_track(&track).is_some();
        context.local_lyrics =
            *self.lyric_line_count() > 0 || *self.lyric_instrumental() || *self.lyrics_loading();
        // The network service still requires opt-in preferences and rejects test runs.
        if let Ok(json) = serde_json::to_string(&vec![context]) {
            self.as_mut().online_automatic_ready(QString::from(&json));
        }
    }
    pub fn prepare_online_batch(
        mut self: core::pin::Pin<&mut Self>,
        kind: &QString,
        selected_only: bool,
    ) {
        if self.rust().online_batch_running.load(Ordering::Acquire) {
            return;
        }
        let kind = kind.to_string();
        if !matches!(kind.as_str(), "cover" | "lyrics") {
            return;
        }
        let resource_root = match online_assets::root() {
            Ok(root) => root,
            Err(error) => {
                self.as_mut()
                    .set_queue_notice(QString::from(&format!("在线资料目录不可用：{error}")));
                return;
            }
        };
        let tracks = if selected_only {
            Arc::new(self.rust().selected_tracks.clone())
        } else {
            self.rust().tracks.clone()
        };
        let stop = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().get_mut().online_batch_cancel = stop.clone();
        let running = self.rust().online_batch_running.clone();
        running.store(true, Ordering::Release);
        self.as_mut().set_online_preparing(true);
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let mut contexts = Vec::new();
            let mut seen = HashSet::new();
            let mut album_files: HashMap<String, Vec<PathBuf>> = HashMap::new();
            if kind == "cover" {
                for track in tracks.iter() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let c = Context::from_track(track);
                    if !c.album_key.is_empty() {
                        album_files
                            .entry(c.album_key)
                            .or_default()
                            .push(PathBuf::from(&track.path));
                    }
                }
            }
            for track in tracks.iter() {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let mut context = Context::from_track(track);
                if kind == "cover" && !context.album_key.is_empty() {
                    context.scope = "album".into();
                }
                let id = if kind == "cover" {
                    context.cover_key()
                } else {
                    &context.track_key
                };
                if !seen.insert(id.to_owned()) {
                    continue;
                }
                let saved = online_assets::read_binding(&resource_root, id, &kind)
                    .filter(|binding| binding.applies(&context))
                    .is_some_and(|binding| {
                        binding.pinned
                            || binding.disabled
                            || binding.instrumental
                            || binding.file(&resource_root).is_some()
                    });
                if saved {
                    continue;
                }
                if kind == "cover" {
                    // All tracks in an album are inspected before deciding it lacks embedded art.
                    let singleton = vec![PathBuf::from(&track.path)];
                    let paths = album_files.get(&context.album_key).unwrap_or(&singleton);
                    context.local_cover = paths.iter().any(|path| {
                        if stop.load(Ordering::Acquire) {
                            return false;
                        }
                        liusheng_core::online_assets::has_local_cover(path)
                    });
                    if context.local_cover {
                        continue;
                    }
                } else {
                    context.local_lyrics = Lyrics::load(Path::new(&track.path))
                        .ok()
                        .flatten()
                        .is_some();
                    if context.local_lyrics {
                        continue;
                    }
                }
                contexts.push(context);
                if contexts.len() >= 5000 {
                    break;
                }
            }
            running.store(false, Ordering::Release);
            let _ = qt.queue(move |mut controller| {
                if !Arc::ptr_eq(&controller.rust().online_batch_cancel, &stop) {
                    return;
                }
                controller.as_mut().set_online_preparing(false);
                if !stop.load(Ordering::Acquire)
                    && let Ok(json) = serde_json::to_string(&contexts)
                {
                    controller
                        .as_mut()
                        .online_batch_ready(QString::from(&json), QString::from(&kind));
                }
            });
        });
    }
    pub fn cancel_online_preparation(self: core::pin::Pin<&mut Self>) {
        self.rust()
            .online_batch_cancel
            .store(true, Ordering::Release);
    }
}
