//! 模型记录：只记本人、同名合并、认出 ID、分两次写入、老数据补读、备份往返。
//! 日志顺序照 2026-11 的真实日志（换模型 → 同一秒存旧模型 → 1–2 秒内读新模型）。

use std::io::Write;
use yymemoir_core::backup::{export, import, ImportMode};
use yymemoir_core::{finalize_file, ingest_file, recap, Store, TimeConv};

const TZ: TimeConv = TimeConv::Fixed(8 * 60);
const A: &str = "avtr_aaaaaaaa-0000-0000-0000-000000000001";
const B: &str = "avtr_bbbbbbbb-0000-0000-0000-000000000002";

const LOG: &str = "\
2026.05.01 21:00:00 Debug      -  [Behaviour] Initialized PlayerAPI \"测试用户\" is local
2026.05.01 21:00:01 Debug      -  [Behaviour] Entering Room: 黑猫酒吧
2026.05.01 21:00:01 Debug      -  [Behaviour] Joining wrld_22222222-2222-2222-2222-222222222222:55555~region(jp)
2026.05.01 21:00:05 Debug      -  [Behaviour] Switching 测试用户 to avatar 小狗
2026.05.01 21:00:06 Debug      -  Loading Avatar Data:avtr_aaaaaaaa-0000-0000-0000-000000000001
2026.05.01 21:00:20 Debug      -  [Behaviour] Switching 小明 to avatar 别人的模型
2026.05.01 21:10:00 Debug      -  [Behaviour] Switching 测试用户 to avatar 小狗
2026.05.01 21:30:00 Debug      -  [Behaviour] Switching 测试用户 to avatar 猫猫 to avatar 版
2026.05.01 21:30:00 Debug      -  Saving Avatar Data:avtr_aaaaaaaa-0000-0000-0000-000000000001
2026.05.01 21:30:02 Debug      -  Loading Avatar Data:avtr_bbbbbbbb-0000-0000-0000-000000000002
2026.05.01 21:31:00 Debug      -  Saving Avatar Data:avtr_bbbbbbbb-0000-0000-0000-000000000002
2026.05.01 21:45:00 Debug      -  [Behaviour] Switching 测试用户 to avatar 小狗
2026.05.01 21:45:00 Debug      -  Saving Avatar Data:avtr_bbbbbbbb-0000-0000-0000-000000000002
2026.05.01 21:46:00 Debug      -  Saving Avatar Data:avtr_aaaaaaaa-0000-0000-0000-000000000001
2026.05.01 22:00:00 Debug      -  [Behaviour] OnLeftRoom
";

fn ms(s: &str) -> i64 {
    TZ.to_utc_ms(chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap())
}

fn write_log(tag: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("yym-avatar-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("output_log_2026-05-01_21-00-00.txt");
    std::fs::write(&path, text).unwrap();
    path
}

fn wears(s: &Store) -> Vec<(String, Option<String>, i64, Option<i64>)> {
    s.conn
        .prepare("SELECT avatar_name, avatar_id, start_at, end_at FROM avatar_wears ORDER BY start_at")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn check(s: &Store) {
    let w = wears(s);
    let names: Vec<&str> = w.iter().map(|x| x.0.as_str()).collect();
    // 别人的模型不记；21:10 的「又换成小狗」并进第一段；名字里带「to avatar」也能完整取到
    assert_eq!(names, ["小狗", "猫猫 to avatar 版", "小狗"]);
    // 第一段 ID 来自换上后 1 秒的 Loading；21:30:00 那条 Saving 是换下来的旧模型，不能算给猫猫
    assert_eq!(w[0].1.as_deref(), Some(A));
    assert_eq!(w[1].1.as_deref(), Some(B));
    // 第三段没有 Loading，靠换上 1 分钟后的 Saving 认出
    assert_eq!(w[2].1.as_deref(), Some(A));
    assert!(w.iter().all(|x| x.3.is_some()), "收尾后每段都有结束时间");

    let r = recap(s, TZ, "day", ms("2026-05-01 00:00"), ms("2026-05-02 00:00"), 5).unwrap();
    assert_eq!(r.avatar_count, 2);
    assert_eq!(r.top_avatars[0].name, "小狗");
    // 21:00:05–21:30:00 + 21:45:00–22:00:00 = 29 分 55 秒 + 15 分
    assert_eq!(r.top_avatars[0].minutes, 44);
    assert_eq!(r.top_avatars[0].wears, 2);
    assert_eq!(r.top_avatars[1].minutes, 15);
}

#[test]
fn records_own_avatars_incrementally() {
    // 在「换上猫猫」和它的 Loading 之间切开，模拟游戏边玩边写
    let cut = LOG.find("2026.05.01 21:30:00 Debug      -  Saving").unwrap();
    let path = write_log("inc", &LOG[..cut]);
    let mut s = Store::open_in_memory().unwrap();
    ingest_file(&mut s, &path, TZ).unwrap();
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(LOG[cut..].as_bytes()).unwrap();
    ingest_file(&mut s, &path, TZ).unwrap();
    finalize_file(&mut s, &path).unwrap();
    check(&s);
}

#[test]
fn backfills_old_data_once() {
    let path = write_log("backfill", LOG);
    let mut s = Store::open_in_memory().unwrap();
    ingest_file(&mut s, &path, TZ).unwrap();
    finalize_file(&mut s, &path).unwrap();
    // 模拟升级前的数据库：有游玩记录，没有模型
    s.conn.execute_batch("DELETE FROM avatar_wears; UPDATE log_files SET avatars_scanned = 0;").unwrap();
    assert_eq!(yymemoir_core::avatars::backfill(&mut s, TZ).unwrap(), 3);
    check(&s);
    // 只补一次
    assert_eq!(yymemoir_core::avatars::backfill(&mut s, TZ).unwrap(), 0);
    assert_eq!(wears(&s).len(), 3);
}

#[test]
fn backup_keeps_avatars() {
    let path = write_log("backup", LOG);
    let mut src = Store::open_in_memory().unwrap();
    ingest_file(&mut src, &path, TZ).unwrap();
    finalize_file(&mut src, &path).unwrap();
    let (bytes, _) = export(&src).unwrap();
    std::fs::remove_file(&path).unwrap();
    let mut dst = Store::open_in_memory().unwrap();
    import(&mut dst, &bytes, ImportMode::Replace).unwrap();
    check(&dst);
    // 合并同一份不重复
    import(&mut dst, &bytes, ImportMode::Merge).unwrap();
    assert_eq!(wears(&dst).len(), 3);
}
