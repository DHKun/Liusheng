-- Exact tracks schema from repository tag v0.1.0; original user_version=0.
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
