//! 简单的文件日志：%APPDATA%\com.yymemoir.app\yymemoir.log，出问题时让用户发这个文件。
//! 超过 1 MB 就从头写，避免无限增长。

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static FILE: OnceLock<PathBuf> = OnceLock::new();

pub fn init(path: PathBuf) {
    if std::fs::metadata(&path).map(|m| m.len() > 1 << 20).unwrap_or(false) {
        std::fs::remove_file(&path).ok();
    }
    FILE.set(path).ok();
}

pub fn log(msg: impl AsRef<str>) {
    let line = format!("{} {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), msg.as_ref());
    eprint!("{line}");
    if let Some(p) = FILE.get() {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            f.write_all(line.as_bytes()).ok();
        }
    }
}
