//! 备份：导出 / 导入，换电脑用。
//!
//! 只导出本程序自己的记录（游玩、去过的世界、遇到的人、照片记录、世界名），
//! 不含 VRChat 原始日志，也不含封面图片（导入后会自动重新取）。
//!
//! 文件格式（.yymb）：`YYMB` + 版本(1 字节) + 随机数(8 字节) + 内容。
//! 内容 = JSON → deflate 压缩 → 用随机数做种子的伪随机流异或打乱。
//! 打乱只是为了让文件不是明文，不是加密。

use crate::store::Store;
use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const MAGIC: &[u8; 4] = b"YYMB";
const FORMAT: u8 = 1;

#[derive(Debug)]
pub enum BackupError {
    NotBackup,
    UnsupportedVersion(u8),
    Corrupt(String),
    Db(rusqlite::Error),
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupError::NotBackup => write!(f, "不是 yyMemoir 备份文件"),
            BackupError::UnsupportedVersion(v) => write!(f, "备份版本 {v} 太新，请先更新 yyMemoir"),
            BackupError::Corrupt(e) => write!(f, "备份文件损坏：{e}"),
            BackupError::Db(e) => write!(f, "数据库错误：{e}"),
        }
    }
}

impl From<rusqlite::Error> for BackupError {
    fn from(e: rusqlite::Error) -> Self {
        BackupError::Db(e)
    }
}

/// 导入方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// 合并：保留本机记录，只加入备份里本机没有的游玩
    Merge,
    /// 覆盖：清空本机的游玩记录，换成备份里的
    Replace,
}

#[derive(Debug, Default, Serialize)]
pub struct BackupSummary {
    pub sessions: usize,
    pub visits: usize,
    pub photos: usize,
    pub bytes: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    /// 新加入的游玩次数
    pub added: usize,
    /// 本机已有、跳过的游玩次数（合并时）
    pub skipped: usize,
}

#[derive(Serialize, Deserialize)]
struct Dump {
    exported_at: i64,
    log_files: Vec<LogFileRow>,
    sessions: Vec<SessionRow>,
    loose_photos: Vec<PhotoRow>,
    worlds: Vec<WorldRow>,
    users: Vec<UserRow>,
}

#[derive(Serialize, Deserialize)]
struct LogFileRow {
    path: String,
    size: i64,
    read_offset: i64,
    first_seen: i64,
    last_event_at: Option<i64>,
    self_user_id: Option<String>,
    self_name: Option<String>,
    finalized: i64,
    total_lines: i64,
    info_lines: i64,
}

#[derive(Serialize, Deserialize)]
struct SessionRow {
    start_at: i64,
    end_at: i64,
    log_file: String,
    visits: Vec<VisitRow>,
}

#[derive(Serialize, Deserialize)]
struct VisitRow {
    world_id: String,
    instance_id: String,
    access_type: String,
    world_name: Option<String>,
    join_at: i64,
    leave_at: Option<i64>,
    encounters: Vec<EncounterRow>,
    photos: Vec<PhotoRow>,
}

#[derive(Serialize, Deserialize)]
struct EncounterRow {
    user_id: String,
    display_name: String,
    join_at: i64,
    leave_at: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct PhotoRow {
    path: String,
    taken_at: i64,
}

#[derive(Serialize, Deserialize)]
struct WorldRow {
    world_id: String,
    name: Option<String>,
    author: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct UserRow {
    user_id: String,
    display_name: Option<String>,
    is_friend: i64,
}

/// xorshift64* 伪随机流，异或打乱 / 还原（同一个函数）
fn scramble(data: &mut [u8], seed: u64) {
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    if x == 0 {
        x = 1;
    }
    for chunk in data.chunks_mut(8) {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        let k = x.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes();
        for (b, k) in chunk.iter_mut().zip(k) {
            *b ^= k;
        }
    }
}

fn read_dump(store: &Store) -> rusqlite::Result<Dump> {
    let c = &store.conn;
    let log_files = c
        .prepare(
            "SELECT path, size, read_offset, first_seen, last_event_at, self_user_id, self_name, finalized, total_lines, info_lines FROM log_files",
        )?
        .query_map([], |r| {
            Ok(LogFileRow {
                path: r.get(0)?,
                size: r.get(1)?,
                read_offset: r.get(2)?,
                first_seen: r.get(3)?,
                last_event_at: r.get(4)?,
                self_user_id: r.get(5)?,
                self_name: r.get(6)?,
                finalized: r.get(7)?,
                total_lines: r.get(8)?,
                info_lines: r.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut enc_st = c.prepare("SELECT user_id, display_name, join_at, leave_at FROM encounters WHERE visit_id = ?1")?;
    let mut photo_st = c.prepare("SELECT path, taken_at FROM photos WHERE visit_id = ?1")?;
    let mut visit_st = c.prepare(
        "SELECT id, world_id, instance_id, access_type, world_name, join_at, leave_at FROM visits WHERE session_id = ?1 ORDER BY join_at",
    )?;
    let raw_sessions: Vec<(i64, i64, i64, String)> = c
        .prepare("SELECT id, start_at, end_at, log_file FROM sessions ORDER BY start_at")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut sessions = Vec::with_capacity(raw_sessions.len());
    for (sid, start_at, end_at, log_file) in raw_sessions {
        let raw_visits: Vec<(i64, VisitRow)> = visit_st
            .query_map([sid], |r| {
                Ok((
                    r.get(0)?,
                    VisitRow {
                        world_id: r.get(1)?,
                        instance_id: r.get(2)?,
                        access_type: r.get(3)?,
                        world_name: r.get(4)?,
                        join_at: r.get(5)?,
                        leave_at: r.get(6)?,
                        encounters: vec![],
                        photos: vec![],
                    },
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut visits = Vec::with_capacity(raw_visits.len());
        for (vid, mut v) in raw_visits {
            v.encounters = enc_st
                .query_map([vid], |r| {
                    Ok(EncounterRow { user_id: r.get(0)?, display_name: r.get(1)?, join_at: r.get(2)?, leave_at: r.get(3)? })
                })?
                .collect::<rusqlite::Result<_>>()?;
            v.photos = photo_st
                .query_map([vid], |r| Ok(PhotoRow { path: r.get(0)?, taken_at: r.get(1)? }))?
                .collect::<rusqlite::Result<_>>()?;
            visits.push(v);
        }
        sessions.push(SessionRow { start_at, end_at, log_file, visits });
    }
    let loose_photos = c
        .prepare("SELECT path, taken_at FROM photos WHERE visit_id IS NULL")?
        .query_map([], |r| Ok(PhotoRow { path: r.get(0)?, taken_at: r.get(1)? }))?
        .collect::<rusqlite::Result<_>>()?;
    let worlds = c
        .prepare("SELECT world_id, name, author FROM worlds_cache WHERE name IS NOT NULL")?
        .query_map([], |r| Ok(WorldRow { world_id: r.get(0)?, name: r.get(1)?, author: r.get(2)? }))?
        .collect::<rusqlite::Result<_>>()?;
    let users = c
        .prepare("SELECT user_id, display_name, is_friend FROM users_cache")?
        .query_map([], |r| Ok(UserRow { user_id: r.get(0)?, display_name: r.get(1)?, is_friend: r.get(2)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Dump { exported_at: chrono::Utc::now().timestamp_millis(), log_files, sessions, loose_photos, worlds, users })
}

/// 导出成 .yymb 文件内容
pub fn export(store: &Store) -> Result<(Vec<u8>, BackupSummary), BackupError> {
    let dump = read_dump(store)?;
    let json = serde_json::to_vec(&dump).map_err(|e| BackupError::Corrupt(e.to_string()))?;
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&json).map_err(|e| BackupError::Corrupt(e.to_string()))?;
    let mut body = enc.finish().map_err(|e| BackupError::Corrupt(e.to_string()))?;
    let seed = (chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64) ^ (body.len() as u64).rotate_left(17);
    scramble(&mut body, seed);
    let mut out = Vec::with_capacity(13 + body.len());
    out.extend_from_slice(MAGIC);
    out.push(FORMAT);
    out.extend_from_slice(&seed.to_le_bytes());
    out.extend_from_slice(&body);
    let summary = BackupSummary {
        sessions: dump.sessions.len(),
        visits: dump.sessions.iter().map(|s| s.visits.len()).sum(),
        photos: dump.sessions.iter().flat_map(|s| &s.visits).map(|v| v.photos.len()).sum::<usize>() + dump.loose_photos.len(),
        bytes: out.len(),
    };
    Ok((out, summary))
}

fn decode(bytes: &[u8]) -> Result<Dump, BackupError> {
    if bytes.len() < 13 || &bytes[..4] != MAGIC {
        return Err(BackupError::NotBackup);
    }
    if bytes[4] > FORMAT {
        return Err(BackupError::UnsupportedVersion(bytes[4]));
    }
    let seed = u64::from_le_bytes(bytes[5..13].try_into().unwrap());
    let mut body = bytes[13..].to_vec();
    scramble(&mut body, seed);
    let mut json = Vec::new();
    DeflateDecoder::new(&body[..]).read_to_end(&mut json).map_err(|e| BackupError::Corrupt(e.to_string()))?;
    serde_json::from_slice(&json).map_err(|e| BackupError::Corrupt(e.to_string()))
}

/// 导入 .yymb 文件内容
pub fn import(store: &mut Store, bytes: &[u8], mode: ImportMode) -> Result<ImportReport, BackupError> {
    let dump = decode(bytes)?;
    let tx = store.conn.transaction()?;
    if mode == ImportMode::Replace {
        tx.execute_batch("DELETE FROM encounters; DELETE FROM photos; DELETE FROM visits; DELETE FROM sessions; DELETE FROM log_files;")?;
    }
    for f in &dump.log_files {
        tx.execute(
            "INSERT OR IGNORE INTO log_files(path, size, read_offset, first_seen, last_event_at, self_user_id, self_name, finalized, total_lines, info_lines)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![f.path, f.size, f.read_offset, f.first_seen, f.last_event_at, f.self_user_id, f.self_name, f.finalized, f.total_lines, f.info_lines],
        )?;
    }
    let mut rep = ImportReport::default();
    for s in &dump.sessions {
        let exists: Option<i64> =
            tx.query_row("SELECT id FROM sessions WHERE log_file = ?1", [&s.log_file], |r| r.get(0)).optional()?;
        if exists.is_some() {
            rep.skipped += 1;
            continue;
        }
        tx.execute("INSERT INTO sessions(start_at, end_at, log_file) VALUES (?1,?2,?3)", params![s.start_at, s.end_at, s.log_file])?;
        let sid = tx.last_insert_rowid();
        for v in &s.visits {
            tx.execute(
                "INSERT INTO visits(session_id, world_id, instance_id, access_type, world_name, join_at, leave_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![sid, v.world_id, v.instance_id, v.access_type, v.world_name, v.join_at, v.leave_at],
            )?;
            let vid = tx.last_insert_rowid();
            for e in &v.encounters {
                tx.execute(
                    "INSERT INTO encounters(visit_id, user_id, display_name, join_at, leave_at) VALUES (?1,?2,?3,?4,?5)",
                    params![vid, e.user_id, e.display_name, e.join_at, e.leave_at],
                )?;
            }
            for p in &v.photos {
                tx.execute("INSERT OR IGNORE INTO photos(path, taken_at, visit_id) VALUES (?1,?2,?3)", params![p.path, p.taken_at, vid])?;
            }
        }
        rep.added += 1;
    }
    for p in &dump.loose_photos {
        tx.execute("INSERT OR IGNORE INTO photos(path, taken_at) VALUES (?1,?2)", params![p.path, p.taken_at])?;
    }
    for w in &dump.worlds {
        tx.execute(
            "INSERT INTO worlds_cache(world_id, name, author) VALUES (?1,?2,?3)
             ON CONFLICT(world_id) DO UPDATE SET name = COALESCE(worlds_cache.name, excluded.name), author = COALESCE(worlds_cache.author, excluded.author)",
            params![w.world_id, w.name, w.author],
        )?;
    }
    for u in &dump.users {
        tx.execute(
            "INSERT OR IGNORE INTO users_cache(user_id, display_name, is_friend) VALUES (?1,?2,?3)",
            params![u.user_id, u.display_name, u.is_friend],
        )?;
    }
    tx.commit()?;
    Ok(rep)
}
