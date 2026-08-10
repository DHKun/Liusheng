use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lofty::picture::{Picture, PictureType};
use lofty::prelude::TaggedFileExt;
use lofty::tag::Tag;

use crate::Result;

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "png", "webp", "gif", "bmp", "tif"];
const SIDECAR_NAMES: &[&str] = &["cover", "folder", "front"];

/// 提取专辑封面并把内嵌图片与目录图片写入带版本号的本地缓存。
pub struct CoverCache {
    root: PathBuf,
}

struct EmbeddedCover {
    path: PathBuf,
    front: bool,
}

impl CoverCache {
    pub fn new(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        Ok(Self {
            root: root.to_owned(),
        })
    }

    pub fn cover_for_album(&self, tracks: &[PathBuf]) -> Result<Option<PathBuf>> {
        let mut embedded_fallback = None;
        for track in tracks {
            if let Some(cover) = self.embedded_cover(track)?
                && let Some(front) = prefer_embedded_cover(&mut embedded_fallback, cover)
            {
                return Ok(Some(front));
            }
        }
        if let Some(cover) = embedded_fallback {
            return Ok(Some(cover));
        }

        let mut checked_directories = HashSet::new();
        for track in tracks {
            let Some(directory) = track.parent() else {
                continue;
            };
            if checked_directories.insert(directory.to_owned())
                && let Some(sidecar) = find_sidecar(directory)
            {
                return self.cache_sidecar(&sidecar).map(Some);
            }
        }
        Ok(None)
    }

    fn embedded_cover(&self, audio_path: &Path) -> Result<Option<EmbeddedCover>> {
        let Some((path_hash, revision)) = file_revision(audio_path) else {
            return Ok(None);
        };
        let base = format!("embedded-{path_hash:016x}-{revision:016x}");
        let front_stem = format!("{base}-front");
        if let Some(path) = self.cached_image(&front_stem) {
            return Ok(Some(EmbeddedCover { path, front: true }));
        }
        let other_stem = format!("{base}-other");
        if let Some(path) = self.cached_image(&other_stem) {
            return Ok(Some(EmbeddedCover { path, front: false }));
        }
        let marker = self.root.join(format!("{base}.none"));
        if marker.is_file() {
            return Ok(None);
        }

        let picture = lofty::read_from_path(audio_path).ok().and_then(|tagged| {
            select_picture(tagged.primary_tag(), tagged.tags()).and_then(|picture| {
                image_extension(picture.picture)
                    .map(|ext| (picture.picture.data().to_vec(), ext, picture.front))
            })
        });

        let prefix = format!("embedded-{path_hash:016x}-");
        if let Some((data, extension, front)) = picture {
            let stem = if front { front_stem } else { other_stem };
            self.remove_stale(&prefix, &stem)?;
            return self
                .write_cache(&stem, extension, &data)
                .map(|path| Some(EmbeddedCover { path, front }));
        }
        self.remove_stale(&prefix, &base)?;
        self.write_marker(&marker)?;
        Ok(None)
    }

    fn cache_sidecar(&self, source: &Path) -> Result<PathBuf> {
        let (path_hash, revision) = file_revision(source).unwrap_or_else(|| {
            let mut hasher = DefaultHasher::new();
            source.hash(&mut hasher);
            (hasher.finish(), 0)
        });
        let stem = format!("sidecar-{path_hash:016x}-{revision:016x}");
        if let Some(path) = self.cached_image(&stem) {
            return Ok(path);
        }
        let extension = normalized_extension(source).unwrap_or("jpg");
        let data = std::fs::read(source)?;
        let prefix = format!("sidecar-{path_hash:016x}-");
        self.remove_stale(&prefix, &stem)?;
        self.write_cache(&stem, extension, &data)
    }

    fn cached_image(&self, stem: &str) -> Option<PathBuf> {
        IMAGE_EXTENSIONS
            .iter()
            .map(|extension| self.root.join(format!("{stem}.{extension}")))
            .find(|path| path.is_file())
    }

    fn write_cache(&self, stem: &str, extension: &str, data: &[u8]) -> Result<PathBuf> {
        let path = self.root.join(format!("{stem}.{extension}"));
        if path.is_file() {
            return Ok(path);
        }
        let temporary = self.root.join(format!(".{stem}.{extension}.tmp"));
        std::fs::write(&temporary, data)?;
        std::fs::rename(temporary, &path)?;
        Ok(path)
    }

    fn write_marker(&self, marker: &Path) -> Result<()> {
        if !marker.is_file() {
            std::fs::write(marker, [])?;
        }
        Ok(())
    }

    fn remove_stale(&self, prefix: &str, current_stem: &str) -> Result<()> {
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(prefix) && !name.starts_with(current_stem) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

fn file_revision(path: &Path) -> Option<(u64, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();

    let mut path_hasher = DefaultHasher::new();
    path.hash(&mut path_hasher);
    let mut revision_hasher = DefaultHasher::new();
    modified.hash(&mut revision_hasher);
    metadata.len().hash(&mut revision_hasher);
    Some((path_hasher.finish(), revision_hasher.finish()))
}

struct SelectedPicture<'a> {
    picture: &'a Picture,
    front: bool,
}

fn select_picture<'a>(primary: Option<&'a Tag>, tags: &'a [Tag]) -> Option<SelectedPicture<'a>> {
    primary
        .and_then(|tag| tag.get_picture_type(PictureType::CoverFront))
        .or_else(|| {
            tags.iter()
                .find_map(|tag| tag.get_picture_type(PictureType::CoverFront))
        })
        .map(|picture| SelectedPicture {
            picture,
            front: true,
        })
        .or_else(|| {
            primary
                .and_then(|tag| tag.pictures().first())
                .or_else(|| tags.iter().find_map(|tag| tag.pictures().first()))
                .map(|picture| SelectedPicture {
                    picture,
                    front: false,
                })
        })
}

fn prefer_embedded_cover(fallback: &mut Option<PathBuf>, cover: EmbeddedCover) -> Option<PathBuf> {
    if cover.front {
        Some(cover.path)
    } else {
        fallback.get_or_insert(cover.path);
        None
    }
}

fn image_extension(picture: &Picture) -> Option<&'static str> {
    picture
        .mime_type()
        .and_then(|mime| mime.ext())
        .and_then(normalize_extension)
        .or_else(|| sniff_extension(picture.data()))
}

fn sniff_extension(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("gif")
    } else if data.starts_with(b"BM") {
        Some("bmp")
    } else if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        Some("tif")
    } else if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

fn find_sidecar(directory: &Path) -> Option<PathBuf> {
    let mut candidates = std::fs::read_dir(directory)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?.to_ascii_lowercase();
            let name_priority = SIDECAR_NAMES.iter().position(|name| *name == stem)?;
            let extension = normalized_extension(&path)?;
            let extension_priority = IMAGE_EXTENSIONS
                .iter()
                .position(|candidate| *candidate == extension)?;
            Some((name_priority, extension_priority, path))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    candidates.into_iter().next().map(|(_, _, path)| path)
}

pub(crate) fn is_cover_sidecar(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    SIDECAR_NAMES
        .iter()
        .any(|candidate| stem.eq_ignore_ascii_case(candidate))
        && normalized_extension(path).is_some()
}

fn normalized_extension(path: &Path) -> Option<&'static str> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(normalize_extension)
}

fn normalize_extension(extension: &str) -> Option<&'static str> {
    match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("jpg"),
        "png" => Some("png"),
        "webp" => Some("webp"),
        "gif" => Some("gif"),
        "bmp" => Some("bmp"),
        "tif" | "tiff" => Some("tif"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lofty::picture::{MimeType, Picture};
    use lofty::tag::TagType;

    use super::*;

    #[test]
    fn front_cover_is_preferred_across_tags() {
        let mut primary = Tag::new(TagType::Id3v2);
        primary.push_picture(
            Picture::unchecked(b"other".to_vec())
                .pic_type(PictureType::Other)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
        let mut secondary = Tag::new(TagType::VorbisComments);
        secondary.push_picture(
            Picture::unchecked(b"front".to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Png)
                .build(),
        );
        let tags = [primary, secondary];

        let selected = select_picture(Some(&tags[0]), &tags).unwrap();

        assert!(selected.front);
        assert_eq!(selected.picture.data(), b"front");
    }

    #[test]
    fn front_cover_from_a_later_track_beats_an_earlier_fallback() {
        let mut fallback = None;
        let first = prefer_embedded_cover(
            &mut fallback,
            EmbeddedCover {
                path: PathBuf::from("first-other.jpg"),
                front: false,
            },
        );
        let second = prefer_embedded_cover(
            &mut fallback,
            EmbeddedCover {
                path: PathBuf::from("second-front.png"),
                front: true,
            },
        );

        assert_eq!(first, None);
        assert_eq!(fallback, Some(PathBuf::from("first-other.jpg")));
        assert_eq!(second, Some(PathBuf::from("second-front.png")));
    }

    #[test]
    fn sidecar_cache_is_versioned_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("album");
        let cache_dir = dir.path().join("cache");
        std::fs::create_dir(&music).unwrap();
        let track = music.join("01.flac");
        let sidecar = music.join("cover.jpg");
        std::fs::write(&track, b"invalid audio is enough for sidecar fallback").unwrap();
        std::fs::write(&sidecar, b"first cover").unwrap();
        let cache = CoverCache::new(&cache_dir).unwrap();

        let first = cache
            .cover_for_album(std::slice::from_ref(&track))
            .unwrap()
            .unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), b"first cover");
        assert_eq!(
            cache.cover_for_album(std::slice::from_ref(&track)).unwrap(),
            Some(first.clone())
        );

        std::thread::sleep(Duration::from_millis(2));
        std::fs::write(&sidecar, b"second cover with a new revision").unwrap();
        let second = cache.cover_for_album(&[track]).unwrap().unwrap();

        assert_ne!(first, second);
        assert_eq!(
            std::fs::read(second).unwrap(),
            b"second cover with a new revision"
        );
        assert!(!first.exists());
    }

    #[test]
    fn cover_name_has_priority_over_folder_name() {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("album");
        std::fs::create_dir(&music).unwrap();
        let track = music.join("01.flac");
        std::fs::write(&track, b"audio").unwrap();
        std::fs::write(music.join("folder.png"), b"folder").unwrap();
        std::fs::write(music.join("COVER.JPEG"), b"cover").unwrap();
        let cache = CoverCache::new(&dir.path().join("cache")).unwrap();

        let cover = cache.cover_for_album(&[track]).unwrap().unwrap();

        assert_eq!(std::fs::read(cover).unwrap(), b"cover");
    }

    #[test]
    fn embedded_picture_magic_supports_webp() {
        let data = b"RIFF\x04\x00\x00\x00WEBPVP8 ";
        assert_eq!(sniff_extension(data), Some("webp"));
    }
}
