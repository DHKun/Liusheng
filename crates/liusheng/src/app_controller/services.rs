use super::*;

impl qobject::AppController {
    pub fn start_services(mut self: core::pin::Pin<&mut Self>) {
        let paths = match AppPaths::discover() {
            Ok(p) => p,
            Err(e) => {
                self.as_mut().set_status(QString::from(&e.to_string()));
                return;
            }
        };
        match LibraryService::start(paths.clone()) {
            Ok(service) => {
                let events = service.events();
                let qt = self.qt_thread();
                std::thread::spawn(move || {
                    while let Ok(event) = events.recv() {
                        if qt
                            .queue(move |controller| controller.handle_library_event(event))
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                self.as_mut().rust_mut().get_mut().library_service = Some(service);
            }
            Err(e) => {
                self.as_mut()
                    .set_status(QString::from(&format!("曲库服务启动失败：{e}")));
                return;
            }
        }
        if let Ok(service) = ArtworkService::start(paths.covers) {
            let events = service.events();
            let qt = self.qt_thread();
            std::thread::spawn(move || {
                while let Ok(event) = events.recv() {
                    if qt
                        .queue(move |controller| controller.handle_artwork_event(event))
                        .is_err()
                    {
                        break;
                    }
                }
            });
            self.as_mut().rust_mut().get_mut().artwork_service = Some(service);
        }
        if let Ok(service) = SearchService::start() {
            let events = service.events();
            let qt = self.qt_thread();
            std::thread::spawn(move || {
                while let Ok((generation, indices)) = events.recv() {
                    if qt
                        .queue(move |mut controller| {
                            if controller.rust().search_generation != generation {
                                return;
                            }
                            let count = indices.len().min(i32::MAX as usize) as i32;
                            controller
                                .as_mut()
                                .rust_mut()
                                .get_mut()
                                .visible_track_indices = indices;
                            controller.as_mut().set_visible_track_count(count);
                            controller.as_mut().set_searching(false);
                            controller.as_mut().bump_library_revision();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            self.as_mut().rust_mut().get_mut().search_service = Some(service);
        }
        self.as_mut().ensure_mpris();
    }

    pub fn send_library(mut self: core::pin::Pin<&mut Self>, command: LibraryCommand) {
        if self
            .rust()
            .library_service
            .as_ref()
            .is_none_or(|s| !s.send(command))
        {
            self.as_mut()
                .set_status(QString::from("曲库任务队列繁忙，请稍后重试"));
        }
    }
    pub fn handle_library_event(mut self: core::pin::Pin<&mut Self>, event: LibraryEvent) {
        match event {
            LibraryEvent::Ready {
                settings,
                session,
                snapshot,
            } => {
                let settings = *settings;
                let session = *session;
                let json = serde_json::to_string(&settings).unwrap_or_default();
                self.as_mut().set_settings_json(QString::from(&json));
                let prefer_exclusive = cfg!(target_os = "linux") && settings.prefer_exclusive;
                self.as_mut().set_exclusive_output(prefer_exclusive);
                self.as_mut().rust_mut().get_mut().settings = Some(settings);
                self.as_mut().set_saved_ui_json(QString::from(
                    &serde_json::to_string(&session).unwrap_or_default(),
                ));
                self.as_mut().rust_mut().get_mut().saved_session = session;
                self.as_mut().apply_library_snapshot(*snapshot);
                self.as_mut().restore_session_state();
                self.as_mut().set_library_ready(true);
                self.as_mut().load_online_covers();
                self.as_mut()
                    .set_status(QString::from("已加载缓存曲库，正在校验文件变化"));
                self.as_mut().refresh_hardware_volume();
                let pending =
                    std::mem::take(&mut self.as_mut().rust_mut().get_mut().pending_open_files);
                if !pending.is_empty() {
                    self.as_mut()
                        .send_library(LibraryCommand::OpenFiles(pending));
                }
            }
            LibraryEvent::Devices(json) => self.as_mut().set_devices_json(QString::from(&json)),
            LibraryEvent::Scanning => self.as_mut().set_scanning(true),
            LibraryEvent::ScanFinished { stats, cancelled } => {
                self.as_mut().set_scanning(false);
                self.as_mut()
                    .set_scan_errors(QString::from(&stats.errors.join("\n")));
                let label = if cancelled {
                    "扫描已取消，已完成的更新已保留"
                } else {
                    "校验完成"
                };
                let count = *self.track_count();
                self.as_mut().set_status(QString::from(&format!(
                    "{label} · {} 首 · 新增 {} / 更新 {} / 移除 {} / 异常 {}",
                    count, stats.added, stats.updated, stats.removed, stats.failed
                )));
            }
            LibraryEvent::Snapshot {
                snapshot,
                stats,
                finished,
            } => {
                self.as_mut().apply_library_snapshot(*snapshot);
                self.as_mut().set_scanning(!finished);
                self.as_mut()
                    .set_scan_errors(QString::from(&stats.errors.join("\n")));
                let message = format!(
                    "{} · {} 首 · 新增 {} / 更新 {} / 移除 {} / 异常 {}",
                    if finished {
                        "校验完成"
                    } else {
                        "正在导入"
                    },
                    self.track_count(),
                    stats.added,
                    stats.updated,
                    stats.removed,
                    stats.failed
                );
                self.as_mut().set_status(QString::from(&message));
            }
            LibraryEvent::InvalidateArtwork(paths) => {
                let affected = self
                    .rust()
                    .albums
                    .iter()
                    .filter(|album| {
                        paths.is_empty()
                            || self.rust().album_tracks.get(&album.key).is_some_and(|ids| {
                                ids.iter().any(|i| {
                                    let track = Path::new(&self.rust().tracks[*i].path);
                                    paths.iter().any(|p| {
                                        track == p
                                            || track.parent() == p.parent()
                                            || track.starts_with(p)
                                    })
                                })
                            })
                    })
                    .map(|a| album_cache_key(&a.key))
                    .collect::<Vec<_>>();
                for key in affected {
                    self.as_mut()
                        .rust_mut()
                        .get_mut()
                        .artwork_cache
                        .remove(&key);
                }
                self.as_mut().invalidate_artwork_requests();
                self.as_mut().refresh_artwork_arrays();
            }
            LibraryEvent::LyricsChanged(paths) => {
                let path = PathBuf::from(self.current_track_path().to_string());
                if paths.iter().any(|p| {
                    p == &path || p.file_stem() == path.file_stem() && p.parent() == path.parent()
                }) {
                    self.as_mut().rust_mut().get_mut().lyrics_request_path = None;
                    self.as_mut().request_lyrics_for_path(path);
                }
            }
            LibraryEvent::Playlists(playlists) => {
                let count = playlists.len() as i32;
                self.as_mut().rust_mut().get_mut().playlists = playlists;
                self.as_mut().set_playlist_count(count);
                let rev = self.playlist_revision().wrapping_add(1);
                self.as_mut().set_playlist_revision(rev);
            }
            LibraryEvent::PlayPaths(paths) => {
                let tracks = self.as_ref().tracks_for_paths(&paths);
                if tracks.is_empty() {
                    self.as_mut()
                        .set_status(QString::from("列表中没有可播放的本地音频"));
                } else {
                    self.as_mut().play_track_queue(tracks, 0);
                }
            }
            LibraryEvent::SettingsSaved(settings) => {
                let device_changed = self
                    .rust()
                    .settings
                    .as_ref()
                    .is_none_or(|s| s.exclusive_device != settings.exclusive_device);
                if device_changed && let Some(session) = &self.rust().output_session {
                    session.send(SessionCommand::SetExclusiveDevice(
                        settings.exclusive_device.clone(),
                    ));
                }
                self.as_mut().set_settings_json(QString::from(
                    &serde_json::to_string(&settings).unwrap_or_default(),
                ));
                self.as_mut().rust_mut().get_mut().settings = Some(settings);
                self.as_mut().refresh_hardware_volume();
                self.as_mut()
                    .set_status(QString::from("设置已保存，输出设备与曲库配置已提交更新"));
            }
            LibraryEvent::Notice(text) => self.as_mut().set_status(QString::from(&text)),
            LibraryEvent::Error(error) => {
                self.as_mut().set_scanning(false);
                self.as_mut().set_scan_errors(QString::from(&error));
                self.as_mut().set_status(QString::from(&error));
            }
        }
    }
    pub fn apply_library_snapshot(mut self: core::pin::Pin<&mut Self>, snapshot: LibrarySnapshot) {
        let selected_album = self
            .album_at(*self.selected_album_index())
            .map(|a| a.key.clone());
        let selected_artist = self
            .artist_at(*self.selected_artist_index())
            .map(|a| a.key.clone());
        let track_count = snapshot.tracks.len() as i32;
        let album_count = snapshot.albums.len() as i32;
        let artist_count = snapshot.artists.len() as i32;
        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.tracks = snapshot.tracks;
            state.albums = snapshot.albums;
            state.artists = snapshot.artists;
            state.album_tracks = snapshot.album_tracks;
            state.artist_tracks = snapshot.artist_tracks;
            state.album_search = snapshot.album_search;
            state.artist_search = snapshot.artist_search;
            state.artist_indices = snapshot.artist_indices;
            state.album_indices = snapshot.album_indices;
        }
        self.as_mut().set_track_count(track_count);
        self.as_mut().set_album_count(album_count);
        self.as_mut().set_artist_count(artist_count);
        self.as_mut().invalidate_artwork_requests();
        self.as_mut().refresh_artwork_arrays();
        if let Some(key) = selected_album {
            if let Some(i) = self.rust().album_indices.get(&key).copied() {
                self.as_mut().open_album(i as i32);
            } else {
                self.as_mut().close_album();
            }
        } else if let Some(key) = selected_artist {
            if let Some(i) = self.rust().artists.iter().position(|a| a.key == key) {
                self.as_mut().open_artist(i as i32);
            } else {
                self.as_mut().close_artist();
            }
        }
        self.publish_library_models();
        self.as_mut().submit_search();
        // The default filter publishes synchronously; an active filter publishes
        // when its generation completes. Album/artist models can update now.
        if *self.searching() {
            let revision = self.library_revision().wrapping_add(1);
            self.as_mut().set_library_revision(revision);
        }
    }
    pub fn submit_search(mut self: core::pin::Pin<&mut Self>) {
        let query = self.track_filter().to_string();
        let generation = self.rust().search_generation.wrapping_add(1);
        self.as_mut().rust_mut().get_mut().search_generation = generation;
        if query.is_empty() && *self.sort_order() == 0 && self.rust().format_filter.is_empty() {
            if let Some(service) = &self.rust().search_service {
                service.cancel(generation);
            }
            let len = self.rust().tracks.len();
            self.as_mut().rust_mut().get_mut().visible_track_indices = (0..len).collect();
            self.as_mut().set_visible_track_count(len as i32);
            self.as_mut().set_searching(false);
            self.as_mut().bump_library_revision();
            return;
        }
        self.as_mut().set_searching(true);
        if let Some(service) = &self.rust().search_service {
            service.submit(SearchJob {
                generation,
                query,
                tracks: self.rust().tracks.clone(),
                sort: *self.sort_order(),
                format: self.rust().format_filter.clone(),
            });
        }
    }
    pub fn sort_tracks(mut self: core::pin::Pin<&mut Self>, order: i32, format: &QString) {
        self.as_mut().set_sort_order(order.clamp(0, 4));
        self.as_mut().rust_mut().get_mut().format_filter = format.to_string().to_ascii_lowercase();
        self.as_mut().submit_search();
    }
    pub fn invalidate_artwork_requests(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().rust_mut().get_mut().artwork_generation =
            self.rust().artwork_generation.wrapping_add(1);
        self.as_mut().rust_mut().get_mut().artwork_requested.clear();
        let rev = self.artwork_revision().wrapping_add(1);
        self.as_mut().set_artwork_revision(rev);
    }
    pub fn refresh_artwork_arrays(mut self: core::pin::Pin<&mut Self>) {
        let urls = self
            .rust()
            .albums
            .iter()
            .map(|a| self.grid_cover(a))
            .collect::<Vec<_>>();
        self.as_mut().rust_mut().get_mut().album_cover_urls = urls;
        let artist_urls = self
            .rust()
            .artists
            .iter()
            .map(|a| {
                self.rust()
                    .artist_tracks
                    .get(&a.key)
                    .into_iter()
                    .flatten()
                    .find_map(|i| {
                        self.cover_url_for_track(&self.rust().tracks[*i])
                            .map(str::to_owned)
                    })
                    .unwrap_or_default()
            })
            .collect();
        self.as_mut().rust_mut().get_mut().artist_cover_urls = artist_urls;
        crate::models::update_covers("albums", &self.rust().album_cover_urls);
        crate::models::update_covers("artists", &self.rust().artist_cover_urls);
        let rev = self.artwork_revision().wrapping_add(1);
        self.as_mut().set_artwork_revision(rev);
        self.as_mut().refresh_current_cover();
    }
    pub fn request_album_cover(mut self: core::pin::Pin<&mut Self>, index: i32) {
        self.as_mut().request_cover(index, 256);
    }
    pub fn request_artist_cover(mut self: core::pin::Pin<&mut Self>, index: i32) {
        let album = self
            .artist_at(index)
            .and_then(|a| self.rust().artist_tracks.get(&a.key))
            .and_then(|ids| ids.first())
            .and_then(|i| {
                let t = &self.rust().tracks[*i];
                self.rust()
                    .album_indices
                    .get(&AlbumKey {
                        album: t.album.clone(),
                        album_artist: t.album_artist.clone(),
                    })
                    .copied()
            });
        if let Some(i) = album {
            self.as_mut().request_cover(i as i32, 256);
        }
    }
    pub fn request_cover(mut self: core::pin::Pin<&mut Self>, index: i32, size: u32) {
        let Some(key) = self.album_at(index).map(|a| a.key.clone()) else {
            return;
        };
        let generation = self.rust().artwork_generation;
        let token = (key.clone(), size, generation);
        if self.rust().artwork_requested.contains(&token) {
            return;
        }
        let tracks = self
            .rust()
            .album_tracks
            .get(&key)
            .into_iter()
            .flatten()
            .map(|i| PathBuf::from(&self.rust().tracks[*i].path))
            .collect();
        if self.rust().artwork_service.as_ref().is_some_and(|s| {
            s.request(ArtworkRequest {
                key,
                generation,
                tracks,
                size,
            })
        }) {
            self.as_mut()
                .rust_mut()
                .get_mut()
                .artwork_requested
                .insert(token);
        }
    }
    fn refresh_album_artwork(mut self: core::pin::Pin<&mut Self>, key: &AlbumKey, size: u32) {
        let mut changed = false;
        if size == 256 {
            if let Some(index) = self.rust().album_indices.get(key).copied() {
                let url = self.grid_cover(&self.rust().albums[index]);
                if let Some(slot) = self
                    .as_mut()
                    .rust_mut()
                    .get_mut()
                    .album_cover_urls
                    .get_mut(index)
                    && *slot != url
                {
                    *slot = url.clone();
                    changed = true;
                }
                crate::models::update_cover("albums", index, &url);
            }
            let artists = self
                .rust()
                .album_tracks
                .get(key)
                .into_iter()
                .flatten()
                .filter_map(|i| {
                    self.rust()
                        .artist_indices
                        .get(&self.rust().tracks[*i].artist)
                })
                .copied()
                .collect::<HashSet<_>>();
            for index in artists {
                let artist_key = &self.rust().artists[index].key;
                let url = self
                    .rust()
                    .artist_tracks
                    .get(artist_key)
                    .into_iter()
                    .flatten()
                    .find_map(|i| {
                        self.cover_url_for_track(&self.rust().tracks[*i])
                            .map(str::to_owned)
                    })
                    .unwrap_or_default();
                if let Some(slot) = self
                    .as_mut()
                    .rust_mut()
                    .get_mut()
                    .artist_cover_urls
                    .get_mut(index)
                    && *slot != url
                {
                    *slot = url.clone();
                    changed = true;
                }
                crate::models::update_cover("artists", index, &url);
            }
        }
        if changed {
            let revision = self.artwork_revision().wrapping_add(1);
            self.as_mut().set_artwork_revision(revision);
        }
        self.as_mut().refresh_current_cover();
    }

    pub fn handle_artwork_event(mut self: core::pin::Pin<&mut Self>, event: ArtworkEvent) {
        match event {
            ArtworkEvent::Cached(cache) => {
                self.as_mut().rust_mut().get_mut().artwork_cache = cache;
                self.as_mut().refresh_artwork_arrays();
            }
            ArtworkEvent::Ready {
                key,
                generation,
                size,
                url,
                accent,
            } => {
                if generation != self.rust().artwork_generation {
                    return;
                }
                let cache_key = album_cache_key(&key);
                {
                    let state = self.as_mut().rust_mut();
                    let state = state.get_mut();
                    let cache = state.artwork_cache.entry(cache_key).or_default();
                    cache.variants.insert(size, url);
                    cache.accent = accent;
                }
                self.as_mut().refresh_album_artwork(&key, size);
                self.as_mut().update_track_extras();
            }
            ArtworkEvent::Failed {
                key,
                generation,
                size,
                message,
            } => {
                if generation == self.rust().artwork_generation {
                    eprintln!("封面 {} / {size}：{message}", key.album);
                }
            }
        }
    }
    pub fn ensure_volume_service(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().volume_service.is_some() {
            return;
        }
        match VolumeService::start() {
            Ok(service) => {
                let events = service.events();
                let qt = self.qt_thread();
                std::thread::spawn(move || {
                    while let Ok(result) = events.recv() {
                        if qt
                            .queue(move |controller| {
                                controller.apply_hardware_volume_result(
                                    result.map_err(liusheng_core::Error::Other),
                                )
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                self.as_mut().rust_mut().get_mut().volume_service = Some(service);
            }
            Err(e) => self
                .as_mut()
                .set_hardware_volume_error(QString::from(&e.to_string())),
        }
    }
    pub fn refresh_devices(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().send_library(LibraryCommand::DiscoverDevices);
    }
    pub fn apply_settings(mut self: core::pin::Pin<&mut Self>, json: &QString) {
        match serde_json::from_str::<AppSettings>(&json.to_string()) {
            Ok(settings) => self
                .as_mut()
                .send_library(LibraryCommand::Settings(settings)),
            Err(e) => self
                .as_mut()
                .set_status(QString::from(&format!("设置格式错误：{e}"))),
        }
    }
    pub fn save_ui_state(mut self: core::pin::Pin<&mut Self>, json: &QString) {
        if !*self.library_ready() {
            return;
        }
        if let Ok(value) = serde_json::from_str::<SavedSession>(&json.to_string()) {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.saved_session.page = value.page;
            state.saved_session.width = value.width;
            state.saved_session.height = value.height;
            state.saved_session.album_scroll = value.album_scroll;
            state.saved_session.track_scroll = value.track_scroll;
        }
        self.checkpoint_session();
    }
    pub fn checkpoint_session(mut self: core::pin::Pin<&mut Self>) {
        if !*self.library_ready() {
            return;
        }
        let revision = *self.queue_revision();
        let queue = if revision != self.rust().saved_queue_revision {
            Arc::new(
                self.rust()
                    .playback_queue
                    .iter()
                    .map(|t| t.path.clone())
                    .collect(),
            )
        } else {
            self.rust().saved_session.queue.clone()
        };
        let index = self.rust().current_queue_index.unwrap_or(0);
        let position = *self.position_ms();
        let repeat = *self.repeat_mode() as u8;
        let shuffle = *self.shuffle_enabled();
        let state = self.as_mut().rust_mut();
        let state = state.get_mut();
        state.saved_session.version = 1;
        state.saved_session.queue = queue;
        state.saved_queue_revision = revision;
        state.saved_session.current_index = index;
        state.saved_session.position_ms = position;
        state.saved_session.repeat_mode = repeat;
        state.saved_session.shuffle = shuffle;
        state.saved_session.playback_order = state
            .navigation
            .order
            .is_valid_for(state.playback_queue.len())
            .then(|| state.navigation.order.clone());
        if let Some(service) = &state.library_service {
            service.save_session(state.saved_session.clone());
        }
    }
    fn tracks_for_paths(&self, paths: &[String]) -> Vec<Arc<TrackRow>> {
        let by_path = self
            .rust()
            .tracks
            .iter()
            .map(|t| (t.path.as_str(), t))
            .collect::<HashMap<_, _>>();
        paths
            .iter()
            .filter(|p| liusheng_core::library::is_audio_path(Path::new(p)))
            .map(|p| {
                by_path
                    .get(p.as_str())
                    .map(|t| Arc::clone(t))
                    .unwrap_or_else(|| {
                        Arc::new(TrackRow {
                            id: 0,
                            path: p.clone(),
                            title: Path::new(p)
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                            artist: String::new(),
                            album: String::new(),
                            album_artist: String::new(),
                            track_no: None,
                            disc_no: None,
                            year: None,
                            genre: String::new(),
                            duration_ms: 0,
                            sample_rate: 0,
                            bit_depth: None,
                            channels: 2,
                            search_text: String::new(),
                            mtime: 0,
                            file_size: 0,
                        })
                    })
            })
            .collect()
    }
    pub fn restore_session_state(mut self: core::pin::Pin<&mut Self>) {
        if !self
            .rust()
            .settings
            .as_ref()
            .is_some_and(|s| s.restore_session)
        {
            return;
        }
        let saved = self.rust().saved_session.clone();
        let queue = self.tracks_for_paths(&saved.queue);
        self.as_mut()
            .set_repeat_mode(i32::from(saved.repeat_mode.min(2)));
        self.as_mut().set_shuffle_enabled(saved.shuffle);
        if queue.is_empty() {
            return;
        }
        let index = saved.current_index.min(queue.len() - 1);
        let mut order = saved
            .playback_order
            .filter(|o| o.is_valid_for(queue.len()))
            .unwrap_or_default();
        if !order.is_valid_for(queue.len()) {
            order.rebuild(queue.len(), index, saved.shuffle);
        }
        self.as_mut().rust_mut().get_mut().navigation = NavigationState {
            order,
            current: index,
            repeat: saved.repeat_mode.min(2),
            shuffle: saved.shuffle,
        };
        let current = queue[index].clone();
        let count = queue.len() as i32;
        self.as_mut().rust_mut().get_mut().playback_queue = queue;
        self.as_mut().rust_mut().get_mut().current_queue_index = Some(index);
        self.as_mut().rust_mut().get_mut().restore_pending = true;
        self.as_mut().set_queue_count(count);
        self.as_mut().set_current_queue_position(index as i32);
        self.as_mut()
            .set_current_title(QString::from(&current.title));
        self.as_mut()
            .set_current_artist(QString::from(display_artist(&current.artist)));
        self.as_mut()
            .set_current_track_path(QString::from(&current.path));
        self.as_mut()
            .set_current_duration_ms(current.duration_ms.min(i32::MAX as u64) as i32);
        self.as_mut().set_position_ms(saved.position_ms.max(0));
        self.as_mut().set_has_current_track(true);
        self.as_mut().set_seekable(true);
        self.as_mut().set_playing(false);
        self.as_mut().bump_queue_revision();
        self.as_mut().refresh_current_cover();
        self.as_mut().update_track_extras();
        self.as_mut()
            .request_lyrics_for_path(PathBuf::from(&current.path));
    }
    pub fn select_restored_track(mut self: core::pin::Pin<&mut Self>, index: usize) {
        if let Some(track) = self.rust().playback_queue.get(index).cloned() {
            self.as_mut().rust_mut().get_mut().navigation.current = index;
            self.as_mut().rust_mut().get_mut().current_queue_index = Some(index);
            self.as_mut().set_current_queue_position(index as i32);
            self.as_mut().set_current_title(QString::from(&track.title));
            self.as_mut()
                .set_current_artist(QString::from(display_artist(&track.artist)));
            self.as_mut()
                .set_current_track_path(QString::from(&track.path));
            self.as_mut()
                .set_current_duration_ms(track.duration_ms.min(i32::MAX as u64) as i32);
            self.as_mut().set_position_ms(0);
            self.as_mut().set_playing(false);
            self.as_mut().bump_queue_revision();
            self.as_mut().refresh_current_cover();
            self.as_mut().update_track_extras();
            self.as_mut()
                .request_lyrics_for_path(PathBuf::from(&track.path));
            self.as_mut().checkpoint_session();
            self.sync_mpris();
        }
    }
    pub fn resume_saved_queue(mut self: core::pin::Pin<&mut Self>) {
        let queue = self.rust().playback_queue.clone();
        let index = self.rust().current_queue_index.unwrap_or(0);
        let position = *self.position_ms();
        self.as_mut().play_track_queue(queue, index);
        if position > 0 {
            self.send_player_command(PlayerCommand::Seek(f64::from(position) / 1000.0));
        }
    }
    pub fn update_track_extras(mut self: core::pin::Pin<&mut Self>) {
        let Some(track) = self
            .rust()
            .current_queue_index
            .and_then(|i| self.rust().playback_queue.get(i))
            .cloned()
        else {
            return;
        };
        let key = AlbumKey {
            album: track.album.clone(),
            album_artist: track.album_artist.clone(),
        };
        let offset = self
            .rust()
            .settings
            .as_ref()
            .and_then(|s| {
                s.lyric_offsets.get(
                    if self
                        .rust()
                        .lyric_offset_key
                        .starts_with(&format!("{}#online:", track.path))
                    {
                        &self.rust().lyric_offset_key
                    } else {
                        &track.path
                    },
                )
            })
            .copied()
            .unwrap_or(0);
        self.as_mut().set_lyrics_offset_ms(offset);
        let cached = self
            .rust()
            .artwork_cache
            .get(&album_cache_key(&key))
            .cloned();
        let accent = cached
            .as_ref()
            .map(|c| c.accent.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("#6f9d99");
        self.as_mut().set_current_accent(QString::from(accent));
        if let Some(url) = cached
            .as_ref()
            .and_then(|c| c.variants.get(&768))
            .filter(|s| !s.is_empty())
        {
            self.as_mut().set_current_cover_url(QString::from(url));
        }
        if let Some(index) = self.rust().album_indices.get(&key).copied() {
            self.as_mut().request_cover(index as i32, 768);
        }
        let backend = if self.rust().backend_details.is_empty() {
            "播放开始后显示输出协商结果"
        } else {
            &self.rust().backend_details
        };
        let text = format!(
            "来源：{} Hz · {} bit · {} 声道\n{}",
            track.sample_rate,
            track.bit_depth.unwrap_or(32),
            track.channels,
            backend
        );
        self.as_mut().set_audio_details(QString::from(&text));
        self.as_mut().refresh_current_cover();
    }
    pub fn request_lyrics_offset(mut self: core::pin::Pin<&mut Self>, offset: i32) {
        let offset = offset.clamp(-30000, 30000);
        let path = self.current_track_path().to_string();
        if path.is_empty() {
            return;
        }
        self.as_mut().set_lyrics_offset_ms(offset);
        self.as_mut().update_current_lyric_index();
        if let Some(mut settings) = self.rust().settings.clone() {
            let key = if self
                .rust()
                .lyric_offset_key
                .starts_with(&format!("{path}#online:"))
            {
                self.rust().lyric_offset_key.clone()
            } else {
                path
            };
            settings.lyric_offsets.insert(key, offset);
            self.as_mut().rust_mut().get_mut().settings = Some(settings.clone());
            self.as_mut()
                .send_library(LibraryCommand::Settings(settings));
        }
    }
    pub fn request_playback_mode(mut self: core::pin::Pin<&mut Self>, repeat: i32, shuffle: bool) {
        if self.rust().restore_pending || self.rust().output_session.is_none() {
            let len = self.rust().playback_queue.len();
            let navigation = &mut self.as_mut().rust_mut().get_mut().navigation;
            if navigation.shuffle != shuffle {
                navigation.order.rebuild(len, navigation.current, shuffle);
            }
            navigation.repeat = repeat.clamp(0, 2) as u8;
            navigation.shuffle = shuffle;
        }
        self.as_mut().set_repeat_mode(repeat.clamp(0, 2));
        self.as_mut().set_shuffle_enabled(shuffle);
        self.send_player_command(PlayerCommand::SetPlaybackMode {
            repeat: repeat.clamp(0, 2) as u8,
            shuffle,
        });
        self.as_mut().checkpoint_session();
        self.sync_mpris();
    }
    pub fn move_queue_track(mut self: core::pin::Pin<&mut Self>, from: i32, to: i32) {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        let len = self.rust().playback_queue.len();
        if from >= len || to >= len || from == to {
            return;
        }
        let current = self.rust().current_queue_index.map(|i| {
            if i == from {
                to
            } else if from < i && i <= to {
                i - 1
            } else if to <= i && i < from {
                i + 1
            } else {
                i
            }
        });
        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            let t = state.playback_queue.remove(from);
            state.playback_queue.insert(to, t);
            state.current_queue_index = current;
            if state.restore_pending {
                state
                    .navigation
                    .order
                    .move_item(from, to, state.navigation.shuffle);
                state.navigation.current = current.unwrap_or(0);
            }
        }
        self.as_mut()
            .set_current_queue_position(current.map(|i| i as i32).unwrap_or(-1));
        self.as_mut().bump_queue_revision();
        self.send_player_command(PlayerCommand::MoveQueueItem { from, to });
        self.as_mut().checkpoint_session();
    }
    pub fn save_queue_playlist(mut self: core::pin::Pin<&mut Self>, name: &QString) {
        let paths = self
            .rust()
            .playback_queue
            .iter()
            .map(|t| t.path.clone())
            .collect();
        self.as_mut().send_library(LibraryCommand::SavePlaylist {
            name: name.to_string(),
            paths,
        });
    }
    pub fn playlist_name(&self, index: i32) -> QString {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().playlists.get(i))
            .map(|p| QString::from(&format!("{} · {} 首", p.name, p.count)))
            .unwrap_or_default()
    }
    pub fn play_playlist(mut self: core::pin::Pin<&mut Self>, index: i32) {
        if let Some(id) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().playlists.get(i))
            .map(|p| p.id)
        {
            self.as_mut().send_library(LibraryCommand::LoadPlaylist(id));
        }
    }
    pub fn delete_playlist(mut self: core::pin::Pin<&mut Self>, index: i32) {
        if let Some(id) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().playlists.get(i))
            .map(|p| p.id)
        {
            self.as_mut()
                .send_library(LibraryCommand::DeletePlaylist(id));
        }
    }
    pub fn rename_playlist(mut self: core::pin::Pin<&mut Self>, index: i32, name: &QString) {
        if let Some(id) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().playlists.get(i))
            .map(|p| p.id)
        {
            self.as_mut().send_library(LibraryCommand::RenamePlaylist {
                id,
                name: name.to_string(),
            });
        }
    }
    pub fn import_playlist(mut self: core::pin::Pin<&mut Self>, path: &QString) {
        match local_path(&path.to_string()) {
            Ok(path) => self
                .as_mut()
                .send_library(LibraryCommand::ImportPlaylist(path)),
            Err(e) => self.as_mut().set_status(QString::from(&e)),
        }
    }
    pub fn export_queue(mut self: core::pin::Pin<&mut Self>, path: &QString) {
        match local_path(&path.to_string()) {
            Ok(path) => {
                let paths = self
                    .rust()
                    .playback_queue
                    .iter()
                    .map(|t| t.path.clone())
                    .collect();
                self.as_mut()
                    .send_library(LibraryCommand::ExportPlaylist { path, paths });
            }
            Err(e) => self.as_mut().set_status(QString::from(&e)),
        }
    }
    pub fn open_files(mut self: core::pin::Pin<&mut Self>, json: &QString) {
        let values = match serde_json::from_str::<Vec<String>>(&json.to_string()) {
            Ok(v) => v,
            Err(e) => {
                self.as_mut().set_status(QString::from(&e.to_string()));
                return;
            }
        };
        let mut paths = Vec::new();
        for value in values {
            match local_path(&value) {
                Ok(p) => paths.push(p),
                Err(e) => {
                    self.as_mut().set_status(QString::from(&e));
                    return;
                }
            }
        }
        if *self.library_ready() {
            self.as_mut().send_library(LibraryCommand::OpenFiles(paths));
        } else {
            self.as_mut()
                .rust_mut()
                .get_mut()
                .pending_open_files
                .extend(paths);
        }
    }
}
fn local_path(value: &str) -> Result<PathBuf, String> {
    if value.starts_with("file:") {
        return url::Url::parse(value)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .ok_or_else(|| "文件 URL 格式错误".into());
    }
    if value.contains("://") {
        return Err("请选择本地音频文件或目录".into());
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|d| d.join(path))
            .map_err(|e| e.to_string())
    }
}

impl qobject::AppController {
    pub fn publish_library_models(&self) {
        use crate::models::{ModelRow, RowValue, publish};
        publish(
            "albums",
            self.rust()
                .albums
                .iter()
                .enumerate()
                .map(|(i, a)| ModelRow {
                    key: album_cache_key(&a.key),
                    source: i as i32,
                    value: RowValue::Album {
                        summary: Arc::new(a.clone()),
                        cover: self
                            .rust()
                            .album_cover_urls
                            .get(i)
                            .cloned()
                            .unwrap_or_default(),
                        search: self
                            .rust()
                            .album_search
                            .get(&a.key)
                            .cloned()
                            .unwrap_or_default(),
                    },
                })
                .collect(),
        );
        publish(
            "artists",
            self.rust()
                .artists
                .iter()
                .enumerate()
                .map(|(i, a)| ModelRow {
                    key: a.key.clone(),
                    source: i as i32,
                    value: RowValue::Artist {
                        summary: Arc::new(a.clone()),
                        cover: self
                            .rust()
                            .artist_cover_urls
                            .get(i)
                            .cloned()
                            .unwrap_or_default(),
                        search: self
                            .rust()
                            .artist_search
                            .get(&a.key)
                            .cloned()
                            .unwrap_or_default(),
                    },
                })
                .collect(),
        );
    }
    pub fn publish_track_model(&self) {
        use crate::models::{ModelRow, RowValue, publish};
        publish(
            "tracks",
            self.rust()
                .visible_track_indices
                .iter()
                .enumerate()
                .filter_map(|(row, index)| {
                    self.rust().tracks.get(*index).map(|t| ModelRow {
                        key: t.path.clone(),
                        source: row as i32,
                        value: RowValue::Track(t.clone()),
                    })
                })
                .collect(),
        );
    }
    pub fn publish_selected_model(&self) {
        use crate::models::{ModelRow, RowValue, publish};
        publish(
            "selected",
            self.rust()
                .selected_tracks
                .iter()
                .enumerate()
                .map(|(i, t)| ModelRow {
                    key: t.path.clone(),
                    source: i as i32,
                    value: RowValue::Track(t.clone()),
                })
                .collect(),
        );
    }
    pub fn publish_queue_model(&self) {
        use crate::models::{ModelRow, RowValue, publish};
        let mut occurrences = HashMap::<&str, usize>::new();
        publish(
            "queue",
            self.rust()
                .playback_queue
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let number = occurrences.entry(t.path.as_str()).or_default();
                    *number += 1;
                    ModelRow {
                        key: format!("{}\0{}", t.path, number),
                        source: i as i32,
                        value: RowValue::Track(t.clone()),
                    }
                })
                .collect(),
        );
    }
}
