use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, Row, params};

use crate::error::Result;
use crate::library::pinyin::search_blob;
use crate::library::tags::TrackMeta;

#[derive(Debug, Clone, PartialEq)]
pub struct TrackRow {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub genre: String,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub bit_depth: Option<u8>,
    pub channels: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumKey {
    pub album: String,
    pub album_artist: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumSummary {
    pub key: AlbumKey,
    pub title: String,
    pub artist: String,
    pub track_count: u32,
    pub year: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistSummary {
    pub key: String,
    pub name: String,
    pub track_count: u32,
    pub album_count: u32,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS tracks (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  mtime INTEGER NOT NULL,
  title TEXT NOT NULL,
  artist TEXT NOT NULL DEFAULT '',
  album TEXT NOT NULL DEFAULT '',
  album_artist TEXT NOT NULL DEFAULT '',
  track_no INTEGER,
  disc_no INTEGER,
  year INTEGER,
  genre TEXT NOT NULL DEFAULT '',
  duration_ms INTEGER NOT NULL DEFAULT 0,
  sample_rate INTEGER NOT NULL DEFAULT 0,
  bit_depth INTEGER,
  channels INTEGER NOT NULL DEFAULT 2,
  title_search TEXT NOT NULL DEFAULT '',
  artist_search TEXT NOT NULL DEFAULT '',
  album_search TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_artist, album, disc_no, track_no);
";

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

/// 现有全部 path -> 纳秒级 mtime，供增量扫描比对。
pub fn path_mtimes(conn: &Connection) -> Result<HashMap<String, i64>> {
    let mut stmt = conn.prepare("SELECT path, mtime FROM tracks")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    let mut map = HashMap::new();
    for row in rows {
        let (p, m) = row?;
        map.insert(p, m);
    }
    Ok(map)
}

pub fn upsert_track(conn: &Connection, path: &str, mtime: i64, meta: &TrackMeta) -> Result<()> {
    let artist_search = format!(
        "{}\n{}",
        search_blob(&meta.artist),
        search_blob(&meta.album_artist)
    );
    conn.execute(
        "INSERT INTO tracks (path, mtime, title, artist, album, album_artist,
                             track_no, disc_no, year, genre, duration_ms,
                             sample_rate, bit_depth, channels,
                             title_search, artist_search, album_search)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
         ON CONFLICT(path) DO UPDATE SET
           mtime=excluded.mtime, title=excluded.title, artist=excluded.artist,
           album=excluded.album, album_artist=excluded.album_artist,
           track_no=excluded.track_no, disc_no=excluded.disc_no, year=excluded.year,
           genre=excluded.genre, duration_ms=excluded.duration_ms,
           sample_rate=excluded.sample_rate, bit_depth=excluded.bit_depth,
           channels=excluded.channels, title_search=excluded.title_search,
           artist_search=excluded.artist_search, album_search=excluded.album_search",
        params![
            path,
            mtime,
            meta.title,
            meta.artist,
            meta.album,
            meta.album_artist,
            meta.track_no,
            meta.disc_no,
            meta.year,
            meta.genre,
            meta.duration_ms as i64,
            meta.sample_rate,
            meta.bit_depth,
            meta.channels,
            search_blob(&meta.title),
            artist_search,
            search_blob(&meta.album),
        ],
    )?;
    Ok(())
}

pub fn delete_by_path(conn: &Connection, path: &str) -> Result<()> {
    conn.execute("DELETE FROM tracks WHERE path = ?1", params![path])?;
    Ok(())
}

fn row_to_track(r: &Row<'_>) -> rusqlite::Result<TrackRow> {
    Ok(TrackRow {
        id: r.get("id")?,
        path: r.get("path")?,
        title: r.get("title")?,
        artist: r.get("artist")?,
        album: r.get("album")?,
        album_artist: r.get("album_artist")?,
        track_no: r.get("track_no")?,
        disc_no: r.get("disc_no")?,
        year: r.get("year")?,
        genre: r.get("genre")?,
        duration_ms: r.get::<_, i64>("duration_ms")? as u64,
        sample_rate: r.get("sample_rate")?,
        bit_depth: r.get("bit_depth")?,
        channels: r.get("channels")?,
    })
}

const TRACK_COLS: &str = "id, path, title, artist, album, album_artist, track_no, disc_no,
                          year, genre, duration_ms, sample_rate, bit_depth, channels";

pub fn all_tracks(conn: &Connection) -> Result<Vec<TrackRow>> {
    let sql = format!(
        "SELECT {TRACK_COLS} FROM tracks ORDER BY album_artist, album, disc_no, track_no, title"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_track)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// query 需已 normalize。三个检索列上做 LIKE 子串匹配。
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<TrackRow>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{escaped}%");
    let sql = format!(
        "SELECT {TRACK_COLS} FROM tracks
         WHERE title_search LIKE ?1 ESCAPE '\\'
            OR artist_search LIKE ?1 ESCAPE '\\'
            OR album_search LIKE ?1 ESCAPE '\\'
         ORDER BY album_artist, album, disc_no, track_no, title
         LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![pattern, limit as i64], row_to_track)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn track_count(conn: &Connection) -> Result<u64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, i64>(0))? as u64)
}

pub fn albums(conn: &Connection) -> Result<Vec<AlbumSummary>> {
    let mut stmt = conn.prepare(
        "SELECT
           album AS source_album,
           album_artist AS source_album_artist,
           CASE WHEN trim(album) = '' THEN '未知专辑' ELSE album END AS display_album,
           CASE
             WHEN trim(album_artist) != '' THEN album_artist
             WHEN COUNT(DISTINCT CASE WHEN trim(artist) != '' THEN artist END) = 1
               THEN MAX(CASE WHEN trim(artist) != '' THEN artist END)
             WHEN COUNT(DISTINCT CASE WHEN trim(artist) != '' THEN artist END) = 0
               THEN '未知艺术家'
             ELSE '多位艺术家'
           END AS display_artist,
           COUNT(*) AS track_count,
           MIN(year) AS year
         FROM tracks
         GROUP BY source_album, source_album_artist
         ORDER BY display_album COLLATE NOCASE, display_artist COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(AlbumSummary {
            key: AlbumKey {
                album: row.get("source_album")?,
                album_artist: row.get("source_album_artist")?,
            },
            title: row.get("display_album")?,
            artist: row.get("display_artist")?,
            track_count: row.get::<_, i64>("track_count")? as u32,
            year: row.get("year")?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn artists(conn: &Connection) -> Result<Vec<ArtistSummary>> {
    let mut stmt = conn.prepare(
        "SELECT
           artist AS source_artist,
           CASE WHEN trim(artist) = '' THEN '未知艺术家' ELSE artist END AS display_artist,
           COUNT(*) AS track_count,
           COUNT(DISTINCT album || char(0) || album_artist) AS album_count
         FROM tracks
         GROUP BY source_artist
         ORDER BY display_artist COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ArtistSummary {
            key: row.get("source_artist")?,
            name: row.get("display_artist")?,
            track_count: row.get::<_, i64>("track_count")? as u32,
            album_count: row.get::<_, i64>("album_count")? as u32,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(album: &str, artist: &str, album_artist: &str, year: Option<u32>) -> TrackMeta {
        TrackMeta {
            title: "曲目".into(),
            album: album.into(),
            artist: artist.into(),
            album_artist: album_artist.into(),
            year,
            ..Default::default()
        }
    }

    #[test]
    fn albums_group_tracks_and_use_album_artist() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        upsert_track(
            &conn,
            "/a.flac",
            1,
            &meta("江南", "林俊杰", "林俊杰", Some(2004)),
        )
        .unwrap();
        upsert_track(
            &conn,
            "/b.flac",
            1,
            &meta("江南", "林俊杰", "林俊杰", Some(2004)),
        )
        .unwrap();

        let rows = albums(&conn).unwrap();
        assert_eq!(
            rows,
            vec![AlbumSummary {
                key: AlbumKey {
                    album: "江南".into(),
                    album_artist: "林俊杰".into(),
                },
                title: "江南".into(),
                artist: "林俊杰".into(),
                track_count: 2,
                year: Some(2004),
            }]
        );
    }

    #[test]
    fn albums_label_compilations_and_missing_tags() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        upsert_track(&conn, "/a.flac", 1, &meta("合辑", "甲", "", None)).unwrap();
        upsert_track(&conn, "/b.flac", 1, &meta("合辑", "乙", "", None)).unwrap();
        upsert_track(&conn, "/c.flac", 1, &meta("", "", "", None)).unwrap();

        let rows = albums(&conn).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|album| {
            album.title == "合辑" && album.artist == "多位艺术家" && album.track_count == 2
        }));
        assert!(rows.iter().any(|album| {
            album.title == "未知专辑" && album.artist == "未知艺术家" && album.track_count == 1
        }));
    }

    #[test]
    fn artists_group_tracks_and_count_albums() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        upsert_track(
            &conn,
            "/a.flac",
            1,
            &meta("第一张", "林俊杰", "林俊杰", None),
        )
        .unwrap();
        upsert_track(
            &conn,
            "/b.flac",
            1,
            &meta("第二张", "林俊杰", "林俊杰", None),
        )
        .unwrap();
        upsert_track(
            &conn,
            "/c.flac",
            1,
            &meta("第二张", "林俊杰", "林俊杰", None),
        )
        .unwrap();
        upsert_track(&conn, "/unknown.flac", 1, &meta("", "", "", None)).unwrap();

        let rows = artists(&conn).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|artist| {
            artist.name == "林俊杰" && artist.track_count == 3 && artist.album_count == 2
        }));
        assert!(rows.iter().any(|artist| {
            artist.name == "未知艺术家" && artist.track_count == 1 && artist.album_count == 1
        }));
    }
}
