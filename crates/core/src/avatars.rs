//! 本人穿的模型（Avatar）记录。只记本人，别人换模型不记。
//!
//! 日志里换模型的顺序（2026-11 真实日志）：
//!   Switching 本人 to avatar 新模型名
//!   Saving Avatar Data:avtr_旧模型      ← 同一秒，存的是换下来的那个
//!   Loading Avatar Data:avtr_新模型     ← 1–2 秒内，这个才是新模型的 ID
//! 之后每分钟一次 `Saving Avatar Data:avtr_当前模型`。
//! 重进房间时会再写一次 `Switching … to avatar 同一个模型`，合并成一段。
//! 模型 ID 现在只是记下来（不登录拿不到预览图），以后做登录时用来取图。

use crate::ingest::TimeConv;
use crate::parse::{parse_line, Event};
use crate::store::Store;
use rusqlite::{params, OptionalExtension, Result, Transaction};
use std::fs::File;
use std::io::Read;

/// 换模型后多久内的 Loading 算新模型的
const LOADING_WINDOW_MS: i64 = 15_000;
/// 换模型同一刻的 Saving 是旧模型的；隔几秒以后的 Saving 才是当前模型
const SAVING_AFTER_MS: i64 = 5_000;

struct Open {
    id: i64,
    name: String,
    /// 最近一次写「换成这个模型」的时间（重进房间会再写一次）
    last_switch: i64,
    has_id: bool,
}

#[derive(Default)]
pub(crate) struct AvatarTrack {
    open: Option<Open>,
}

impl AvatarTrack {
    /// 接着上次读到的地方：这次游玩里还没结束的那一段
    pub(crate) fn load(tx: &Transaction, session: Option<i64>) -> Result<Self> {
        let Some(s) = session else { return Ok(Self::default()) };
        let open = tx
            .query_row(
                "SELECT id, avatar_name, start_at, avatar_id IS NOT NULL FROM avatar_wears
                 WHERE session_id = ?1 AND end_at IS NULL ORDER BY id DESC LIMIT 1",
                [s],
                |r| Ok(Open { id: r.get(0)?, name: r.get(1)?, last_switch: r.get(2)?, has_id: r.get(3)? }),
            )
            .optional()?;
        Ok(AvatarTrack { open })
    }

    /// 换上一个模型；和正在穿的同名时只是合并，返回 false
    pub(crate) fn switch(&mut self, tx: &Transaction, session: i64, t: i64, name: &str) -> Result<bool> {
        if let Some(o) = self.open.as_mut() {
            if o.name == name {
                o.last_switch = t;
                return Ok(false);
            }
        }
        self.close(tx, t)?;
        tx.execute(
            "INSERT INTO avatar_wears(session_id, avatar_name, start_at) VALUES (?1, ?2, ?3)",
            params![session, name, t],
        )?;
        self.open = Some(Open { id: tx.last_insert_rowid(), name: name.to_string(), last_switch: t, has_id: false });
        Ok(true)
    }

    pub(crate) fn data(&mut self, tx: &Transaction, t: i64, avatar_id: &str, loading: bool) -> Result<()> {
        let Some(o) = self.open.as_mut().filter(|o| !o.has_id) else { return Ok(()) };
        let dt = t - o.last_switch;
        let ours = if loading { (0..=LOADING_WINDOW_MS).contains(&dt) } else { dt >= SAVING_AFTER_MS };
        if ours {
            tx.execute("UPDATE avatar_wears SET avatar_id = ?2 WHERE id = ?1", params![o.id, avatar_id])?;
            o.has_id = true;
        }
        Ok(())
    }

    pub(crate) fn close(&mut self, tx: &Transaction, t: i64) -> Result<()> {
        if let Some(o) = self.open.take() {
            tx.execute("UPDATE avatar_wears SET end_at = ?2 WHERE id = ?1", params![o.id, t])?;
        }
        Ok(())
    }
}

/// 老数据补读模型：v3 之前入库的日志没记模型。日志文件还在的，只读到上次入库的位置
/// （后面的部分交给正常的增量读取），补完标记 `avatars_scanned = 1`，以后不再读。
/// 日志已经被删掉的也标记，不再重试。返回补到的段数。
pub fn backfill(store: &mut Store, tz: TimeConv) -> Result<usize> {
    let todo: Vec<(String, i64, Option<String>, bool, Option<i64>, Option<i64>)> = store
        .conn
        .prepare(
            "SELECT f.path, f.read_offset, f.self_name, f.finalized, f.last_event_at, s.id
             FROM log_files f LEFT JOIN sessions s ON s.log_file = f.path WHERE f.avatars_scanned = 0",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? != 0, r.get(4)?, r.get(5)?)))?
        .collect::<Result<_>>()?;
    let mut added = 0;
    for (path, offset, mut self_name, finalized, last, session) in todo {
        let tx = store.conn.transaction()?;
        if let Some(sid) = session {
            let mut buf = Vec::new();
            if let Ok(f) = File::open(&path) {
                f.take(offset.max(0) as u64).read_to_end(&mut buf).ok();
            }
            if !buf.is_empty() {
                tx.execute("DELETE FROM avatar_wears WHERE session_id = ?1", [sid])?;
                let mut track = AvatarTrack::default();
                let mut clock = i64::MIN;
                for line in String::from_utf8_lossy(&buf).lines() {
                    let Some((ts, Some(ev))) = parse_line(line) else { continue };
                    // 和增量读取一样：时间只往前走
                    clock = clock.max(tz.to_utc_ms(ts));
                    match ev {
                        Event::SelfUser { name, .. } | Event::LocalPlayer { name } => self_name = Some(name),
                        Event::AvatarSwitch { who, avatar } if self_name.as_deref() == Some(who.as_str()) => {
                            if track.switch(&tx, sid, clock, &avatar)? {
                                added += 1;
                            }
                        }
                        Event::AvatarData { avatar_id, loading } => track.data(&tx, clock, &avatar_id, loading)?,
                        _ => {}
                    }
                }
                if finalized {
                    if let Some(l) = last {
                        track.close(&tx, l)?;
                    }
                }
            }
        }
        tx.execute("UPDATE log_files SET avatars_scanned = 1 WHERE path = ?1", [&path])?;
        tx.commit()?;
    }
    Ok(added)
}
