//! 统计：任意时间段（今天 / 周 / 月 / 年 / 自定义）用同一套算法，输出 `Recap`（JSON 给前端模板）。
//! 「一起待了多久」= 自己在房间的时间段 ∩ 对方在房间的时间段，累加。

use crate::ingest::TimeConv;
use crate::store::Store;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use rusqlite::{params, Result};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

const MIN: i64 = 60_000;

#[derive(Debug, Serialize, Clone, Default)]
pub struct Recap {
    pub product: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    pub self_name: Option<String>,
    pub total_minutes: i64,
    pub play_days: usize,
    pub session_count: usize,
    pub longest_session: Option<SessionStat>,
    pub latest_logoff: Option<SessionStat>,
    /// 时间段内最后一次下线（真实结束时间，跨零点不截断），今日卡片用
    pub last_logoff: Option<String>,
    pub peak_hour: Option<u32>,
    pub hour_minutes: Vec<i64>,
    pub daily_minutes: Vec<DayStat>,
    pub monthly_minutes: Vec<i64>,
    pub busiest_day: Option<DayStat>,
    pub world_count: usize,
    pub new_world_count: usize,
    pub top_worlds: Vec<WorldStat>,
    pub people_count: usize,
    pub top_people: Vec<PersonStat>,
    pub access_minutes: BTreeMap<String, i64>,
    pub photo_count: usize,
    pub top_photo_world: Option<WorldStat>,
    pub photos: Vec<PhotoStat>,
    /// 按时间顺序的行程（连续待在同一个世界的合并成一段），最多 40 段
    pub timeline: Vec<Stop>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SessionStat {
    pub start: String,
    pub end: String,
    pub minutes: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DayStat {
    pub date: String,
    pub minutes: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorldStat {
    pub world_id: String,
    pub name: String,
    pub minutes: i64,
    pub visits: usize,
    pub photos: usize,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PersonStat {
    pub user_id: String,
    pub name: String,
    pub minutes: i64,
    pub is_friend: bool,
    pub avatar: Option<String>,
    /// 一起最久的那一天
    pub best_day: Option<DayStat>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Stop {
    pub world_id: String,
    pub name: String,
    pub access_type: String,
    pub start: String,
    pub end: String,
    pub minutes: i64,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PhotoStat {
    pub path: String,
    pub taken_at: String,
    pub world_name: Option<String>,
}

impl TimeConv {
    pub fn to_local(&self, ms: i64) -> NaiveDateTime {
        use chrono::{FixedOffset, Local, TimeZone, Utc};
        let utc = Utc.timestamp_millis_opt(ms).unwrap();
        match self {
            TimeConv::Local => utc.with_timezone(&Local).naive_local(),
            TimeConv::Fixed(m) => utc.with_timezone(&FixedOffset::east_opt(m * 60).unwrap()).naive_local(),
        }
    }
}

struct Visit {
    id: i64,
    world_id: String,
    world_name: Option<String>,
    access: String,
    a: i64,
    b: i64,
}

fn fmt(tz: TimeConv, ms: i64) -> String {
    tz.to_local(ms).format("%Y-%m-%d %H:%M").to_string()
}

/// 按本地日期切分区间，回调 (日期, 小时, 分钟数)。
fn split_by_hour(tz: TimeConv, a: i64, b: i64, mut f: impl FnMut(NaiveDate, u32, i64)) {
    let mut t = a;
    while t < b {
        let local = tz.to_local(t);
        let into_hour = (local.minute() as i64 * 60 + local.second() as i64) * 1000 + (local.nanosecond() / 1_000_000) as i64;
        let next = (t - into_hour + 60 * MIN).min(b);
        f(local.date(), local.hour(), next - t);
        t = next;
    }
}

/// 统计 [from_ms, to_ms) 区间。
pub fn recap(store: &Store, tz: TimeConv, kind: &str, from_ms: i64, to_ms: i64, top_n: usize) -> Result<Recap> {
    let c = &store.conn;
    let mut r = Recap {
        product: "yyMemoir".into(),
        kind: kind.into(),
        from: fmt(tz, from_ms),
        to: fmt(tz, to_ms),
        hour_minutes: vec![0; 24],
        monthly_minutes: vec![0; 12],
        ..Default::default()
    };

    r.self_name = c
        .query_row(
            "SELECT self_name FROM log_files WHERE self_name IS NOT NULL ORDER BY last_event_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    // ---- 会话：总时长、天数、时段、最长、最晚下线
    let mut daily: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    // 游玩时间从第一次进房间算起；登录失败、没进过任何房间的启动不算
    let mut st = c.prepare(
        "SELECT MIN(v.join_at), s.end_at FROM sessions s JOIN visits v ON v.session_id = s.id
         GROUP BY s.id HAVING s.end_at > ?1 AND MIN(v.join_at) < ?2",
    )?;
    let sessions: Vec<(i64, i64)> =
        st.query_map(params![from_ms, to_ms], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<_>>()?;
    let mut latest_key = i64::MIN;
    for &(s, e) in &sessions {
        let (a, b) = (s.max(from_ms), e.min(to_ms));
        if b <= a {
            continue;
        }
        r.session_count += 1;
        r.total_minutes += (b - a) / MIN;
        split_by_hour(tz, a, b, |d, h, ms| {
            *daily.entry(d).or_default() += ms;
            r.hour_minutes[h as usize] += ms;
        });
        // 最长 / 最晚用整次会话的真实起止（跨零点的会话不被时间段截断）
        let stat = SessionStat { start: fmt(tz, s), end: fmt(tz, e), minutes: (e - s) / MIN };
        if r.longest_session.as_ref().map_or(true, |l| stat.minutes > l.minutes) {
            r.longest_session = Some(stat.clone());
        }
        if r.last_logoff.as_ref().map_or(true, |x| fmt(tz, e) > *x) {
            r.last_logoff = Some(fmt(tz, e));
        }
        // 「最晚」：把一天的起点挪到早上 6 点，凌晨 3 点算比晚上 11 点更晚
        let end_local = tz.to_local(e);
        let key = ((end_local.hour() as i64 + 18) % 24) * 60 + end_local.minute() as i64;
        if key > latest_key {
            latest_key = key;
            r.latest_logoff = Some(stat);
        }
    }
    for m in r.hour_minutes.iter_mut() {
        *m /= MIN;
    }
    r.peak_hour = r.hour_minutes.iter().enumerate().rev().max_by_key(|(_, m)| **m).filter(|(_, m)| **m > 0).map(|(h, _)| h as u32);
    for (d, ms) in &daily {
        let minutes = ms / MIN;
        if minutes > 0 {
            r.play_days += 1;
        }
        r.monthly_minutes[d.month0() as usize] += minutes;
        r.daily_minutes.push(DayStat { date: d.to_string(), minutes });
    }
    r.busiest_day = r.daily_minutes.iter().max_by_key(|d| d.minutes).filter(|d| d.minutes > 0).cloned();

    // ---- 房间
    let mut st = c.prepare(
        "SELECT v.id, v.world_id, COALESCE(v.world_name, w.name), v.access_type, v.join_at, COALESCE(v.leave_at, s.end_at)
         FROM visits v JOIN sessions s ON s.id=v.session_id LEFT JOIN worlds_cache w ON w.world_id=v.world_id
         WHERE COALESCE(v.leave_at, s.end_at) > ?1 AND v.join_at < ?2",
    )?;
    let visits: Vec<Visit> = st
        .query_map(params![from_ms, to_ms], |row| {
            Ok(Visit {
                id: row.get(0)?,
                world_id: row.get(1)?,
                world_name: row.get(2)?,
                access: row.get(3)?,
                a: row.get::<_, i64>(4)?.max(from_ms),
                b: row.get::<_, i64>(5)?.min(to_ms),
            })
        })?
        .collect::<Result<_>>()?;

    let mut worlds: HashMap<String, WorldStat> = HashMap::new();
    for v in &visits {
        let w = worlds.entry(v.world_id.clone()).or_insert_with(|| WorldStat {
            world_id: v.world_id.clone(),
            name: String::new(),
            minutes: 0,
            visits: 0,
            photos: 0,
            thumbnail: None,
        });
        w.minutes += (v.b - v.a).max(0);
        w.visits += 1;
        if let Some(n) = &v.world_name {
            w.name = n.clone();
        }
        *r.access_minutes.entry(v.access.clone()).or_default() += (v.b - v.a).max(0);
    }
    for m in r.access_minutes.values_mut() {
        *m /= MIN;
    }
    r.world_count = worlds.len();
    {
        let mut st = c.prepare("SELECT world_id FROM visits GROUP BY world_id HAVING MIN(join_at) >= ?1 AND MIN(join_at) < ?2")?;
        let new: HashSet<String> = st.query_map(params![from_ms, to_ms], |row| row.get(0))?.collect::<Result<_>>()?;
        r.new_world_count = new.len();
    }
    let mut thumb = c.prepare("SELECT thumbnail_path FROM worlds_cache WHERE world_id=?1")?;
    for w in worlds.values_mut() {
        w.minutes /= MIN;
        if w.name.is_empty() {
            w.name = w.world_id.clone();
        }
        w.thumbnail = thumb.query_row([&w.world_id], |row| row.get(0)).ok().flatten();
    }

    // ---- 人：重叠时长
    let visit_by_id: HashMap<i64, &Visit> = visits.iter().map(|v| (v.id, v)).collect();
    struct Acc {
        name: String,
        ms: i64,
        days: HashMap<NaiveDate, i64>,
    }
    let mut people: HashMap<String, Acc> = HashMap::new();
    if !visits.is_empty() {
        let mut st = c.prepare(
            "SELECT e.visit_id, e.user_id, e.display_name, e.join_at, e.leave_at FROM encounters e
             JOIN visits v ON v.id=e.visit_id JOIN sessions s ON s.id=v.session_id
             WHERE COALESCE(v.leave_at, s.end_at) > ?1 AND v.join_at < ?2 ORDER BY e.join_at",
        )?;
        let rows = st.query_map(params![from_ms, to_ms], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?, row.get::<_, Option<i64>>(4)?))
        })?;
        for row in rows {
            let (vid, uid, name, ja, la) = row?;
            let Some(v) = visit_by_id.get(&vid) else { continue };
            let a = ja.max(v.a);
            let b = la.unwrap_or(v.b).min(v.b);
            let acc = people.entry(uid).or_insert_with(|| Acc { name: String::new(), ms: 0, days: HashMap::new() });
            acc.name = name;
            if b > a {
                acc.ms += b - a;
                split_by_hour(tz, a, b, |d, _, ms| *acc.days.entry(d).or_default() += ms);
            }
        }
    }
    r.people_count = people.len();
    let mut ucache = c.prepare("SELECT is_friend, avatar_path FROM users_cache WHERE user_id=?1")?;
    let mut top: Vec<PersonStat> = people
        .into_iter()
        .map(|(uid, acc)| {
            let (is_friend, avatar) = ucache
                .query_row([&uid], |row| Ok((row.get::<_, i64>(0)? != 0, row.get::<_, Option<String>>(1)?)))
                .unwrap_or((false, None));
            let best_day = acc.days.iter().max_by_key(|(_, ms)| **ms).map(|(d, ms)| DayStat { date: d.to_string(), minutes: ms / MIN });
            PersonStat { user_id: uid, name: acc.name, minutes: acc.ms / MIN, is_friend, avatar, best_day }
        })
        .collect();
    top.sort_by(|a, b| b.minutes.cmp(&a.minutes).then(a.name.cmp(&b.name)));
    top.truncate(top_n);
    r.top_people = top;

    // ---- 照片
    let mut st = c.prepare(
        "SELECT p.path, p.taken_at, v.world_id FROM photos p LEFT JOIN visits v ON v.id=p.visit_id
         WHERE p.taken_at >= ?1 AND p.taken_at < ?2 ORDER BY p.taken_at",
    )?;
    let photos: Vec<(String, i64, Option<String>)> =
        st.query_map(params![from_ms, to_ms], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?.collect::<Result<_>>()?;
    r.photo_count = photos.len();
    for (path, t, wid) in &photos {
        if let Some(w) = wid.as_ref().and_then(|w| worlds.get_mut(w)) {
            w.photos += 1;
        }
        if r.photos.len() < 24 {
            r.photos.push(PhotoStat {
                path: path.clone(),
                taken_at: fmt(tz, *t),
                world_name: wid.as_ref().and_then(|w| worlds.get(w)).map(|w| w.name.clone()),
            });
        }
    }
    r.top_photo_world = worlds.values().filter(|w| w.photos > 0).max_by_key(|w| w.photos).cloned();

    let mut tw: Vec<WorldStat> = worlds.into_values().collect();
    tw.sort_by(|a, b| b.minutes.cmp(&a.minutes).then(a.name.cmp(&b.name)));
    tw.truncate(top_n);
    r.top_worlds = tw;

    // ---- 行程
    let mut sorted: Vec<&Visit> = visits.iter().filter(|v| v.b > v.a).collect();
    sorted.sort_by_key(|v| v.a);
    let mut stops: Vec<(String, String, String, i64, i64)> = Vec::new();
    for v in sorted {
        let name = v.world_name.clone().unwrap_or_else(|| v.world_id.clone());
        match stops.last_mut() {
            // 同一个世界、中间断开不超过 2 分钟（换房间 / 重连）就合并
            Some(last) if last.0 == v.world_id && v.a - last.4 <= 2 * MIN => last.4 = last.4.max(v.b),
            _ => stops.push((v.world_id.clone(), name, v.access.clone(), v.a, v.b)),
        }
    }
    r.timeline = stops
        .into_iter()
        .take(40)
        .map(|(world_id, name, access_type, a, b)| Stop {
            thumbnail: thumb.query_row([&world_id], |row| row.get(0)).ok().flatten(),
            world_id,
            name,
            access_type,
            start: fmt(tz, a),
            end: fmt(tz, b),
            minutes: (b - a) / MIN,
        })
        .collect();
    Ok(r)
}

/// 一次游玩（一次启动到关闭）的时间范围：从第一次进房间到最后一条日志。没进过房间的启动返回 None。
pub fn session_range(store: &Store, session_id: i64) -> Result<Option<(i64, i64)>> {
    store
        .conn
        .query_row(
            "SELECT MIN(v.join_at), s.end_at FROM sessions s JOIN visits v ON v.session_id = s.id WHERE s.id = ?1 GROUP BY s.id",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map(Some)
        .or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })
}

/// 有效的游玩（进过房间的），按开始时间倒序：(session_id, 开始, 结束)
pub fn list_sessions(store: &Store, limit: usize) -> Result<Vec<(i64, i64, i64)>> {
    let mut st = store.conn.prepare(
        "SELECT s.id, MIN(v.join_at), s.end_at FROM sessions s JOIN visits v ON v.session_id = s.id
         GROUP BY s.id ORDER BY MIN(v.join_at) DESC LIMIT ?1",
    )?;
    let rows = st.query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    rows.collect()
}

/// 本次报告：下线时生成、推送的就是这个。范围 = 这次游玩的 [第一次进房间, 关闭]。
pub fn recap_session(store: &Store, tz: TimeConv, session_id: i64, top_n: usize) -> Result<Option<Recap>> {
    match session_range(store, session_id)? {
        Some((a, b)) => recap(store, tz, "session", a, b + 1, top_n).map(Some),
        None => Ok(None),
    }
}
