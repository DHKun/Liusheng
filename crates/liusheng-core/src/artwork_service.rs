//! Bounded artwork jobs, persistent size-bucket thumbnails and cached palette extraction.
use crate::{Error, Result, artwork::CoverCache, library::AlbumKey};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ArtworkRequest {
    pub key: AlbumKey,
    pub generation: u64,
    pub tracks: Vec<PathBuf>,
    pub size: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CachedArtwork {
    pub signature: u64,
    pub variants: HashMap<u32, String>,
    pub accent: String,
}
#[derive(Debug)]
pub enum ArtworkEvent {
    Cached(HashMap<String, CachedArtwork>),
    Ready {
        key: AlbumKey,
        generation: u64,
        size: u32,
        url: String,
        accent: String,
    },
    Failed {
        key: AlbumKey,
        generation: u64,
        size: u32,
        message: String,
    },
}
pub fn album_cache_key(key: &AlbumKey) -> String {
    serde_json::to_string(&(key.album.as_str(), key.album_artist.as_str())).unwrap_or_default()
}
pub struct ArtworkService {
    commands: Sender<ArtworkRequest>,
    events: Receiver<ArtworkEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl ArtworkService {
    pub fn start(root: PathBuf) -> std::io::Result<Self> {
        let (commands, rx) = bounded::<ArtworkRequest>(128);
        let (tx, events) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let worker = std::thread::Builder::new()
            .name("liusheng-artwork".into())
            .spawn(move || {
                let cache = match CoverCache::new(&root) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("封面缓存：{e}");
                        return;
                    }
                };
                let manifest_path = root.join("thumbnails-v1.json");
                let mut manifest: HashMap<String, CachedArtwork> = std::fs::read(&manifest_path)
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or_default();
                tx.send(ArtworkEvent::Cached(manifest.clone())).ok();
                let mut dirty = false;
                while !cancelled.load(Ordering::Acquire) {
                    match rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(request) => {
                            let key = album_cache_key(&request.key);
                            match resolve(&root, &cache, &request, manifest.get(&key)) {
                                Ok(value) => {
                                    let url = value
                                        .variants
                                        .get(&request.size)
                                        .cloned()
                                        .unwrap_or_default();
                                    tx.send(ArtworkEvent::Ready {
                                        key: request.key,
                                        generation: request.generation,
                                        size: request.size,
                                        url,
                                        accent: value.accent.clone(),
                                    })
                                    .ok();
                                    manifest.insert(key, value);
                                    dirty = true;
                                }
                                Err(e) => {
                                    tx.send(ArtworkEvent::Failed {
                                        key: request.key,
                                        generation: request.generation,
                                        size: request.size,
                                        message: e.to_string(),
                                    })
                                    .ok();
                                }
                            }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            if dirty {
                                let _ = crate::settings::save_json(&manifest_path, &manifest);
                                let _ = cache.prune_stale();
                                let _ = prune_orphan_thumbnails(&root, &manifest);
                                dirty = false;
                            }
                        }
                        Err(_) => break,
                    }
                }
                if dirty {
                    let _ = crate::settings::save_json(&manifest_path, &manifest);
                }
                let _ = cache.prune_stale();
                let _ = prune_orphan_thumbnails(&root, &manifest);
            })?;
        Ok(Self {
            commands,
            events,
            stop,
            worker: Some(worker),
        })
    }
    pub fn request(&self, request: ArtworkRequest) -> bool {
        self.commands.try_send(request).is_ok()
    }
    pub fn events(&self) -> Receiver<ArtworkEvent> {
        self.events.clone()
    }
}
impl Drop for ArtworkService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
fn signature(tracks: &[PathBuf]) -> u64 {
    let mut hash = DefaultHasher::new();
    let mut directories = std::collections::BTreeSet::new();
    for path in tracks {
        path.hash(&mut hash);
        file_stamp(path, &mut hash);
        if let Some(p) = path.parent() {
            directories.insert(p);
        }
    }
    for directory in directories {
        let mut covers = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| crate::artwork::is_cover_sidecar(p))
            .collect::<Vec<_>>();
        covers.sort();
        for path in covers {
            path.hash(&mut hash);
            file_stamp(&path, &mut hash);
        }
    }
    hash.finish()
}
fn file_stamp(path: &Path, hash: &mut DefaultHasher) {
    if let Ok(m) = std::fs::metadata(path) {
        m.len().hash(hash);
        m.modified().ok().hash(hash);
    }
}
fn resolve(
    root: &Path,
    cache: &CoverCache,
    request: &ArtworkRequest,
    old: Option<&CachedArtwork>,
) -> Result<CachedArtwork> {
    let signature = signature(&request.tracks);
    let mut value = old
        .filter(|v| v.signature == signature)
        .cloned()
        .unwrap_or(CachedArtwork {
            signature,
            ..Default::default()
        });
    if let Some(url) = value.variants.get(&request.size)
        && (url.is_empty()
            || url::Url::parse(url)
                .ok()
                .and_then(|u| u.to_file_path().ok())
                .is_some_and(|p| p.is_file()))
    {
        return Ok(value);
    }
    let Some(source) = cache.cover_for_album(&request.tracks)? else {
        value.variants.insert(request.size, String::new());
        return Ok(value);
    };
    let mut reader = image::ImageReader::open(&source).map_err(Error::Io)?;
    reader = reader.with_guessed_format().map_err(Error::Io)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| Error::Other(format!("封面解码失败：{e}")))?;
    let thumbnail = image.thumbnail(request.size, request.size).to_rgb8();
    value.accent = extract_accent(&image.thumbnail(24, 24).to_rgb8());
    let mut hasher = DefaultHasher::new();
    album_cache_key(&request.key).hash(&mut hasher);
    let path = root.join(format!(
        "thumb-{:016x}-{signature:016x}-{}.jpg",
        hasher.finish(),
        request.size
    ));
    let mut data = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, 88)
        .encode_image(&thumbnail)
        .map_err(|e| Error::Other(e.to_string()))?;
    crate::settings::atomic_write(&path, &data)?;
    let url = url::Url::from_file_path(&path)
        .map_err(|_| Error::Other("封面缓存需要绝对路径".into()))?
        .to_string();
    value.variants.insert(request.size, url);
    Ok(value)
}
fn extract_accent(image: &image::RgbImage) -> String {
    let mut rgb = [0u64; 3];
    let mut weight_sum = 0u64;
    for p in image.pixels() {
        let max = *p.0.iter().max().unwrap() as u64;
        let min = *p.0.iter().min().unwrap() as u64;
        if max < 30 || min > 230 {
            continue;
        }
        let w = 1 + max - min;
        for (i, v) in p.0.iter().enumerate() {
            rgb[i] += u64::from(*v) * w;
        }
        weight_sum += w;
    }
    if weight_sum == 0 {
        return "#6f9d99".into();
    }
    format!(
        "#{:02x}{:02x}{:02x}",
        (rgb[0] / weight_sum).clamp(55, 215),
        (rgb[1] / weight_sum).clamp(55, 215),
        (rgb[2] / weight_sum).clamp(55, 215)
    )
}
/// Remove obsolete generated thumbnail versions in one idle batch; original images stay untouched.
fn prune_orphan_thumbnails(root: &Path, manifest: &HashMap<String, CachedArtwork>) -> Result<()> {
    let referenced = manifest
        .values()
        .flat_map(|c| c.variants.values())
        .filter_map(|value| url::Url::parse(value).ok()?.to_file_path().ok())
        .collect::<std::collections::HashSet<_>>();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("thumb-")
            && name.ends_with(".jpg")
            && entry.file_type()?.is_file()
            && !referenced.contains(&path)
        {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_is_deterministic() {
        let im = image::RgbImage::from_pixel(8, 8, image::Rgb([180, 90, 60]));
        assert_eq!(extract_accent(&im), "#b45a3c");
    }
    #[test]
    fn missing_artwork_is_cached() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CoverCache::new(dir.path()).unwrap();
        let r = ArtworkRequest {
            key: AlbumKey {
                album: "a".into(),
                album_artist: "b".into(),
            },
            generation: 1,
            tracks: vec![],
            size: 256,
        };
        let value = resolve(dir.path(), &cache, &r, None).unwrap();
        assert_eq!(value.variants[&256], "");
    }
}
