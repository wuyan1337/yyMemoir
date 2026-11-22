//! 增量读取日志文件并入库。开机补扫和实时监听共用这一套：
//! 每个文件记住读到的字节位置，只解析新增的完整行；进行中的房间/玩家用 leave_at IS NULL 表示，
//! 所以监听程序中途重启也能接着记。

use crate::parse::{parse_line, Event};
use crate::store::Store;
use chrono::{FixedOffset, Local, NaiveDateTime, TimeZone};
use rusqlite::{params, OptionalExtension, Result, Transaction};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// 日志里的时间是写日志那台电脑的本地时间。
#[derive(Debug, Clone, Copy)]
pub enum TimeConv {
    Local,
    /// 固定时区偏移（分钟），测试和跨机器导入用
    Fixed(i32),
}

impl TimeConv {
    pub fn to_utc_ms(&self, t: NaiveDateTime) -> i64 {
        match self {
            TimeConv::Local => Local
                .from_local_datetime(&t)
                .earliest()
                .map(|d| d.timestamp_millis())
                .unwrap_or_else(|| t.and_utc().timestamp_millis()),
            TimeConv::Fixed(m) => FixedOffset::east_opt(m * 60)
                .unwrap()
                .from_local_datetime(&t)
                .unwrap()
                .timestamp_millis(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct IngestReport {
    pub bytes_read: u64,
    pub lines: u64,
    pub events: u64,
    /// 非 Error 级别的行数；为 0 说明游戏日志设成了「仅错误」
    pub info_lines: u64,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// 读取 `path` 从上次位置到文件末尾的新增内容。
pub fn ingest_file(store: &mut Store, path: &Path, tz: TimeConv) -> Result<IngestReport> {
    let key = path.to_string_lossy().to_string();
    let tx = store.conn.transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO log_files(path, first_seen) VALUES (?1, ?2)",
        params![key, now_ms()],
    )?;
    let offset: i64 = tx.query_row("SELECT read_offset FROM log_files WHERE path=?1", [&key], |r| r.get(0))?;

    let mut buf = Vec::new();
    let size = match File::open(path) {
        Ok(mut f) => {
            let size = f.metadata().map(|m| m.len()).unwrap_or(0);
            if size > offset as u64 {
                f.seek(SeekFrom::Start(offset as u64)).ok();
                f.read_to_end(&mut buf).ok();
            }
            size
        }
        Err(_) => 0,
    };
    // 只处理到最后一个换行；半行留到下次
    let complete = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1).unwrap_or(0);
    let text = String::from_utf8_lossy(&buf[..complete]);

    let mut rep = IngestReport { bytes_read: complete as u64, ..Default::default() };
    let mut tracker = Tracker::load(&tx, &key)?;
    for line in text.lines() {
        if let Some((ts, ev)) = parse_line(line) {
            rep.lines += 1;
            if !crate::parse::is_error_line(line) {
                rep.info_lines += 1;
            }
            let t = tz.to_utc_ms(ts);
            tracker.touch(&tx, t)?;
            if let Some(ev) = ev {
                rep.events += 1;
                tracker.apply(&tx, t, ev)?;
            }
        }
    }
    tracker.save(&tx)?;
    tx.execute(
        "UPDATE log_files SET read_offset=read_offset+?2, size=?3, total_lines=total_lines+?4, info_lines=info_lines+?5 WHERE path=?1",
        params![key, complete as i64, size as i64, rep.lines as i64, rep.info_lines as i64],
    )?;
    tx.commit()?;
    Ok(rep)
}

/// 游戏退出或确认文件不再写入时调用：把还开着的房间和玩家在最后一条日志的时间关掉。
pub fn finalize_file(store: &mut Store, path: &Path) -> Result<()> {
    let key = path.to_string_lossy().to_string();
    let tx = store.conn.transaction()?;
    let last: Option<i64> = tx
        .query_row("SELECT last_event_at FROM log_files WHERE path=?1", [&key], |r| r.get(0))
        .optional()?
        .flatten();
    if let Some(last) = last {
        if let Some(sid) = session_id(&tx, &key)? {
            tx.execute(
                "UPDATE encounters SET leave_at=?2 WHERE leave_at IS NULL AND visit_id IN (SELECT id FROM visits WHERE session_id=?1)",
                params![sid, last],
            )?;
            tx.execute("UPDATE visits SET leave_at=?2 WHERE leave_at IS NULL AND session_id=?1", params![sid, last])?;
        }
    }
    tx.execute("UPDATE log_files SET finalized=1 WHERE path=?1", [&key])?;
    tx.commit()
}

fn session_id(tx: &Transaction, key: &str) -> Result<Option<i64>> {
    tx.query_row("SELECT id FROM sessions WHERE log_file=?1", [key], |r| r.get(0)).optional()
}

struct Tracker {
    key: String,
    session: Option<i64>,
    visit: Option<i64>,
    self_id: Option<String>,
    self_name: Option<String>,
    pending_name: Option<String>,
    last: Option<i64>,
}

impl Tracker {
    fn load(tx: &Transaction, key: &str) -> Result<Self> {
        let (self_id, self_name, pending_name, last) = tx.query_row(
            "SELECT self_user_id, self_name, pending_world_name, last_event_at FROM log_files WHERE path=?1",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let session = session_id(tx, key)?;
        let visit = match session {
            Some(s) => tx
                .query_row(
                    "SELECT id FROM visits WHERE session_id=?1 AND leave_at IS NULL ORDER BY id DESC LIMIT 1",
                    [s],
                    |r| r.get(0),
                )
                .optional()?,
            None => None,
        };
        Ok(Tracker { key: key.to_string(), session, visit, self_id, self_name, pending_name, last })
    }

    fn save(&self, tx: &Transaction) -> Result<()> {
        tx.execute(
            "UPDATE log_files SET self_user_id=?2, self_name=?3, pending_world_name=?4, last_event_at=?5 WHERE path=?1",
            params![self.key, self.self_id, self.self_name, self.pending_name, self.last],
        )?;
        Ok(())
    }

    fn touch(&mut self, tx: &Transaction, t: i64) -> Result<()> {
        // 日志偶尔有乱序，时间只往前走
        let t = self.last.map_or(t, |l| l.max(t));
        self.last = Some(t);
        match self.session {
            None => {
                tx.execute(
                    "INSERT INTO sessions(start_at, end_at, log_file) VALUES (?1, ?1, ?2)",
                    params![t, self.key],
                )?;
                self.session = Some(tx.last_insert_rowid());
            }
            Some(s) => {
                tx.execute("UPDATE sessions SET end_at=?2 WHERE id=?1", params![s, t])?;
            }
        }
        Ok(())
    }

    fn close_visit(&mut self, tx: &Transaction, t: i64) -> Result<()> {
        if let Some(v) = self.visit.take() {
            tx.execute("UPDATE encounters SET leave_at=?2 WHERE visit_id=?1 AND leave_at IS NULL", params![v, t])?;
            tx.execute("UPDATE visits SET leave_at=?2 WHERE id=?1", params![v, t])?;
        }
        Ok(())
    }

    fn is_self(&self, name: &str, uid: &Option<String>) -> bool {
        match (uid, &self.self_id) {
            (Some(a), Some(b)) => a == b,
            _ => self.self_name.as_deref() == Some(name),
        }
    }

    fn apply(&mut self, tx: &Transaction, t: i64, ev: Event) -> Result<()> {
        let t = self.last.unwrap_or(t);
        match ev {
            Event::SelfUser { name, user_id } => {
                self.self_name = Some(name);
                if user_id.is_some() {
                    self.self_id = user_id;
                }
            }
            Event::Joining { world_id, instance_id, access_type } => {
                self.close_visit(tx, t)?;
                tx.execute(
                    "INSERT INTO visits(session_id, world_id, instance_id, access_type, world_name, join_at) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![self.session, world_id, instance_id, access_type, self.pending_name.take(), t],
                )?;
                self.visit = Some(tx.last_insert_rowid());
            }
            Event::EnteringRoom { world_name } => {
                // Entering Room 和 Joining 的先后顺序不固定，两种都兼容
                let updated = match self.visit {
                    Some(v) => tx.execute(
                        "UPDATE visits SET world_name=?2 WHERE id=?1 AND world_name IS NULL",
                        params![v, world_name],
                    )?,
                    None => 0,
                };
                if updated == 0 {
                    self.pending_name = Some(world_name);
                }
            }
            Event::LeftRoom => self.close_visit(tx, t)?,
            Event::PlayerJoined { name, user_id } => {
                if self.is_self(&name, &user_id) {
                    return Ok(());
                }
                if let Some(v) = self.visit {
                    let uid = user_id.unwrap_or_else(|| format!("name:{name}"));
                    let open: Option<i64> = tx
                        .query_row(
                            "SELECT id FROM encounters WHERE visit_id=?1 AND user_id=?2 AND leave_at IS NULL",
                            params![v, uid],
                            |r| r.get(0),
                        )
                        .optional()?;
                    if open.is_none() {
                        tx.execute(
                            "INSERT INTO encounters(visit_id, user_id, display_name, join_at) VALUES (?1,?2,?3,?4)",
                            params![v, uid, name, t],
                        )?;
                    }
                }
            }
            Event::PlayerLeft { name, user_id } => {
                if let Some(v) = self.visit {
                    let uid = user_id.unwrap_or_else(|| format!("name:{name}"));
                    tx.execute(
                        "UPDATE encounters SET leave_at=?3 WHERE visit_id=?1 AND user_id=?2 AND leave_at IS NULL",
                        params![v, uid, t],
                    )?;
                }
            }
            Event::Video { .. } => {}
        }
        Ok(())
    }
}
