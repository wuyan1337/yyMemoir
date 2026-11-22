//! 备份：导出 / 导入，换电脑用。
//!
//! 内容：本程序自己的记录（游玩、去过的世界、遇到的人、照片记录、世界名）+ 图片（照片、世界封面）。
//! 不含 VRChat 原始日志。
//!
//! 文件格式（.yymb）：`YYMB` + 版本(1 字节) + 随机数(8 字节) + 内容，内容整体用随机数做种子的伪随机流异或打乱
//! （只是让文件不是明文，不是加密）。
//! - v1：内容 = deflate(JSON)
//! - v2：内容 = [u64 长度][deflate(JSON)] + 若干图片条目 [u8 类型][u16 键长][键][u64 长度][数据] + [u8 0] 结束
//!   类型 1 = 照片原图，2 = 照片压缩 JPEG，3 = 世界封面 JPEG；照片的键是原路径，封面的键是世界 ID。
//! 图片一张一张流式读写，不会把整个备份读进内存。

use crate::store::Store;
use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"YYMB";
const FORMAT: u8 = 2;
const KIND_END: u8 = 0;
const KIND_PHOTO_ORIGINAL: u8 = 1;
const KIND_PHOTO_JPEG: u8 = 2;
const KIND_COVER: u8 = 3;

/// 照片怎么放进备份
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoQuality {
    /// 长边缩到 1920、JPEG 88，每张约 300–500 KB（默认）
    Compressed,
    /// 原图原样放进去（VRChat 原图一张约 5 MB）
    Original,
}

#[derive(Debug, Clone, Copy)]
pub struct ExportOptions {
    /// None = 不带照片
    pub photos: Option<PhotoQuality>,
    pub covers: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions { photos: Some(PhotoQuality::Compressed), covers: true }
    }
}

/// 导入时图片还原到哪
#[derive(Debug, Clone)]
pub struct ImportTargets {
    pub photo_dir: PathBuf,
    pub cover_dir: PathBuf,
}

#[derive(Debug)]
pub enum BackupError {
    NotBackup,
    UnsupportedVersion(u8),
    Corrupt(String),
    Io(std::io::Error),
    Db(rusqlite::Error),
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupError::NotBackup => write!(f, "不是 yyMemoir 备份文件"),
            BackupError::UnsupportedVersion(v) => write!(f, "备份版本 {v} 太新，请先更新 yyMemoir"),
            BackupError::Corrupt(e) => write!(f, "备份文件损坏：{e}"),
            BackupError::Io(e) => write!(f, "读写文件出错：{e}"),
            BackupError::Db(e) => write!(f, "数据库错误：{e}"),
        }
    }
}

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        BackupError::Io(e)
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
    /// 照片记录数
    pub photos: usize,
    /// 实际放进去的照片文件数（原图找不到的不算）
    pub photo_files: usize,
    pub covers: usize,
    pub bytes: u64,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    /// 新加入的游玩次数
    pub added: usize,
    /// 本机已有、跳过的游玩次数（合并时）
    pub skipped: usize,
    /// 还原出来的照片文件数（原路径还在的不重复写）
    pub photos_restored: usize,
    pub covers_restored: usize,
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
    /// v3 加的；旧备份没有这个字段，导入后按「还没补读模型」处理
    #[serde(default)]
    avatars_scanned: i64,
}

#[derive(Serialize, Deserialize)]
struct SessionRow {
    start_at: i64,
    end_at: i64,
    log_file: String,
    visits: Vec<VisitRow>,
    /// 本人穿的模型；旧备份没有
    #[serde(default)]
    avatars: Vec<AvatarRow>,
}

#[derive(Serialize, Deserialize)]
struct AvatarRow {
    avatar_name: String,
    avatar_id: Option<String>,
    start_at: i64,
    end_at: Option<i64>,
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

/// xorshift64* 伪随机流：异或打乱 / 还原是同一个操作，可以分段连续处理
struct Keystream {
    x: u64,
    buf: [u8; 8],
    pos: usize,
}

impl Keystream {
    fn new(seed: u64) -> Self {
        let x = seed ^ 0x9E37_79B9_7F4A_7C15;
        Keystream { x: if x == 0 { 1 } else { x }, buf: [0; 8], pos: 8 }
    }
    fn apply(&mut self, data: &mut [u8]) {
        for b in data {
            if self.pos == 8 {
                self.x ^= self.x >> 12;
                self.x ^= self.x << 25;
                self.x ^= self.x >> 27;
                self.buf = self.x.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes();
                self.pos = 0;
            }
            *b ^= self.buf[self.pos];
            self.pos += 1;
        }
    }
}

struct ScrambleWriter<W: Write> {
    inner: W,
    ks: Keystream,
    written: u64,
}

impl<W: Write> Write for ScrambleWriter<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut tmp = data.to_vec();
        self.ks.apply(&mut tmp);
        self.inner.write_all(&tmp)?;
        self.written += tmp.len() as u64;
        Ok(tmp.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

struct ScrambleReader<R: Read> {
    inner: R,
    ks: Keystream,
}

impl<R: Read> Read for ScrambleReader<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(out)?;
        self.ks.apply(&mut out[..n]);
        Ok(n)
    }
}

/// 照片缩成长边 1920 的 JPEG
fn compress_photo(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() > 1920 || img.height() > 1920 { img.thumbnail(1920, 1920) } else { img };
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88);
    img.to_rgb8().write_with_encoder(enc).ok()?;
    Some(out)
}

/// 封面统一成 JPEG（长边 1200）
fn compress_cover(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() > 1200 || img.height() > 1200 { img.thumbnail(1200, 1200) } else { img };
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
    img.to_rgb8().write_with_encoder(enc).ok()?;
    Some(out)
}

fn write_entry(w: &mut impl Write, kind: u8, key: &str, data: &[u8]) -> std::io::Result<()> {
    w.write_all(&[kind])?;
    w.write_all(&(key.len() as u16).to_le_bytes())?;
    w.write_all(key.as_bytes())?;
    w.write_all(&(data.len() as u64).to_le_bytes())?;
    w.write_all(data)
}

fn read_dump(store: &Store) -> rusqlite::Result<Dump> {
    let c = &store.conn;
    let log_files = c
        .prepare(
            "SELECT path, size, read_offset, first_seen, last_event_at, self_user_id, self_name, finalized, total_lines, info_lines, avatars_scanned FROM log_files",
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
                avatars_scanned: r.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut enc_st = c.prepare("SELECT user_id, display_name, join_at, leave_at FROM encounters WHERE visit_id = ?1")?;
    let mut photo_st = c.prepare("SELECT path, taken_at FROM photos WHERE visit_id = ?1")?;
    let mut visit_st = c.prepare(
        "SELECT id, world_id, instance_id, access_type, world_name, join_at, leave_at FROM visits WHERE session_id = ?1 ORDER BY join_at",
    )?;
    let mut avatar_st =
        c.prepare("SELECT avatar_name, avatar_id, start_at, end_at FROM avatar_wears WHERE session_id = ?1 ORDER BY start_at")?;
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
        let avatars = avatar_st
            .query_map([sid], |r| Ok(AvatarRow { avatar_name: r.get(0)?, avatar_id: r.get(1)?, start_at: r.get(2)?, end_at: r.get(3)? }))?
            .collect::<rusqlite::Result<_>>()?;
        sessions.push(SessionRow { start_at, end_at, log_file, visits, avatars });
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

/// 导出到 `out`（.yymb）。图片一张一张写，内存占用小。
pub fn export_to(store: &Store, out: impl Write, opts: ExportOptions) -> Result<BackupSummary, BackupError> {
    let dump = read_dump(store)?;
    let json = serde_json::to_vec(&dump).map_err(|e| BackupError::Corrupt(e.to_string()))?;
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&json)?;
    let packed = enc.finish()?;

    let mut out = BufWriter::new(out);
    let seed = (chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64) ^ (packed.len() as u64).rotate_left(17);
    out.write_all(MAGIC)?;
    out.write_all(&[FORMAT])?;
    out.write_all(&seed.to_le_bytes())?;
    let mut w = ScrambleWriter { inner: out, ks: Keystream::new(seed), written: 13 };
    w.write_all(&(packed.len() as u64).to_le_bytes())?;
    w.write_all(&packed)?;

    let mut summary = BackupSummary {
        // 只算进过房间的游玩（登录失败之类的启动不算）
        sessions: dump.sessions.iter().filter(|s| !s.visits.is_empty()).count(),
        visits: dump.sessions.iter().map(|s| s.visits.len()).sum(),
        photos: dump.sessions.iter().flat_map(|s| &s.visits).map(|v| v.photos.len()).sum::<usize>() + dump.loose_photos.len(),
        ..Default::default()
    };
    if let Some(q) = opts.photos {
        let paths = dump.sessions.iter().flat_map(|s| &s.visits).flat_map(|v| &v.photos).chain(&dump.loose_photos).map(|p| &p.path);
        for path in paths {
            let Ok(bytes) = std::fs::read(path) else { continue }; // 原图已经不在了
            match q {
                PhotoQuality::Original => write_entry(&mut w, KIND_PHOTO_ORIGINAL, path, &bytes)?,
                PhotoQuality::Compressed => match compress_photo(&bytes) {
                    Some(j) => write_entry(&mut w, KIND_PHOTO_JPEG, path, &j)?,
                    None => write_entry(&mut w, KIND_PHOTO_ORIGINAL, path, &bytes)?,
                },
            }
            summary.photo_files += 1;
        }
    }
    if opts.covers {
        let covers: Vec<(String, String)> = store
            .conn
            .prepare("SELECT world_id, thumbnail_path FROM worlds_cache WHERE thumbnail_path IS NOT NULL")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (wid, path) in covers {
            let Some(j) = std::fs::read(&path).ok().and_then(|b| compress_cover(&b)) else { continue };
            write_entry(&mut w, KIND_COVER, &wid, &j)?;
            summary.covers += 1;
        }
    }
    w.write_all(&[KIND_END])?;
    w.flush()?;
    summary.bytes = w.written;
    Ok(summary)
}

/// 导出到内存（不带图片），测试和小工具用
pub fn export(store: &Store) -> Result<(Vec<u8>, BackupSummary), BackupError> {
    let mut buf = Vec::new();
    let s = export_to(store, &mut buf, ExportOptions { photos: None, covers: false })?;
    Ok((buf, s))
}

fn read_exact_vec(r: &mut impl Read, n: u64) -> Result<Vec<u8>, BackupError> {
    if n > 1 << 34 {
        return Err(BackupError::Corrupt("长度不对".into()));
    }
    let mut v = vec![0; n as usize];
    r.read_exact(&mut v).map_err(|e| BackupError::Corrupt(e.to_string()))?;
    Ok(v)
}

fn read_u64(r: &mut impl Read) -> Result<u64, BackupError> {
    let mut b = [0; 8];
    r.read_exact(&mut b).map_err(|e| BackupError::Corrupt(e.to_string()))?;
    Ok(u64::from_le_bytes(b))
}

fn inflate_json(packed: &[u8]) -> Result<Dump, BackupError> {
    let mut json = Vec::new();
    DeflateDecoder::new(packed).read_to_end(&mut json).map_err(|e| BackupError::Corrupt(e.to_string()))?;
    serde_json::from_slice(&json).map_err(|e| BackupError::Corrupt(e.to_string()))
}

/// 不重名的文件路径：重名就加 (2)、(3)…
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.to_string(), String::new()),
    };
    (2..).map(|i| dir.join(format!("{stem} ({i}){ext}"))).find(|p| !p.exists()).unwrap()
}

/// 从 `input` 导入备份。`targets` 为 None 时不还原图片。
pub fn import_from(store: &mut Store, input: impl Read, mode: ImportMode, targets: Option<&ImportTargets>) -> Result<ImportReport, BackupError> {
    let mut input = BufReader::new(input);
    let mut head = [0u8; 13];
    input.read_exact(&mut head).map_err(|_| BackupError::NotBackup)?;
    if &head[..4] != MAGIC {
        return Err(BackupError::NotBackup);
    }
    let version = head[4];
    if version > FORMAT || version == 0 {
        return Err(BackupError::UnsupportedVersion(version));
    }
    let seed = u64::from_le_bytes(head[5..13].try_into().unwrap());
    let mut r = ScrambleReader { inner: input, ks: Keystream::new(seed) };
    let dump = if version == 1 {
        let mut packed = Vec::new();
        r.read_to_end(&mut packed)?;
        inflate_json(&packed)?
    } else {
        let n = read_u64(&mut r)?;
        inflate_json(&read_exact_vec(&mut r, n)?)?
    };
    let tx = store.conn.transaction()?;
    if mode == ImportMode::Replace {
        tx.execute_batch("DELETE FROM encounters; DELETE FROM photos; DELETE FROM visits; DELETE FROM avatar_wears; DELETE FROM sessions; DELETE FROM log_files;")?;
    }
    for f in &dump.log_files {
        tx.execute(
            "INSERT OR IGNORE INTO log_files(path, size, read_offset, first_seen, last_event_at, self_user_id, self_name, finalized, total_lines, info_lines, avatars_scanned)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![f.path, f.size, f.read_offset, f.first_seen, f.last_event_at, f.self_user_id, f.self_name, f.finalized, f.total_lines, f.info_lines, f.avatars_scanned],
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
        for a in &s.avatars {
            tx.execute(
                "INSERT INTO avatar_wears(session_id, avatar_name, avatar_id, start_at, end_at) VALUES (?1,?2,?3,?4,?5)",
                params![sid, a.avatar_name, a.avatar_id, a.start_at, a.end_at],
            )?;
        }
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

    // v2：图片条目
    if version >= 2 {
        loop {
            let mut kind = [0u8; 1];
            r.read_exact(&mut kind).map_err(|e| BackupError::Corrupt(e.to_string()))?;
            if kind[0] == KIND_END {
                break;
            }
            let mut kl = [0u8; 2];
            r.read_exact(&mut kl).map_err(|e| BackupError::Corrupt(e.to_string()))?;
            let key = String::from_utf8(read_exact_vec(&mut r, u16::from_le_bytes(kl) as u64)?)
                .map_err(|e| BackupError::Corrupt(e.to_string()))?;
            let n = read_u64(&mut r)?;
            let data = read_exact_vec(&mut r, n)?;
            let Some(t) = targets else { continue };
            match kind[0] {
                KIND_PHOTO_ORIGINAL | KIND_PHOTO_JPEG => {
                    // 原路径还在（同一台电脑）就不重复写
                    if Path::new(&key).exists() {
                        continue;
                    }
                    let base = key.rsplit(['\\', '/']).next().unwrap_or("photo");
                    let name = if kind[0] == KIND_PHOTO_JPEG {
                        format!("{}.jpg", base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base))
                    } else {
                        base.to_string()
                    };
                    // 按拍摄月份分文件夹（文件名 VRChat_2026-04-30_… 里取）
                    let month = base.strip_prefix("VRChat_").and_then(|s| s.get(..7)).filter(|m| m.as_bytes().get(4) == Some(&b'-'));
                    let dir = month.map(|m| t.photo_dir.join(m)).unwrap_or_else(|| t.photo_dir.clone());
                    std::fs::create_dir_all(&dir)?;
                    let dest = unique_path(&dir, &name);
                    std::fs::write(&dest, &data)?;
                    tx.execute("UPDATE photos SET path = ?2 WHERE path = ?1", params![key, dest.to_string_lossy()])?;
                    rep.photos_restored += 1;
                }
                KIND_COVER => {
                    let local: Option<Option<String>> =
                        tx.query_row("SELECT thumbnail_path FROM worlds_cache WHERE world_id = ?1", [&key], |r| r.get(0)).optional()?;
                    if local.flatten().is_some_and(|p| Path::new(&p).exists()) {
                        continue;
                    }
                    std::fs::create_dir_all(&t.cover_dir)?;
                    let dest = t.cover_dir.join(format!("{key}.jpg"));
                    std::fs::write(&dest, &data)?;
                    tx.execute(
                        "INSERT INTO worlds_cache(world_id, thumbnail_path, fetched_at) VALUES (?1, ?2, ?3)
                         ON CONFLICT(world_id) DO UPDATE SET thumbnail_path = excluded.thumbnail_path",
                        params![key, dest.to_string_lossy(), chrono::Utc::now().timestamp_millis()],
                    )?;
                    rep.covers_restored += 1;
                }
                _ => {} // 以后的新类型：跳过
            }
        }
    }
    tx.commit()?;
    Ok(rep)
}

/// 从内存导入（不还原图片），测试和小工具用
pub fn import(store: &mut Store, bytes: &[u8], mode: ImportMode) -> Result<ImportReport, BackupError> {
    import_from(store, bytes, mode, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v1（旧版，不带图片）的备份还能导入
    #[test]
    fn imports_v1() {
        let src = Store::open_in_memory().unwrap();
        src.conn.execute("INSERT INTO log_files(path, first_seen) VALUES ('a.txt', 1)", []).unwrap();
        src.conn.execute("INSERT INTO sessions(start_at, end_at, log_file) VALUES (1, 2, 'a.txt')", []).unwrap();
        let json = serde_json::to_vec(&read_dump(&src).unwrap()).unwrap();
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&json).unwrap();
        let mut body = enc.finish().unwrap();
        Keystream::new(42).apply(&mut body);
        let mut v1 = b"YYMB\x01".to_vec();
        v1.extend_from_slice(&42u64.to_le_bytes());
        v1.extend_from_slice(&body);
        let mut dst = Store::open_in_memory().unwrap();
        assert_eq!(import(&mut dst, &v1, ImportMode::Merge).unwrap().added, 1);
    }
}
