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

/// 带图片：换电脑（原图不在了）→ 照片和封面还原到新位置，记录指向新位置；同一台电脑原图还在 → 不重复写
#[test]
fn images_roundtrip() {
    use yymemoir_core::backup::{export_to, import_from, ExportOptions, ImportTargets};
    let tmp = std::env::temp_dir().join(format!("yym-img-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let photo = tmp.join("VRChat_2026-04-30_20-20-00.123_1920x1080.png");
    image::RgbImage::from_fn(2400, 1350, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])).save(&photo).unwrap();
    let cover = tmp.join("cover.png");
    image::RgbImage::from_pixel(800, 600, image::Rgb([20, 40, 60])).save(&cover).unwrap();

    let src = store_with_fixture("img");
    src.conn.execute("UPDATE photos SET path = ?1", [photo.to_string_lossy()]).unwrap();
    src.conn
        .execute(
            "INSERT INTO worlds_cache(world_id, name, thumbnail_path) VALUES ('wrld_22222222-2222-2222-2222-222222222222', '黑猫酒吧', ?1)",
            [cover.to_string_lossy()],
        )
        .unwrap();
    let mut buf = Vec::new();
    let sum = export_to(&src, &mut buf, ExportOptions::default()).unwrap();
    assert_eq!((sum.photo_files, sum.covers), (1, 1));
    assert_eq!(sum.bytes as usize, buf.len());

    let targets = ImportTargets { photo_dir: tmp.join("restored-photos"), cover_dir: tmp.join("restored-covers") };

    // 同一台电脑：原图还在，不重复写
    let mut same = Store::open_in_memory().unwrap();
    let rep = import_from(&mut same, &buf[..], ImportMode::Merge, Some(&targets)).unwrap();
    assert_eq!(rep.photos_restored, 0);

    // 换电脑：原图和封面都不在了
    std::fs::remove_file(&photo).unwrap();
    std::fs::remove_file(&cover).unwrap();
    let mut dst = Store::open_in_memory().unwrap();
    let rep = import_from(&mut dst, &buf[..], ImportMode::Replace, Some(&targets)).unwrap();
    assert_eq!((rep.added, rep.photos_restored, rep.covers_restored), (1, 1, 1));
    let new_path: String = dst.conn.query_row("SELECT path FROM photos", [], |r| r.get(0)).unwrap();
    assert!(new_path.contains("restored-photos") && new_path.contains("2026-04") && new_path.ends_with(".jpg"));
    let img = image::open(&new_path).unwrap();
    assert_eq!(img.width(), 1920); // 压缩：长边 1920
    let thumb: String = dst.conn.query_row("SELECT thumbnail_path FROM worlds_cache", [], |r| r.get(0)).unwrap();
    assert!(std::path::Path::new(&thumb).exists());
    assert_eq!(summary(&dst), summary(&src));
    std::fs::remove_dir_all(&tmp).ok();
}
