//! yymemoir-cli：开发/校对用的命令行。
//!
//!   yymemoir-cli scan  --db data.db --logs <日志目录> [--photos <截图目录>] [--tz 480] [--live]
//!   yymemoir-cli sessions --db data.db [--tz 480]        # 列出每次游玩
//!   yymemoir-cli recap --db data.db --kind session --session <id|last>   # 本次报告（下线推送用）
//!   yymemoir-cli recap --db data.db --kind day|week|month|year|all --date 2026-04-30 [--tz 480] [--top 5]
//!   yymemoir-cli fetch-worlds --db data.db --cache <图片目录> [--limit 50]   # 联网取世界名和封面
//!   yymemoir-cli dump  <日志文件>      # 打印识别出的事件，用来校对真实日志

use chrono::{Datelike, Duration, NaiveDate};
use std::path::{Path, PathBuf};
use yymemoir_core::{finalize_file, ingest_file, parse::parse_line, photos::scan_photos, recap, Store, TimeConv};

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1).cloned())
}

fn tz(args: &[String]) -> TimeConv {
    arg(args, "--tz").and_then(|s| s.parse().ok()).map(TimeConv::Fixed).unwrap_or(TimeConv::Local)
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
    v.sort(); // 文件名就是启动时间
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("");
    let tz = tz(&args);
    match cmd {
        "scan" => {
            let db = arg(&args, "--db").expect("--db");
            let mut store = Store::open(&db).expect("open db");
            if let Some(logs) = arg(&args, "--logs") {
                let files = log_files(Path::new(&logs));
                let live = args.iter().any(|a| a == "--live");
                for (i, f) in files.iter().enumerate() {
                    let rep = ingest_file(&mut store, f, tz).expect("ingest");
                    // --live 时最新一个文件可能还在写，不收尾
                    if !(live && i + 1 == files.len()) {
                        finalize_file(&mut store, f).expect("finalize");
                    }
                    eprintln!("{}  +{}B  {} 行  {} 事件", f.display(), rep.bytes_read, rep.lines, rep.events);
                }
            }
            let quiet = yymemoir_core::store::errors_only_logs(&store, 10).expect("check");
            if !quiet.is_empty() {
                eprintln!("⚠ 有 {} 个日志只记录了报错，没有世界/玩家记录。请在 VRChat 设置 → Debug → 日志 改成「完整（Full）」：", quiet.len());
                for q in quiet {
                    eprintln!("   {q}");
                }
            }
            if let Some(p) = arg(&args, "--photos") {
                let n = scan_photos(&mut store, Path::new(&p), tz).expect("photos");
                eprintln!("新增照片 {n} 张");
            }
        }
        "recap" => {
            let db = arg(&args, "--db").expect("--db");
            let store = Store::open(&db).expect("open db");
            let kind = arg(&args, "--kind").unwrap_or_else(|| "day".into());
            let date = arg(&args, "--date")
                .map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").expect("--date YYYY-MM-DD"))
                .unwrap_or_else(|| chrono::Local::now().date_naive());
            let top: usize = arg(&args, "--top").and_then(|s| s.parse().ok()).unwrap_or(5);
            if kind == "session" {
                let which = arg(&args, "--session").unwrap_or_else(|| "last".into());
                let id = if which == "last" {
                    yymemoir_core::list_sessions(&store, 1).expect("sessions").first().map(|s| s.0).expect("没有有效的游玩")
                } else {
                    which.parse().expect("--session <id|last>")
                };
                let r = yymemoir_core::recap_session(&store, tz, id, top).expect("recap").expect("这次游玩没进过房间");
                println!("{}", serde_json::to_string_pretty(&r).unwrap());
                return;
            }
            let (a, b) = match kind.as_str() {
                "day" => (date, date + Duration::days(1)),
                "week" => {
                    let s = date - Duration::days(date.weekday().num_days_from_monday() as i64);
                    (s, s + Duration::days(7))
                }
                "month" => {
                    let s = date.with_day(1).unwrap();
                    let e = if s.month() == 12 {
                        NaiveDate::from_ymd_opt(s.year() + 1, 1, 1).unwrap()
                    } else {
                        NaiveDate::from_ymd_opt(s.year(), s.month() + 1, 1).unwrap()
                    };
                    (s, e)
                }
                "year" => (
                    NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap(),
                    NaiveDate::from_ymd_opt(date.year() + 1, 1, 1).unwrap(),
                ),
                _ => (NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(), NaiveDate::from_ymd_opt(2100, 1, 1).unwrap()),
            };
            let ms = |d: NaiveDate| tz.to_utc_ms(d.and_hms_opt(0, 0, 0).unwrap());
            let r = recap(&store, tz, &kind, ms(a), ms(b), top).expect("recap");
            println!("{}", serde_json::to_string_pretty(&r).unwrap());
        }
        "fetch-worlds" => {
            let db = arg(&args, "--db").expect("--db");
            let cache = arg(&args, "--cache").expect("--cache");
            let limit: usize = arg(&args, "--limit").and_then(|s| s.parse().ok()).unwrap_or(50);
            let store = Store::open(&db).expect("open db");
            let rep = yymemoir_core::vrcapi::fetch_missing_worlds(&store, Path::new(&cache), limit).expect("fetch");
            eprintln!("取到 {} 个世界", rep.fetched);
            for f in rep.failed {
                eprintln!("失败 {f}");
            }
        }
        "sessions" => {
            let store = Store::open(arg(&args, "--db").expect("--db")).expect("open db");
            for (id, a, b) in yymemoir_core::list_sessions(&store, 100).expect("sessions") {
                let f = |ms| tz.to_local(ms).format("%Y-%m-%d %H:%M").to_string();
                println!("{id}\t{} → {}\t{} 分钟", f(a), f(b), (b - a) / 60_000);
            }
        }
        "dump" => {
            let p = args.get(2).expect("file");
            let bytes = std::fs::read(p).expect("read");
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines() {
                if let Some((ts, Some(ev))) = parse_line(line) {
                    println!("{ts}  {ev:?}");
                }
            }
        }
        _ => {
            eprintln!("用法见 crates/cli/src/main.rs 顶部注释");
            std::process::exit(2);
        }
    }
}
