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
    finalized     INTEGER NOT NULL DEFAULT 0,
    total_lines   INTEGER NOT NULL DEFAULT 0,
    info_lines    INTEGER NOT NULL DEFAULT 0
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
        migrate(&conn)?;
        Ok(Store { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }
}

/// 老数据库补列
fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        // v1：照片改为只认日志里「VRC Camera」记录的；旧版按文件夹扫进来的（可能混进别的截图）清掉
        conn.execute_batch("DELETE FROM photos; PRAGMA user_version = 1;")?;
    }
    if version < 2 {
        repair_self(conn)?;
        conn.execute_batch("PRAGMA user_version = 2;")?;
    }
    for col in ["total_lines", "info_lines"] {
        let has: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('log_files') WHERE name = ?1")?
            .exists([col])?;
        if !has {
            conn.execute_batch(&format!("ALTER TABLE log_files ADD COLUMN {col} INTEGER NOT NULL DEFAULT 0"))?;
        }
    }
    Ok(())
}

/// 「仅错误」模式的日志：写了至少 `min_lines` 行带时间戳的内容，却没有一条非 Error 的行。
/// 返回这些文件路径，app 和通知里据此提醒用户把 VRChat 设置 → Debug → 日志 改成「完整」。
pub fn errors_only_logs(store: &Store, min_lines: i64) -> Result<Vec<String>> {
    let mut st = store
        .conn
        .prepare("SELECT path FROM log_files WHERE info_lines = 0 AND total_lines >= ?1 ORDER BY path")?;
    let rows = st.query_map([min_lines], |r| r.get(0))?;
    rows.collect()
}

/// v2 修复：旧版有时没认出本人，把自己算成了「遇到的人」。
/// 只按已知的本人名字修：同名的记录就是本人，补上 ID 后删掉。
/// 不知道本人是谁时不猜（按「出现次数最多」猜会把整晚一起待在一个房间的朋友误删）。
fn repair_self(conn: &Connection) -> Result<()> {
    let names: Vec<String> = conn
        .prepare("SELECT DISTINCT self_name FROM log_files WHERE self_name IS NOT NULL")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_>>()?;
    let mut ids: Vec<String> = Vec::new();
    for n in &names {
        let found: Vec<String> = conn
            .prepare("SELECT DISTINCT user_id FROM encounters WHERE display_name = ?1 AND user_id LIKE 'usr_%'")?
            .query_map([n], |r| r.get(0))?
            .collect::<Result<_>>()?;
        ids.extend(found);
    }
    for id in &ids {
        conn.execute("UPDATE log_files SET self_user_id = ?1 WHERE self_user_id IS NULL", [id])?;
        conn.execute("DELETE FROM encounters WHERE user_id = ?1", [id])?;
    }
    for n in &names {
        conn.execute("DELETE FROM encounters WHERE display_name = ?1 AND user_id NOT LIKE 'usr_%'", [n])?;
    }
    Ok(())
}
