//! Playlists store paths independently of library availability, preserving order and duplicates.
use crate::{Error, Result};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub count: usize,
}

pub fn initialize(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS playlists(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS playlist_entries(playlist_id INTEGER NOT NULL, position INTEGER NOT NULL, path TEXT NOT NULL,
        PRIMARY KEY(playlist_id, position), FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE);")?;
    Ok(())
}
pub fn list(conn: &Connection) -> Result<Vec<Playlist>> {
    let mut stmt = conn.prepare("SELECT p.id,p.name,COUNT(e.position) FROM playlists p LEFT JOIN playlist_entries e ON e.playlist_id=p.id GROUP BY p.id ORDER BY p.id")?;
    Ok(stmt
        .query_map([], |r| {
            Ok(Playlist {
                id: r.get(0)?,
                name: r.get(1)?,
                count: r.get::<_, i64>(2)? as usize,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}
pub fn entries(conn: &Connection, id: i64) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT path FROM playlist_entries WHERE playlist_id=?1 ORDER BY position")?;
    Ok(stmt
        .query_map([id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}
pub fn save(conn: &mut Connection, name: &str, paths: &[String]) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Other("请输入歌单名称".into()));
    }
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO playlists(name) VALUES(?1)", [name])?;
    let id = tx.last_insert_rowid();
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO playlist_entries(playlist_id,position,path) VALUES(?1,?2,?3)",
        )?;
        for (i, path) in paths.iter().enumerate() {
            stmt.execute(params![id, i as i64, path])?;
        }
    }
    tx.commit()?;
    Ok(id)
}
pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(Error::Other("请输入歌单名称".into()));
    }
    conn.execute(
        "UPDATE playlists SET name=?1 WHERE id=?2",
        params![name.trim(), id],
    )?;
    Ok(())
}
pub fn delete(conn: &mut Connection, id: i64) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM playlist_entries WHERE playlist_id=?1", [id])?;
    tx.execute("DELETE FROM playlists WHERE id=?1", [id])?;
    tx.commit()?;
    Ok(())
}
pub fn read_m3u(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut result = Vec::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let item = if line.starts_with("file:") {
            url::Url::parse(line)
                .ok()
                .and_then(|u| u.to_file_path().ok())
                .ok_or_else(|| Error::Other("歌单包含无效本地 URL".into()))?
        } else if line.contains("://") {
            return Err(Error::Other("当前歌单支持本地文件路径".into()));
        } else {
            let item = PathBuf::from(line);
            if item.is_absolute() {
                item
            } else {
                base.join(item)
            }
        };
        let item = std::fs::canonicalize(&item).unwrap_or(item);
        result.push(item.to_string_lossy().into_owned());
    }
    Ok(result)
}
pub fn write_m3u(path: &Path, paths: &[String]) -> Result<()> {
    let mut text = String::from("#EXTM3U\n");
    for item in paths {
        if item.contains(['\r', '\n']) {
            return Err(Error::Other("M3U8 文件路径须位于单行".into()));
        }
        // File URLs also preserve leading '#' and trailing spaces in file names.
        let url = url::Url::from_file_path(item)
            .map_err(|_| Error::Other("导出歌单需要绝对路径".into()))?;
        text.push_str(url.as_str());
        text.push('\n');
    }
    crate::settings::atomic_write(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playlists_preserve_duplicate_entries_and_order() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let paths = vec!["/a.flac".into(), "/b.flac".into(), "/a.flac".into()];
        let id = save(&mut conn, "收藏", &paths).unwrap();
        assert_eq!(entries(&conn, id).unwrap(), paths);
        rename(&conn, id, "新名称").unwrap();
        assert_eq!(list(&conn).unwrap()[0].name, "新名称");
        delete(&mut conn, id).unwrap();
        assert!(entries(&conn, id).unwrap().is_empty());
    }
    #[test]
    fn m3u_round_trip_unicode_and_special_names() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("list.m3u8");
        let entries = vec![
            d.path()
                .join("#中文 #?.flac")
                .to_string_lossy()
                .into_owned(),
        ];
        write_m3u(&path, &entries).unwrap();
        assert_eq!(read_m3u(&path).unwrap(), entries);
    }
}
