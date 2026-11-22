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
    assert_eq!(r.peak_hour, Some(21));
    // 日志里的相机照片记到当时所在的世界
    assert_eq!(r.photo_count, 1);
    assert_eq!(r.top_photo_world.as_ref().map(|w| w.name.as_str()), Some("黑猫酒吧")); // 20 点只有 59 分钟，21 点满 60 分钟
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

/// 日志里没有登录行（VRChat 改了写法 / 缺失），靠每次进房间的 `is local` 认出本人
#[test]
fn self_from_local_player_line() {
    let src: String = include_str!("fixtures_basic.txt")
        .lines()
        .filter(|l| !l.contains("User Authenticated") && !l.contains("post login checks"))
        .flat_map(|l| {
            let mut v = vec![l.to_string()];
            if l.contains("OnPlayerJoined 测试用户") {
                v.push(format!("{} Debug      -  [Behaviour] Initialized PlayerAPI \"测试用户\" is local", &l[..19]));
            }
            v
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let dir = std::env::temp_dir().join(format!("yym-local-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("output_log_2026-04-30_20-00-00.txt");
    std::fs::write(&path, src).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    ingest_file(&mut store, &path, TZ).unwrap();
    finalize_file(&mut store, &path).unwrap();
    let r = recap(&store, TZ, "day", ms("2026-04-30 00:00"), ms("2026-05-01 00:00"), 5).unwrap();
    assert_eq!(r.people_count, 3);
    assert!(r.top_people.iter().all(|p| p.name != "测试用户"));
    std::fs::remove_dir_all(&dir).ok();
}

/// 旧数据库里把自己算进去了：升级后自动修掉
#[test]
fn repairs_self_in_old_db() {
    let dir = std::env::temp_dir().join(format!("yym-repair-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("old.db");
    {
        let s = Store::open(&db).unwrap();
        s.conn.execute_batch(
            "PRAGMA user_version = 1;
             INSERT INTO log_files(path, first_seen, self_name) VALUES ('a.txt', 1, '本人');
             INSERT INTO sessions(id, start_at, end_at, log_file) VALUES (1, 0, 3600000, 'a.txt');
             INSERT INTO visits(id, session_id, world_id, instance_id, access_type, join_at, leave_at) VALUES (1, 1, 'wrld_1', '1', 'public', 0, 3600000);
             INSERT INTO encounters(visit_id, user_id, display_name, join_at, leave_at) VALUES (1, 'usr_self', '本人', 0, 3600000);
             INSERT INTO encounters(visit_id, user_id, display_name, join_at, leave_at) VALUES (1, 'usr_friend', '朋友', 0, 1800000);",
        ).unwrap();
    }
    let s = Store::open(&db).unwrap();
    let r = recap(&s, TZ, "all", 0, 7_200_000, 5).unwrap();
    assert_eq!(r.people_count, 1);
    assert_eq!(r.top_people[0].name, "朋友");
    let sid: String = s.conn.query_row("SELECT self_user_id FROM log_files", [], |x| x.get(0)).unwrap();
    assert_eq!(sid, "usr_self");
    std::fs::remove_dir_all(&dir).ok();
}
