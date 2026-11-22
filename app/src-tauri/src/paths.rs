use std::path::PathBuf;

/// 所有用到的路径
#[derive(Clone, Debug)]
pub struct Paths {
    /// %USERPROFILE%\AppData\LocalLow\VRChat\VRChat
    pub vrchat_logs: PathBuf,
    /// 图片\VRChat（默认截图位置；找不到就是 None）
    pub screenshots: Option<PathBuf>,
    /// 本程序数据：数据库、世界封面缓存
    pub db: PathBuf,
    pub world_cache: PathBuf,
}

impl Paths {
    pub fn detect(app_data: PathBuf) -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let vrchat_logs = home.join("AppData").join("LocalLow").join("VRChat").join("VRChat");
        let screenshots = dirs::picture_dir().map(|p| p.join("VRChat")).filter(|p| p.is_dir());
        std::fs::create_dir_all(&app_data).ok();
        Paths {
            vrchat_logs,
            screenshots,
            db: app_data.join("yymemoir.db"),
            world_cache: app_data.join("world-cache"),
        }
    }
}
