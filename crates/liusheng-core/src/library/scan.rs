//! Filesystem traversal and metadata parsing are separate from the SQLite writer.
//! Plans contain scoped file-state snapshots; output is bounded and cancellable.
use super::{ScanStats, file_state, is_audio_path, tags};
use crate::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) type FileState = (i64, i64);
pub(super) type MetadataReader =
    std::sync::Arc<dyn Fn(&Path) -> Result<tags::TrackMeta> + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScanKind {
    Full,
    Changes,
    Import,
}

pub(crate) struct Scope {
    pub path: PathBuf,
    pub root: PathBuf,
    pub known: HashMap<String, FileState>,
}

pub(crate) struct Plan {
    pub scopes: Vec<Scope>,
    pub excluded: Vec<PathBuf>,
    pub kind: ScanKind,
}

pub(crate) struct Prepared {
    pub path: String,
    pub state: FileState,
    pub expected: Option<FileState>,
    pub meta: tags::TrackMeta,
    pub import: bool,
}

pub(crate) struct Removal {
    pub path: String,
    pub root: PathBuf,
    pub expected: FileState,
}

#[derive(Default)]
pub(crate) struct Batch {
    pub prepared: Vec<Prepared>,
    pub removed: Vec<Removal>,
    pub opened: Vec<String>,
    pub stats: ScanStats,
}

fn check_cancel(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        Err(Error::Interrupted)
    } else {
        Ok(())
    }
}

fn flush(batch: &mut Batch, emit: &mut impl FnMut(Batch) -> Result<()>) -> Result<()> {
    if batch.stats.visited > 0
        || batch.stats.failed > 0
        || batch.stats.state_rows > 0
        || !batch.prepared.is_empty()
        || !batch.removed.is_empty()
        || !batch.opened.is_empty()
    {
        emit(std::mem::take(batch))?;
    }
    Ok(())
}

fn prepare(
    path: &Path,
    scope: &Scope,
    import: bool,
    batch: &mut Batch,
    reader: &dyn Fn(&Path) -> Result<tags::TrackMeta>,
) {
    batch.stats.visited += 1;
    let key = path.to_string_lossy().into_owned();
    if import {
        batch.opened.push(key.clone());
    }
    let before = match file_state(path) {
        Ok(state) => state,
        Err(error) => {
            batch.stats.failure(path, error);
            return;
        }
    };
    let expected = scope.known.get(&key).copied();
    if expected == Some(before) {
        batch.stats.unchanged += 1;
        return;
    }
    match reader(path) {
        Ok(meta) if file_state(path).ok() == Some(before) => {
            batch.prepared.push(Prepared {
                path: key,
                state: before,
                expected,
                meta,
                import,
            });
        }
        Ok(_) => batch
            .stats
            .failure(path, "文件仍在写入，将在下一次更新重试"),
        Err(error) => batch.stats.failure(path, error),
    }
}

/// Executes on a filesystem worker, or synchronously for the standalone Library
/// API. The callback owns database writes and must also honor cancellation.
pub(crate) fn execute(
    plan: Plan,
    cancelled: &AtomicBool,
    emit: impl FnMut(Batch) -> Result<()>,
) -> Result<()> {
    execute_with_reader(plan, cancelled, emit, &tags::read_meta)
}

pub(super) fn execute_with_reader(
    plan: Plan,
    cancelled: &AtomicBool,
    mut emit: impl FnMut(Batch) -> Result<()>,
    reader: &dyn Fn(&Path) -> Result<tags::TrackMeta>,
) -> Result<()> {
    let mut batch = Batch::default();
    for scope in plan.scopes {
        check_cancel(cancelled)?;
        let import = plan.kind == ScanKind::Import;
        batch.stats.state_rows += scope.known.len();
        if !import && !scope.root.is_dir() {
            batch
                .stats
                .failure(&scope.root, "音乐目录离线或不可读，保留缓存");
            flush(&mut batch, &mut emit)?;
            continue;
        }
        if plan.excluded.iter().any(|p| scope.path.starts_with(p)) {
            continue;
        }
        let mut seen = HashSet::new();
        let mut failed = Vec::new();
        let metadata = std::fs::metadata(&scope.path);
        let directory = metadata.as_ref().is_ok_and(|m| m.is_dir());
        let missing = metadata
            .as_ref()
            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
        if directory {
            let entries = walkdir::WalkDir::new(&scope.path)
                .follow_links(true)
                .sort_by_file_name()
                .into_iter()
                .filter_entry(|e| !plan.excluded.iter().any(|p| e.path().starts_with(p)));
            for entry in entries {
                check_cancel(cancelled)?;
                match entry {
                    Ok(entry) if entry.file_type().is_file() && is_audio_path(entry.path()) => {
                        let path = entry.path();
                        seen.insert(path.to_string_lossy().into_owned());
                        prepare(path, &scope, import, &mut batch, reader);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        let path = error.path().unwrap_or(&scope.path).to_owned();
                        batch.stats.failure(&path, &error);
                        failed.push(path);
                    }
                }
                // Unchanged libraries still yield regularly to service commands.
                if batch.stats.visited >= 128 || batch.stats.failed >= 32 {
                    check_cancel(cancelled)?;
                    flush(&mut batch, &mut emit)?;
                }
            }
        } else if metadata.as_ref().is_ok_and(|m| m.is_file()) {
            if is_audio_path(&scope.path) {
                seen.insert(scope.path.to_string_lossy().into_owned());
                prepare(&scope.path, &scope, import, &mut batch, reader);
            } else if import
                && scope.path.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8")
                })
            {
                // M3U entries stay in their original order, including duplicates.
                match super::playlists::read_m3u(&scope.path) {
                    Ok(paths) => {
                        for path in paths {
                            check_cancel(cancelled)?;
                            let path = PathBuf::from(path);
                            if is_audio_path(&path) {
                                prepare(&path, &scope, true, &mut batch, reader);
                            }
                            if batch.stats.visited >= 128 {
                                flush(&mut batch, &mut emit)?;
                            }
                        }
                    }
                    Err(error) => batch.stats.failure(&scope.path, error),
                }
            }
        } else if !missing || plan.kind == ScanKind::Full || import {
            batch.stats.failure(
                &scope.path,
                metadata
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "路径类型不可扫描".into()),
            );
            failed.push(scope.path.clone());
        }
        check_cancel(cancelled)?;
        // Only a complete, still-online scope can confirm deletions. A failed
        // subtree, an excluded path, and an offline mount keep their records.
        if !import && scope.root.is_dir() && (directory || missing) {
            for (path, expected) in &scope.known {
                check_cancel(cancelled)?;
                let p = Path::new(path);
                if !seen.contains(path)
                    && !plan.excluded.iter().any(|e| p.starts_with(e))
                    && !failed.iter().any(|e| p.starts_with(e))
                {
                    batch.removed.push(Removal {
                        path: path.clone(),
                        root: scope.root.clone(),
                        expected: *expected,
                    });
                    if batch.removed.len() >= 128 {
                        flush(&mut batch, &mut emit)?;
                    }
                }
            }
        }
        check_cancel(cancelled)?;
        flush(&mut batch, &mut emit)?;
    }
    Ok(())
}

/// Lexical component-aware compaction. File paths cannot swallow similarly
/// prefixed sibling names, and ancestor notifications retain their full scope.
pub(crate) fn minimal_scopes(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut paths = paths.to_vec();
    paths.sort();
    paths.dedup();
    let mut result: Vec<PathBuf> = Vec::new();
    for path in paths {
        if !result.iter().any(|parent| path.starts_with(parent)) {
            result.push(path);
        }
    }
    result
}
