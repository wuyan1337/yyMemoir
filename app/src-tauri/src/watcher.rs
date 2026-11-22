//! 后台监听：每 5 秒看一次 VRChat 是否在运行、增量读日志。
//! 检测到 VRChat 关闭 → 收尾这次游玩 → 取世界封面 → 生成「本次回顾」并推送通知。
//! 照片只认日志里「VRC Camera」记录的（在读日志时就入库了），不扫截图文件夹。

use crate::log::log;
use crate::paths::Paths;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, System};
use yymemoir_core::{finalize_file, ingest_file, list_sessions, recap_session, Store, TimeConv};

const TZ: TimeConv = TimeConv::Local;

/// 下线后要推送的内容
pub enum Outcome {
    /// 正常：这次游玩的 session id 和一句话摘要
    Session { id: i64, title: String, body: String },
    /// 这次的日志是「仅错误」，什么都没记下来
    ErrorsOnly,
}

pub fn vrchat_running(sys: &mut System) -> bool {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let found = sys.processes_by_exact_name("VRChat.exe".as_ref()).next().is_some();
    found
}

fn log_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("output_log_") && n.ends_with(".txt"))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// 读所有日志的新增部分；`running` 为 false 时把还没收尾的文件全部收尾，否则只收尾除最新一个以外的。
pub fn sync(store: &mut Store, paths: &Paths, running: bool) {
    let files = log_files(&paths.vrchat_logs);
    for f in &files {
        if let Err(e) = ingest_file(store, f, TZ) {
            log(format!("读取 {} 失败: {e}", f.display()));
        }
    }
    let unfinished: Vec<String> = store
        .conn
        .prepare("SELECT path FROM log_files WHERE finalized = 0 ORDER BY path")
        .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
        .unwrap_or_default();
    let newest = files.last().map(|p| p.to_string_lossy().to_string());
    for p in unfinished {
        if running && Some(&p) == newest.as_ref() {
            continue;
        }
        finalize_file(store, Path::new(&p)).ok();
    }
}

/// 下线后的收尾工作，返回要推送的内容
pub fn on_game_exit(store: &mut Store, paths: &Paths) -> Option<Outcome> {
    sync(store, paths, false);
    // 最新一个日志是「仅错误」→ 提醒去改设置
    let latest_quiet = store
        .conn
        .query_row(
            "SELECT info_lines = 0 AND total_lines >= 10 FROM log_files ORDER BY path DESC LIMIT 1",
            [],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(false);
    if latest_quiet {
        return Some(Outcome::ErrorsOnly);
    }
    // 联网取世界名和封面（失败不影响推送）
    fetch_worlds(store, paths);

    let (id, _start, end) = *list_sessions(store, 1).ok()?.first()?;
    // 只推送刚结束的那次（10 分钟内），避免开机补扫时推送很久以前的
    if chrono::Utc::now().timestamp_millis() - end > 10 * 60_000 {
        return None;
    }
    let r = recap_session(store, TZ, id, 3).ok()??;
    let hm = |m: i64| if m >= 60 { format!("{}h {:02}m", m / 60, m % 60) } else { format!("{m}m") };
    let title = format!("本次回顾 · {}", hm(r.total_minutes));
    let mut body = format!("{} 个世界 · 遇见 {} 人", r.world_count, r.people_count);
    if let Some(w) = r.top_worlds.first() {
        body = format!("在「{}」待得最久\n{body}", w.name);
    }
    if let Some(p) = r.top_people.first() {
        body.push_str(&format!(" · 和 {} 一起 {}", p.name, hm(p.minutes)));
    }
    Some(Outcome::Session { id, title, body })
}

/// 给还没有封面的世界联网取名字和封面，返回新取到几个
pub fn fetch_worlds(store: &Store, paths: &Paths) -> usize {
    match yymemoir_core::vrcapi::fetch_missing_worlds(store, &paths.world_cache, 30) {
        Ok(rep) => {
            if rep.fetched > 0 || !rep.failed.is_empty() {
                log(format!("世界封面：取到 {} 个，失败 {} 个", rep.fetched, rep.failed.len()));
            }
            for f in rep.failed.iter().take(5) {
                log(format!("  失败 {f}"));
            }
            rep.fetched
        }
        Err(e) => {
            log(format!("世界封面：出错 {e}"));
            0
        }
    }
}

/// 监听主循环（在独立线程里跑）。`notify` 推送本次回顾；`updated` 通知窗口数据有更新（比如新取到了封面）
pub fn run(paths: Paths, notify: impl Fn(Outcome) + Send + 'static, updated: impl Fn() + Send + 'static) {
    let mut store = match Store::open(&paths.db) {
        Ok(s) => s,
        Err(e) => {
            log(format!("打开数据库失败: {e}"));
            return;
        }
    };
    log(format!("启动：日志目录 {}（{}）", paths.vrchat_logs.display(), if paths.vrchat_logs.is_dir() { "找到" } else { "没找到" }));
    let mut sys = System::new();
    let mut was_running = vrchat_running(&mut sys);
    // 开机补扫：游戏没在跑就把所有日志收尾
    sync(&mut store, &paths, was_running);
    // 补扫进来的历史游玩也要有封面
    if fetch_worlds(&store, &paths) > 0 {
        updated();
    }
    let mut last_fetch = std::time::Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(5));
        let running = vrchat_running(&mut sys);
        if was_running && !running {
            // 等游戏把最后几行写完
            std::thread::sleep(Duration::from_secs(3));
            log("检测到 VRChat 关闭");
            if let Some(o) = on_game_exit(&mut store, &paths) {
                notify(o);
            }
            last_fetch = std::time::Instant::now();
        } else if running {
            if !was_running {
                log("检测到 VRChat 启动");
            }
            sync(&mut store, &paths, true);
        } else if last_fetch.elapsed() > Duration::from_secs(600) {
            // 没在玩时每 10 分钟补一次缺的封面（之前没网 / 失败的）
            last_fetch = std::time::Instant::now();
            if fetch_worlds(&store, &paths) > 0 {
                updated();
            }
        }
        was_running = running;
    }
}
