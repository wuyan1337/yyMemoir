//! 截图：文件名自带拍摄时间 `VRChat_2026-04-30_21-00-00.808_2560x1440.png`，
//! 旧版是 `VRChat_1920x1080_2021-01-01_00-00-00.000.png`。按时间对应到当时所在的房间。

use crate::ingest::TimeConv;
use crate::store::Store;
use chrono::NaiveDateTime;
use regex::Regex;
use rusqlite::{params, OptionalExtension, Result};
use std::path::Path;
use std::sync::OnceLock;

fn name_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^VRChat_(?:\d+x\d+_)?(\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2})(?:\.\d+)?(?:_\d+x\d+)?.*\.(?i:png|jpe?g|webp)$").unwrap()
    })
}

pub fn parse_photo_time(file_name: &str) -> Option<NaiveDateTime> {
    let c = name_re().captures(file_name)?;
    NaiveDateTime::parse_from_str(&c[1], "%Y-%m-%d_%H-%M-%S").ok()
}

/// 扫描截图文件夹（含按月分的子文件夹），返回新登记的张数。已登记但没对上房间的会重新匹配。
pub fn scan_photos(store: &mut Store, dir: &Path, tz: TimeConv) -> Result<usize> {
    let mut files = Vec::new();
    walk(dir, &mut files, 0);
    let tx = store.conn.transaction()?;
    let mut added = 0;
    for p in files {
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
        let Some(t) = parse_photo_time(name) else { continue };
        let t = tz.to_utc_ms(t);
        let n = tx.execute(
            "INSERT OR IGNORE INTO photos(path, taken_at) VALUES (?1, ?2)",
            params![p.to_string_lossy(), t],
        )?;
        added += n;
    }
    // 给还没对上房间的照片找房间（房间可能是之后才入库的）
    let pending: Vec<(String, i64)> = {
        let mut st = tx.prepare("SELECT path, taken_at FROM photos WHERE visit_id IS NULL")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_>>()?
    };
    for (path, t) in pending {
        let v: Option<i64> = tx
            .query_row(
                "SELECT v.id FROM visits v JOIN sessions s ON s.id=v.session_id
                 WHERE v.join_at <= ?1 AND COALESCE(v.leave_at, s.end_at) >= ?1
                 ORDER BY v.join_at DESC LIMIT 1",
                [t],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(v) = v {
            tx.execute("UPDATE photos SET visit_id=?2 WHERE path=?1", params![path, v])?;
        }
    }
    tx.commit()?;
    Ok(added)
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>, depth: u32) {
    if depth > 3 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out, depth + 1);
        } else {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn photo_names() {
        let t = super::parse_photo_time("VRChat_2026-04-30_21-00-00.808_2560x1440.png").unwrap();
        assert_eq!(t.to_string(), "2026-04-30 21:00:00");
        // 旧版命名：分辨率在前
        assert!(super::parse_photo_time("VRChat_1920x1080_2021-01-01_00-00-00.000.png").is_some());
        assert!(super::parse_photo_time("random.png").is_none());
    }
}
