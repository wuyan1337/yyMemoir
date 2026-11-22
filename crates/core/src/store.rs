//! SQLite 存储。监听程序写、主程序读，开 WAL 互不阻塞。所有时间都是 UTC 毫秒。

use rusqlite::{Connection, Result};
use std::path::Path;

pub struct Store {
    pub conn: Connection,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS log_files (
    path          TEXT PRIMARY KEY,
    size          INTEGER NOT NULL DEFAULT 0,
    read_offset   INTEGER NOT NULL DEFAULT 0,
    first_seen    INTEGER NOT NULL,
    last_event_at INTEGER,
    self_user_id  TEXT,
    self_name     TEXT,
    pending_world_name TEXT,
    finalized     INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS sessions (
    id       INTEGER PRIMARY KEY,
    start_at INTEGER NOT NULL,
    end_at   INTEGER NOT NULL,
    log_file TEXT NOT NULL UNIQUE REFERENCES log_files(path)
);
CREATE TABLE IF NOT EXISTS visits (
    id          INTEGER PRIMARY KEY,
    session_id  INTEGER NOT NULL REFERENCES sessions(id),
    world_id    TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    access_type TEXT NOT NULL,
    world_name  TEXT,
    join_at     INTEGER NOT NULL,
    leave_at    INTEGER
);
CREATE TABLE IF NOT EXISTS encounters (
    id           INTEGER PRIMARY KEY,
    visit_id     INTEGER NOT NULL REFERENCES visits(id),
    user_id      TEXT NOT NULL,
    display_name TEXT NOT NULL,
    join_at      INTEGER NOT NULL,
    leave_at     INTEGER
);
CREATE TABLE IF NOT EXISTS photos (
    path     TEXT PRIMARY KEY,
    taken_at INTEGER NOT NULL,
    visit_id INTEGER REFERENCES visits(id)
);
CREATE TABLE IF NOT EXISTS worlds_cache (
    world_id       TEXT PRIMARY KEY,
    name           TEXT,
    author         TEXT,
    thumbnail_path TEXT,
    fetched_at     INTEGER
);
CREATE TABLE IF NOT EXISTS users_cache (
    user_id      TEXT PRIMARY KEY,
    display_name TEXT,
    avatar_path  TEXT,
    is_friend    INTEGER NOT NULL DEFAULT 0,
    fetched_at   INTEGER
);
CREATE INDEX IF NOT EXISTS idx_visits_join ON visits(join_at);
CREATE INDEX IF NOT EXISTS idx_visits_session ON visits(session_id, leave_at);
CREATE INDEX IF NOT EXISTS idx_enc_user ON encounters(user_id);
CREATE INDEX IF NOT EXISTS idx_enc_visit ON encounters(visit_id, user_id);
CREATE INDEX IF NOT EXISTS idx_photos_taken ON photos(taken_at);
"#;

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }
}
