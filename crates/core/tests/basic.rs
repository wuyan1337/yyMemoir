use std::io::Write;
use yymemoir_core::{finalize_file, ingest_file, recap, Store, TimeConv};

const TZ: TimeConv = TimeConv::Fixed(8 * 60);

fn ms(s: &str) -> i64 {
    let t = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap();
    TZ.to_utc_ms(t)
}

#[test]
fn incremental_ingest_and_recap() {
    let full = include_str!("fixtures_basic.txt");
    let dir = std::env::temp_dir().join(format!("yym-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("output_log_2026-04-30_20-00-00.txt");

    // 分两次写入（第二次从半行开始），模拟游戏边玩边写
    let cut = full.find("阿花 (usr_00000000-0000-0000-0000-00000000000c)\n2026.04.30 20:50").unwrap() + 3;
    std::fs::write(&path, &full.as_bytes()[..cut]).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    ingest_file(&mut store, &path, TZ).unwrap();
    // 再读一次不会重复
    ingest_file(&mut store, &path, TZ).unwrap();
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(&full.as_bytes()[cut..]).unwrap();
    ingest_file(&mut store, &path, TZ).unwrap();
    finalize_file(&mut store, &path).unwrap();

    let r = recap(&store, TZ, "day", ms("2026-04-30 00:00"), ms("2026-05-01 00:00"), 5).unwrap();
    println!("{}", serde_json::to_string_pretty(&r).unwrap());

    assert_eq!(r.self_name.as_deref(), Some("测试用户"));
    assert_eq!(r.session_count, 1);
    // 从第一次进房间（20:01）算到最后一条日志（22:00:32）
    assert_eq!(r.total_minutes, 119);
    assert_eq!(r.world_count, 3);
    assert_eq!(r.top_worlds[0].name, "黑猫酒吧");
    assert_eq!(r.top_worlds[0].minutes, 90);
    // 自己不算进遇到的人
    assert_eq!(r.people_count, 3);
    assert_eq!(r.top_people[0].name, "阿花"); // 30 + 40 分钟
    assert_eq!(r.top_people[0].minutes, 70);
    assert_eq!(r.top_people[1].name, "小明");
    assert_eq!(r.top_people[1].minutes, 60);
    // 路人甲一直待到游戏退出（finalize 关掉）
    assert_eq!(r.top_people[2].minutes, 19);
    assert_eq!(r.access_minutes.get("friends"), Some(&90));
    assert_eq!(r.access_minutes.get("invite"), Some(&9));
    assert_eq!(r.peak_hour, Some(21)); // 20 点只有 59 分钟，21 点满 60 分钟
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn session_recap() {
    let dir = std::env::temp_dir().join(format!("yym-sess-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("output_log_2026-04-30_20-00-00.txt");
    std::fs::write(&path, include_str!("fixtures_basic.txt")).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    ingest_file(&mut store, &path, TZ).unwrap();
    finalize_file(&mut store, &path).unwrap();
    let sessions = yymemoir_core::list_sessions(&store, 10).unwrap();
    assert_eq!(sessions.len(), 1);
    let r = yymemoir_core::recap_session(&store, TZ, sessions[0].0, 5).unwrap().unwrap();
    assert_eq!(r.kind, "session");
    assert_eq!(r.total_minutes, 119);
    assert_eq!(r.from, "2026-04-30 20:01");
    assert_eq!(r.last_logoff.as_deref(), Some("2026-04-30 22:00"));
    std::fs::remove_dir_all(&dir).ok();
}
