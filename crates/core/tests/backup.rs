use yymemoir_core::backup::{export, import, BackupError, ImportMode};
use yymemoir_core::{finalize_file, ingest_file, recap, Store, TimeConv};

const TZ: TimeConv = TimeConv::Fixed(8 * 60);

fn store_with_fixture(tag: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("yym-bk-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("output_log_2026-04-30_20-00-00.txt");
    std::fs::write(&path, include_str!("fixtures_basic.txt")).unwrap();
    let mut s = Store::open_in_memory().unwrap();
    ingest_file(&mut s, &path, TZ).unwrap();
    finalize_file(&mut s, &path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    s
}

fn summary(s: &Store) -> (i64, usize, usize, usize) {
    let a = TZ.to_utc_ms(chrono::NaiveDate::from_ymd_opt(2026, 4, 30).unwrap().and_hms_opt(0, 0, 0).unwrap());
    let r = recap(s, TZ, "day", a, a + 86_400_000, 5).unwrap();
    (r.total_minutes, r.world_count, r.people_count, r.photo_count)
}

#[test]
fn roundtrip_merge_replace() {
    let src = store_with_fixture("src");
    let (bytes, sum) = export(&src).unwrap();
    assert_eq!(sum.sessions, 1);
    assert_eq!(sum.photos, 1);
    // 文件头是 YYMB，内容不是明文
    assert_eq!(&bytes[..4], b"YYMB");
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("黑猫酒吧") && !text.contains("wrld_") && !text.contains("usr_"));

    // 导入到空库：数据一样
    let mut dst = Store::open_in_memory().unwrap();
    let rep = import(&mut dst, &bytes, ImportMode::Merge).unwrap();
    assert_eq!((rep.added, rep.skipped), (1, 0));
    assert_eq!(summary(&dst), summary(&src));

    // 再合并一次：不会重复
    let rep = import(&mut dst, &bytes, ImportMode::Merge).unwrap();
    assert_eq!((rep.added, rep.skipped), (0, 1));
    assert_eq!(summary(&dst), summary(&src));

    // 覆盖：清空后换成备份
    let rep = import(&mut dst, &bytes, ImportMode::Replace).unwrap();
    assert_eq!((rep.added, rep.skipped), (1, 0));
    assert_eq!(summary(&dst), summary(&src));
}

#[test]
fn rejects_other_files() {
    let mut s = Store::open_in_memory().unwrap();
    assert!(matches!(import(&mut s, b"hello world, not a backup", ImportMode::Merge), Err(BackupError::NotBackup)));
    let mut bad = b"YYMB\x01\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
    bad.extend_from_slice(b"garbage garbage");
    assert!(matches!(import(&mut s, &bad, ImportMode::Merge), Err(BackupError::Corrupt(_))));
}
